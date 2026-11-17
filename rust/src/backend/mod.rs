//! C11 代码生成（规格书 §12–§13）：AST + 语义槽 → 单个 C 翻译单元。
//!
//! 生成约定：
//!   * 类引用是 `T*`，类结构首两字段固定为 `vt` 与 `__tag`（前缀一致保证上转）。
//!   * 所有实例方法经 vtable 调用；vtable 条目首参 `void*`，避免函数指针类型不兼容。
//!   * 求值顺序：每个子表达式在生成时立即落临时变量，语句顺序即求值顺序。
//!   * 生成的 C 为纯 ASCII（非 ASCII 字符串字面量经 `cstr()` 转义为 `\xHH`），
//!     运行时按 UTF-8 输出，Windows 控制台代码页由 `axc/rt/axrt.c` 的
//!     `ax_console_utf8()` 处理（仅交互式终端，见 README「Windows 中文显示」）。
//!
//! 分工：本文件是 `CodeGen` 主类（程序级结构、main 包装、运行时拼接）；
//! 语句级生成（含 try/catch/defer 展开）在 `codegen_fn.rs`，
//! 表达式级生成与 match 语句在 `codegen_expr.rs`。
//! 运行时库在仓库 `axc/rt/axrt.{h,c}`，生成代码 `#include "axrt.h"` 并链接它。

mod codegen_expr;
mod codegen_fn;

use crate::frontend::ast;
use crate::sema::{ConstVal, Sema, TypeDef};
use crate::types::*;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub const AX_OK: u32 = 1;
pub const AX_FAIL: u32 = 0;

/// 节点种类名，用于生成 `"0 /* kind */"` 式的调试注释。
pub fn kind_name(e: &ast::Expr) -> &'static str {
    match e {
        ast::Expr::IntLit { .. } => "IntLit",
        ast::Expr::FloatLit { .. } => "FloatLit",
        ast::Expr::DecLit { .. } => "DecLit",
        ast::Expr::BoolLit { .. } => "BoolLit",
        ast::Expr::NullLit => "NullLit",
        ast::Expr::StrLit { .. } => "StrLit",
        ast::Expr::Path { .. } => "Path",
        ast::Expr::This => "This",
        ast::Expr::Parent => "Parent",
        ast::Expr::TupleLit { .. } => "TupleLit",
        ast::Expr::ArrayLit { .. } => "ArrayLit",
        ast::Expr::BraceLit { .. } => "BraceLit",
        ast::Expr::Call { .. } => "Call",
        ast::Expr::MethodCall { .. } => "MethodCall",
        ast::Expr::FieldAccess { .. } => "FieldAccess",
        ast::Expr::SafeAccess { .. } => "SafeAccess",
        ast::Expr::SafeCall { .. } => "SafeCall",
        ast::Expr::Index { .. } => "Index",
        ast::Expr::Unary { .. } => "Unary",
        ast::Expr::Binary { .. } => "Binary",
        ast::Expr::Ternary { .. } => "Ternary",
        ast::Expr::Coalesce { .. } => "Coalesce",
        ast::Expr::Range { .. } => "Range",
        ast::Expr::Try { .. } => "Try",
        ast::Expr::New { .. } => "New",
        ast::Expr::If(_) => "IfExpr",
        ast::Expr::Match(_) => "MatchExpr",
        ast::Expr::AnonFunc { .. } => "AnonFunc",
    }
}

/// 把任意标识符/文件名压成合法的 C 标识符片段。
/// C 标识符不能以数字开头（`01_hello.ax` → `01_hello__main`），以数字开头时补 `_`。
pub fn san(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        return "_".to_string();
    }
    let first = out.as_bytes()[0];
    if first.is_ascii_digit() {
        return format!("_{}", out);
    }
    out
}

/// 把 UTF-8 文本写成 C 字符串字面量（非 ASCII 一律 `\ooo` 八进制转义）。
///
/// 用八进制而非十六进制：C 的 `\x` 是**贪婪**的，`"\xe8\xa1\x8c1"` 会把
/// `\x8c1` 读成一个十六进制转义（0x8c1，越界）。中文后面紧跟 ASCII 数字或
/// a–f 时就会静默损坏（例：`"行1"`）。八进制固定吃 3 位，后随任何字符都安全。
pub fn cstr(text: &str) -> String {
    let mut out = String::from("\"");
    for b in text.as_bytes() {
        match *b {
            0x22 => out.push_str("\\\""),
            0x5C => out.push_str("\\\\"),
            0x0A => out.push_str("\\n"),
            0x09 => out.push_str("\\t"),
            0x0D => out.push_str("\\r"),
            32..=126 => out.push(*b as char),
            _ => out.push_str(&format!("\\{:03o}", b)),
        }
    }
    out.push('"');
    out
}

/// 浮点最短往返表示：无小数点/指数时补 `.0`。
pub fn float_repr(f: f64) -> String {
    if f.is_nan() {
        return "nan".to_string();
    }
    if f.is_infinite() {
        return if f > 0.0 { "inf" } else { "-inf" }.to_string();
    }
    let s = format!("{}", f);
    if s.contains('.') || s.contains('e') {
        s
    } else {
        s + ".0"
    }
}

