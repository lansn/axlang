//! 语义检查（规格书 §6–§11）：名称解析、类型检查、所有权、错误处理。
//!
//! 结构：
//! - 本文件是 `Sema` 主类与公共入口；语句级检查在 `stmt.rs`，
//!   表达式级与 coerce 在 `expr.rs`，成员访问 / 方法调用 / match / 常量求值在 `member.rs`。
//! - 语义槽统一放 `slots: HashMap<u32, Slot>`，键为节点 id（每个 `N<T>` 的 id
//!   全局唯一）；backend 只读 slots，不再重新检查。
//! - 类型一律用 `types::Ty`（`Rc<TyS>`），结构相等即可作 key；可赋值性只走
//!   `types::assignable`。
//!
//! 行为契约（一经发布不改，细则与沿革见根目录 CHANGELOG.md）：
//! - 诊断码与文案一经发布不改字；测试以期望输出钉死。
//! - E7008 永不触发——该检查自首次发布即不可达，为行为兼容保留，
//!   不要"顺手修正"。

mod expr;
mod member;
mod stmt;

use crate::frontend::ast;
use crate::frontend::ast::{Expr, Item, Stmt};
use crate::diag::Diags;
use crate::frontend::lexer::Tok;
use crate::frontend::lexer::TokVal;
use crate::types::*;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

pub const RESERVED_EXTRA: [&str; 9] = [
    "func", "map", "Self", "result", "error", "range", "Rounding", "Arena", "ArenaRef",
];

pub fn is_reserved(name: &str) -> bool {
    is_builtin_scalar(name) || RESERVED_EXTRA.contains(&name)
}

pub const BUILTIN_MODULES: [&str; 7] = ["db", "file", "io", "json", "math", "string", "time"];

pub fn is_builtin_type_name(name: &str) -> bool {
    is_builtin_scalar(name)
        || matches!(
            name,
            "object" | "result" | "map" | "func" | "error" | "Rounding" | "range" | "Arena"
                | "ArenaRef" | "string"
        )
}

// ---------------------------------------------------------------- 常量值

/// 常量求值（eval_const）产出的动态值。
#[derive(Clone, Debug, PartialEq)]
pub enum ConstVal {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Dec(i128, i32),
    Null,
    Rounding(String),
    Enum(u32, String),
}

/// 常量值的 repr 字符串（repr 语义），E6006 的重复字面量文案里会用到。
pub fn py_repr(v: &ConstVal) -> String {
    match v {
        ConstVal::Int(i) => i.to_string(),
        ConstVal::Float(f) => {
            if f.is_finite() {
                let s = format!("{}", f);
                if s.contains('.') || s.contains('e') {
                    s
                } else {
                    s + ".0"
                }
            } else if f.is_nan() {
                "nan".to_string()
            } else if *f > 0.0 {
                "inf".to_string()
            } else {
                "-inf".to_string()
            }
        }
        ConstVal::Bool(true) => "True".to_string(),
        ConstVal::Bool(false) => "False".to_string(),
        ConstVal::Str(s) => {
            let mut o = String::from("'");
            for c in s.chars() {
                match c {
                    '\\' => o.push_str("\\\\"),
                    '\'' => o.push_str("\\'"),
                    '\n' => o.push_str("\\n"),
                    '\r' => o.push_str("\\r"),
                    '\t' => o.push_str("\\t"),
                    c => o.push(c),
                }
            }
            o.push('\'');
            o
        }
        ConstVal::Dec(c, s) => format!("('dec', {}, {})", c, s),
        ConstVal::Null => "('null',)".to_string(),
        ConstVal::Rounding(n) => format!("('rounding', '{}')", n),
        ConstVal::Enum(id, n) => format!("('enum', {}, '{}')", id, n),
    }
}

enum Num {
    I(i128),
    F(f64),
}

fn as_num(v: &ConstVal) -> Option<Num> {
    match v {
        ConstVal::Int(i) => Some(Num::I(*i as i128)),
        ConstVal::Float(f) => Some(Num::F(*f)),
        ConstVal::Bool(b) => Some(Num::I(*b as i128)),
        _ => None,
    }
}

fn truthy(v: &ConstVal) -> bool {
    match v {
        ConstVal::Int(i) => *i != 0,
        ConstVal::Float(f) => *f != 0.0,
        ConstVal::Bool(b) => *b,
        ConstVal::Str(s) => !s.is_empty(),
        _ => true,
    }
}

fn num_eq(x: &Num, y: &Num) -> bool {
    num_cmp(x, y) == Some(std::cmp::Ordering::Equal)
}

/// 数值比较；类型不可比时返回 None（调用方按"非常量"处理，不报错）。
fn num_cmp(x: &Num, y: &Num) -> Option<std::cmp::Ordering> {
    match (x, y) {
        (Num::I(a), Num::I(b)) => Some(a.cmp(b)),
        _ => {
            let xf = match x {
                Num::I(i) => *i as f64,
                Num::F(f) => *f,
            };
            let yf = match y {
                Num::I(i) => *i as f64,
                Num::F(f) => *f,
            };
            xf.partial_cmp(&yf)
        }
    }
}

/// 常量二元折叠；任何类型不匹配返回 None（调用方放弃折叠，运行期再算）。
pub fn const_binop(op: &str, a: &ConstVal, b: &ConstVal) -> Option<ConstVal> {
    if op == "+" {
        if let (ConstVal::Str(x), ConstVal::Str(y)) = (a, b) {
            return Some(ConstVal::Str(x.clone() + y));
        }
    }
    match op {
        "==" => {
            if let (Some(x), Some(y)) = (as_num(a), as_num(b)) {
                return Some(ConstVal::Bool(num_eq(&x, &y)));
            }
            Some(ConstVal::Bool(match (a, b) {
                (ConstVal::Str(x), ConstVal::Str(y)) => x == y,
                (ConstVal::Dec(c1, s1), ConstVal::Dec(c2, s2)) => c1 == c2 && s1 == s2,
                (ConstVal::Null, ConstVal::Null) => true,
                (ConstVal::Rounding(x), ConstVal::Rounding(y)) => x == y,
                (ConstVal::Enum(i1, n1), ConstVal::Enum(i2, n2)) => i1 == i2 && n1 == n2,
                _ => false,
            }))
        }
        "!=" => match const_binop("==", a, b) {
            Some(ConstVal::Bool(x)) => Some(ConstVal::Bool(!x)),
            _ => None,
        },
        "<" | "<=" | ">" | ">=" => {
            let ord = match (as_num(a), as_num(b)) {
                (Some(x), Some(y)) => num_cmp(&x, &y)?,
                (None, None) => match (a, b) {
                    (ConstVal::Str(x), ConstVal::Str(y)) => x.cmp(y),
                    (ConstVal::Dec(c1, s1), ConstVal::Dec(c2, s2)) => (c1, s1).cmp(&(c2, s2)),
                    _ => return None,
                },
                _ => return None,
            };
            use std::cmp::Ordering::*;
            let r = match op {
                "<" => ord == Less,
                "<=" => ord != Greater,
                ">" => ord == Greater,
                _ => ord != Less,
            };
            Some(ConstVal::Bool(r))
        }
        "&&" => Some(ConstVal::Bool(truthy(a) && truthy(b))),
        "||" => Some(ConstVal::Bool(truthy(a) || truthy(b))),
        _ => {
            let (x, y) = (as_num(a)?, as_num(b)?);
            match (x, y) {
                (Num::I(x), Num::I(y)) => {
                    let r = match op {
                        "+" => x.wrapping_add(y),
                        "-" => x.wrapping_sub(y),
                        "*" => x.wrapping_mul(y),
                        "/" => {
                            if y == 0 {
                                return None;
                            }
                            let q = x.abs() / y.abs();
                            if (x >= 0) == (y >= 0) {
                                q
                            } else {
                                -q
                            }
                        }
                        "%" => {
                            if y == 0 {
                                return None;
                            }
                            let mut q = x.abs() / y.abs();
                            if (x >= 0) != (y >= 0) {
                                q = -q;
                            }
                            x - q * y
                        }
                        "&" => x & y,
                        "|" => x | y,
                        "^" => x ^ y,
                        "<<" => {
                            if !(0..=120).contains(&y) {
                                return None;
                            }
                            x.wrapping_shl(y as u32)
                        }
                        ">>" => {
                            if !(0..=120).contains(&y) {
                                return None;
                            }
                            x.wrapping_shr(y as u32)
                        }
                        _ => return None,
                    };
                    Some(ConstVal::Int(r as i64))
                }
                (x, y) => {
                    let xf = match x {
                        Num::F(f) => f,
                        Num::I(i) => i as f64,
                    };
                    let yf = match y {
                        Num::F(f) => f,
                        Num::I(i) => i as f64,
                    };
                    let r = match op {
                        "+" => xf + yf,
                        "-" => xf - yf,
                        "*" => xf * yf,
                        "/" => {
                            if yf == 0.0 {
                                return None;
                            }
                            xf / yf
                        }
                        "%" => {
                            if yf == 0.0 {
                                return None;
                            }
                            xf - (xf / yf).floor() * yf
                        }
                        _ => return None,
                    };
                    Some(ConstVal::Float(r))
                }
            }
        }
    }
}

// ---------------------------------------------------------------- 符号

/// 变量所有权状态（Move 语义追踪）。
#[derive(Clone, Debug, PartialEq)]
pub enum VarState {
    /// 可用
    Live,
    /// 已被移走，再使用报所有权错误
    Moved,
}

/// 一个变量（局部/参数/常量绑定）的全部静态信息。
#[derive(Clone, Debug)]
pub struct VarInfo {
    pub name: String,
    pub ty: Ty,
    pub is_const: bool,
    pub state: VarState,
    pub decl_line: u32,
    pub is_param: bool,
    pub idx: usize,
}

pub struct ConstCell {
    pub ty: RefCell<Ty>,
    pub value: RefCell<Option<ConstVal>>,
}

impl std::fmt::Debug for ConstCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConstCell").finish()
    }
}

pub struct FuncInfo<'a> {
    pub name: String,
    pub tok: Tok,
    pub module: String,
    pub owner: Option<u32>,
    pub params: Vec<(String, Ty, Option<String>)>,
    pub ret: Ty,
    pub generics: Vec<(String, Vec<String>)>,
    pub vis: String,
    pub is_static: bool,
    pub is_override: bool,
    pub body: Option<&'a ast::Block>,
    /// 隐式 `main`：入口文件的顶层语句包出来的（文稿 9.4）
    pub implicit: bool,
}

#[derive(Clone)]
pub struct TypeDef {
    pub id: u32,
    pub name: String,
    pub kind: &'static str, // class | struct | interface | enum | alias
    pub tok: Tok,
    pub vis: String,
    pub generics: Vec<(String, Vec<String>)>,
    pub fields: Vec<(String, Ty, String, u32)>,
    pub methods: BTreeMap<String, usize>,
    pub ctor: Option<usize>,
    pub ctor_count: u32,
    pub parent: Option<u32>,
    pub impls: Vec<u32>,
    pub extends: Vec<u32>,
    pub variants: Vec<String>,
    pub alias_target: Option<Ty>,
    pub alias_node: Option<ast::TPtr>,
    pub nested: BTreeMap<String, u32>,
    pub expanding: bool,
}