/// 运行时类型标签：基础类型 1–14，复合类型 1000 + fnv1a32(canonical) % 100000。
/// 注意：这里刻意用不带 namer 的 canonical——具名类型须落到数字 id，
/// 否则跨编译单元的标签对不上；改动会破坏 ABI 兼容。
pub fn tag_of(ty: &Ty) -> u32 {    match &**ty {
        TyS::Bool => 1,
        TyS::Int(bits, signed) => {
            if *bits == 64 {
                if *signed {
                    2
                } else {
                    10
                }
            } else {
                match (bits, signed) {
                    (8, true) => 3,
                    (8, false) => 11,
                    (16, true) => 4,
                    (16, false) => 12,
                    (32, true) => 9,
                    (32, false) => 13,
                    _ => 1000 + fnv1a32(&canonical(ty, None)) % 100000,
                }
            }
        }
        TyS::Float => 6,
        TyS::Decimal => 7,
        TyS::Str => 14,
        _ => 1000 + fnv1a32(&canonical(ty, None)) % 100000,
    }
}

// ---------------------------------------------------------------- CodeGen
/// 程序级 C 生成器：类型/类/接口定义、vtable、函数声明与定义、
/// main 包装（含 `ax_console_utf8()` 初始化）、运行时拼接。
/// `tmp_n` 为临时变量编号；`emitted` 去重已生成 C 定义的类型。
pub struct CodeGen<'s, 'a> {
    pub s: &'s Sema<'a>,
    pub path: String,
    pub mod_name: String,
    pub emitted: HashSet<Ty>,
    pub emitted_defs: HashSet<u32>,
    pub type_defs: Vec<String>,
    pub forward: Vec<String>,
    pub fn_decls: Vec<String>,
    /// 函数体生成期间临时派生的文件级定义（如 `Sort(cmp)` 的 qsort 适配器），
    /// 拼接时排在 fns 之后（其引用的 axfn_/元素类型定义在 type_defs，先于此处）。
    pub extra_fns: Vec<String>,
    pub tmp_n: u32,
}

impl<'s, 'a> CodeGen<'s, 'a> {
    pub fn new(s: &'s Sema<'a>, path: &str) -> Self {
        let base = std::path::Path::new(path)
            .file_stem()
            .map(|x| x.to_string_lossy().to_string())
            .unwrap_or_else(|| "unit".to_string());
        CodeGen {
            s,
            path: path.to_string(),
            mod_name: san(&base),
            emitted: HashSet::new(),
            emitted_defs: HashSet::new(),
            type_defs: Vec::new(),
            forward: Vec::new(),
            fn_decls: Vec::new(),
            extra_fns: Vec::new(),
            tmp_n: 0,
        }
    }

    // ------------------------------------------------------------ 类型