pub struct ScopeNode {
    pub parent: Option<Rc<RefCell<ScopeNode>>>,
    pub names: HashMap<String, Rc<RefCell<VarInfo>>>,
}

// ---------------------------------------------------------------- 语义槽

/// 名称解析结果：一个标识符最终解析到什么（局部变量 / 常量 / 函数 / 类型 /
/// 内建 / 模块 / 枚举变体……），决定 backend 走哪条生成路径。
#[derive(Clone, Debug)]
pub enum Res {
    Local(Rc<RefCell<VarInfo>>),
    Const(Rc<ConstCell>),
    Func(usize),
    Type(u32),
    BuiltinType(String),
    Module(String),
    Builtin(String),
    Parent,
    EnumVariant(u32, String),
    Rounding(String),
    StaticBuiltin(String, String),
}

/// 节点上挂的语义槽：sema 检查的全部结论（类型、名称解析结果、转换方式、
/// 常量折叠值……）按节点 id 写在这里，backend 只读取不再检查。
/// 字段多为 Option——按节点种类部分填充，见各检查点的写入处。
#[derive(Clone, Default, Debug)]
pub struct Slot {
    pub ty: Option<Ty>,
    pub res: Option<Res>,
    pub conv: Option<&'static str>,
    pub conv_to: Option<Ty>,
    pub cast_to: Option<Ty>,
    pub dec_conv: Option<&'static str>,
    pub folded: Option<ConstVal>,
    pub as_dec: bool,
    pub is_move: bool,
    pub ckind: Option<&'static str>,
    pub bname: Option<String>,
    pub func: Option<usize>,
    pub generic_args: Vec<Ty>,
    pub mkind: Option<&'static str>,
    pub method: Option<usize>,
    pub method_owner: Option<u32>,
    pub field_owner: Option<u32>,
    pub field_idx: u32,
    pub recv_ty: Option<Ty>,
    pub scrut_ty: Option<Ty>,
    pub parse_to: Option<Ty>,
    pub trunc_to: Option<Ty>,
    pub parent_def: Option<u32>,
    pub decl_ty: Option<Ty>,
    pub inner_node: Option<ast::EPtr>,
    pub key: Option<String>,
    pub varinfo: Option<Rc<RefCell<VarInfo>>>,
    pub enum_variant: Option<(u32, String)>,
    pub const_value: Option<ConstVal>,
    /// 在 `try` 块内，本节点的 `result<T>` 被隐式展开为 `T`（等价于自动加了 `try`）。
    /// codegen 见到此标记时按"失败跳 catch"生成。
    pub implicit_try: bool,
}

// ---------------------------------------------------------------- Sema

/// 语义检查主上下文：类型表、函数表、常量表、作用域链、
/// 收窄状态、控制流深度（loop/try），以及最终的语义槽 `slots`。
pub struct Sema<'a> {
    pub mod_: &'a ast::Module,
    pub d: &'a mut Diags,
    pub path: String,
    pub require_main: bool,
    pub types: HashMap<String, u32>,
    pub typedefs: Vec<TypeDef>,
    pub funcs: Vec<FuncInfo<'a>>,
    pub func_by_name: HashMap<String, usize>,
    pub consts: HashMap<String, Rc<ConstCell>>,
    pub imports: HashSet<String>,
    pub decl_order: Vec<&'a ast::N<Item>>,
    pub scope: Option<Rc<RefCell<ScopeNode>>>,
    pub current_func: Option<usize>,
    pub cur_class: Option<u32>,
    pub generic_env: HashMap<String, Ty>,
    pub generic_cons: HashMap<String, Vec<String>>,
    pub narrow: Vec<HashMap<String, bool>>,
    pub in_result: bool,
    pub in_main: bool,
    /// `try { }` 块的嵌套深度。>0 时 `throw` / `try` 有就地捕获目标，
    /// 不再要求所在函数返回 `result<...>`；同时 `result<T>` 可隐式展开为 `T`。
    pub try_depth: usize,
    /// 每层 try 块进入时的 `loop_depth`（E6013 判据基准）。
    /// `break`/`continue` 若当前 `loop_depth` 不深于该基准，说明目标是 try 块
    /// **之外**的循环，跳转会越过 finally —— 那时报 E6013。
    /// 注意只能用 `loop_depth`：`loop_labels` 只在带标签的循环上入栈，
    /// 无标签循环不占位，用它会把合法的 `try { for { break } }` 误判。
    pub try_loop_base: Vec<u32>,
    pub loop_depth: u32,
    pub loop_labels: Vec<String>,
    pub iter_targets: Vec<String>,
    pub next_type_id: u32,
    pub slots: HashMap<u32, Slot>,
    pub checked_funcs: HashSet<usize>,
    /// 匿名函数合成的 FuncInfo 下标（文稿 4.1；codegen 据此生成顶层函数）
    pub anon_funcs: Vec<usize>,
    /// 导入的用户模块（cli 的 loader 加载，拓扑序：被导入者在前）
    pub extras: Vec<ExtraMod<'a>>,
    /// 导入写法（"utils" / "a.b"，取 alias 或完整路径）→ 规范前缀。
    /// 全局表：loader 保证同一写法只指向一个模块文件。
    pub path_canon: HashMap<String, String>,
    /// 规范前缀 → 导出表（仅 `public` 顶层符号；具名导入在解析处过滤）
    pub module_exports: HashMap<String, HashMap<String, ExportSym>>,
    /// 规范前缀 → 该模块全部顶层符号（含私有，供模块自身裸名解析）
    mod_syms: HashMap<String, Vec<(String, ExportSym)>>,
    /// 规范前缀 → 该模块的导入视图（view → canon 或 "*builtin*"）
    mod_views: HashMap<String, HashMap<String, String>>,
    /// 规范前缀 → 该模块的具名导入限制（view → 白名单；无限制为 None）
    mod_view_names: HashMap<String, HashMap<String, Option<Vec<String>>>>,
    /// `模块|视图` → 产生该视图的 import 原文（短名冲突时给出"被谁占了"）
    view_src: HashMap<String, String>,
    /// 规范前缀 → 模块文件路径（诊断归属）
    mod_paths: HashMap<String, String>,
    /// 已报过的裸类型名歧义（`文件:行:名字`）——同一行既有类型标注又有 `new` 时只报一次
    bare_type_ambig: HashSet<String>,
    /// 当前正在检查的模块：None = 入口文件，Some = 用户模块前缀。
    /// 决定裸名解析的限定名与模块级符号的注册名。
    cur_mod_prefix: Option<String>,
    /// 当前模块的文件路径（FuncInfo.module 用）
    cur_mod_path: String,
    /// 当前模块生效的导入视图
    active_views: HashMap<String, String>,
    /// 当前模块生效的具名导入限制
    active_view_names: HashMap<String, Option<Vec<String>>>,
    /// decl_order 平行的所属模块（None = 入口文件）
    decl_mod: Vec<Option<String>>,
    /// typedefs 平行的所属模块
    typedef_mod: Vec<Option<String>>,
    next_node_id: u32,
}

/// 用户模块的导出符号（模块内 `public` 的顶层声明）。
#[derive(Clone, Debug)]
pub enum ExportSym {
    /// 顶层函数在 `funcs` 表中的下标（注册名已带 `模块__` 前缀）
    Func(usize),
    /// 模块级 `const`
    Const,
    /// 类型（class/struct/interface/enum/alias）在 `typedefs` 表中的下标
    Type(u32),
}

/// cli 阶段加载好的一个用户模块（文稿 9.1：一个 .ax 文件 = 一个模块）。
pub struct ExtraMod<'a> {
    /// 规范前缀：C 符号与 sema 注册名的前缀（取文件名 sanitized，冲突时加序号）
    pub canon: String,
    /// 模块文件路径（诊断定位 + `FuncInfo.module`，用于跨模块可见性判定）
    pub file_path: String,
    /// 已解析的模块 AST
    pub module: &'a ast::Module,
}

pub fn dummy_tok() -> Tok {
    Tok {
        kind: String::new(),
        val: TokVal::Text(String::new()),
        line: 1,
        col: 1,
        eline: 1,
        ecol: 2,
        i0: 0,
        i1: 0,
    }
}

impl<'a> Sema<'a> {
    pub fn new(mod_: &'a ast::Module, d: &'a mut Diags, path: &str, require_main: bool) -> Self {
        let mut sema = Sema {
            mod_,
            d,
            path: path.to_string(),
            require_main,
            types: HashMap::new(),
            typedefs: Vec::new(),
            funcs: Vec::new(),
            func_by_name: HashMap::new(),
            consts: HashMap::new(),
            imports: HashSet::new(),
            decl_order: Vec::new(),
            scope: None,
            current_func: None,
            cur_class: None,
            generic_env: HashMap::new(),
            generic_cons: HashMap::new(),
            narrow: Vec::new(),
            in_result: false,
            in_main: false,
            try_depth: 0,
            try_loop_base: Vec::new(),
            loop_depth: 0,
            loop_labels: Vec::new(),
            iter_targets: Vec::new(),
            next_type_id: 1,
            slots: HashMap::new(),
            checked_funcs: HashSet::new(),
            anon_funcs: Vec::new(),
            extras: Vec::new(),
            path_canon: HashMap::new(),
            module_exports: HashMap::new(),
            mod_syms: HashMap::new(),
            mod_views: HashMap::new(),
            mod_view_names: HashMap::new(),
            view_src: HashMap::new(),
            mod_paths: HashMap::new(),
            cur_mod_prefix: None,
            cur_mod_path: String::new(),
            active_views: HashMap::new(),
            active_view_names: HashMap::new(),
            decl_mod: Vec::new(),
            typedef_mod: Vec::new(),
            bare_type_ambig: HashSet::new(),
            next_node_id: 1_000_000_000,
        };
        sema.seed_builtin_ifaces();
        sema
    }

    /// 内建 interface（文稿 7.1）：`Comparable` / `Hashable` / `Printable`
    /// 预置在根模块，无需 import，用户不可重名声明（types 表占用 → E3018）。
    /// 方法签名是占位 FuncInfo（无 body）：实现类必须提供实现，
    /// `Comparable.compare(Self other)` 的 Self 在实现方写实现类类型即可（见
    /// implement 签名对照处的 Self 放宽）。
    fn seed_builtin_ifaces(&mut self) {
        let mut ids: HashMap<&str, u32> = HashMap::new();
        for name in ["Comparable", "Hashable", "Printable"] {
            // id 约定 == typedefs 下标（见 new_typedef），预置的三条占 0..3
            let id = self.typedefs.len() as u32;
            self.next_type_id += 1;
            self.typedefs.push(TypeDef {
                id,
                name: name.to_string(),
                kind: "interface",
                tok: dummy_tok(),
                vis: "public".to_string(),
                generics: Vec::new(),
                fields: Vec::new(),
                methods: BTreeMap::new(),
                ctor: None,
                ctor_count: 0,
                parent: None,
                impls: Vec::new(),
                extends: Vec::new(),
                variants: Vec::new(),
                alias_target: None,
                alias_node: None,
                nested: BTreeMap::new(),
                expanding: false,
            });
            // typedef_mod 与 typedefs 平行：预置的三条都属于根模块
            self.typedef_mod.push(None);
            self.types.insert(name.to_string(), id);
            ids.insert(name, id);
        }
        let sigs: [(&str, &str, Vec<(&str, Ty)>, Ty); 3] = [
            (
                "Comparable",
                "compare",
                vec![(
                    "other",
                    t_named(
                        DefId::Id(*ids.get("Comparable").unwrap()),
                        Vec::new(),
                    ),
                )],
                t_int(),
            ),
            ("Hashable", "hash", vec![], t_uint()),
            ("Printable", "to_string", vec![], t_string()),
        ];
        for (iname, mname, params, ret) in sigs {
            let owner = *ids.get(iname).unwrap();
            let f = FuncInfo {
                name: mname.to_string(),
                tok: dummy_tok(),
                module: String::new(),
                owner: Some(owner),
                params: params
                    .into_iter()
                    .map(|(n, t)| (n.to_string(), t, None))
                    .collect(),
                ret,
                generics: Vec::new(),
                vis: "public".to_string(),
                is_static: false,
                is_override: false,
                body: None,
                implicit: false,
            };
            self.funcs.push(f);
            let fid = self.funcs.len() - 1;
            self.typedefs[owner as usize]
                .methods
                .insert(mname.to_string(), fid);
        }
    }

    // ------------------------------------------------------ 用户模块（文稿 9）

    /// 注册名：入口文件符号保持裸名；用户模块符号带 `模块__` 前缀，
    /// 避免跨模块同名冲突（文稿 9.1：文件内符号默认私有，`public` 导出）。
    fn reg_name(&self, name: &str) -> String {
        match &self.cur_mod_prefix {
            Some(p) => format!("{}__{}", p, name),
            None => name.to_string(),
        }
    }

    /// 裸名解析的兜底限定：先查裸名（入口符号 / 局部名），再查当前模块的限定名。
    fn qual_lookup<'m, V>(&'m self, map: &'m HashMap<String, V>, name: &str) -> Option<&'m V> {
        if let Some(v) = map.get(name) {
            return Some(v);
        }
        if let Some(p) = &self.cur_mod_prefix {
            if let Some(v) = map.get(&format!("{}__{}", p, name)) {
                return Some(v);
            }
        }
        // 具名导入把名字引入了当前作用域（文稿 9.2）：`import m.{a}` 之后裸 `a` 也要能解析。
        let c = self.named_import_canon(name)?;
        map.get(&format!("{}__{}", c, name))
    }

    /// 具名导入引入的名字 → 所属模块的 canon。
    /// 多个模块引入同名符号时按视图名字典序取第一个，行为确定、不随 HashMap 顺序漂移。
    fn named_import_canon(&self, name: &str) -> Option<String> {
        let mut hits: Vec<(&String, &String)> = self
            .active_view_names
            .iter()
            .filter(|(_, names)| matches!(names, Some(ns) if ns.iter().any(|n| n == name)))
            .filter_map(|(view, _)| self.active_views.get(view).map(|c| (view, c)))
            .collect();
        hits.sort_by(|a, b| a.0.cmp(b.0));
        hits.first().map(|(_, c)| (*c).clone())
    }

    /// 裸类型名解析（文稿 9.2）：入口/本模块符号 → 具名导入 → 已导入模块的公开同名类型。
    /// 最后一档是"`import catalog.product` 之后直接写 `Product`"，
    /// 与 `catalog.product.Product` / 末段短名 `product.Product` 三者等价。
    pub(crate) fn lookup_type_name(&mut self, name: &str, tok: &Tok) -> Option<u32> {
        if let Some(&d) = self.types.get(name) {
            return Some(d);
        }
        if let Some(p) = self.cur_mod_prefix.clone() {
            if let Some(&d) = self.types.get(&format!("{}__{}", p, name)) {
                return Some(d);
            }
        }
        if let Some(c) = self.named_import_canon(name) {
            if let Some(&d) = self.types.get(&format!("{}__{}", c, name)) {
                return Some(d);
            }
        }
        self.imported_type_bare(name, tok)
    }

    /// 在已导入模块的公开导出里找同名类型。唯一命中直接用；
    /// 两个模块都导出同名类型时报 `E3042`——裸名此时没有唯一答案，猜一个必错。
    fn imported_type_bare(&mut self, name: &str, tok: &Tok) -> Option<u32> {
        let views: Vec<(String, String)> = {
            let mut v: Vec<(String, String)> = self
                .active_views
                .iter()
                .map(|(k, c)| (k.clone(), c.clone()))
                .collect();
            v.sort();
            v
        };
        // (视图名, typedef id)，同一模块的多个视图（全路径 / 短名 / as）只算一次
        let mut hits: Vec<(String, u32)> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        for (view, canon) in views {
            if canon == "*builtin*" || seen.contains(&canon) {
                continue;
            }
            if !self.export_allowed(&view, name) {
                continue;
            }
            if let Some(ExportSym::Type(d)) = self.module_exports.get(&canon).and_then(|m| m.get(name)) {
                seen.push(canon);
                hits.push((view, *d));
            }
        }
        if hits.is_empty() {
            return None;
        }
        if hits.len() > 1 {
            let mut srcs: Vec<String> = Vec::new();
            for (v, _) in &hits {
                let s = self.view_display(v);
                if !srcs.contains(&s) {
                    srcs.push(s);
                }
            }
            // 同一行常常「类型标注 + new」各解析一次，只报一条
            let key = format!("{}:{}:{}", self.cur_mod_path, tok.line, name);
            if self.bare_type_ambig.insert(key) {
                self.err(
                    "E3042",
                    tok,
                    "文稿 9.2",
                    "`{0}` 有歧义：{1} 都导出了同名类型；请写全路径 `{2}.{3}` 或改用 `import {4}.{{{5}}}`",
                    &[
                        &name,
                        &srcs.join(" 与 "),
                        &srcs[0],
                        &name,
                        &srcs[0],
                        &name,
                    ],
                );
            }
            return Some(hits[0].1);
        }
        Some(hits[0].1)
    }

    /// decl_mod/typedef_mod 元素（所属模块）→ 恢复该模块的检查环境。
    /// 之后所有裸名解析经 qual_lookup 落到 `canon__name`，导入视图也随之生效。
    fn set_active_module(&mut self, canon: &Option<String>) {
        match canon {
            None => {
                self.cur_mod_prefix = None;
                self.cur_mod_path = self.path.clone();
                // 入口文件的视图存在 canon="" 下
                self.active_views = self.mod_views.get("").cloned().unwrap_or_default();
                self.active_view_names = self
                    .mod_view_names
                    .get("")
                    .cloned()
                    .unwrap_or_default();
            }
            Some(c) => {
                self.cur_mod_prefix = Some(c.clone());
                self.cur_mod_path = self.mod_paths.get(c).cloned().unwrap_or_default();
                self.active_views = self.mod_views.get(c).cloned().unwrap_or_default();
                self.active_view_names = self
                    .mod_view_names
                    .get(c)
                    .cloned()
                    .unwrap_or_default();
            }
        }
    }

    /// 模块视图解析：精确匹配优先，其次点号路径前缀（`import net.http` 时
    /// `net` 也是合法的中间视图）。返回 (canon, 是否精确匹配)。
    pub(crate) fn view_canon_of(&self, m: &str) -> Option<(String, bool)> {
        if let Some(c) = self.active_views.get(m) {
            return Some((c.clone(), true));
        }
        let dot = format!("{}.", m);
        if self.active_views.keys().any(|v| v.starts_with(&dot)) {
            return Some(("__prefix__".to_string(), false));
        }
        None
    }

    /// 解析表达式路径前缀指向的导入模块：返回 (匹配段数, 规范前缀)。
    /// 视图名取 alias 或完整导入路径，最长匹配优先（`import a.b` → `a.b.foo`）。
    fn view_of(&self, segs: &[String]) -> Option<(usize, String)> {
        for k in (1..=segs.len()).rev() {
            let v = segs[..k].join(".");
            if let Some(c) = self.active_views.get(&v) {
                return Some((k, c.clone()));
            }
        }
        None
    }

    /// 登记一条导入视图（全路径 / `as` 别名 / 末段短名）。
    /// 同一模块里两个 `import` 抢同一个顶层名字时报 `E3040`：末段短名是编译器自动生成的，
    /// 静默覆盖会让 `http.ping()` 悄悄指到另一个模块，这种错查起来极费劲。
    fn insert_view(
        &mut self,
        owner: &str,
        view: &str,
        canon: &str,
        names: Option<&Vec<String>>,
        tok: &Tok,
        src: &str,
    ) -> bool {
        let key = format!("{}|{}", owner, view);
        if let Some(old) = self
            .mod_views
            .get(owner)
            .and_then(|m| m.get(view))
            .cloned()
        {
            if old != canon {
                let prev = self.view_src.get(&key).cloned().unwrap_or_default();
                self.err(
                    "E3040",
                    tok,
                    "文稿 9.2",
                    "`{0}` 这个名字已经被 `import {1}` 占用，`import {2}` 不能再使用；请写成 `import {3} as 别名` 加以区分",
                    &[&view, &prev, &src, &src],
                );
                return false;
            }
        }
        self.mod_views
            .entry(owner.to_string())
            .or_default()
            .insert(view.to_string(), canon.to_string());
        if let Some(ns) = names {
            self.mod_view_names
                .entry(owner.to_string())
                .or_default()
                .insert(view.to_string(), Some(ns.clone()));
        }
        self.view_src.entry(key).or_insert_with(|| src.to_string());
        true
    }

    /// 视图名 → 产生它的 import 原文（诊断用；短名视图显示全路径）。
    pub(crate) fn view_display(&self, view: &str) -> String {
        let owner = self.cur_mod_prefix.clone().unwrap_or_default();
        self.view_src
            .get(&format!("{}|{}", owner, view))
            .cloned()
            .unwrap_or_else(|| view.to_string())
    }

    /// 导出符号的白名单校验（具名导入 `import m.{a, b}` 只放行列出的名字）。
    fn export_allowed(&self, view: &str, export: &str) -> bool {
        match self.active_view_names.get(view) {
            Some(Some(names)) => names.iter().any(|n| n == export),
            _ => true,
        }
    }

    // ---------------------------------------------------------------- 工具

    pub fn err(
        &mut self,
        code: &str,
        tok: &Tok,
        rule: &str,
        zh: &str,
        args: &[&dyn std::fmt::Display],
    ) {
        // 诊断归属当前正在检查的模块文件（用户模块的错误行号对应模块文件本身）
        let path = if self.cur_mod_path.is_empty() {
            self.path.clone()
        } else {
            self.cur_mod_path.clone()
        };
        let msg = crate::diag_msg::tr(self.d.lang, code, zh, args);
        self.d
            .error(code, msg, &path, tok.line, tok.col, tok.eline, tok.ecol, rule);
    }