    pub fn def_of(&self, defid: &DefId) -> Option<&'s TypeDef> {
        match defid {
            DefId::Id(i) => self.s.typedefs.get(*i as usize),
            DefId::Name(_) => None,
        }
    }

    pub fn cn(&self, ty: &Ty) -> String {
        let s = self.s;
        san(&canonical(ty, Some(&|d| s.namer_name(d))))
    }

    pub fn ct(&mut self, ty: &Ty) -> String {
        self.need(ty);
        match &**ty {
            TyS::Bool => "bool".to_string(),
            TyS::Int(bits, signed) => {
                if *bits == 64 {
                    if *signed {
                        "int64_t".to_string()
                    } else {
                        "uint64_t".to_string()
                    }
                } else if *signed {
                    format!("int{}_t", bits)
                } else {
                    format!("uint{}_t", bits)
                }
            }
            TyS::Float => "double".to_string(),
            TyS::Decimal => "ax_dec".to_string(),
            TyS::Str => "ax_str".to_string(),
            TyS::Void => "void".to_string(),
            TyS::Object => "ax_box*".to_string(),
            TyS::Error => "ax_err".to_string(),
            TyS::Range => "int64_t".to_string(),
            TyS::Never => "void".to_string(),
            TyS::Array(elem, len) => match len {
                None => "ax_arr".to_string(),
                Some(_) => {
                    let _ = elem;
                    format!("axfix_{}", self.cn(ty))
                }
            },
            TyS::Map(..) => "ax_map".to_string(),
            TyS::Tuple(_) => format!("axtup_{}", self.cn(ty)),
            TyS::Record(_) => format!("axrec_{}", self.cn(ty)),
            TyS::Func(..) => format!("axfn_{}", self.cn(ty)),
            TyS::Nullable(_) => format!("axopt_{}", self.cn(ty)),
            TyS::Result(_) => format!("axres_{}", self.cn(ty)),
            TyS::ArenaRef(inner) => format!("{}*", self.ct(inner)),
            TyS::Enum(_) => "uint32_t".to_string(),
            TyS::Param(_) => "int64_t".to_string(),
            TyS::Meta(_) | TyS::ModuleT(_) => "int64_t".to_string(),
            TyS::Named(nm, _) => {
                match nm {
                    DefId::Name("Row") => return "ax_row".to_string(),
                    DefId::Name("File") => return "ax_file*".to_string(),
                    DefId::Name("Arena") => return "void*".to_string(),
                    DefId::Name("Rounding") => return "int32_t".to_string(),
                    _ => {}
                }
                match self.def_of(nm) {
                    None => "void*".to_string(),
                    Some(d) => {
                        let d = d.clone();
                        if d.kind == "class" {
                            format!("{}*", self.class_name(&d))
                        } else if d.kind == "interface" {
                            // interface 是胖指针 { obj, vt }（GAP #13）
                            format!("Iface_{}", san(&d.name))
                        } else {
                            self.struct_name(&d)
                        }
                    }
                }
            }
            _ => "int64_t".to_string(),
        }
    }

    pub fn class_name(&self, d: &TypeDef) -> String {
        let mut base = san(&d.name);
        if !d.generics.is_empty() {
            base += "_g";
        }
        base
    }

    pub fn struct_name(&self, d: &TypeDef) -> String {
        san(&d.name)
    }

    pub fn need(&mut self, ty: &Ty) {
        if self.emitted.contains(ty) {
            return;
        }
        let k: &TyS = ty;
        self.emitted.insert(ty.clone());
        match k {
            TyS::Bool
            | TyS::Int(..)
            | TyS::Float
            | TyS::Decimal
            | TyS::Str
            | TyS::Void
            | TyS::Object
            | TyS::Error
            | TyS::Range
            | TyS::Never
            | TyS::Param(_)
            | TyS::Meta(_)
            | TyS::ModuleT(_)
            | TyS::BuiltinT(_)
            | TyS::ParentT
            | TyS::Null
            | TyS::Enum(_)
            | TyS::Unknown => {}
            TyS::Array(elem, len) => {
                self.need(elem);
                if let Some(n) = len {
                    let name = format!("axfix_{}", self.cn(ty));
                    let et = self.ct(elem);
                    self.type_defs
                        .push(format!("typedef struct {{ {} v[{}]; }} {};", et, n, name));
                }
            }
            TyS::Map(k2, v2) => {
                self.need(k2);
                self.need(v2);
            }
            TyS::Nullable(inner) => {
                self.need(inner);
                let name = format!("axopt_{}", self.cn(ty));
                let it = self.ct(inner);
                self.type_defs
                    .push(format!("typedef struct {{ uint8_t has; {} v; }} {};", it, name));
            }
            TyS::Result(inner) => {
                self.need(inner);
                let name = format!("axres_{}", self.cn(ty));
                let row = matches!(&**inner, TyS::Named(DefId::Name("Row"), args) if args.is_empty());
                if row {
                    // 运行时已定义同形结构（axrt.h 的 ax_res_row）
                    self.type_defs.push(format!("typedef ax_res_row {};", name));
                    return;
                }
                let file_ = matches!(&**inner, TyS::Named(DefId::Name("File"), args) if args.is_empty());
                if file_ {
                    // 运行时已定义同形结构（axrt.h 的 ax_res_file）
                    self.type_defs.push(format!("typedef ax_res_file {};", name));
                    return;
                }
                let it = self.ct(inner);
                self.type_defs.push(format!(
                    "typedef struct {{ uint8_t tag; union {{ {} ok; ax_err err; }} u; }} {};",
                    it, name
                ));
            }
            TyS::Tuple(elems) => {
                let mut fs = Vec::new();
                for (i, et) in elems.iter().enumerate() {
                    self.need(et);
                    fs.push(format!("  {} f{};", self.ct(et), i));
                }
                let name = format!("axtup_{}", self.cn(ty));
                self.type_defs
                    .push(format!("typedef struct {{\n{}\n}} {};", fs.join("\n"), name));
            }
            TyS::Record(fields) => {
                // record 是匿名 struct：键在类型里固定，同名同型的 record 共用一个 typedef
                let mut fs = Vec::new();
                for (key, ft) in fields {
                    self.need(ft);
                    fs.push(format!("  {} f_{};", self.ct(ft), san(key)));
                }
                if fs.is_empty() {
                    fs.push("  uint8_t _dummy;".to_string());
                }
                let name = format!("axrec_{}", self.cn(ty));
                self.type_defs
                    .push(format!("typedef struct {{\n{}\n}} {};", fs.join("\n"), name));
            }
            TyS::Func(ps, ret) => {
                for p in ps {
                    self.need(p);
                }
                self.need(ret);
                let name = format!("axfn_{}", self.cn(ty));
                let pnames: Vec<String> = ps.iter().map(|p| self.ct(p)).collect();
                let ps2 = if pnames.is_empty() {
                    "void".to_string()
                } else {
                    pnames.join(", ")
                };
                let retc = self.ct(ret);
                self.type_defs
                    .push(format!("typedef {} (*{})({});", retc, name, ps2));
            }
            TyS::ArenaRef(inner) => {
                self.need(inner);
            }
            TyS::Named(nm, _) => {
                if matches!(nm, DefId::Name("Row" | "File" | "Arena" | "Rounding")) {
                    return;
                }
                if let Some(d) = self.def_of(nm) {
                    let d = d.clone();
                    self.need_typedef(&d);
                }
            }
        }
    }

    /// 继承链上的字段按「父类在前」拉平（C 布局前缀兼容）。
    pub fn flat_fields(&self, d: &TypeDef) -> Vec<(String, Ty)> {
        let mut out = Vec::new();
        if let Some(p) = d.parent {
            if let Some(pd) = self.s.typedefs.get(p as usize) {
                out.extend(self.flat_fields(pd));
            }
        }
        for (fname, ft, _vis, _dflt) in &d.fields {
            out.push((fname.clone(), ft.clone()));
        }
        out
    }

    pub fn need_typedef(&mut self, d: &TypeDef) {
        if self.emitted_defs.contains(&d.id) {
            return;
        }
        self.emitted_defs.insert(d.id);
        if d.kind == "class" {
            // 字段先就位（值语义字段要求被依赖的 struct 已定义）
            let flats = self.flat_fields(d);
            for (_, ft) in &flats {
                self.need(ft);
            }
            let cname = san(&d.name);
            let mut fs = vec![
                format!("  {}_VT* vt;", cname),
                "  uint32_t __tag;".to_string(),
            ];
            for (fname, ft) in &flats {
                fs.push(format!("  {} f_{};", self.ct(ft), san(fname)));
            }
            if fs.len() == 2 {
                fs.push("  uint8_t _dummy;".to_string());
            }
            self.type_defs.push(format!(
                "typedef struct {} {{\n{}\n}} {};",
                cname,
                fs.join("\n"),
                cname
            ));
        } else if d.kind == "struct" {
            for (_, ft, _, _) in &d.fields {
                self.need(ft);
            }
            let mut fs: Vec<String> = d
                .fields
                .iter()
                .map(|(fname, ft, _, _)| format!("  {} f_{};", self.ct(ft), san(fname)))
                .collect();
            if fs.is_empty() {
                fs.push("  uint8_t _dummy;".to_string());
            }
            self.type_defs.push(format!(
                "typedef struct {{\n{}\n}} {};",
                fs.join("\n"),
                san(&d.name)
            ));
        } else if d.kind == "enum" {
            // enum 落成 uint32_t，无 typedef
        } else if d.kind == "interface" {
            // interface 落成胖指针：obj 指向实现类实例，vt 指向该 (类, interface)
            // 组合专属的 shim 虚表（GAP #13）。方法序 = all_iface_methods（自身在前、
            // extend 的追加在后），与 shim 初始化、调用点按字段名访问保持一致。
            let entries = self.iface_entries(&d);
            for (_, ret, ptys, _) in &entries {
                self.need(ret);
                for p in ptys {
                    self.need(p);
                }
            }
            let iname = format!("Iface_{}", san(&d.name));
            let mut lines: Vec<String> = Vec::new();
            for (name, ret, ptys, _) in &entries {
                let mut ps = vec!["void*".to_string()];
                ps.extend(ptys.iter().map(|p| self.ct(p)));
                lines.push(format!(
                    "  {} (*{})({});",
                    self.ct(ret),
                    san(name),
                    ps.join(", ")
                ));
            }
            if lines.is_empty() {
                lines.push("  uint8_t _empty;".to_string());
            }
            self.type_defs.push(format!(
                "typedef struct {0}_VT {{\n{1}\n}} {0}_VT;",
                iname,
                lines.join("\n")
            ));
            self.type_defs.push(format!(
                "typedef struct {0} {{\n  void* obj;\n  {0}_VT* vt;\n}} {0};",
                iname
            ));
        }
    }

    /// interface 的虚表条目（拉平 extends 链）：方法名 → (返回类型，参数类型，签名 fid)。
    pub fn iface_entries(&self, d: &TypeDef) -> Vec<(String, Ty, Vec<Ty>, usize)> {
        self.s
            .all_iface_methods(d.id)
            .into_iter()
            .map(|(name, fid)| {
                let f = &self.s.funcs[fid];
                (
                    name,
                    f.ret.clone(),
                    f.params.iter().map(|p| p.1.clone()).collect(),
                    fid,
                )
            })
            .collect()
    }

    // ------------------------------------------------------------ 入口

    pub fn run(&mut self) -> String {
        let s = self.s;
        // 前向声明所有 class 及其虚表
        for d in &s.typedefs {
            if d.kind == "class" {
                let n = san(&d.name);
                self.forward
                    .push(format!("typedef struct {0} {0};", n));
                self.forward
                    .push(format!("typedef struct {0}_VT {0}_VT;", n));
            }
            if d.kind == "interface" {
                let n = format!("Iface_{}", san(&d.name));
                self.forward
                    .push(format!("typedef struct {0}_VT {0}_VT;", n));
                self.forward
                    .push(format!("typedef struct {0} {0};", n));
            }
        }
        // 结构体 / 类体定义
        for d in &s.typedefs {
            if d.kind == "class" || d.kind == "struct" {
                let d = d.clone();
                self.need_typedef(&d);
            }
        }
        // interface 结构（即使没被当类型引用，(类, interface) shim 虚表也需要它）
        for d in &s.typedefs {
            if d.kind == "interface" {
                let d = d.clone();
                self.need_typedef(&d);
            }
        }
        // 收集所有用到的类型
        for d in &s.typedefs {
            if d.kind == "class" || d.kind == "struct" {
                let d = d.clone();
                for (_, ft, _, _) in &d.fields {
                    self.need(ft);
                }
                if let Some(c) = d.ctor {
                    for (_, pt, _b) in self.s.funcs[c].params.clone() {
                        self.need(&pt);
                    }
                }
                let method_ids: Vec<usize> = d.methods.values().copied().collect();
                for fid in method_ids {
                    let f = &s.funcs[fid];
                    for (_, pt, _b) in f.params.clone() {
                        self.need(&pt);
                    }
                    let ret = f.ret.clone();
                    self.need(&ret);
                }
            }
        }
        for f in &s.funcs {
            if f.owner.is_some() {
                continue;
            }
            for (_, pt, _b) in f.params.clone() {
                self.need(&pt);
            }
            let ret = f.ret.clone();
            self.need(&ret);
        }
        let const_names: Vec<String> = s.consts.keys().cloned().collect();
        for n in &const_names {
            let c = s.consts[n].clone();
            let ty = c.ty.borrow().clone();
            self.need(&ty);
        }
        // 常量
        let consts_c = self.gen_consts();
        // 函数
        let mut fns: Vec<String> = Vec::new();
        let free_ids: Vec<usize> = (0..s.funcs.len())
            .filter(|&i| s.funcs[i].owner.is_none())
            .collect();
        for fid in free_ids {
            let cname = self.fn_name_idx(fid);
            fns.push(self.gen_func(fid, cname, None));
        }
        for d in &s.typedefs {
            if d.kind == "class" || d.kind == "struct" {
                let d = d.clone();
                fns.extend(self.gen_type_methods(&d));
            }
        }
        // interface 默认方法（带方法体的）：只在被 shim 引用时才有意义，
        // 但统一生成声明 + 定义最简单——无 body 的纯声明不生成
        for d in &s.typedefs {
            if d.kind == "interface" {
                let d = d.clone();
                for (_, fid) in &d.methods {
                    if self.s.funcs[*fid].body.is_some() {
                        let nm = format!(
                            "{}__{}__{}",
                            self.mod_name,
                            self.class_name(&d),
                            san(&self.s.funcs[*fid].name)
                        );
                        fns.push(self.gen_func(*fid, nm, Some(d.id)));
                    }
                }
            }
        }
        // vtable 定义（含 (类, interface) shim 虚表）
        let mut vts = self.build_vtables();
        vts.extend(self.build_iface_vtables());
        let base = std::path::Path::new(&self.path)
            .file_name()
            .map(|x| x.to_string_lossy().to_string())
            .unwrap_or_default();
        let mut body: Vec<String> = Vec::new();
        body.push(format!(
            "/* generated by axc {} from {} -- do not edit */",
            env!("CARGO_PKG_VERSION"),
            base
        ));
        body.push("#include \"axrt.h\"".to_string());
        body.push(String::new());
        // 历史行为（v0.9 起测试钉死）：forward 为空时也占一行，保持输出逐字节稳定
        body.push(self.forward.join("\n"));
        body.push(String::new());
        body.extend(self.type_defs.iter().cloned());
        body.push(String::new());
        body.extend(self.fn_decls.iter().cloned());
        body.push(String::new());
        body.extend(vts);
        body.push(String::new());
        body.extend(consts_c);
        body.push(String::new());
        body.extend(fns);
        body.extend(self.extra_fns.iter().cloned());
        body.push(String::new());
        body.push(self.gen_main_wrapper());
        body.join("\n")
    }

    // ------------------------------------------------------------ 命名

    pub fn fn_name_idx(&self, fid: usize) -> String {
        let f = &self.s.funcs[fid];
        match f.owner {
            Some(o) => {
                let d = self.s.typedefs[o as usize].clone();
                format!(
                    "{}__{}__{}",
                    self.mod_name,
                    self.class_name(&d),
                    san(&f.name)
                )
            }
            None => format!("{}__{}", self.mod_name, san(&f.name)),
        }
    }

    pub fn const_name(&self, name: &str) -> String {
        format!("k_{}__{}", self.mod_name, san(name))
    }

    pub fn lv(&self, name: &str) -> String {
        format!("v_{}", san(name))
    }

    pub fn pv(&self, name: &str) -> String {
        format!("p_{}", san(name))
    }

    // ------------------------------------------------------------ 常量

    pub fn gen_consts(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        let s = self.s;
        let mut names: Vec<&String> = s.consts.keys().collect();
        names.sort();
        for n in names {
            let c = s.consts[n].clone();
            let value = c.value.borrow().clone();
            let ty = c.ty.borrow().clone();
            let value = match value {
                Some(v) => v,
                None => continue,
            };
            // decimal 常量没有 C 静态初始化式（ax_dec_lit / ax_dec_from_i64 都是函数
            // 调用），而所有引用点本来就内联 const_c 的表达式，所以不落 C 变量。
            if matches!(&*ty, TyS::Decimal) {
                continue;
            }
            // __attribute__((unused))：只被编译期常量折叠用到的 const，
            // 在生成代码里可能一次都没出现，避免 -Werror 报 unused-const-variable
            out.push(format!(
                "static const {} {} __attribute__((unused)) = {};",
                self.ct(&ty),
                self.const_name(n),
                self.const_c(&value, &ty)
            ));
        }
        out
    }

    pub fn const_c(&self, v: &ConstVal, ty: &Ty) -> String {
        match (&**ty, v) {
            (TyS::Str, ConstVal::Str(x)) => format!("AX_S({})", cstr(x)),
            (TyS::Str, _) => "AX_S(\"\")".to_string(),
            (TyS::Bool, ConstVal::Bool(b)) => {
                if *b {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            (TyS::Bool, ConstVal::Int(i)) => {
                if *i != 0 {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            (TyS::Int(..), ConstVal::Int(i)) => format!("INT64_C({})", i),
            (TyS::Int(..), ConstVal::Bool(b)) => format!("INT64_C({})", *b as i64),
            (TyS::Float, ConstVal::Float(f)) => float_repr(*f),
            (TyS::Decimal, ConstVal::Dec(c, sc)) => {
                format!("ax_dec_lit(\"{}\", {})", c, sc)
            }
            (TyS::Decimal, ConstVal::Int(i)) => format!("ax_dec_from_i64({})", i),
            _ => "0".to_string(),
        }
    }

    // ------------------------------------------------------------ 函数

    pub fn gen_func(&mut self, fid: usize, cname: String, owner: Option<u32>) -> String {
        // self.s 是 &'s Sema，这里取到的 &'s FuncInfo 与 &mut self 互不冲突
        let s = self.s;
        let f: &'s crate::sema::FuncInfo<'a> = &s.funcs[fid];
        let mut fg = crate::backend::codegen_fn::FnGen::new(self, f, cname, owner, false);
        let decl = fg.decl();
        let body = fg.body(true);
        self.fn_decls.push(decl);
        body
    }

    pub fn gen_type_methods(&mut self, d: &TypeDef) -> Vec<String> {
        let mut out = Vec::new();
        if d.ctor.is_some() {
            out.push(self.gen_ctor(d));
        }
        let method_ids: Vec<usize> = d.methods.values().copied().collect();
        for fid in method_ids {
            let nm = format!(
                "{}__{}__{}",
                self.mod_name,
                self.class_name(d),
                san(&self.s.funcs[fid].name)
            );
            out.push(self.gen_func(fid, nm, Some(d.id)));
        }
        out
    }

    pub fn gen_ctor(&mut self, d: &TypeDef) -> String {
        let mut out = Vec::new();
        let nm = format!("{}__{}__init", self.mod_name, self.class_name(d));
        let ctor_fid = d.ctor;
        let ctor_params: Vec<(String, Ty, Option<String>)> = match ctor_fid {
            Some(c) => self.s.funcs[c].params.clone(),
            None => Vec::new(),
        };
        let mut ps = vec!["void* self_".to_string()];
        for (n, t, _b) in &ctor_params {
            ps.push(format!("{} {}", self.ct(t), self.pv(n)));
        }
        self.fn_decls
            .push(format!("static void {}({});", nm, ps.join(", ")));
        if let Some(c) = ctor_fid {
            let f: &'s crate::sema::FuncInfo<'a> = &self.s.funcs[c];
            let mut fg = crate::backend::codegen_fn::FnGen::new(self, f, nm.clone(), Some(d.id), true);
            out.push(fg.ctor_body(d, &nm));
        }
        // new
        let nm2 = format!("{}__{}__new", self.mod_name, self.class_name(d));
        let mut ps2 = Vec::new();
        for (n, t, _b) in &ctor_params {
            ps2.push(format!("{} {}", self.ct(t), self.pv(n)));
        }
        let ps2 = if ps2.is_empty() {
            "void".to_string()
        } else {
            ps2.join(", ")
        };
        self.fn_decls
            .push(format!("static {}* {}({});", san(&d.name), nm2, ps2));
        let mut call = vec!["(void*)t".to_string()];
        for (n, _, _) in &ctor_params {
            call.push(self.pv(n));
        }
        let gargs: Vec<Ty> = d
            .generics
            .iter()
            .map(|g| Rc::new(TyS::Param(g.0.clone())) as Ty)
            .collect();
        let tag = tag_of(&t_named(DefId::Id(d.id), gargs));
        out.push(format!(
            "static {0}* {1}({2}) {{\n  {0}* t = ({0}*)ax_calloc(1, (int64_t)sizeof({0}));\n  t->vt = &{3}__VT_;\n  t->__tag = {4}u;\n  {5}({6});\n  return t;\n}}",
            san(&d.name),
            nm2,
            ps2,
            san(&d.name),
            tag,
            nm,
            call.join(", ")
        ));
        out.join("\n")
    }

    pub fn build_vtables(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        for d in &self.s.typedefs {
            if d.kind != "class" {
                continue;
            }
            let d = d.clone();
            let entries = self.vt_entries(&d);
            let cname = san(&d.name);
            let mut lines = vec![
                format!("typedef struct {0}_VT {0}_VT;", cname),
                format!("struct {}_VT {{", cname),
            ];
            for (name, ret, ptys) in &entries {
                let mut ps = vec!["void*".to_string()];
                ps.extend(ptys.iter().map(|p| self.ct(p)));
                lines.push(format!("  {} (*{})({});", self.ct(ret), san(name), ps.join(", ")));
            }
            if entries.is_empty() {
                lines.push("  uint8_t _empty;".to_string());
            }
            lines.push("};".to_string());
            lines.push(format!("static {}_VT {}__VT_ = {{", cname, cname));
            for (i, (name, _ret, _ptys)) in entries.iter().enumerate() {
                let owner = self.vt_impl_owner(&d, name);
                let owner_d = self.s.typedefs[owner as usize].clone();
                let impl_ = format!(
                    "{}__{}__{}",
                    self.mod_name,
                    self.class_name(&owner_d),
                    san(name)
                );
                let sep = if i + 1 < entries.len() { "," } else { "" };
                lines.push(format!("  {}{}", impl_, sep));
            }
            if entries.is_empty() {
                lines.push("  0".to_string());
            }
            lines.push("};".to_string());
            out.push(lines.join("\n"));
        }
        out
    }

    /// (类, interface) shim 虚表：把实现类的方法（沿继承链找 override）按
    /// interface 方法序填进 `Iface_X_VT`。interface 接收者调用时经
    /// `tv.vt` 走到这里，实现动态派发（GAP #13）。
    pub fn build_iface_vtables(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        for d in &self.s.typedefs {
            if d.kind != "class" {
                continue;
            }
            let d = d.clone();
            // 本类 + 各级父类直接 implement 的 interface 全集
            let mut ifaces: Vec<u32> = Vec::new();
            let mut cur = Some(d.id);
            while let Some(c) = cur {
                let cd = &self.s.typedefs[c as usize];
                for &i in &cd.impls {
                    ifaces.push(i);
                }
                cur = cd.parent;
            }
            ifaces.sort();
            ifaces.dedup();
            // implement Pet 蕴含也实现 Pet extends 链上的所有父 interface，
            // 每个都要有 shim（`Animal a = new Dog()` 用的是 Dog__Animal__VT_）
            let mut expanded: Vec<u32> = Vec::new();
            for i in ifaces {
                let mut stack = vec![i];
                while let Some(x) = stack.pop() {
                    if expanded.contains(&x) {
                        continue;
                    }
                    expanded.push(x);
                    if let Some(td) = self.s.typedefs.get(x as usize) {
                        for &e in &td.extends {
                            stack.push(e);
                        }
                    }
                }
            }
            expanded.sort();
            expanded.dedup();
            for i in expanded {
                let id = self.s.typedefs[i as usize].clone();
                let entries = self.iface_entries(&id);
                let iname = format!("Iface_{}", san(&id.name));
                let vt_name = format!("{}__{}__VT_", san(&d.name), san(&id.name));
                let mut lines = vec![format!(
                    "__attribute__((unused)) static {}_VT {} = {{",
                    iname, vt_name
                )];
                for (k, (mname, _, _, sig_fid)) in entries.iter().enumerate() {
                    let sep = if k + 1 < entries.len() { "," } else { "" };
                    let entry = match self.class_impl_owner(&d, mname) {
                        Some(owner) => {
                            let od = self.s.typedefs[owner as usize].clone();
                            let impl_fid = match od.methods.get(mname) {
                                Some(&f) => f,
                                None => usize::MAX,
                            };
                            let direct = format!(
                                "{}__{}__{}",
                                self.mod_name,
                                self.class_name(&od),
                                san(mname)
                            );
                            // Self 适配（文稿 7.1）：签名参数声明为 interface
                            // （Self），实现参数是实现类——C 函数指针类型不同，
                            // 生成转换 thunk（取胖指针 obj 落到实现类指针）。
                            let adapter = match (impl_fid != usize::MAX)
                                .then(|| ())
                                .map(|_| {
                                    let impl_f = &self.s.funcs[impl_fid];
                                    let sig_f = &self.s.funcs[*sig_fid];
                                    (
                                        impl_f.params.iter().map(|p| p.1.clone()).collect::<Vec<_>>(),
                                        sig_f.params.iter().map(|p| p.1.clone()).collect::<Vec<_>>(),
                                        impl_f.ret.clone(),
                                    )
                                }) {
                                Some((ipts, spts, ret)) if ipts != spts => {
                                    let ad = format!(
                                        "{}__{}__{}__ad",
                                        self.mod_name,
                                        self.class_name(&od),
                                        san(mname)
                                    );
                                    let ret_c = self.ct(&ret);
                                    let mut ps_decl = vec!["void* self_".to_string()];
                                    let mut args = vec!["self_".to_string()];
                                    for (j, (sp, ip)) in spts.iter().zip(ipts.iter()).enumerate() {
                                        let pn = format!("p{}_{}", j, k);
                                        ps_decl.push(format!("{} {}", self.ct(sp), pn));
                                        let conv = match (&**sp, &**ip) {
                                            (
                                                TyS::Named(DefId::Id(i), _),
                                                TyS::Named(DefId::Id(c), _),
                                            ) => {
                                                let is_iface = self
                                                    .s
                                                    .typedefs
                                                    .get(*i as usize)
                                                    .map_or(false, |td| td.kind == "interface");
                                                let is_class = self
                                                    .s
                                                    .typedefs
                                                    .get(*c as usize)
                                                    .map_or(false, |td| td.kind == "class");
                                                if is_iface && is_class {
                                                    format!("({}){}.obj", self.ct(ip), pn)
                                                } else {
                                                    pn.clone()
                                                }
                                            }
                                            _ => pn.clone(),
                                        };
                                        args.push(conv);
                                    }
                                    let proto = format!(
                                        "static {} {}(void*, {});",
                                        ret_c,
                                        ad,
                                        spts
                                            .iter()
                                            .map(|p| self.ct(p))
                                            .collect::<Vec<_>>()
                                            .join(", ")
                                    );
                                    let def = format!(
                                        "static {} {}({}) {{\n  return {}({});\n}}",
                                        ret_c,
                                        ad,
                                        ps_decl.join(", "),
                                        direct,
                                        args.join(", ")
                                    );
                                    self.fn_decls.push(proto);
                                    self.extra_fns.push(def);
                                    ad
                                }
                                _ => direct,
                            };
                            adapter
                        }
                        None => {
                            // 类链上没有实现：用 interface 默认方法体
                            format!(
                                "{}__{}__{}",
                                self.mod_name,
                                self.class_name(&id),
                                san(mname)
                            )
                        }
                    };
                    lines.push(format!("  {}{}", entry, sep));
                }
                if entries.is_empty() {
                    lines.push("  0".to_string());
                }
                lines.push("};".to_string());
                out.push(lines.join("\n"));
            }
        }
        out
    }

    /// 实现方法在继承链上的落点（不含 interface 默认）；没有则 None。
    fn class_impl_owner(&self, d: &TypeDef, name: &str) -> Option<u32> {
        let mut cur = Some(d.id);
        while let Some(c) = cur {
            let cd = &self.s.typedefs[c as usize];
            if let Some(&fid) = cd.methods.get(name) {
                if !self.s.funcs[fid].is_static {
                    return Some(c);
                }
            }
            cur = cd.parent;
        }
        None
    }

    /// 父类在前，子类新增在后；override 替换实现。
    pub fn vt_entries(&self, d: &TypeDef) -> Vec<(String, Ty, Vec<Ty>)> {
        let mut chain = Vec::new();
        let mut cur = Some(d.id);
        while let Some(c) = cur {
            chain.push(c);
            cur = self.s.typedefs[c as usize].parent;
        }
        chain.reverse();
        let mut names: Vec<String> = Vec::new();
        let mut rets: HashMap<String, Ty> = HashMap::new();
        let mut ptys: HashMap<String, Vec<Ty>> = HashMap::new();
        for c in chain {
            let cd = self.s.typedefs[c as usize].clone();
            for (name, &fid) in &cd.methods {
                let f = &self.s.funcs[fid];
                if f.is_static {
                    continue;
                }
                if !names.contains(name) {
                    names.push(name.clone());
                }
                rets.insert(name.clone(), f.ret.clone());
                ptys.insert(
                    name.clone(),
                    f.params.iter().map(|p| p.1.clone()).collect(),
                );
            }
        }
        names
            .into_iter()
            .map(|n| {
                (
                    n.clone(),
                    rets[&n].clone(),
                    ptys.get(&n).cloned().unwrap_or_default(),
                )
            })
            .collect()
    }

    pub fn vt_impl_owner(&self, d: &TypeDef, name: &str) -> u32 {
        let mut cur = Some(d.id);
        while let Some(c) = cur {
            let cd = &self.s.typedefs[c as usize];
            if let Some(&fid) = cd.methods.get(name) {
                if !self.s.funcs[fid].is_static {
                    return c;
                }
            }
            cur = cd.parent;
        }
        d.id
    }

    pub fn gen_main_wrapper(&mut self) -> String {
        let main_fid = match self.s.func_by_name.get("main") {
            Some(&f) => f,
            None => return String::new(),
        };
        let nm = self.fn_name_idx(main_fid);
        let np = self.s.funcs[main_fid].params.len();
        let call = if np == 0 {
            format!("{}()", nm)
        } else {
            format!("{}(args)", nm)
        };
        [
            "int main(int argc, char** argv) {".to_string(),
            "  ax_console_utf8();".to_string(),
            "  ax_arr args;".to_string(),
            "  ax_arr_init(&args);".to_string(),
            "  for (int i = 1; i < argc; i++) {".to_string(),
            "    ax_str s = ax_str_from_cstr(argv[i]);".to_string(),
            "    ax_arr_push(&args, &s, (int64_t)sizeof(ax_str));".to_string(),
            "  }".to_string(),
            "  (void)args;".to_string(),
            format!("  int64_t rc = {};", call),
            "  if (rc < 0) rc = 1;".to_string(),
            "  if (rc > 255) rc = 255;".to_string(),
            "  return (int)rc;".to_string(),
            "}".to_string(),
        ]
        .join("\n")
    }

    /// 顶层入口：生成整份 C 源码。
    pub fn build_c(s: &'s Sema<'a>, path: &str) -> String {
        let mut cg = CodeGen::new(s, path);
        cg.run()
    }
}