    pub fn err_dummy(
        &mut self,
        code: &str,
        rule: &str,
        zh: &str,
        args: &[&dyn std::fmt::Display],
    ) {
        let t = dummy_tok();
        self.err(code, &t, rule, zh, args);
    }

    pub fn sl(&mut self, id: u32) -> &mut Slot {
        self.slots.entry(id).or_default()
    }

    pub fn ty_of(&self, id: u32) -> Option<Ty> {
        self.slots.get(&id).and_then(|s| s.ty.clone())
    }

    pub fn res_of(&self, id: u32) -> Option<Res> {
        self.slots.get(&id).and_then(|s| s.res.clone())
    }

    pub fn new_node_id(&mut self) -> u32 {
        self.next_node_id += 1;
        self.next_node_id
    }

    pub fn namer_name(&self, defid: DefId) -> String {
        match defid {
            DefId::Name(s) => s.to_string(),
            DefId::Id(i) => match self.typedefs.get(i as usize) {
                Some(d) => d.name.clone(),
                None => format!("type#{}", i),
            },
        }
    }

    pub fn canon(&self, ty: &Ty) -> String {
        canonical(ty, Some(&|d| self.namer_name(d)))
    }

    pub fn ts(&self, ty: &Ty) -> String {
        ty_str(ty, Some(&|d| self.namer_name(d)))
    }

    fn new_typedef(&mut self, name: &str, kind: &'static str, tok: &Tok) -> u32 {
        let id = self.typedefs.len() as u32;
        self.typedef_mod.push(self.cur_mod_prefix.clone());
        self.typedefs.push(TypeDef {
            id,
            name: name.to_string(),
            kind,
            tok: tok.clone(),
            vis: "private".to_string(),
            generics: Vec::new(),
            fields: Vec::new(),
            methods: BTreeMap::new(),
            ctor: None,
            ctor_count: 0,
            parent: None,
            impls: Vec::new(),
            extends: Vec::new(),
            variants: Vec::new(),
            alias_target: None,
            alias_node: None,
            nested: BTreeMap::new(),
            expanding: false,
        });
        self.next_type_id += 1;
        id
    }

    pub fn is_subclass(&self, sub_id: u32, super_id: u32) -> bool {
        let mut cur = self.typedefs.get(sub_id as usize).and_then(|d| d.parent);
        while let Some(p) = cur {
            if p == super_id {
                return true;
            }
            cur = self.typedefs.get(p as usize).and_then(|d| d.parent);
        }
        false
    }

    pub fn implements(&self, class_id: u32, iface_id: u32) -> bool {
        let mut cur = Some(class_id);
        while let Some(c) = cur {
            if let Some(d) = self.typedefs.get(c as usize) {
                for &i in &d.impls {
                    if i == iface_id {
                        return true;
                    }
                    if let Some(id) = self.typedefs.get(i as usize) {
                        if id.extends.iter().any(|&e| e == iface_id) {
                            return true;
                        }
                    }
                }
                cur = d.parent;
            } else {
                cur = None;
            }
        }
        false
    }

    /// 该类型的值能否直接进字符串插值——在 interp_supported 之外，
    /// class / struct（含可空形式）实现内建 `Printable`（`to_string()`）即可
    /// （文稿 7.1 / 11.2，GAP #12 剩余部分的接入点）。
    pub fn implements_printable(&self, t: &Ty) -> bool {
        let cid = match &**t {
            TyS::Named(DefId::Id(i), _) => *i,
            TyS::Nullable(inner) => match &**inner {
                TyS::Named(DefId::Id(i), _) => *i,
                _ => return false,
            },
            _ => return false,
        };
        let pd = match self.types.get("Printable") {
            Some(&p) => p,
            None => return false,
        };
        let td = match self.typedefs.get(cid as usize) {
            Some(td) => td,
            None => return false,
        };
        // Printable 类型变量本身（接口引用，动态派发 to_string）
        if td.kind == "interface" && cid == pd {
            return true;
        }
        (td.kind == "class" || td.kind == "struct") && self.implements(cid, pd)
    }

    // ---------------------------------------------------------------- 收集

    fn collect(&mut self) {
        // 模块处理顺序：extras（拓扑序，被导入者在前）→ 入口文件。
        // extras 的顶层符号以 `canon__name` 注册进共享表并记入导出表；
        // 入口文件符号保持裸名。
        let mut order: Vec<(Option<String>, &'a ast::Module, String)> = self
            .extras
            .iter()
            .map(|e| (Some(e.canon.clone()), e.module, e.file_path.clone()))
            .collect();
        order.push((None, self.mod_, self.path.clone()));

        for (prefix, m, fpath) in order {
            self.cur_mod_prefix = prefix.clone();
            self.cur_mod_path = fpath.clone();
            let canon = prefix.clone().unwrap_or_default();
            if let Some(p) = prefix.clone() {
                self.mod_paths.insert(p, fpath);
            }
            for item in &m.items {
                match &item.k {
                    Item::Import { path, alias, names } => {
                        let joined = path.join(".");
                        let view = alias.clone().unwrap_or_else(|| joined.clone());
                        let builtin = BUILTIN_MODULES.contains(&path[0].as_str());
                        let target = if builtin {
                            "*builtin*".to_string()
                        } else {
                            match self.path_canon.get(&joined) {
                                Some(c) => c.clone(),
                                None => {
                                    // loader 没加载到：文件缺失或写法未注册
                                    self.err("E3027", &item.tok, "文稿 9.2", "找不到模块 `{0}`（模块路径即目录结构：`import a.b` 对应入口文件同目录的 `a/b.ax`）", &[&joined]);
                                    continue;
                                }
                            }
                        };
                        // 全路径（或 `as` 别名）永远可用
                        let ok = self.insert_view(&canon, &view, &target, names.as_ref(), &item.tok, &joined);
                        // 末段短名：`import net.http` 之后 `http.ping()` 也能写（文稿 9.2）。
                        // `as` 给了就只留别名——别名是"我要的名字"，不再自动生成第二个。
                        if ok && !builtin && alias.is_none() && path.len() > 1 {
                            let short = path.last().unwrap().clone();
                            self.insert_view(&canon, &short, &target, names.as_ref(), &item.tok, &joined);
                        }
                    }
                    Item::FuncDecl { .. } => {
                        self.decl_order.push(item);
                        self.decl_mod.push(prefix.clone());
                        self.register_func(item);
                    }
                    Item::ClassDecl { .. }
                    | Item::StructDecl { .. }
                    | Item::InterfaceDecl { .. }
                    | Item::EnumDecl { .. }
                    | Item::TypeAlias { .. } => {
                        self.decl_order.push(item);
                        self.decl_mod.push(prefix.clone());
                        self.register_type(item);
                    }
                    Item::ConstDecl { .. } => {
                        self.decl_order.push(item);
                        self.decl_mod.push(prefix.clone());
                    }
                }
            }
        }
        self.cur_mod_prefix = None;
        self.cur_mod_path = String::new();
        self.active_views.clear();
    }

    fn declare_name(&mut self, name: &str, tok: &Tok, is_type: bool) -> bool {
        if is_reserved(name) {
            self.err("E3018", tok, "N-15", "`{0}` 是内建类型名，不能重新声明", &[&name]);
            return false;
        }
        if self.types.contains_key(name) || self.func_by_name.contains_key(name) {
            self.err("E3018", tok, "N-14", "`{0}` 与已声明的{1}重名（类型名与函数名共用一个命名空间）", &[&name, &if is_type { "类型" } else { "函数" }]);
            return false;
        }
        true
    }

    fn register_type(&mut self, item: &'a ast::N<Item>) {
        let (name, vis, generics, kind) = match &item.k {
            Item::ClassDecl { vis, name, generics, .. } => (name, vis, generics, "class"),
            Item::StructDecl { vis, name, generics, .. } => (name, vis, generics, "struct"),
            Item::InterfaceDecl { vis, name, .. } => (name, vis, &Vec::new(), "interface"),
            Item::EnumDecl { vis, name, .. } => (name, vis, &Vec::new(), "enum"),
            Item::TypeAlias { vis, name, generics, .. } => (name, vis, generics, "alias"),
            _ => return,
        };
        let is_public = vis.as_deref() == Some("public");
        let reg = self.reg_name(name);
        if !self.declare_name(&reg, &item.tok, true) {
            return;
        }
        let id = self.new_typedef(&reg, kind, &item.tok);
        let g: Vec<(String, Vec<String>)> = generics
            .iter()
            .map(|g| (g.name.clone(), g.constraints.clone()))
            .collect();
        {
            let d = &mut self.typedefs[id as usize];
            d.vis = vis.clone().unwrap_or_else(|| "private".to_string());
            d.generics = g;
        }
        self.types.insert(reg.clone(), id);
        if let Some(canon) = self.cur_mod_prefix.clone() {
            self.mod_syms
                .entry(canon.clone())
                .or_default()
                .push((name.clone(), ExportSym::Type(id)));
            if is_public {
                self.module_exports
                    .entry(canon)
                    .or_default()
                    .insert(name.clone(), ExportSym::Type(id));
            }
        }
    }

    fn register_func(&mut self, item: &'a ast::N<Item>) {
        let (name, vis, generics, body, implicit) = match &item.k {
            Item::FuncDecl { vis, name, generics, body, implicit, .. } => {
                (name, vis, generics, body, *implicit)
            }
            _ => return,
        };
        // 文稿 9.4：全工程只能有一个 int main——非入口文件声明 main 直接报错。
        // 隐式 main 是"顶层语句"包出来的，被 import 的模块里出现它要说清楚是"不能写语句"。
        if self.cur_mod_prefix.is_some() && name == "main" {
            if implicit {
                self.err(
                    "E3024",
                    &item.tok,
                    "文稿 9.4",
                    "被 import 的文件不能有顶层语句（语句只能写在函数体内）；顶层代码只允许出现在程序入口文件",
                    &[],
                );
                return;
            }
            self.err("E3028", &item.tok, "文稿 9.4", "`main` 只能出现在程序入口文件（被导入的模块不能声明 main）", &[]);
        }
        let is_public = vis.as_deref() == Some("public");
        let reg = self.reg_name(name);
        // 隐式 main 与显式 main 撞车：这是"两处入口"，不是"重名"，要说清楚。
        // 隐式 main 由 parser 追加在 items 末尾，所以显式 main 总在前面。
        if implicit && self.cur_mod_prefix.is_none() && self.func_by_name.contains_key(&reg) {
            self.err(
                "E3041",
                &item.tok,
                "文稿 9.4",
                "既有 `int main()` 又有顶层语句（顶层语句会自动包成 `main`）；二者只留一个——要么把语句搬进 `main`，要么删掉 `main`",
                &[],
            );
            return;
        }
        if !self.declare_name(&reg, &item.tok, false) {
            return;
        }
        let f = FuncInfo {
            name: reg.clone(),
            tok: item.tok.clone(),
            module: self.cur_mod_path.clone(),
            owner: None,
            params: Vec::new(),
            ret: t_void(),
            generics: generics
                .iter()
                .map(|g| (g.name.clone(), g.constraints.clone()))
                .collect(),
            vis: vis.clone().unwrap_or_else(|| "private".to_string()),
            is_static: false,
            is_override: false,
            body: Some(body),
            implicit,
        };
        self.funcs.push(f);
        self.func_by_name.insert(reg.clone(), self.funcs.len() - 1);
        if let Some(canon) = self.cur_mod_prefix.clone() {
            self.mod_syms
                .entry(canon.clone())
                .or_default()
                .push((name.clone(), ExportSym::Func(self.funcs.len() - 1)));
            if is_public {
                self.module_exports
                    .entry(canon)
                    .or_default()
                    .insert(name.clone(), ExportSym::Func(self.funcs.len() - 1));
            }
        }
    }

    // ---------------------------------------------------------------- 类型解析

    pub fn resolve_type(&mut self, ast: &ast::TPtr) -> Ty {
        let env = self.generic_env.clone();
        self.resolve_type_g(ast, &env)
    }

    fn resolve_type_g(&mut self, ast: &ast::TPtr, env: &HashMap<String, Ty>) -> Ty {
        match &ast.k {
            ast::Ty::Named { path, args } => {
                if path.len() == 1 {
                    let n = &path[0];
                    if let Some(t) = env.get(n) {
                        if matches!(&**t, TyS::Param(_)) {
                            return t.clone();
                        }
                    }
                    if let Some(t) = builtin_scalar(n) {
                        return t;
                    }
                    match n.as_str() {
                        "map" => {
                            if args.len() != 2 {
                                self.err("E4035", &ast.tok, "G-1", "`map` 需要 2 个类型实参（键、值）", &[]);
                                return t_object();
                            }
                            let key = self.resolve_type_g(&args[0], env);
                            let val = self.resolve_type_g(&args[1], env);
                            let bad = matches!(
                                &*key,
                                TyS::Object
                                    | TyS::Float
                                    | TyS::Nullable(_)
                                    | TyS::Void
                                    | TyS::Array(..)
                                    | TyS::Tuple(_)
                                    | TyS::Func(..)
                                    | TyS::Range
                            );
                            if bad {
                                self.err("E4018", &ast.tok, "文稿 3.3", "map 键类型必须是 Hashable + Comparable（不能是 {0}）", &[&self.ts(&key)]);
                            }
                            return t_map(key, val);
                        }
                        "result" => {
                            if args.len() != 1 {
                                self.err("E4035", &ast.tok, "G-1", "`result` 需要 1 个类型实参", &[]);
                                return t_result(t_object());
                            }
                            return t_result(self.resolve_type_g(&args[0], env));
                        }
                        "ArenaRef" => {
                            if args.len() != 1 {
                                self.err("E4035", &ast.tok, "G-1", "`ArenaRef` 需要 1 个类型实参", &[]);
                                return Rc::new(TyS::ArenaRef(t_object()));
                            }
                            return Rc::new(TyS::ArenaRef(self.resolve_type_g(&args[0], env)));
                        }
                        "func" => {
                            self.err("E2001", &ast.tok, "G-1", "`func` 类型请写成 `func(参数) 返回`", &[]);
                            return t_object();
                        }
                        "Self" => {
                            let cc = match self.cur_class {
                                None => {
                                    self.err("E4039", &ast.tok, "N-19", "`Self` 只能用在 class/struct/interface 体内", &[]);
                                    return t_object();
                                }
                                Some(c) => c,
                            };
                            let gs = self.typedefs[cc as usize].generics.clone();
                            return t_named(
                                DefId::Id(cc),
                                gs.iter()
                                    .map(|(g, _)| Rc::new(TyS::Param(g.clone())) as Ty)
                                    .collect(),
                            );
                        }
                        "Row" => return t_named(DefId::Name("Row"), Vec::new()),
                        "File" => return t_named(DefId::Name("File"), Vec::new()),
                        "void" => return t_void(),
                        "range" => return t_range(),
                        "Arena" => return t_named(DefId::Name("Arena"), Vec::new()),
                        "Rounding" => return t_named(DefId::Name("Rounding"), Vec::new()),
                        _ => {}
                    }
                    if let Some(id) = self.lookup_type_name(n, &ast.tok) {
                        let args: Vec<Ty> =
                            args.iter().map(|a| self.resolve_type_g(a, env)).collect();
                        let (ngen, dname, dkind) = {
                            let d = &self.typedefs[id as usize];
                            (d.generics.len(), d.name.clone(), d.kind)
                        };
                        if args.len() != ngen {
                            self.err("E4035", &ast.tok, "G-1", "`{0}` 需要 {1} 个类型实参", &[&dname, &ngen]);
                        }
                        if dkind == "alias" {
                            return self.expand_alias(id, args);
                        }
                        if dkind == "enum" {
                            return Rc::new(TyS::Enum(id));
                        }
                        return t_named(DefId::Id(id), args);
                    }
                    if self.active_views.contains_key(n) {
                        self.err("E4035", &ast.tok, "N-20", "`{0}` 是模块，不是类型", &[&n]);
                        return t_object();
                    }
                    self.err("E3003", &ast.tok, "N-7", "未定义的类型 `{0}`", &[&n]);
                    return t_object();
                }
                // 多段：模块.Type 或 Outer.Inner
                // 模块路径可以多段（`shop.model.Product`），所以用最长视图匹配，
                // 剩下最后一段才是类型名——不能只看 `path.len() == 2`
                if path.len() >= 2 {
                    // 用户模块导出类型：`mod.Type`（文稿 9.2）
                    if let Some((k, canon)) = self.view_of(&path) {
                        if canon != "*builtin*" && k + 1 == path.len() {
                            let view = path[..k].join(".");
                            let export = &path[k];
                            if !self.export_allowed(&view, export) {
                                let src = self.view_display(&view);
                                self.err("E3030", &ast.tok, "文稿 9.2", "`{0}` 不在 `import {1}.{{...}}` 的具名导入清单里", &[&export, &src]);
                                return t_object();
                            }
                            let sym = self
                                .module_exports
                                .get(&canon)
                                .and_then(|m| m.get(export))
                                .cloned();
                            return match sym {
                                Some(ExportSym::Type(d)) => {
                                    let rargs: Vec<Ty> =
                                        args.iter().map(|a| self.resolve_type_g(a, env)).collect();
                                    let (ngen, dname, dkind) = {
                                        let d = &self.typedefs[d as usize];
                                        (d.generics.len(), d.name.clone(), d.kind)
                                    };
                                    if rargs.len() != ngen {
                                        self.err("E4035", &ast.tok, "G-1", "`{0}` 需要 {1} 个类型实参", &[&dname, &ngen]);
                                    }
                                    if dkind == "alias" {
                                        return self.expand_alias(d, rargs);
                                    }
                                    if dkind == "enum" {
                                        return Rc::new(TyS::Enum(d));
                                    }
                                    self.named_of(d, rargs)
                                }
                                _ => {
                                    self.err("E3030", &ast.tok, "文稿 9.1", "模块 `{0}` 没有公开类型 `{1}`（文件内符号默认私有，导出需写 `public`）", &[&view, &export]);
                                    t_object()
                                }
                            };
                        }
                    }
                    if let Some(oid) = self.lookup_type_name(&path[0], &ast.tok) {
                        let inner = self.typedefs[oid as usize].nested.get(&path[1]).copied();
                        return match inner {
                            Some(iid) => self.named_of(iid, Vec::new()),
                            None => {
                                self.err("E3003", &ast.tok, "N-7", "`{0}` 里没有类型 `{1}`", &[&path[0], &path[1]]);
                                t_object()
                            }
                        };
                    }
                }
                self.err("E3003", &ast.tok, "N-7", "未定义的类型 `{0}`", &[&path.join(".")]);
                t_object()
            }
            ast::Ty::Array { elem, len } => {
                let elem = self.resolve_type_g(elem, env);
                let len = match len {
                    None => return t_array(elem, None),
                    Some(v) => v,
                };
                let v = match self.eval_const(len) {
                    Some(ConstVal::Int(i)) => i,
                    _ => {
                        self.err("E4004", &len.tok, "S-1", "定长数组的长度必须是常量表达式", &[]);
                        return t_array(elem, Some(0));
                    }
                };
                if v <= 0 {
                    self.err("E4004", &len.tok, "S-1", "定长数组长度必须是正整数", &[]);
                    return t_array(elem, Some(1));
                }
                if v > (1i64 << 31) - 1 {
                    self.err("E4004", &len.tok, "S-1", "定长数组长度超出上限", &[]);
                    return t_array(elem, Some(1));
                }
                if let Some(size) = self.type_size(&elem) {
                    if size * v > 8 * 1024 * 1024 {
                        self.err("E4041", &ast.tok, "GAP-10", "定长数组过大（{0} × {1} 字节 > 8 MiB）", &[&v, &size]);
                    }
                }
                t_array(elem, Some(v))
            }
            ast::Ty::Nullable { inner } => {
                let inner = self.resolve_type_g(inner, env);
                if matches!(&*inner, TyS::Void) {
                    self.err("E2001", &ast.tok, "G-1", "`void?` 不合法", &[]);
                    return t_object();
                }
                t_nullable(inner)
            }
            ast::Ty::Func { params, ret } => {
                let ps: Vec<Ty> = params.iter().map(|p| self.resolve_type_g(p, env)).collect();
                let r = self.resolve_type_g(ret, env);
                t_fn(ps, r)
            }
            ast::Ty::Tuple { elems } => Rc::new(TyS::Tuple(
                elems.iter().map(|e| self.resolve_type_g(e, env)).collect(),
            )),
        }
    }

    pub fn named_of(&self, d: u32, args: Vec<Ty>) -> Ty {
        match self.typedefs.get(d as usize).map(|d| d.kind) {
            Some("enum") => Rc::new(TyS::Enum(d)),
            Some("alias") => match self.typedefs[d as usize].alias_target.clone() {
                Some(t) => t,
                None => t_object(),
            },
            _ => t_named(DefId::Id(d), args),
        }
    }

    fn expand_alias(&mut self, d: u32, args: Vec<Ty>) -> Ty {
        if self.typedefs[d as usize].expanding {
            let (name, tok) = {
                let td = &self.typedefs[d as usize];
                (td.name.clone(), td.tok.clone())
            };
            self.err("E3003", &tok, "N-21", "类型别名循环引用 `{0}`", &[&name]);
            return t_object();
        }
        self.typedefs[d as usize].expanding = true;
        let mut env = self.generic_env.clone();
        let gs = self.typedefs[d as usize].generics.clone();
        for (i, (gname, _)) in gs.iter().enumerate() {
            if let Some(a) = args.get(i) {
                env.insert(gname.clone(), a.clone());
            }
        }
        let target = self.typedefs[d as usize].alias_node.clone();
        let ty = match target {
            Some(t) => self.resolve_type_g(&t, &env),
            None => t_object(),
        };
        self.typedefs[d as usize].expanding = false;
        self.typedefs[d as usize].alias_target = Some(ty.clone());
        ty
    }

    pub fn type_size(&self, ty: &Ty) -> Option<i64> {
        match &**ty {
            TyS::Bool => Some(1),
            TyS::Int(bits, _) => Some(std::cmp::max(1, *bits as i64 / 8)),
            TyS::Float | TyS::Object => Some(8),
            TyS::Str => Some(16),
            TyS::Decimal => Some(32),
            TyS::Array(elem, len) => match len {
                None => Some(24),
                Some(n) => {
                    let es = self.type_size(elem)?;
                    Some(es * n)
                }
            },
            TyS::Map(..) => Some(48),
            _ => None,
        }
    }

    pub fn contains_class(&self, ty: &Ty) -> bool {
        match &**ty {
            TyS::Named(DefId::Id(id), _) => {
                self.typedefs
                    .get(*id as usize)
                    .map_or(false, |d| d.kind == "class")
            }
            TyS::Array(e, _) => self.contains_class(e),
            TyS::Map(k, v) => self.contains_class(k) || self.contains_class(v),
            TyS::Tuple(elems) => elems.iter().any(|x| self.contains_class(x)),
            TyS::Nullable(inner) => self.contains_class(inner),
            _ => false,
        }
    }

    // ---------------------------------------------------------------- 声明解析

    fn resolve_decls(&mut self) {
        let items: Vec<&'a ast::N<Item>> = self.decl_order.clone();
        let mods = self.decl_mod.clone();
        for (item, modc) in items.into_iter().zip(mods.iter()) {
            self.set_active_module(modc);
            match &item.k {
                Item::FuncDecl { name, .. } => {
                    let reg = self.reg_name(name);
                    let f = self.func_by_name.get(&reg).copied();
                    self.resolve_func_sig(f, item);
                }
                Item::ClassDecl { name, .. }
                | Item::StructDecl { name, .. }
                | Item::InterfaceDecl { name, .. }
                | Item::EnumDecl { name, .. }
                | Item::TypeAlias { name, .. } => {
                    let reg = self.reg_name(name);
                    if let Some(&id) = self.types.get(&reg) {
                        self.resolve_typedef(id, item);
                    }
                }
                Item::ConstDecl { .. } => self.resolve_const_decl(item),
                Item::Import { .. } => {}
            }
        }
        self.set_active_module(&None);
    }

    fn resolve_func_sig(&mut self, f: Option<usize>, item: &'a ast::N<Item>) {
        let f = match f {
            Some(f) => f,
            None => return,
        };
        let (params, ret, generics) = match &item.k {
            Item::FuncDecl { params, ret, generics, .. } => (params, ret, generics),
            _ => return,
        };
        let old = self.generic_env.clone();
        self.generic_env = generics
            .iter()
            .map(|g| (g.name.clone(), Rc::new(TyS::Param(g.name.clone())) as Ty))
            .collect();
        let mut ps = Vec::new();
        for p in params {
            let ty = self.resolve_type(&p.k.ty);
            ps.push((p.k.name.clone(), ty, p.k.borrow.clone()));
        }
        self.funcs[f].params = ps;
        self.funcs[f].ret = self.resolve_type(ret);
        self.generic_env = old;
    }

    fn resolve_typedef(&mut self, d: u32, item: &'a ast::N<Item>) {
        // TypeAlias：声明期只校验 target（用环境为空的 generic_env，泛型参数
        // 在此会报 E3003——历史行为，测试钉死，不做"顺手修正"）；
        // 真正的展开在 expand_alias（使用处）。
        if let Item::TypeAlias { target, .. } = &item.k {
            self.typedefs[d as usize].alias_node = Some(target.clone());
            let _ = self.resolve_type(target);
            return;
        }
        let kind = self.typedefs[d as usize].kind;
        let generics: Vec<ast::TypeParam> = match &item.k {
            Item::ClassDecl { generics, .. } | Item::StructDecl { generics, .. } => generics.clone(),
            _ => Vec::new(),
        };
        self.generic_env = generics
            .iter()
            .map(|g| (g.name.clone(), Rc::new(TyS::Param(g.name.clone())) as Ty))
            .collect();
        self.cur_class = Some(d);
        if kind == "class" {
            if let Item::ClassDecl { parent, impls, .. } = &item.k {
                if let Some(pt_node) = parent {
                    let pt = self.resolve_type(pt_node);
                    let pd = match &*pt {
                        TyS::Named(DefId::Id(id), _) => match self.typedefs.get(*id as usize) {
                            Some(pd) if pd.kind == "class" => Some(pd.id),
                            _ => None,
                        },
                        _ => None,
                    };
                    match pd {
                        Some(pd) => self.typedefs[d as usize].parent = Some(pd),
                        None => self.err("E3020", &pt_node.tok, "文稿 6.2", "`extend` 后面必须是 class", &[]),
                    }
                }
                for impl_ in impls {
                    let it = self.resolve_type(impl_);
                    let idd = match &*it {
                        TyS::Named(DefId::Id(id), _) => match self.typedefs.get(*id as usize) {
                            Some(idd) if idd.kind == "interface" => Some(idd.id),
                            _ => None,
                        },
                        _ => None,
                    };
                    match idd {
                        Some(idd) => self.typedefs[d as usize].impls.push(idd),
                        None => self.err("E3019", &impl_.tok, "文稿 6.3", "`implement` 后面必须是 interface", &[]),
                    }
                }
            }
        }
        if kind == "interface" {
            if let Item::InterfaceDecl { extends, .. } = &item.k {
                for ext in extends {
                    let it = self.resolve_type(ext);
                    let idd = match &*it {
                        TyS::Named(DefId::Id(id), _) => match self.typedefs.get(*id as usize) {
                            Some(idd) if idd.kind == "interface" => Some(idd.id),
                            _ => None,
                        },
                        _ => None,
                    };
                    match idd {
                        Some(idd) => self.typedefs[d as usize].extends.push(idd),
                        None => self.err("E3019", &ext.tok, "文稿 6.3", "interface 只能 extend interface", &[]),
                    }
                }
            }
        }
        if kind == "enum" {
            if let Item::EnumDecl { variants, .. } = &item.k {
                let mut sorted = variants.clone();
                sorted.sort();
                sorted.dedup();
                if sorted.len() != variants.len() {
                    self.err("E3031", &item.tok, "文稿 6.4", "enum 成员名重复", &[]);
                }
                self.typedefs[d as usize].variants = variants.clone();
            }
        }
        if kind == "class" || kind == "struct" {
            if let Item::ClassDecl { members, .. } | Item::StructDecl { members, .. } = &item.k {
                let mut idx = 0u32;
                for m in members {
                    match &m.k {
                        ast::Member::Field { vis, ty, name } => {
                            let ft = self.resolve_type(ty);
                            let dup = self.typedefs[d as usize]
                                .fields
                                .iter()
                                .any(|(fn_, _, _, _)| fn_ == name);
                            if dup {
                                self.err("E3004", &m.tok, "N-4", "字段 `{0}` 重复声明", &[&name]);
                            }
                            self.typedefs[d as usize].fields.push((
                                name.clone(),
                                ft.clone(),
                                vis.clone().unwrap_or_else(|| "private".to_string()),
                                idx,
                            ));
                            self.sl(m.id).field_idx = idx;
                            idx += 1;
                            if kind == "struct" && self.contains_class(&ft) {
                                self.err("E3023", &m.tok, "T-29", "struct 字段不能含 class 引用（包括容器元素）：`{0}` 是 {1}", &[&name, &self.ts(&ft)]);
                            }
                        }
                        ast::Member::Method {
                            vis,
                            is_static,
                            is_override,
                            ret,
                            name,
                            params,
                            body,
                        } => {
                            let f = FuncInfo {
                                name: name.clone(),
                                tok: m.tok.clone(),
                                module: self.path.clone(),
                                owner: Some(d),
                                params: Vec::new(),
                                ret: t_void(),
                                generics: Vec::new(),
                                vis: vis.clone().unwrap_or_else(|| "private".to_string()),
                                is_static: *is_static,
                                is_override: *is_override,
                                implicit: false,
                                body: Some(body),
                            };
                            self.funcs.push(f);
                            let fid = self.funcs.len() - 1;
                            let mut ps = Vec::new();
                            for p in params {
                                let ty = self.resolve_type(&p.k.ty);
                                ps.push((p.k.name.clone(), ty, p.k.borrow.clone()));
                            }
                            self.funcs[fid].params = ps;
                            self.funcs[fid].ret = self.resolve_type(ret);
                            let dup = self.typedefs[d as usize].methods.contains_key(name);
                            if dup {
                                self.err("E3004", &m.tok, "N-4", "方法 `{0}` 重复声明", &[&name]);
                            }
                            self.typedefs[d as usize].methods.insert(name.clone(), fid);
                        }
                        ast::Member::Ctor { params, body } => {
                            self.typedefs[d as usize].ctor_count += 1;
                            let f = FuncInfo {
                                name: "constructor".to_string(),
                                tok: m.tok.clone(),
                                module: self.path.clone(),
                                owner: Some(d),
                                params: Vec::new(),
                                ret: t_void(),
                                generics: Vec::new(),
                                vis: "private".to_string(),
                                is_static: false,
                                is_override: false,
                                implicit: false,
                                body: Some(body),
                            };
                            self.funcs.push(f);
                            let fid = self.funcs.len() - 1;
                            let mut ps = Vec::new();
                            for p in params {
                                let ty = self.resolve_type(&p.k.ty);
                                ps.push((p.k.name.clone(), ty, p.k.borrow.clone()));
                            }
                            self.funcs[fid].params = ps;
                            self.typedefs[d as usize].ctor = Some(fid);
                        }
                        ast::Member::Item(_) => {}
                    }
                }
            }
        }
        if kind == "interface" {
            if let Item::InterfaceDecl { methods, .. } = &item.k {
                for m in methods {
                    let f = FuncInfo {
                        name: m.k.name.clone(),
                        tok: m.tok.clone(),
                        module: self.path.clone(),
                        owner: Some(d),
                        params: Vec::new(),
                        ret: t_void(),
                        generics: Vec::new(),
                        vis: "public".to_string(),
                        is_static: false,
                        is_override: false,
                        implicit: false,
                        body: m.k.default_body.as_ref(),
                    };
                    self.funcs.push(f);
                    let fid = self.funcs.len() - 1;
                    let mut ps = Vec::new();
                    for p in &m.k.params {
                        let ty = self.resolve_type(&p.k.ty);
                        ps.push((p.k.name.clone(), ty, p.k.borrow.clone()));
                    }
                    self.funcs[fid].params = ps;
                    self.funcs[fid].ret = self.resolve_type(&m.k.ret);
                    self.typedefs[d as usize]
                        .methods
                        .insert(m.k.name.clone(), fid);
                }
            }
        }
        // 嵌套类型
        let members_opt: Option<&Vec<ast::N<ast::Member>>> = match &item.k {
            Item::ClassDecl { members, .. } | Item::StructDecl { members, .. } => Some(members),
            _ => None,
        };
        if let Some(members) = members_opt {
            for m in members {
                if let ast::Member::Item(inner) = &m.k {
                    let (name, kind, generics) = match &inner.k {
                        Item::ClassDecl { name, generics, .. } => (name, "class", generics),
                        Item::StructDecl { name, generics, .. } => (name, "struct", generics),
                        Item::InterfaceDecl { name, .. } => (name, "interface", &Vec::new()),
                        Item::EnumDecl { name, .. } => (name, "enum", &Vec::new()),
                        Item::TypeAlias { name, generics, .. } => (name, "alias", generics),
                        _ => continue,
                    };
                    let nid = self.new_typedef(name, kind, &inner.tok);
                    let g: Vec<(String, Vec<String>)> = generics
                        .iter()
                        .map(|g| (g.name.clone(), g.constraints.clone()))
                        .collect();
                    self.typedefs[nid as usize].generics = g;
                    if kind == "alias" {
                        if let Item::TypeAlias { target, .. } = &inner.k {
                            self.typedefs[nid as usize].alias_node = Some(target.clone());
                        }
                    }
                    self.typedefs[d as usize].nested.insert(name.clone(), nid);
                }
            }
        }
        self.generic_env = HashMap::new();
        self.cur_class = None;
    }

    fn resolve_const_decl(&mut self, item: &'a ast::N<Item>) {
        let (name, ty) = match &item.k {
            Item::ConstDecl { name, ty, .. } => (name, ty),
            _ => return,
        };
        let reg = self.reg_name(name);
        if self.consts.contains_key(&reg) {
            self.err("E3004", &item.tok, "N-4", "常量 `{0}` 重复声明", &[&name]);
            return;
        }
        let t = match ty {
            Some(t) => self.resolve_type(t),
            None => t_int(),
        };
        let cell = Rc::new(ConstCell {
            ty: RefCell::new(t),
            value: RefCell::new(None),
        });
        self.consts.insert(reg, cell);
        if let Some(canon) = self.cur_mod_prefix.clone() {
            self.mod_syms
                .entry(canon.clone())
                .or_default()
                .push((name.clone(), ExportSym::Const));
            if self.cur_mod_is_public_const(item) {
                self.module_exports
                    .entry(canon)
                    .or_default()
                    .insert(name.clone(), ExportSym::Const);
            }
        }
    }

    /// 模块级 const 的 `public` 可见性（Item::ConstDecl 的 vis 字段）
    fn cur_mod_is_public_const(&self, item: &'a ast::N<Item>) -> bool {
        matches!(&item.k, Item::ConstDecl { vis, .. } if vis.as_deref() == Some("public"))
    }

    // ---------------------------------------------------------------- 入口

    pub fn run(&mut self) {
        self.collect();
        self.resolve_decls();
        // 常量
        let items: Vec<&'a ast::N<Item>> = self.decl_order.clone();
        let mods = self.decl_mod.clone();
        for (item, modc) in items.iter().zip(mods.iter()) {
            self.set_active_module(modc);
            if let Item::ConstDecl { ty, name, init, .. } = &item.k {
                let reg = self.reg_name(name);
                let cell = match self.consts.get(&reg) {
                    Some(c) => c.clone(),
                    None => continue, // 重复声明：前一次声明占用了 key，跳过
                };
                let declared = if ty.is_some() {
                    Some(cell.ty.borrow().clone())
                } else {
                    None
                };
                let t2 = self.check_expr(init, declared.as_ref());
                if declared.is_none() {
                    if matches!(&*t2, TyS::Null | TyS::Void | TyS::Range | TyS::Never) {
                        self.err("E4007", &init.tok, "T-12", "无法从初始化器推断 const 类型", &[]);
                        *cell.ty.borrow_mut() = t_int();
                    } else {
                        *cell.ty.borrow_mut() = t2;
                    }
                } else {
                    let dst = declared.clone().unwrap();
                    self.coerce(init, &dst, "const 初始化");
                }
                let v = self.eval_const(init);
                if v.is_none() {
                    self.err("E3003", &init.tok, "S-2", "const 初始化器必须是常量表达式", &[]);
                }
                *cell.value.borrow_mut() = v;
            }
        }
        // 函数体
        for (item, modc) in items.iter().zip(mods.iter()) {
            self.set_active_module(modc);
            if let Item::FuncDecl { name, .. } = &item.k {
                let reg = self.reg_name(name);
                if let Some(&f) = self.func_by_name.get(&reg) {
                    self.check_func(f);
                }
            }
        }
        let n = self.typedefs.len() as u32;
        for id in 0..n {
            let modc = self.typedef_mod.get(id as usize).cloned().unwrap_or(None);
            self.set_active_module(&modc);
            match self.typedefs[id as usize].kind {
                "class" | "struct" => self.check_class(id),
                _ => {}
            }
        }
        self.set_active_module(&None);
        self.check_main();
        self.check_struct_recursion();
    }

    // ---------------------------------------------------------------- 函数体

    fn check_func(&mut self, f: usize) {
        if self.checked_funcs.contains(&f) {
            return;
        }
        self.checked_funcs.insert(f);
        let (params, ret, name, tok, owner, generics, body) = {
            let fi = &self.funcs[f];
            (
                fi.params.clone(),
                fi.ret.clone(),
                fi.name.clone(),
                fi.tok.clone(),
                fi.owner,
                fi.generics.clone(),
                fi.body,
            )
        };
        let saved_ctx = (
            self.scope.clone(),
            self.current_func,
            self.cur_class,
            self.generic_env.clone(),
            self.generic_cons.clone(),
        );
        self.current_func = Some(f);
        self.cur_class = owner;
        self.generic_env = generics
            .iter()
            .map(|(g, _)| (g.clone(), Rc::new(TyS::Param(g.clone())) as Ty))
            .collect();
        self.generic_cons = generics.iter().cloned().collect();
        self.scope = Some(Rc::new(RefCell::new(ScopeNode {
            parent: None,
            names: HashMap::new(),
        })));
        for (i, (pname, pty, _borrow)) in params.iter().enumerate() {
            let dup = {
                let scope = self.scope.as_ref().unwrap();
                scope.borrow().names.contains_key(pname)
            };
            if dup {
                self.err("E3004", &tok, "N-4", "参数 `{0}` 重复", &[&pname]);
            }
            let scope = self.scope.as_ref().unwrap();
            scope.borrow_mut().names.insert(
                pname.clone(),
                Rc::new(RefCell::new(VarInfo {
                    name: pname.clone(),
                    ty: pty.clone(),
                    is_const: false,
                    state: VarState::Live,
                    decl_line: 0,
                    is_param: true,
                    idx: i,
                })),
            );
        }
        let saved_narrow = std::mem::replace(&mut self.narrow, vec![HashMap::new()]);
        let saved_in_result = self.in_result;
        let saved_in_main = self.in_main;
        let saved_loop_depth = self.loop_depth;
        let saved_labels = std::mem::take(&mut self.loop_labels);
        let saved_iters = std::mem::take(&mut self.iter_targets);
        self.in_result = matches!(&*ret, TyS::Result(_));
        self.in_main = name == "main";
        self.loop_depth = 0;
        // ctor：parent(...) 必须是首条语句
        if name == "constructor" {
            if let Some(o) = owner {
                if self.typedefs[o as usize].parent.is_some() {
                    let stmts: &[ast::SPtr] = body.map(|b| &b.stmts[..]).unwrap_or(&[]);
                    let ok = match stmts.first().map(|s| &s.k) {
                        Some(Stmt::Simple {
                            lhs,
                            op: None,
                            rhs: None,
                            incdec: None,
                        }) => matches!(&lhs.k, Expr::Call { callee, .. } if matches!(callee.k, Expr::Parent)),
                        _ => false,
                    };
                    if !ok {
                        let ftok = match stmts.first() {
                            Some(s) => s.tok.clone(),
                            None => tok.clone(),
                        };
                        self.err("E3035", &ftok, "文稿 6.2", "子类 constructor 的第一条语句必须是 `parent(...)`", &[]);
                    }
                }
            }
        }
        if let Some(b) = body {
            self.check_block(b, false);
        }
        let is_void = matches!(&*ret, TyS::Void);
        if !is_void && !self.in_main && !self.block_returns(body) {
            self.err("E2006", &tok, "文稿 9.4", "函数 `{0}` 存在没有 return 的路径", &[&name]);
        }
        self.narrow = saved_narrow;
        self.in_result = saved_in_result;
        self.in_main = saved_in_main;
        self.loop_depth = saved_loop_depth;
        self.loop_labels = saved_labels;
        self.iter_targets = saved_iters;
        // 恢复调用方上下文：check_func 也会被匿名函数合成在表达式检查中途调用，
        // 此时 scope / current_func / 泛型环境属于外层函数，必须原样还回去，
        // 否则匿名函数体检查完，外层局部变量会全部"消失"（E3003）。
        self.scope = saved_ctx.0;
        self.current_func = saved_ctx.1;
        self.cur_class = saved_ctx.2;
        self.generic_env = saved_ctx.3;
        self.generic_cons = saved_ctx.4;
    }

    /// 诊断里给人看的类型名：模块内的类型注册成 `模块__类名`（C 侧要唯一），
    /// 但用户没见过这个前缀，报错时去掉它。
    fn disp_tyname(&self, d: u32) -> String {
        let td = &self.typedefs[d as usize];
        if self.typedef_mod.get(d as usize).cloned().flatten().is_some() {
            if let Some(p) = td.name.rfind("__") {
                return td.name[p + 2..].to_string();
            }
        }
        td.name.clone()
    }

    fn check_class(&mut self, d: u32) {
        let (kind, name, tok) = {
            let td = &self.typedefs[d as usize];
            (td.kind, td.name.clone(), td.tok.clone())
        };
        if kind == "class" {
            let has_fields = !self.typedefs[d as usize].fields.is_empty();
            let ctor = self.typedefs[d as usize].ctor;
            if has_fields && ctor.is_none() {
                let disp = self.disp_tyname(d);
                self.err("E3008", &tok, "文稿 6.1", "类 `{0}` 有字段但没有 constructor", &[&disp]);
            }
            if let Some(cf) = ctor {
                if self.typedefs[d as usize].ctor_count > 1 {
                    self.err("E3010", &tok, "文稿 6.1", "一个类只能有一个 constructor", &[]);
                }
                let mut assigned = HashSet::new();
                if let Some(b) = self.funcs[cf].body {
                    self.collect_this_assign(b, &mut assigned);
                }
                let fields = self.typedefs[d as usize].fields.clone();
                let ctor_tok = self.funcs[cf].tok.clone();
                for (fn_, _, _, _) in &fields {
                    if !assigned.contains(fn_) {
                        self.err("E3009", &ctor_tok, "文稿 6.1", "constructor 没有给字段 `{0}` 赋值", &[&fn_]);
                    }
                }
            }
            // override 检查（父类方法优先，其次 interface 声明）
            let mut iface_names = HashSet::new();
            let impls = self.typedefs[d as usize].impls.clone();
            for iface in &impls {
                for (n, _) in self.all_iface_methods(*iface) {
                    iface_names.insert(n);
                }
            }
            let parent = self.typedefs[d as usize].parent;
            let methods: Vec<(String, usize)> = self.typedefs[d as usize]
                .methods
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect();
            for (mname, m) in &methods {
                let pm = match parent {
                    Some(p) => self.find_method(p, mname),
                    None => None,
                };
                let (m_ov, m_tok) = {
                    let fi = &self.funcs[*m];
                    (fi.is_override, fi.tok.clone())
                };
                if let Some(pm) = pm {
                    if !m_ov {
                        self.err("E3017", &m_tok, "文稿 6.2", "方法 `{0}` 覆写了父类方法但没写 `override`", &[&mname]);
                    } else {
                        let same = {
                            let a = &self.funcs[pm];
                            let b = &self.funcs[*m];
                            a.ret == b.ret
                                && a.params
                                    .iter()
                                    .map(|p| &p.1)
                                    .collect::<Vec<_>>()
                                    == b.params
                                        .iter()
                                        .map(|p| &p.1)
                                        .collect::<Vec<_>>()
                        };
                        if !same {
                            self.err("E3019", &m_tok, "文稿 6.2", "覆写 `{0}` 的签名与父类不一致", &[&mname]);
                        }
                    }
                } else if m_ov && !iface_names.contains(mname) {
                    self.err("E3016", &m_tok, "文稿 6.2", "`override` 没有对应的父类或 interface 方法", &[]);
                }
            }
            // interface 实现
            for iface in &impls {
                for (iname, sig) in self.all_iface_methods(*iface) {
                    // 只沿父类链找实现——不能用 find_method：它的兜底会把
                    // interface 方法签名本身返回回来（类接收者调 interface
                    // 独有方法的路径），缺实现检测会被它骗成 Some。
                    let mut m: Option<usize> = None;
                    let mut cur = Some(d);
                    while let Some(c) = cur {
                        let td = &self.typedefs[c as usize];
                        if let Some(&x) = td.methods.get(&iname) {
                            m = Some(x);
                            break;
                        }
                        cur = td.parent;
                    }
                    match m {
                        None => {
                            if self.funcs[sig].body.is_none() {
                                self.err("E3019", &tok, "文稿 6.3", "类 `{0}` 没有实现 interface 方法 `{1}`", &[&name, &iname]);
                            }
                        }
                        Some(m) => {
                            let same = {
                                let a = &self.funcs[m];
                                let b = &self.funcs[sig];
                                // Self 语义（文稿 7.1）：签名参数声明为某个
                                // interface（如 Comparable 的 compare(Self)）
                                // 时，实现方写实现类类型——Self 在实现类里
                                // 就是实现类自己。
                                let params_match =
                                    a.params.len() == b.params.len()
                                        && a.params.iter().zip(b.params.iter()).all(
                                            |(pa, pb)| {
                                                if pa.1 == pb.1 {
                                                    return true;
                                                }
                                                match (&*pa.1, &*pb.1) {
                                                    (
                                                        TyS::Named(DefId::Id(c), _),
                                                        TyS::Named(DefId::Id(i), _),
                                                    ) => {
                                                        *c == d
                                                            && self
                                                                .typedefs
                                                                .get(*i as usize)
                                                                .map_or(
                                                                    false,
                                                                    |td| {
                                                                        td.kind
                                                                            == "interface"
                                                                    },
                                                                )
                                                    }
                                                    _ => false,
                                                }
                                            },
                                        );
                                params_match && a.ret == b.ret
                            };
                            if !same {
                                let m_tok = self.funcs[m].tok.clone();
                                self.err("E3019", &m_tok, "文稿 6.3", "方法 `{0}` 与 interface 签名不一致", &[&iname]);
                            }
                        }
                    }
                }
            }
        }
        if kind == "struct" {
            let names: Vec<String> = self.typedefs[d as usize]
                .fields
                .iter()
                .map(|f| f.0.clone())
                .collect();
            let mut sorted = names.clone();
            sorted.sort();
            sorted.dedup();
            if sorted.len() != names.len() {
                self.err("E3004", &tok, "N-4", "struct 字段重名", &[]);
            }
        }
        // 方法体
        let mut holders: Vec<usize> = self
            .typedefs[d as usize]
            .methods
            .values()
            .copied()
            .collect();
        if let Some(c) = self.typedefs[d as usize].ctor {
            holders.push(c);
        }
        if kind == "interface" {
            for (_, sig) in self.all_iface_methods(d) {
                holders.push(sig);
            }
        }
        for f in holders {
            self.check_func(f);
        }
    }

    pub fn all_iface_methods(&self, d: u32) -> Vec<(String, usize)> {
        let mut out: Vec<(String, usize)> = Vec::new();
        let mut seen = HashSet::new();
        self.all_iface_methods_rec(d, &mut seen, &mut out);
        out
    }

    fn all_iface_methods_rec(
        &self,
        d: u32,
        seen: &mut HashSet<u32>,
        out: &mut Vec<(String, usize)>,
    ) {
        if !seen.insert(d) {
            return;
        }
        if let Some(td) = self.typedefs.get(d as usize) {
            for (name, m) in &td.methods {
                match out.iter_mut().find(|(n, _)| n == name) {
                    Some(e) => e.1 = *m,
                    None => out.push((name.clone(), *m)),
                }
            }
            for &e in &td.extends {
                self.all_iface_methods_rec(e, seen, out);
            }
        }
    }

    pub fn find_method(&self, d: u32, name: &str) -> Option<usize> {
        let mut cur = Some(d);
        while let Some(c) = cur {
            if let Some(td) = self.typedefs.get(c as usize) {
                if let Some(&m) = td.methods.get(name) {
                    return Some(m);
                }
                cur = td.parent;
            } else {
                cur = None;
            }
        }
        // interface 自身的 extends 链也是方法来源（B extend A 时 b.a_method()）
        if let Some(td) = self.typedefs.get(d as usize) {
            if td.kind == "interface" {
                let exts = td.extends.clone();
                for e in exts {
                    if let Some(m) = self.find_method(e, name) {
                        return Some(m);
                    }
                }
            }
        }
        if let Some(td) = self.typedefs.get(d as usize) {
            for &i in &td.impls {
                for (n, m) in self.all_iface_methods(i) {
                    if n == name {
                        return Some(m);
                    }
                }
            }
        }
        None
    }

    fn collect_this_assign(&self, block: &ast::Block, out: &mut HashSet<String>) {
        for s in &block.stmts {
            match &s.k {
                Stmt::Simple { lhs, op, .. } if op.as_deref() == Some("=") => {
                    if let Expr::FieldAccess { recv, name } = &lhs.k {
                        if matches!(recv.k, Expr::This) {
                            out.insert(name.clone());
                        }
                    }
                }
                Stmt::If(n) => {
                    self.collect_this_assign(&n.then_, out);
                    if let Some(ast::IfTail::Block(b)) = &n.els {
                        self.collect_this_assign(b, out);
                    }
                }
                Stmt::Block(b) => self.collect_this_assign(b, out),
                _ => {}
            }
        }
    }

    fn check_main(&mut self) {
        let mains: Vec<usize> = self
            .funcs
            .iter()
            .enumerate()
            .filter(|(_, f)| f.name == "main")
            .map(|(i, _)| i)
            .collect();
        if mains.is_empty() {
            // 只有 run/build 要求入口；check 允许检查无 main 的库模块（文稿 9.4）。
            if self.require_main {
                self.err_dummy("E3025", "文稿 9.4", "缺少 `int main()` 入口", &[]);
            }
            return;
        }
        if mains.len() > 1 {
            // 显式 main + 顶层语句 → 两处入口意图冲突，直接说清楚怎么改；
            // 两个显式 main 才用「有多个 main」。
            let imp = mains.iter().copied().find(|i| self.funcs[*i].implicit);
            let tok = self.funcs[*mains.last().unwrap()].tok.clone();
            match imp {
                Some(_) => self.err(
                    "E3041",
                    &tok,
                    "文稿 9.4",
                    "既有 `int main()` 又有顶层语句（顶层语句会自动包成 `main`）；二者只留一个——要么把语句搬进 `main`，要么删掉 `main`",
                    &[],
                ),
                None => self.err("E3025", &tok, "文稿 9.4", "有多个 `main`", &[]),
            }
            return;
        }
        let f = mains[0];
        let (ret, params) = {
            let fi = &self.funcs[f];
            (fi.ret.clone(), fi.params.clone())
        };
        let mut ok = ret == t_int();
        if ok {
            if params.len() == 1 {
                let pty = &params[0].1;
                ok = match &**pty {
                    TyS::Array(e, None) => **e == *t_string(),
                    _ => false,
                };
            }
        }
        if !ok {
            let tok = self.funcs[f].tok.clone();
            self.err("E3025", &tok, "文稿 9.4", "`main` 签名必须是 `int main()` 或 `int main(string[] args)`", &[]);
        }
    }

    fn check_struct_recursion(&mut self) {
        let n = self.typedefs.len() as u32;
        for id in 0..n {
            if self.typedefs[id as usize].kind != "struct" {
                continue;
            }
            let mut seen = HashSet::new();
            if self.recursive_struct(id, id, &mut seen) {
                let (name, tok) = {
                    let td = &self.typedefs[id as usize];
                    (td.name.clone(), td.tok.clone())
                };
                self.err("E3022", &tok, "T-30", "struct `{0}` 递归内联包含自身", &[&name]);
            }
        }
    }

    fn recursive_struct(&self, start: u32, d: u32, seen: &mut HashSet<u32>) -> bool {
        let fields = match self.typedefs.get(d as usize) {
            Some(td) => td.fields.clone(),
            None => return false,
        };
        for (_, ft, _, _) in &fields {
            if let TyS::Named(DefId::Id(id), _) = &**ft {
                let nd = match self.typedefs.get(*id as usize) {
                    Some(nd) => nd,
                    None => continue,
                };
                if nd.id == start {
                    return true;
                }
                if nd.kind == "struct" && !seen.contains(&nd.id) {
                    seen.insert(nd.id);
                    if self.recursive_struct(start, nd.id, seen) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

// ---------------------------------------------------------------- 可赋值上下文

impl<'a> AssignCtx for Sema<'a> {
    fn def_of(&self, _id: u32) -> bool {
        self.typedefs.get(_id as usize).is_some()
    }
    fn def_kind(&self, id: u32) -> &'static str {
        self.typedefs.get(id as usize).map(|d| d.kind).unwrap_or("")
    }
    fn is_subclass(&self, sub_id: u32, super_id: u32) -> bool {
        Sema::is_subclass(self, sub_id, super_id)
    }
    fn implements(&self, class_id: u32, iface_id: u32) -> bool {
        Sema::implements(self, class_id, iface_id)
    }
}

// ---------------------------------------------------------------- 入口

pub fn analyze<'a>(
    mod_: &'a ast::Module,
    d: &'a mut Diags,
    path: &str,
    require_main: bool,
) -> Sema<'a> {
    analyze_multi(mod_, d, path, require_main, Vec::new(), HashMap::new())
}

/// 多模块入口：`extras` 为 cli loader 按拓扑序（被导入者在前）加载的用户模块，
/// `path_canon` 为导入写法 → 规范前缀的映射。
pub fn analyze_multi<'a>(
    mod_: &'a ast::Module,
    d: &'a mut Diags,
    path: &str,
    require_main: bool,
    extras: Vec<ExtraMod<'a>>,
    path_canon: HashMap<String, String>,
) -> Sema<'a> {
    let mut s = Sema::new(mod_, d, path, require_main);
    s.extras = extras;
    s.path_canon = path_canon;
    s.run();
    s
}
