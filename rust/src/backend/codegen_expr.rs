//! FnGen 的表达式级 C 生成 + match 语句。
//!
//! 借用约定：所有节点都从 `&'a` 的 AST 借用（不 clone EPtr——clone 出来的
//! 盒子的生命周期只有函数栈帧，无法传给需要 `&'a` 的递归 gen）。

use crate::frontend::ast::{self, Expr, Pat, SPart, Stmt};
use crate::backend::{cstr, san, tag_of, AX_FAIL, AX_OK};
use crate::sema::{ConstVal, camel_member};
use crate::types::*;

use super::codegen_fn::{BinInfo, FnGen};

impl<'s, 'a, 'c> FnGen<'s, 'a, 'c> {
    // ---------- 统一出口 ----------

    /// 兼容入口：转换已统一到 gen 的出口，这里只做一次转发，不要重复 coerce。
    pub fn gen_to(&mut self, e: &'a ast::EPtr, want: &Ty) -> String {
        self.gen(e, Some(want), false, false)
    }

    fn coerce_c(
        &mut self,
        v: String,
        src: &Ty,
        dst: &Ty,
        tok: &'a crate::frontend::lexer::Tok,
    ) -> String {
        if let TyS::Nullable(dinner) = &**dst {
            if src == dinner {
                return format!("({}){{ 1, {} }}", self.cg.ct(dst), v);
            }
        }
        if let (TyS::Result(dinner), TyS::Result(sinner)) = (&**dst, &**src) {
            if sinner == dinner {
                return v;
            }
            // result<null> / result<T> 统一到目标载荷类型
            let inner = dinner.clone();
            if matches!(&**sinner, TyS::Null) {
                let zero = if matches!(&*inner, TyS::Nullable(_)) {
                    format!("({}){{0}}", self.cg.ct(&inner))
                } else {
                    "0".to_string()
                };
                let dstc = self.cg.ct(dst);
                return format!("({}){{ .tag = {}, .u.ok = {} }}", dstc, AX_OK, zero);
            }
        }
        if matches!(&**dst, TyS::Object) && !matches!(&**src, TyS::Object) {
            return self.box_(v, src);
        }
        // 解箱（object → 具体类型，文稿 6.7）：目标类型已知时自动进行，
        // 运行时核对盒内类型标签，不符即终止。装箱的逆操作，与 cast 表达式
        // 的显式 `T(o)` 检查完全相同。
        if matches!(&**src, TyS::Object) && !matches!(&**dst, TyS::Object) {
            return self.unbox(v, dst, tok);
        }
        // 上转到 interface（GAP #13）：组装胖指针 { obj, vt }，vt 指向
        // (类, interface) 专属 shim 虚表；接收者调用时经 tv.vt 动态派发。
        if let Some(fat) = self.iface_upcast(&v, src, dst) {
            return fat;
        }
        // 上转（子类实例 → 父类指针）：布局兼容（父类字段排在最前），
        // 但 C 看到的是两个不同的 struct 指针类型，不显式强转会刷
        // -Wincompatible-pointer-types（GAP #14）。
        if let Some(target) = self.upcast_target(src, dst) {
            return format!("({}){}", self.cg.ct(&target), v);
        }
        v
    }

    /// src 类实现 dst interface 时返回 Some(胖指针 C 形态)；dst 也可以是
    /// 可空 interface（外面包一层 axopt）。
    fn iface_upcast(&mut self, v: &str, src: &Ty, dst: &Ty) -> Option<String> {
        let d = match &**dst {
            TyS::Named(DefId::Id(i), _) => *i,
            TyS::Nullable(inner) => match &**inner {
                TyS::Named(DefId::Id(i), _) => *i,
                _ => return None,
            },
            _ => return None,
        };
        let s = match &**src {
            TyS::Named(DefId::Id(c), _) => *c,
            _ => return None,
        };
        if d == s {
            return None;
        }
        let dd = self.cg.s.typedefs.get(d as usize)?;
        if dd.kind != "interface" {
            return None;
        }
        let sd = self.cg.s.typedefs.get(s as usize)?;
        if sd.kind != "class" || !self.cg.s.implements(s, d) {
            return None;
        }
        let fat_ty = t_named(DefId::Id(d), Vec::new());
        let fat_ct = self.cg.ct(&fat_ty);
        let vt = format!("{}__{}__VT_", san(&sd.name), san(&dd.name));
        let fat = format!("({}){{ (void*){}, &{} }}", fat_ct, v, vt);
        if matches!(&**dst, TyS::Nullable(_)) {
            let outer = self.cg.ct(dst);
            Some(format!("({}){{ 1, {} }}", outer, fat))
        } else {
            Some(fat)
        }
    }

    /// src 是 dst 的严格子类时返回 Some(dst)，其余返回 None。仅限 class →
    /// class 的裸指针场景（可空目标是 axopt 结构体、interface 是胖指针
    /// ——分别属另一套表示与 GAP #13，不走这条强转）。
    fn upcast_target(&self, src: &Ty, dst: &Ty) -> Option<Ty> {
        let d = match &**dst {
            TyS::Named(DefId::Id(i), _) => *i,
            _ => return None,
        };
        let s = match &**src {
            TyS::Named(DefId::Id(i), _) => *i,
            _ => return None,
        };
        if d == s {
            return None;
        }
        let dd = self.cg.s.typedefs.get(d as usize)?;
        if dd.kind != "class" {
            return None;
        }
        // src 沿 parent 链向上找 dst；链上出现非 class（不该发生）即停。
        let mut cur = self.cg.s.typedefs.get(s as usize).and_then(|t| t.parent);
        while let Some(i) = cur {
            if i == d {
                return Some(dst.clone());
            }
            cur = self
                .cg
                .s
                .typedefs
                .get(i as usize)
                .filter(|t| t.kind == "class")
                .and_then(|t| t.parent);
        }
        None
    }

    pub fn gen(
        &mut self,
        e: &'a ast::EPtr,
        want: Option<&Ty>,
        inline: bool,
        discard: bool,
    ) -> String {
        // try 块内的隐式展开：这个节点的静态类型是 T，但表达式本身产出 result<T>。
        // 先生成 result 值，再按"失败跳 catch"展开，等价于用户写了 `expr try`。
        if !self.suppress_implicit
            && self
                .cg
                .slot(&e.id)
                .map_or(false, |s| s.implicit_try)
        {
            self.suppress_implicit = true;
            let v = self._gen(e, None, false, false);
            self.suppress_implicit = false;
            let inner = self.ty_or_int(e.id);
            let res_ty = t_result(inner);
            let rv = self.declare(&res_ty, &v);
            self.emit_try_branch(&rv, &res_ty);
            return format!("{}.u.ok", rv);
        }
        // 统一出口：先生成表达式的 C 形态，再补 src→want 的表示层转换。
        // 各表达式分支自己只负责"按自己的类型生成值"，不要把 want 再往里塞——
        // 否则同一个转换会在内外两层各做一次（装箱会套两层盒子，可空会包两层结构体）。
        let v = self._gen(e, want, inline, discard);
        let want = match want {
            Some(w) => w,
            None => return v,
        };
        let src = match self.ty_opt(e.id) {
            Some(t) => t,
            None => return v,
        };
        if src == *want {
            return v;
        }
        self.coerce_c(v, &src, want, &e.tok)
    }

    fn _gen(
        &mut self,
        e: &'a ast::EPtr,
        want: Option<&Ty>,
        inline: bool,
        _discard: bool,
    ) -> String {
        match &e.k {
            Expr::IntLit { value, .. } => {
                let ty = self.ty_opt(e.id);
                if let Some(t) = &ty {
                    if matches!(&**t, TyS::Decimal) {
                        return format!("ax_dec_from_i64(INT64_C({}))", value);
                    }
                }
                format!("INT64_C({})", value)
            }
            Expr::FloatLit { value, is_f32, .. } => {
                let mut s = super::float_repr(*value);
                if *is_f32 {
                    // C 的 float 字面量需加 f 后缀，否则默认为 double
                    if !s.ends_with('f') && !s.ends_with('F') {
                        s.push('f');
                    }
                }
                s
            }
            Expr::DecLit { coeff, scale, .. } => {
                format!("ax_dec_lit(\"{}\", {})", coeff, scale)
            }
            Expr::BoolLit { value } => {
                if *value {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            Expr::NullLit => {
                let w_owned = self.ty_opt(e.id);
                let w: Option<&Ty> = want.or(w_owned.as_ref());
                match w {
                    Some(t) if matches!(&**t, TyS::Nullable(_)) => {
                        let tc = self.cg.ct(t);
                        format!("({}){{0}}", tc)
                    }
                    _ => "NULL".to_string(),
                }
            }
            Expr::StrLit { .. } => self.str_lit(e),
            Expr::Path { .. } => self.path(e),
            Expr::This => "self".to_string(),
            Expr::Parent => "self".to_string(),
            Expr::TupleLit { elems } => {
                let ty = self.ty_or_int(e.id);
                let mut vals = Vec::new();
                for (i, x) in elems.iter().enumerate() {
                    let v = self.gen(x, None, false, false);
                    vals.push(format!(".f{} = {}", i, v));
                }
                let tyc = self.cg.ct(&ty);
                format!("({}){{{}}}", tyc, vals.join(", "))
            }
            Expr::ArrayLit { .. } => self.array_lit(e),
            Expr::BraceLit { .. } => self.brace_lit(e),
            Expr::Unary { .. } => self.unary(e),
            Expr::Binary { .. } => self.binary(e),
            Expr::Ternary { cond, then_, els } => {
                // 两分支不在这里转 want：转换统一由 gen 的出口做一次，
                // 否则装箱/可空会被内外各包一层
                let c = self.gen(cond, None, true, false);
                let a2 = self.gen(then_, None, true, false);
                let b2 = self.gen(els, None, true, false);
                format!("({} ? {} : {})", c, a2, b2)
            }
            Expr::Coalesce { lhs, rhs } => {
                let l = self.gen(lhs, None, false, false);
                let lty = self.ty_or_int(lhs.id);
                let lv = self.declare(&lty, &l);
                // 右侧要转成可空的内层类型再参与三目（object? 的兜底值要装箱）
                let inner = match &*lty {
                    TyS::Nullable(i) => i.clone(),
                    _ => lty.clone(),
                };
                let r = self.gen(rhs, Some(&inner), false, false);
                format!("({}.has ? {}.v : {})", lv, lv, r)
            }
            Expr::Range { .. } => "0".to_string(),
            Expr::Try { .. } => self.try_expr(e),
            Expr::Call { .. } => self.call(e, want),
            Expr::MethodCall { .. } => self.method_call(e),
            Expr::SafeCall { .. } => self.safe_call(),
            Expr::FieldAccess { .. } => self.field_access(e),
            Expr::SafeAccess { .. } => self.safe_access(e),
            Expr::Index { .. } => self.index(e),
            Expr::New { .. } => self.new_expr(e),
            Expr::If(n) => {
                if inline {
                    return "0".to_string();
                }
                let ty: Ty = match want {
                    Some(w) => w.clone(),
                    None => self.ty_or_int(e.id),
                };
                let tv = self.tmpn();
                let tyc = self.cg.ct(&ty);
                self.emit(format!("{} {};", tyc, tv));
                self.emit(format!("memset(&{}, 0, sizeof({}));", tv, tv));
                self.if_stmt(n, e.id, Some(&ty), Some(&tv));
                tv
            }
            Expr::Match(n) => {
                let ty: Ty = match want {
                    Some(w) => w.clone(),
                    None => self.ty_or_int(e.id),
                };
                let tv = self.tmpn();
                let tyc = self.cg.ct(&ty);
                self.emit(format!("{} {};", tyc, tv));
                self.emit(format!("memset(&{}, 0, sizeof({}));", tv, tv));
                self.match_stmt(n, e.id, Some(&ty), Some(&tv));
                tv
            }
            Expr::AnonFunc { .. } => {
                // 文稿 4.1：匿名函数值 = 合成顶层函数的地址（不捕获环境）
                let fid = self
                    .cg
                    .slot(&e.id)
                    .and_then(|sl| sl.func)
                    .unwrap_or(0);
                self.cg.fn_name_idx(fid)
            }
            Expr::IsType { .. } => {
                // `is` 是编译期求值，结果已存为 const_value；直接输出布尔字面量
                if let Some(slot) = self.cg.s.slot(&e.id) {
                    if let Some(cv) = slot.const_value.clone() {
                        match cv {
                            ConstVal::Bool(b) => {
                                if b { "true".to_string() } else { "false".to_string() }
                            }
                            _ => "false".to_string(),
                        }
                    } else {
                        "false".to_string()
                    }
                } else {
                    "false".to_string()
                }
            }
        }
    }

    pub fn path(&mut self, e: &'a ast::EPtr) -> String {
        let segs: &'a [String] = match &e.k {
            Expr::Path { segs, .. } => segs,
            _ => &[],
        };
        // 单段名：优先作为局部变量/参数解析，返回 C 层名称（v_x / p_x）。
        // var_c 已按是否为参数返回 pv_x / lv_x，直接使用 base 即可。
        if segs.len() == 1 {
            // 函数引用（如 thread(producer, q1) 中的 producer）：直接返回函数名，无 v_ 前缀
            if let Some(crate::sema::Res::Func(fid)) = self.res_of(e.id) {
                return self.cg.fn_name_idx(fid);
            }
            // 模块级常量（如 const PART = ...）：直接内联常量值
            if let Some(crate::sema::Res::Const(c)) = self.res_of(e.id) {
                let value = c.value.borrow().clone();
                let ty = c.ty.borrow().clone();
                return match value {
                    Some(v) => self.cg.const_c(&v, &ty),
                    None => "0".to_string(),
                };
            }
            let (base, v) = self.var_c(e);
            // ref 参数（如 &const int[] xs）：C 层是指针，需解引用才能访问 .len/.data/.v 等字段
            if let Some(vi) = &v {
                let b = vi.borrow();
                if b.is_param && b.borrow.is_some() {
                    return format!("(*{})", base);
                }
            }
            return base;
        }
        let res = self.res_of(e.id);
        match res {
            Some(crate::sema::Res::Local(_)) => {
                let (base, v) = self.var_c(e);
                // ref 参数：C 层是指针，读取值时需解引用。加括号避免 * 与 . / [ 等后缀运算符优先级冲突：
                // (*p_xs).len 正确，*p_xs.len 会被解析为 *(p_xs.len)。
                // 若 base 已带 .v（可空拆箱），需只对基名解引用：(*p_s).v 而非 (*p_s.v)。
                if let Some(vi) = &v {
                    let b = vi.borrow();
                    if b.is_param && b.borrow.is_some() {
                        if let Some(base_name) = base.strip_suffix(".v") {
                            return format!("(*{}).v", base_name);
                        }
                        return format!("(*{})", base);
                    }
                }
                return base;
            }
            Some(crate::sema::Res::Const(c)) => {
                let value = c.value.borrow().clone();
                let ty = c.ty.borrow().clone();
                return match value {
                    Some(v) => self.cg.const_c(&v, &ty),
                    None => "0".to_string(),
                };
            }
            Some(crate::sema::Res::Func(fid)) => {
                // 单态化实例：slot.func 指向实例，使用实例名（GX-04 泛型函数值实例化）
                if let Some(inst_fid) = self.cg.slot(&e.id).and_then(|sl| sl.func) {
                    return self.cg.fn_name_idx(inst_fid);
                }
                return self.cg.fn_name_idx(fid);
            }
            Some(crate::sema::Res::EnumVariant(d, var)) => {
                let idx = self
                    .cg
                    .s
                    .typedefs
                    .get(d as usize)
                    .and_then(|td| td.variants.iter().position(|v| v == &var))
                    .unwrap_or(0);
                return format!("{}u", idx);
            }
            Some(crate::sema::Res::Rounding(name)) => {
                return match name.as_str() {
                    "halfUp" => "0".to_string(),
                    "halfEven" => "1".to_string(),
                    "down" => "2".to_string(),
                    _ => "0".to_string(),
                };
            }
            Some(crate::sema::Res::Builtin(b)) => {
                return b;
            }
            _ => {}
        }
        if segs.len() == 1 && segs[0] == "parent" {
            return "self".to_string();
        }
        segs.first().cloned().unwrap_or_default()
    }

    /// 把一个值按类型追加进字符串缓冲。
    ///
    /// 可空值单独成一支：有值追加内部值的显示形式，无值追加 `null`——
    /// 插值只做"显示"，不产生可空值逃逸，因此不触发 E4001（与赋值/传参不同）。
    /// 其余标量与 `enum` 各有直接对应；`fmt`（`:.N`）只对数值生效，sema 已保证。
    fn push_interp(
        &mut self,
        sb: &str,
        val: &str,
        ty: &Ty,
        fmt: Option<u32>,
        expr: &'a ast::EPtr,
    ) {
        if let TyS::Nullable(inner) = &**ty {
            let inner = inner.clone();
            let v = self.declare(ty, val);
            self.emit(format!("if ({}.has) {{", v));
            self.ind += 1;
            self.push_interp(sb, &format!("{}.v", v), &inner, fmt, expr);
            self.ind -= 1;
            self.emit("} else {".to_string());
            self.ind += 1;
            self.emit(format!(
                "ax_sb_push(&{}, ax_str_from_cstr(\"null\"));",
                sb
            ));
            self.ind -= 1;
            self.emit("}".to_string());
            return;
        }
        match fmt {
            Some(f) => match &**ty {
                TyS::Decimal => self.emit(format!("ax_sb_push_dec_n(&{}, {}, {});", sb, val, f)),
                TyS::Float | TyS::Float32 => self.emit(format!("ax_sb_push_f64_n(&{}, {}, {});", sb, val, f)),
                _ => self.emit(format!("ax_sb_push_int(&{}, {});", sb, val)),
            },
            None => match &**ty {
                TyS::Str => self.emit(format!("ax_sb_push(&{}, {});", sb, val)),
                TyS::Decimal => {
                    self.emit(format!("ax_sb_push(&{}, ax_dec_to_str({}));", sb, val))
                }
                TyS::Float | TyS::Float32 => self.emit(format!("ax_sb_push_f64(&{}, {});", sb, val)),
                TyS::Bool => self.emit(format!("ax_sb_push(&{}, ax_str_of_bool({}));", sb, val)),
                TyS::Enum(_) => {
                    let en = self.enum_name_expr(expr, val);
                    self.emit(format!("ax_sb_push(&{}, {});", sb, en))
                }
                TyS::Int(..) => self.emit(format!("ax_sb_push_int(&{}, (int64_t)({}));", sb, val)),
                TyS::Array(elem, _) => self.push_arr_interp(sb, val, elem),
                TyS::Map(kt, vt) => self.push_map_interp(sb, val, kt, vt),
                TyS::Tuple(elems) => self.push_tuple_interp(sb, val, elems),
                TyS::Named(did, _) => {
                    // Printable（文稿 7.1）：实现类/struct 直接调 toString；
                    // Printable 类型变量本身经虚表动态派发。可空形式已在
                    // 上面的 Nullable 分支拆包，落到这里的内层值。
                    let printable = self.cg.s.types.get("Printable").copied();
                    let handled = match (printable, did) {
                        (Some(p), DefId::Id(c)) => {
                            let c = *c;
                            let td = self.cg.s.typedefs[c as usize].clone();
                            if td.kind == "class" && self.cg.s.implements(c, p) {
                                let fid = match td.methods.get("toString") {
                                    Some(&f) => f,
                                    None => 0,
                                };
                                let fname = self.cg.fn_name_idx(fid);
                                self.emit(format!("ax_sb_push(&{}, {}({}));", sb, fname, val));
                                true
                            } else if td.kind == "struct" && self.cg.s.implements(c, p) {
                                let fid = match td.methods.get("toString") {
                                    Some(&f) => f,
                                    None => 0,
                                };
                                let fname = self.cg.fn_name_idx(fid);
                                self.emit(format!("ax_sb_push(&{}, {}(&{}));", sb, fname, val));
                                true
                            } else if td.kind == "interface" && c == p {
                                let iname = format!("Iface_{}", san(&td.name));
                                self.emit(format!(
                                    "ax_sb_push(&{sb}, (({iname}_VT*){val}.vt)->toString((void*){val}.obj));",
                                    sb = sb,
                                    iname = iname,
                                    val = val
                                ));
                                true
                            } else {
                                false
                            }
                        }
                        _ => false,
                    };
                    if !handled {
                        // 兜底：sema 应已拦下（E5006）。留注释而非静默 `?`，便于定位漏网类型。
                        let name = self.cg.ct(ty);
                        self.emit(format!(
                            "ax_sb_push(&{}, ax_str_from_cstr(\"?\")); /* 插值未覆盖: {} */",
                            sb, name
                        ));
                    }
                }
                // 兜底：sema 应已拦下（E5006）。留注释而非静默 `?`，便于定位漏网类型。
                _ => {
                    let name = self.cg.ct(ty);
                    self.emit(format!(
                        "ax_sb_push(&{}, ax_str_from_cstr(\"?\")); /* 插值未覆盖: {} */",
                        sb, name
                    ))
                }
            },
        }
    }

    /// 将单个值表达式 `expr`（类型 `ty`）转为字符串，推入 `sb`。
    /// 仅覆盖标量；复合嵌套与 box 对象输出 `?`。
    fn push_one_to_sb(&mut self, sb: &str, expr: &str, ty: &Ty) {
        match &**ty {
            TyS::Int(..) => self.emit(format!("ax_sb_push_int(&{}, (int64_t)({}));", sb, expr)),
            TyS::Float => self.emit(format!("ax_sb_push_f64(&{}, {});", sb, expr)),
            TyS::Decimal => self.emit(format!("ax_sb_push(&{}, ax_dec_to_str({}));", sb, expr)),
            TyS::Bool => self.emit(format!("ax_sb_push(&{}, ax_str_of_bool({}));", sb, expr)),
            TyS::Str => self.emit(format!("ax_sb_push(&{}, {});", sb, expr)),
            TyS::Enum(_) => self.emit(format!("ax_sb_push(&{}, ax_str_from_cstr(\"?\"));", sb)),
            _ => self.emit(format!("ax_sb_push(&{}, ax_str_from_cstr(\"?\"));", sb)),
        }
    }

    /// 数组默认显示：`[e0, e1, ...]`
    fn push_arr_interp(&mut self, sb: &str, val: &str, elem: &Ty) {
        let etc = self.cg.ct(elem);
        let tmp = self.tmpn();
        let i = self.tmpn();
        let lb = format!("AX_S({})", cstr("["));
        let rb = format!("AX_S({})", cstr("]"));
        let comma = format!("AX_S({})", cstr(", "));
        self.emit(format!("ax_sb {}; ax_sb_init(&{});", tmp, tmp));
        self.emit(format!("ax_sb_push(&{}, {});", tmp, lb));
        self.emit(format!("int64_t {};", i));
        self.emit(format!("for ({} = 0; {} < {}.len; {}++) {{", i, i, val, i));
        self.ind += 1;
        self.emit(format!("if ({} > 0) ax_sb_push(&{}, {});", i, tmp, comma));
        let elem_expr = format!("(({}*){}.data)[{}]", etc, val, i);
        self.push_one_to_sb(&tmp, &elem_expr, elem);
        self.ind -= 1;
        self.emit("}".to_string());
        self.emit(format!("ax_sb_push(&{}, {});", tmp, rb));
        self.emit(format!("ax_sb_push(&{}, ax_sb_finish(&{}));", sb, tmp));
    }

    /// map 默认显示：`{k0: v0, k1: v1, ...}`
    fn push_map_interp(&mut self, sb: &str, val: &str, kt: &Ty, vt: &Ty) {
        let tmp = self.tmpn();
        let i = self.tmpn();
        let lb = format!("AX_S({})", cstr("{"));
        let rb = format!("AX_S({})", cstr("}"));
        let comma = format!("AX_S({})", cstr(", "));
        let colon = format!("AX_S({})", cstr(":"));
        self.emit(format!("ax_sb {}; ax_sb_init(&{});", tmp, tmp));
        self.emit(format!("ax_sb_push(&{}, {});", tmp, lb));
        self.emit(format!("int64_t {};", i));
        self.emit(format!("for ({} = 0; {} < {}.len; {}++) {{", i, i, val, i));
        self.ind += 1;
        self.emit(format!("if ({} > 0) ax_sb_push(&{}, {});", i, tmp, comma));
        let ktc = self.cg.ct(kt);
        let key_expr = format!("(*({}*){}.entries[{}].k)", ktc, val, i);
        self.push_one_to_sb(&tmp, &key_expr, kt);
        self.emit(format!("ax_sb_push(&{}, {});", tmp, colon));
        let vtc = self.cg.ct(vt);
        let val_expr = format!("(*({}*){}.entries[{}].v)", vtc, val, i);
        self.push_one_to_sb(&tmp, &val_expr, vt);
        self.ind -= 1;
        self.emit("}".to_string());
        self.emit(format!("ax_sb_push(&{}, {});", tmp, rb));
        self.emit(format!("ax_sb_push(&{}, ax_sb_finish(&{}));", sb, tmp));
    }

    /// 元组默认显示：`(e0, e1, ...)`
    fn push_tuple_interp(&mut self, sb: &str, val: &str, elems: &[Ty]) {
        let tmp = self.tmpn();
        let lp = format!("AX_S({})", cstr("("));
        let rp = format!("AX_S({})", cstr(")"));
        let comma = format!("AX_S({})", cstr(", "));
        self.emit(format!("ax_sb {}; ax_sb_init(&{});", tmp, tmp));
        self.emit(format!("ax_sb_push(&{}, {});", tmp, lp));
        for (idx, elem_ty) in elems.iter().enumerate() {
            if idx > 0 {
                self.emit(format!("ax_sb_push(&{}, {});", tmp, comma));
            }
            let field_expr = format!("{}.f{}", val, idx);
            self.push_one_to_sb(&tmp, &field_expr, elem_ty);
        }
        self.emit(format!("ax_sb_push(&{}, {});", tmp, rp));
        self.emit(format!("ax_sb_push(&{}, ax_sb_finish(&{}));", sb, tmp));
    }

    fn str_lit(&mut self, e: &'a ast::EPtr) -> String {
        let parts: &'a [SPart] = match &e.k {
            Expr::StrLit { parts } => parts,
            _ => &[],
        };
        if parts.len() == 1 {
            if let SPart::Text(t) = &parts[0] {
                return format!("AX_S({})", cstr(t));
            }
        }
        if parts.is_empty() {
            return "AX_S(\"\")".to_string();
        }
        let sb = self.tmpn();
        self.emit(format!("ax_sb {}; ax_sb_init(&{});", sb, sb));
        for p in parts {
            match p {
                SPart::Text(t) => {
                    if !t.is_empty() {
                        let lit = format!("AX_S({})", cstr(t));
                        self.emit(format!("ax_sb_push(&{}, {});", sb, lit));
                    }
                }
                SPart::Interp { expr, fmt } => {
                    let ty = self.ty_or_int(expr.id);
                    let val = self.gen(expr, None, false, false);
                    self.push_interp(&sb, &val, &ty, *fmt, expr);
                }
            }
        }
        let t = self.tmpn();
        self.emit(format!("ax_str {} = ax_sb_finish(&{});", t, sb));
        t
    }

    fn enum_name_expr(&mut self, expr: &'a ast::EPtr, val: &str) -> String {
        let ty = self.ty_or_int(expr.id);
        let d = match &*ty {
            TyS::Enum(id) => self.cg.s.typedefs.get(*id as usize).cloned(),
            _ => None,
        };
        let d = match d {
            Some(d) => d,
            None => return "ax_str_from_cstr(\"?\")".to_string(),
        };
        let mut parts = Vec::new();
        for (i, v) in d.variants.iter().enumerate() {
            parts.push(format!(
                "({} == {} ? ax_str_from_cstr({}) : ",
                val,
                i,
                cstr(v)
            ));
        }
        parts.push("ax_str_from_cstr(\"?\")".to_string());
        parts.push(")".repeat(d.variants.len()));
        parts.concat()
    }

    fn array_lit(&mut self, e: &'a ast::EPtr) -> String {
        let ty = self.ty_or_int(e.id);
        let elems: &'a [ast::EPtr] = match &e.k {
            Expr::ArrayLit { elems } => elems,
            _ => &[],
        };
        let (et, fixed) = match &*ty {
            TyS::Array(e2, l) => (e2.clone(), *l),
            _ => (t_int(), None),
        };
        if fixed.is_some() {
            let vals: Vec<String> = elems
                .iter()
                .map(|x| self.gen(x, Some(&et), false, false))
                .collect();
            let tyc = self.cg.ct(&ty);
            return format!("({}){{ .v = {{ {} }} }}", tyc, vals.join(", "));
        }
        if elems.is_empty() {
            return "((ax_arr){0, 0, NULL})".to_string();
        }
        let vals: Vec<String> = elems
            .iter()
            .map(|x| self.gen(x, Some(&et), false, false))
            .collect();
        let etc = self.cg.ct(&et);
        format!(
            "ax_arr_of(({}[]){{ {} }}, {}, (int64_t)sizeof({}))",
            etc,
            vals.join(", "),
            elems.len(),
            etc
        )
    }

    fn brace_lit(&mut self, e: &'a ast::EPtr) -> String {
        let ty = self.ty_or_int(e.id);
        let pairs: &'a [(String, ast::EPtr)] = match &e.k {
            Expr::BraceLit { pairs } => pairs,
            _ => &[],
        };
        if let TyS::Map(kt, vt) = &*ty {
            let kt = kt.clone();
            let vt = vt.clone();
            let n = self.tmpn();
            let ktc = self.cg.ct(&kt);
            let vtc = self.cg.ct(&vt);
            let kind = self.map_kind(&kt);
            self.emit(format!(
                "ax_map {}; ax_map_init(&{}, {}, (int64_t)sizeof({}), (int64_t)sizeof({}));",
                n, n, kind, ktc, vtc
            ));
            for (kk, v) in pairs {
                let klit = format!("AX_S({})", cstr(kk));
                let kc = self.declare(&t_string(), &klit);
                let vc = self.gen(v, Some(&vt), false, false);
                let vc = self.declare(&vt, &vc);
                self.emit(format!("ax_map_put(&{}, &{}, &{});", n, kc, vc));
            }
            return n;
        }
        // record
        let mut fields: Vec<String> = Vec::new();
        for (kk, v) in pairs {
            let g = self.gen(v, None, false, false);
            fields.push(format!(".f_{} = {}", san(kk), g));
        }
        let tyc = self.cg.ct(&ty);
        format!("({}){{ {} }}", tyc, fields.join(", "))
    }

    fn map_kind(&self, kty: &Ty) -> u32 {
        match &**kty {
            TyS::Str => 1,
            TyS::Bool => 2,
            TyS::Decimal => 3,
            TyS::Enum(_) => 4,
            TyS::Object => 5,
            TyS::Named(did, _) => {
                let is_class = self.cg.def_of(did).map_or(false, |d| d.kind == "class");
                if is_class {
                    5
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    fn unary(&mut self, e: &'a ast::EPtr) -> String {
        let (op, operand) = match &e.k {
            Expr::Unary { op, operand } => (op, operand),
            _ => unreachable!(),
        };
        if let Some(crate::sema::ConstVal::Int(v)) = self.slot_folded(e.id) {
            return format!("INT64_C({})", v);
        }
        let v = self.gen(operand, None, false, false);
        if op == "-" {
            let t = self.ty_or_int(e.id);
            match &*t {
                TyS::Int(..) => {
                    let pa = self.pos_args(&e.tok);
                    return format!("ax_neg_i64({}, {})", v, pa);
                }
                TyS::Float => return format!("(-{})", v),
                _ => return format!("ax_dec_sub(ax_dec_from_i64(0), {})", v),
            }
        }
        if op == "!" {
            return format!("(!{})", v);
        }
        if op == "~" {
            return format!("(~{})", v);
        }
        v
    }

    fn slot_folded(&self, id: u32) -> Option<crate::sema::ConstVal> {
        self.cg.slot(&id).and_then(|s| s.folded.clone())
    }

    fn binary(&mut self, e: &'a ast::EPtr) -> String {
        let (op, lhs, rhs) = match &e.k {
            Expr::Binary { op, lhs, rhs } => (op, lhs, rhs),
            _ => unreachable!(),
        };
        let lt = self.ty_or_int(lhs.id);
        let rt = self.ty_or_int(rhs.id);
        if op == "&&" || op == "||" {
            let a = self.gen(lhs, None, false, false);
            let b = self.gen(rhs, None, true, false);
            return format!("({} {} {})", a, op, b);
        }
        let a = self.gen(lhs, None, false, false);
        let b = self.gen(rhs, None, true, false);
        let info = BinInfo {
            tok: &e.tok,
            dec_conv: self.cg.slot(&e.id).and_then(|s| s.dec_conv),
            null_lhs: matches!(&lhs.k, Expr::NullLit),
            null_rhs: matches!(&rhs.k, Expr::NullLit),
        };
        self.binop_c(op, a, b, &lt, &info, Some(&rt))
    }

    pub fn binop_c(
        &mut self,
        op: &str,
        a: String,
        b: String,
        ty: &Ty,
        node: &BinInfo,
        rt: Option<&Ty>,
    ) -> String {
        let _rt = rt.unwrap_or(ty);
        // decimal 与整数混算（文稿 §12 `this.price * count`）：整数在运行期精确升为 decimal
        let (mut a, mut b) = (a, b);
        if let Some(conv) = node.dec_conv {
            if conv == "rhs" {
                b = format!("ax_dec_from_i64((int64_t)({}))", b);
            } else if conv == "lhs" {
                a = format!("ax_dec_from_i64((int64_t)({}))", a);
            }
        }
        let k: &TyS = ty;
        match k {
            TyS::Str => match op {
                "+" => format!("ax_str_concat({}, {})", a, b),
                "==" => format!("ax_str_eq({}, {})", a, b),
                "!=" => format!("(!ax_str_eq({}, {}))", a, b),
                "<" | "<=" | ">" | ">=" => format!("(ax_str_cmp({}, {}) {} 0)", a, b, op),
                _ => format!("({} {} {})", a, op, b),
            },
            TyS::Array(et, _) => match op {
                "==" => format!(
                    "ax_arr_eq(&{}, &{}, (int64_t)sizeof({}))",
                    a,
                    b,
                    self.cg.ct(&et.clone())
                ),
                "!=" => format!(
                    "(!ax_arr_eq(&{}, &{}, (int64_t)sizeof({})))",
                    a,
                    b,
                    self.cg.ct(&et.clone())
                ),
                _ => format!("({} {} {})", a, op, b),
            },
            // 实现 Comparable 的类（文稿 7.1）：`a > b` 按 `a.compare(b) > 0`
            TyS::Named(DefId::Id(d), _) if matches!(op, "<" | "<=" | ">" | ">=") => {
                let mut cur = Some(*d);
                let mut cfid = None;
                while let Some(c) = cur {
                    let td = self.cg.s.typedefs.get(c as usize);
                    if td.map(|t| t.kind == "class") != Some(true) {
                        break;
                    }
                    if let Some(&fid) = td.and_then(|t| t.methods.get("compare")) {
                        cfid = Some(fid);
                        break;
                    }
                    cur = td.and_then(|t| t.parent);
                }
                match cfid {
                    Some(fid) => {
                        let cn = self.cg.fn_name_idx(fid);
                        format!("({}({}, {}) {} 0)", cn, a, b, op)
                    }
                    None => format!("({} {} {})", a, op, b),
                }
            }
            TyS::Decimal => match op {
                "+" => format!("ax_dec_add({}, {})", a, b),
                "-" => format!("ax_dec_sub({}, {})", a, b),
                "*" => format!("ax_dec_mul({}, {})", a, b),
                "/" => {
                    let pa = self.pos_args(node.tok);
                    format!("ax_dec_div_exact({}, {}, {})", a, b, pa)
                }
                "==" => format!("(ax_dec_cmp({}, {}) == 0)", a, b),
                "!=" => format!("(ax_dec_cmp({}, {}) != 0)", a, b),
                "<" | "<=" | ">" | ">=" => format!("(ax_dec_cmp({}, {}) {} 0)", a, b, op),
                "%" => format!("ax_dec_rem({}, {})", a, b),
                _ => format!("({} {} {})", a, op, b),
            },
            TyS::Float => match op {
                "+" => format!("ax_fadd({}, {})", a, b),
                "-" => format!("ax_fsub({}, {})", a, b),
                "*" => format!("ax_fmul({}, {})", a, b),
                "/" => {
                    let pa = self.pos_args(node.tok);
                    format!("ax_fdiv({}, {}, {})", a, b, pa)
                }
                _ => format!("({} {} {})", a, op, b),
            },
            TyS::Bool => {
                if op == "==" {
                    return format!("({} == {})", a, b);
                }
                if op == "!=" {
                    return format!("({} != {})", a, b);
                }
                format!("({} {} {})", a, op, b)
            }
            TyS::Int(bits, _signed) => {
                let is64 = *bits == 64;
                match op {
                    "+" | "-" | "*" => {
                        let f = match op {
                            "+" => "ax_add_i64",
                            "-" => "ax_sub_i64",
                            _ => "ax_mul_i64",
                        };
                        if is64 {
                            let pa = self.pos_args(node.tok);
                            format!("{}({}, {}, {})", f, a, b, pa)
                        } else {
                            format!("({} {} {})", a, op, b)
                        }
                    }
                    "/" => {
                        if is64 {
                            let pa = self.pos_args(node.tok);
                            format!("ax_div_i64({}, {}, {})", a, b, pa)
                        } else {
                            format!("({} / {})", a, b)
                        }
                    }
                    "%" => {
                        if is64 {
                            let pa = self.pos_args(node.tok);
                            format!("ax_mod_i64({}, {}, {})", a, b, pa)
                        } else {
                            format!("({} % {})", a, b)
                        }
                    }
                    "&" | "|" | "^" => format!("({} {} {})", a, op, b),
                    "<<" => {
                        if is64 {
                            let pa = self.pos_args(node.tok);
                            format!("ax_shl_i64({}, {}, {})", a, b, pa)
                        } else {
                            format!("({} << {})", a, b)
                        }
                    }
                    ">>" => {
                        if is64 {
                            let pa = self.pos_args(node.tok);
                            format!("ax_shr_i64({}, {}, {})", a, b, pa)
                        } else {
                            format!("({} >> {})", a, b)
                        }
                    }
                    _ => format!("({} {} {})", a, op, b),
                }
            }
            TyS::Enum(_) => format!("({} {} {})", a, op, b),
            TyS::Nullable(_) => {
                if op == "==" {
                    format!("(!{}.has)", a)
                } else if op == "!=" {
                    format!("({}.has)", a)
                } else {
                    format!("({} {} {})", a, op, b)
                }
            }
            _ => {
                // 历史行为（测试钉死）：nullable 分支外的 NullLit 兜底、object/named 与最终兜底
                if node.null_lhs || node.null_rhs {
                    if op == "==" {
                        return format!("(!{}.has)", a);
                    }
                    if op == "!=" {
                        return format!("({}.has)", a);
                    }
                }
                if matches!(k, TyS::Object | TyS::Named(..)) {
                    if op == "==" {
                        return format!("({} == {})", a, b);
                    }
                    if op == "!=" {
                        return format!("({} != {})", a, b);
                    }
                }
                format!("({} {} {})", a, op, b)
            }
        }
    }

    fn try_expr(&mut self, e: &'a ast::EPtr) -> String {
        let inner = match &e.k {
            Expr::Try { inner } => inner,
            _ => unreachable!(),
        };
        let inner_c = self.gen(inner, None, false, false);
        let res_ty = self.ty_or_int(inner.id);
        let rv = self.declare(&res_ty, &inner_c);
        self.emit_try_branch(&rv, &res_ty);
        format!("{}.u.ok", rv)
    }

    /// 展开失败时的分支：在 `try` 块内跳 catch，否则照旧返回 `fail`（main 打印后退出 1）。
    pub fn emit_try_branch(&mut self, rv: &str, res_ty: &Ty) {
        let ctx = self.try_stack.last().map(|c| {
            (
                c.err_var.clone(),
                c.target.clone(),
                c.defers_depth,
            )
        });
        if let Some((err_var, target, depth)) = ctx {
            self.emit(format!("if ({}.tag == {}) {{", rv, AX_FAIL));
            self.ind += 1;
            self.emit_defers_from(depth);
            self.emit(format!("{} = {}.u.err;", err_var, rv));
            self.emit(format!("goto {};", target));
            self.ind -= 1;
            self.emit("}".to_string());
            return;
        }
        self.emit(format!("if ({}.tag == {}) {{", rv, AX_FAIL));
        self.ind += 1;
        self.emit_defers(None);
        if self.is_main {
            self.emit(format!("ax_write_stderr({}.u.err.msg);", rv));
            self.emit("ax_write_stderr(AX_S(\"\\n\"));".to_string());
            self.emit("return 1;".to_string());
        } else {
            let ret = if matches!(&*self.f.ret, TyS::Result(_)) {
                self.f.ret.clone()
            } else {
                res_ty.clone()
            };
            let retc = self.cg.ct(&ret);
            self.emit(format!(
                "return ({}){{ .tag = {}, .u.err = {}.u.err }};",
                retc, AX_FAIL, rv
            ));
        }
        self.ind -= 1;
        self.emit("}".to_string());
    }

    // ---------- 调用 ----------

    fn call(&mut self, e: &'a ast::EPtr, want: Option<&Ty>) -> String {
        let args: &'a [ast::N<ast::Arg>] = match &e.k {
            Expr::Call { args, .. } => args,
            _ => &[],
        };
        // 函数值调用（文稿 4.1）：`f(x)`，f 是 func 类型的局部变量/参数
        // → C 函数指针可直接调用
        if let Expr::Call { callee, .. } = &e.k {
            if let Expr::Path { segs, .. } = &callee.k {
                if segs.len() == 1 {
                    let res = self.res_of(callee.id);
                    if let Some(crate::sema::Res::Local(_v)) = &res {
                        let ct = self.ty_or_int(callee.id);
                        if matches!(&*ct, TyS::Func(..)) {
                            let (nm, _vi) = self.var_c(callee);
                            let mut a: Vec<String> = Vec::new();
                            for x in args {
                                a.push(self.gen(&x.k.expr, None, false, false));
                            }
                            return format!("{}({})", nm, a.join(", "));
                        }
                    }
                }
            }
        }
        let ck = self
            .cg
            .slot(&e.id)
            .and_then(|s| s.ckind)
            .unwrap_or("");
        if ck == "ctor" {
            // ctor_def 槽在 Rust 侧由 e.ty 反推（Named(Id(d))/Enum(d)）
            let ty = self.ty_or_int(e.id);
            let d: u32 = match &*ty {
                TyS::Named(DefId::Id(d), _) => *d,
                TyS::Enum(d) => *d,
                _ => 0,
            };
            let td = self.cg.s.typedefs[d as usize].clone();
            if td.kind == "struct" {
                return self.struct_literal(&ty, &td, args);
            }
            // 优先使用名为 "constructor" 的构造函数；否则取第一个
            let ctor_fid = td.ctors.get("constructor")
                .copied()
                .or_else(|| td.ctors.values().next().copied());
            let nm = match ctor_fid {
                Some(fid) => {
                    let fname = &self.cg.s.funcs[fid].name;
                    format!("{}__new_{}", self.cg.class_name(&td), fname)
                }
                None => format!("{}__new", self.cg.class_name(&td)),
            };
            let params: Vec<(String, Ty, Option<String>)> = match ctor_fid {
                Some(c) => self.cg.s.funcs[c].params.clone(),
                None => Vec::new(),
            };
            let mut out = Vec::new();
            for (i, arg) in args.iter().enumerate() {
                let wt = params.get(i).map(|p| p.1.clone());
                let v = match wt {
                    Some(w) => self.gen(&arg.k.expr, Some(&w), false, false),
                    None => self.gen(&arg.k.expr, None, false, false),
                };
                out.push(v);
            }
            return format!("{}({})", nm, out.join(", "));
        }
        if ck == "ok" {
            let ty = self.ty_or_int(e.id);
            let rt: Ty = match want {
                Some(w) if matches!(&**w, TyS::Result(_)) => w.clone(),
                _ => ty.clone(),
            };
            let inner: Ty = match &*rt {
                TyS::Result(i) => i.clone(),
                _ => match &*ty {
                    TyS::Result(i) => i.clone(),
                    _ => t_object(),
                },
            };
            let v = self.gen_to(&args[0].k.expr, &inner);
            let rtc = self.cg.ct(&rt);
            return format!("({}){{ .tag = {}, .u.ok = {} }}", rtc, AX_OK, v);
        }
        if ck == "fail" {
            let v = self.gen(&args[0].k.expr, Some(&t_error()), false, false);
            let rt = if matches!(&*self.f.ret, TyS::Result(_)) {
                self.f.ret.clone()
            } else {
                self.ty_or_int(e.id)
            };
            let rtc = self.cg.ct(&rt);
            return format!("({}){{ .tag = {}, .u.err = {} }}", rtc, AX_FAIL, v);
        }
        if ck == "parent_ctor" {
            let mut parts = vec!["(void*)self".to_string()];
            for arg in args {
                parts.push(self.gen(&arg.k.expr, None, false, false));
            }
            let pd = self
                .cg
                .slot(&e.id)
                .and_then(|s| s.parent_def)
                .unwrap_or(0);
            let ptd = self.cg.s.typedefs[pd as usize].clone();
            // 父类的 init 函数名带 constructor 名后缀（如 Animal__init_constructor）
            let ctor_name = ptd
                .ctors
                .get("constructor")
                .map(|_| "constructor".to_string())
                .or_else(|| ptd.ctors.keys().next().cloned())
                .unwrap_or_else(|| "constructor".to_string());
            return format!(
                "{}__init_{}({})",
                self.cg.class_name(&ptd),
                ctor_name,
                parts.join(", ")
            );
        }
        if ck == "sizeof" {
            // 编译期大小：实参不出现在生成代码里，只发 C 的 sizeof；
            // ct 已按语义定形——class 是 `T*`（引用），string 是 `ax_str`，
            // 数组是 `ax_arr`，对象大小不在此问
            let arg = &args[0].k.expr;
            let aty = self.ty_or_int(arg.id);
            let cty = self.cg.ct(&aty);
            return format!("((int64_t)sizeof({}))", cty);
        }
        if ck == "typeof" {
            // 编译期常量：直接生成字符串字面量，实参根本不出现在生成代码里
            let name = self
                .cg
                .slot(&e.id)
                .and_then(|s| s.bname.clone())
                .unwrap_or_default();
            return format!("AX_S({})", cstr(&name));
        }
        if ck == "thread" {
            // thread(func, arg) -> ax_thread_create(func, arg)
            // func 必须是 func 类型，arg 作为 void* 上下文传入
            let args: &'a [ast::N<ast::Arg>] = match &e.k {
                Expr::Call { args, .. } => args,
                _ => return "0".to_string(),
            };
            let fv = if args.is_empty() {
                "NULL".to_string()
            } else {
                self.gen(&args[0].k.expr, None, false, false)
            };
            let av = if args.len() < 2 {
                "NULL".to_string()
            } else {
                self.gen(&args[1].k.expr, None, false, false)
            };
            return format!("ax_thread_create((ax_thread_fn){}, {})", fv, av);
        }
        if ck == "queue_create" {
            // queue<T>() -> ax_queue_create((int64_t)sizeof(T))
            let ty = self.ty_or_int(e.id);
            let elem_ty = match &*ty {
                TyS::Named(DefId::Name("queue"), args) => {
                    args.first().cloned().unwrap_or_else(t_object)
                }
                _ => t_object(),
            };
            let etc = self.cg.ct(&elem_ty);
            return format!("ax_queue_create((int64_t)sizeof({}))", etc);
        }
        if ck == "builtin" {
            let bname = self
                .cg
                .slot(&e.id)
                .and_then(|s| s.bname.clone())
                .unwrap_or_default();
            if bname == "print" || bname == "println" {
                self.emit_print(args, bname == "println");
                return "0".to_string();
            }
            return "0".to_string();
        }
        if ck == "module" {
            // std.time.now() / std.time.parse() 等代码生成内建（Call + 路径形式）：
            // 从 callee 路径中提取模块名，去掉 "std." 前缀后匹配 codegen builtin。
            if let Expr::Call { callee, args } = &e.k {
                if let Expr::Path { segs, .. } = &callee.k {
                    if segs.len() >= 3 && segs[0] == "std" {
                        let mod_name = segs[1].clone();
                        let fn_name = segs.last().unwrap().clone();
                        let ptypes = crate::sema::module_sig(&mod_name, &fn_name).map(|(ps, _)| ps);
                        let mut a: Vec<String> = Vec::new();
                        for (i, x) in args.iter().enumerate() {
                            let pt = ptypes.as_ref().and_then(|ps| ps.get(i).cloned().flatten());
                            let v = match pt {
                                Some(pt) => self.gen(&x.k.expr, Some(&pt), false, false),
                                None => self.gen(&x.k.expr, None, false, false),
                            };
                            a.push(v);
                        }
                        return match (mod_name.as_str(), fn_name.as_str()) {
                            ("time", "now") => "ax_now_ms()".to_string(),
                            ("time", "from_unix_ms") => a[0].clone(),
                            ("time", "parse") => {
                                format!("ax_time_parse_result({}, {})", a[0], a[1])
                            }
                            _ => "0".to_string(),
                        };
                    }
                }
            }
            return "0".to_string();
        }
        if ck == "func" {
            let fid = self
                .cg
                .slot(&e.id)
                .and_then(|s| s.func)
                .unwrap_or(0);
            let f = &self.cg.s.funcs[fid];
            let nm = self.cg.fn_name_idx(fid);
            // extern "C" 与 C 库交互：
            //   - axrt 函数（名以 ax_ 开头，如 ax_str_cmp）：期望 ax_str 结构体，直接传递，不转换
            //   - C 库函数（如 puts）：期望 const char*，将 ax_str 转为 (const char*)({}).p
            let is_extern_c_lib = f.is_extern && !nm.starts_with("ax_");
            let ret_is_str = matches!(&*f.ret, TyS::Str);
            let params = f.params.clone();
            let mut out = Vec::new();
            for (i, arg) in args.iter().enumerate() {
                let v = match params.get(i) {
                    Some(p) => {
                        // ref 形参：实参需传地址。
                        //   - 局部变量：取 &var
                        //   - 本身是 ref 形参：直接传指针（var_c 取 base，不走 path() 的 *ptr）
                        //   - 字面量/复杂表达式：先落临时变量，再取其地址
                        if p.2.is_some() {
                            if let Expr::Path { .. } = &arg.k.expr.k {
                                if let Some(crate::sema::Res::Local(vi)) = self.res_of(arg.k.expr.id) {
                                    let b = vi.borrow();
                                    let (base, _) = self.var_c(&arg.k.expr);
                                    if b.is_param && b.borrow.is_some() {
                                        out.push(base);
                                        continue;
                                    } else if !b.is_param {
                                        out.push(format!("&{}", base));
                                        continue;
                                    }
                                }
                            }
                            // 非左值实参：创建临时变量，传其地址
                            let val = self.gen(&arg.k.expr, Some(&p.1), false, false);
                            let temp = self.declare(&p.1, &val);
                            out.push(format!("&{}", temp));
                            continue;
                        }
                        let mut val =
                            self.gen(&arg.k.expr, Some(&p.1), false, false);
                        if is_extern_c_lib && matches!(&*p.1, TyS::Str) {
                            // ax_str -> const char*：取底层指针（AX_S 等字面量已空终止）
                            val = format!("(const char*)({}).p", val);
                        }
                        val
                    }
                    None => self.gen(&arg.k.expr, None, false, false),
                };
                out.push(v);
            }
            let mut call = format!("{}({})", nm, out.join(", "));
            if f.is_ref_return {
                // ref 返回值：解引用指针返回（*{...}）
                call = format!("*({})", call);
            }
            if is_extern_c_lib && ret_is_str {
                // const char* -> ax_str
                call = format!("ax_str_from_cstr({})", call);
            }
            return call;
        }
        // 转换 / 构造外的情况
        self.convert_call(e)
    }

    fn convert_call(&mut self, e: &'a ast::EPtr) -> String {
        let args: &'a [ast::N<ast::Arg>] = match &e.k {
            Expr::Call { args, .. } => args,
            _ => &[],
        };
        let conv = self.cg.slot(&e.id).and_then(|s| s.conv);
        let arg = match args.first() {
            Some(a) => &a.k.expr,
            None => return "0".to_string(),
        };
        let src_ty = self.ty_or_int(arg.id);
        let cast_to = self
            .cg
            .slot(&e.id)
            .and_then(|s| s.cast_to.clone());
        let dst = cast_to.unwrap_or_else(|| self.ty_or_int(e.id));
        let v = match conv {
            // upcast（含 interface 胖指针组装）交给 gen 出口的 coerce_c
            Some("upcast") => self.gen(arg, Some(&dst), false, false),
            Some(_) => self.gen(arg, None, false, false),
            None => {
                let d = dst.clone();
                self.gen(arg, Some(&d), false, false)
            }
        };
        match conv {
            Some("cast") => self.cast(v, &src_ty, &dst, &e.tok),
            Some("unbox") => self.unbox(v, &dst, &e.tok),
            Some("box") => self.box_(v, &src_ty),
            Some("downcast") => {
                let tid = tag_of(&dst);
                let tv = self.declare(&src_ty, &v);
                let dstc = self.cg.ct(&dst);
                // 源是 interface（胖指针）时先取 .obj——对象指针在盒子里
                let obj = match self.cg.def_of(&match &*src_ty {
                    TyS::Named(d, _) => d.clone(),
                    _ => DefId::Id(0),
                }) {
                    Some(d) if d.kind == "interface" => format!("(({}){}.obj)", dstc, tv),
                    _ => tv.clone(),
                };
                let pa = self.pos_args(&e.tok);
                self.emit(format!(
                    "if ({} == NULL || {}->__tag != {}u) ax_trap(107, {}, \"downcast\");",
                    obj, obj, tid, pa
                ));
                format!("({}){}", dstc, obj)
            }
            Some("record2struct") => {
                let rv = self.declare(&src_ty, &v);
                let did = match &*dst {
                    TyS::Named(d, _) => d.clone(),
                    _ => DefId::Id(0),
                };
                let fields: Vec<String> = match self.cg.def_of(&did) {
                    Some(d) => d.fields.iter().map(|(n, _, _, _)| n.clone()).collect(),
                    None => Vec::new(),
                };
                let n = self.tmpn();
                let dstc = self.cg.ct(&dst);
                self.emit(format!(
                    "{} {}; memset(&{}, 0, sizeof({}));",
                    dstc, n, n, n
                ));
                for fname in &fields {
                    self.emit(format!(
                        "{}.f_{} = {}.f_{};",
                        n,
                        san(fname),
                        rv,
                        san(fname)
                    ));
                }
                n
            }
            _ => v,
        }
    }

    fn cast(&mut self, v: String, src: &Ty, dst: &Ty, tok: &'a crate::frontend::lexer::Tok) -> String {
        if src == dst {
            return v;
        }
        match (&**dst, &**src) {
            (TyS::Int(dbits, dsigned), TyS::Int(sbits, ssigned)) => {
                if (*dbits, *dsigned) == (64, true) && *sbits == 64 && *ssigned {
                    return v;
                }
                let pa = self.pos_args(tok);
                format!(
                    "ax_narrow((int64_t)({}), {}, {}, {})",
                    v,
                    dbits,
                    if *dsigned { 1 } else { 0 },
                    pa
                )
            }
            (TyS::Int(..), TyS::Float) => format!("((int64_t)({}))", v),
            (TyS::Int(..), TyS::Decimal) => {
                let pa = self.pos_args(tok);
                format!("ax_dec_to_i64({}, {})", v, pa)
            }
            (TyS::Float, TyS::Float) => v,
            (TyS::Float, TyS::Decimal) => format!("ax_dec_to_f64({})", v),
            (TyS::Float, _) => format!("((double)({}))", v),
            (TyS::Decimal, TyS::Decimal) => v,
            (TyS::Decimal, TyS::Int(..)) => format!("ax_dec_from_i64((int64_t)({}))", v),
            _ => v,
        }
    }

    fn unbox(&mut self, v: String, dst: &Ty, tok: &'a crate::frontend::lexer::Tok) -> String {
        let tv = self.declare(&t_object(), &v);
        let pa = self.pos_args(tok);
        match &**dst {
            TyS::Int(..) => format!("ax_unbox_int({}, {})", tv, pa),
            TyS::Float => format!("ax_unbox_f64({}, {})", tv, pa),
            TyS::Decimal => format!("ax_unbox_dec({}, {})", tv, pa),
            TyS::Str => format!("ax_unbox_str({}, {})", tv, pa),
            TyS::Bool => format!("ax_unbox_bool({}, {})", tv, pa),
            TyS::Nullable(inner) => {
                let inner = inner.clone();
                let tn = self.tmpn();
                let dstc = self.cg.ct(dst);
                self.emit(format!("{} {};", dstc, tn));
                self.emit(format!("{}.has = {} != NULL;", tn, tv));
                let tag = tag_of(&inner);
                let innerc = self.cg.ct(&inner);
                self.emit(format!(
                    "if ({0}.has) {0}.v = ({1})ax_unbox_ref({2}, {3}u, {4});",
                    tn, innerc, tv, tag, pa
                ));
                tn
            }
            _ => {
                let dstc = self.cg.ct(dst);
                self.emit(format!("/* unbox to {} */", dstc));
                let tag = tag_of(dst);
                format!("({})ax_unbox_ref({}, {}u, {})", dstc, tv, tag, pa)
            }
        }
    }

    fn box_(&mut self, v: String, src: &Ty) -> String {
        let tv = self.declare(src, &v);
        let srcc = self.cg.ct(src);
        format!(
            "ax_box_new({}u, &{}, (int64_t)sizeof({}))",
            tag_of(src),
            tv,
            srcc
        )
    }

    // ---------- 方法调用 ----------

    fn method_call(&mut self, e: &'a ast::EPtr) -> String {
        let (recv, args): (&'a ast::EPtr, &'a [ast::N<ast::Arg>]) = match &e.k {
            Expr::MethodCall { recv, args, .. } => (recv, args),
            _ => unreachable!(),
        };
        let sl = self.cg.slot(&e.id).cloned().unwrap_or_default();
        let mk = sl.mkind.unwrap_or("");
        if mk == "module" {
            return self.module_call(e);
        }
        if mk == "intrinsic" {
            return self.intrinsic(e);
        }
        if mk == "builtin_instance" {
            return self.builtin_instance(e);
        }
        if mk == "static" {
            let fid = sl.method.unwrap_or(0);
            let (params, fname, owner) = {
                let f = &self.cg.s.funcs[fid];
                (f.params.clone(), f.name.clone(), f.owner.unwrap_or(0))
            };
            let dd = self.cg.s.typedefs[owner as usize].clone();
            let nm = format!(
                "{}__{}",
                self.cg.class_name(&dd),
                san(&fname)
            );
            let mut out = Vec::new();
            for (i, a) in args.iter().enumerate() {
                let v = match params.get(i) {
                    Some(p) => self.gen(&a.k.expr, Some(&p.1), false, false),
                    None => self.gen(&a.k.expr, None, false, false),
                };
                out.push(v);
            }
            return format!("{}({})", nm, out.join(", "));
        }
        if mk == "super" {
            let fid = sl.method.unwrap_or(0);
            let (params, fname) = {
                let f = &self.cg.s.funcs[fid];
                (f.params.clone(), f.name.clone())
            };
            let pd = sl.method_owner.unwrap_or(0);
            let ptd = self.cg.s.typedefs[pd as usize].clone();
            let mut out = vec!["(void*)self".to_string()];
            for (i, a) in args.iter().enumerate() {
                let v = match params.get(i) {
                    Some(p) => self.gen(&a.k.expr, Some(&p.1), false, false),
                    None => self.gen(&a.k.expr, None, false, false),
                };
                out.push(v);
            }
            return format!(
                "{}__{}({})",
                self.cg.class_name(&ptd),
                san(&fname),
                out.join(", ")
            );
        }
        if mk == "parse" {
            return self.parse_call(e);
        }
        if mk == "truncate" {
            let v = self.gen(&args[0].k.expr, None, false, false);
            let bits = match &*sl.trunc_to.clone().unwrap_or_else(t_int) {
                TyS::Int(b, _) => *b,
                _ => 64,
            };
            let pa = self.pos_args(&e.tok);
            return format!("ax_narrow((int64_t)({}), {}, 1, {})", v, bits, pa);
        }
        if mk == "fromCodepoint" {
            return self.from_codepoint(e);
        }
        if mk == "strCompare" {
            // `string.compare(a, b)` 是模块级静态方法，接收者 `string` 仅是命名空间，
            // 不是 string 值，故直接取两个实参，不使用 recv。
            let a0 = self.gen(&args[0].k.expr, None, false, false);
            let a1 = self.gen(&args[1].k.expr, None, false, false);
            return format!("ax_str_cmp({}, {})", a0, a1);
        }
        if mk == "strFields" {
            // `string.fields(s)` 同理，接收者 `string` 仅是命名空间。
            let a0 = self.gen(&args[0].k.expr, None, false, false);
            return format!("ax_str_fields({})", a0);
        }
        if mk == "toUpperRune" {
            // `string.to_upper_rune(c)`：接收者 `string` 仅是命名空间，取单个 int 实参。
            let a0 = self.gen(&args[0].k.expr, None, false, false);
            return format!("ax_str_to_upper_rune({})", a0);
        }
        if mk == "toLowerRune" {
            // `string.to_lower_rune(c)`：接收者 `string` 仅是命名空间，取单个 int 实参。
            let a0 = self.gen(&args[0].k.expr, None, false, false);
            return format!("ax_str_to_lower_rune({})", a0);
        }
        if mk == "arena_alloc" {
            // O-17：`ar.Alloc(x)` —— 区域收养对象指针，返回值即 ArenaRef<T>
            //（C 层 T*）。实参在 sema 已按 move 处理。
            let recv = match &e.k {
                Expr::MethodCall { recv, .. } => recv,
                _ => unreachable!(),
            };
            let ar = self.gen(recv, None, false, false);
            let x = self.gen(&args[0].k.expr, None, false, false);
            let inner = match &*self.ty_or_int(e.id) {
                TyS::ArenaRef(i) => self.cg.ct(i),
                _ => "void*".to_string(),
            };
            return format!("(({})ax_arena_adopt({}, {}))", inner, ar, x);
        }
        // virtual
        let fid = match sl.method {
            Some(f) => f,
            None => return "0".to_string(),
        };
        let (params, fname) = {
            let f = &self.cg.s.funcs[fid];
            (f.params.clone(), f.name.clone())
        };
        let recv_ty = self.ty_or_int(recv.id);
        let rv = self.gen(recv, None, false, false);
        let tv = self.declare(&recv_ty, &rv);
        let mut out = vec![format!("(void*){}", tv)];
        for (i, a) in args.iter().enumerate() {
            let v = match params.get(i) {
                Some(p) => self.gen(&a.k.expr, Some(&p.1), false, false),
                None => self.gen(&a.k.expr, None, false, false),
            };
            out.push(v);
        }
        let owner = sl.method_owner.unwrap_or(0);
        let od = self.cg.s.typedefs[owner as usize].clone();
        if od.kind == "interface" {
            // 方法落在 interface 上（接收者是 interface，或类接收者调用
            // interface 独有方法——类虚表里没有这个条目）。
            let recv_def = match self.ty_opt(recv.id).map(|t| (*t).clone()) {
                Some(TyS::Named(DefId::Id(c), _)) => Some(c),
                _ => None,
            };
            match recv_def {
                Some(c) if self.cg.s.typedefs[c as usize].kind == "class" => {
                    // 类接收者：直接取该类的 shim 地址（override 已按类静态落定）
                    let cd = self.cg.s.typedefs[c as usize].clone();
                    let iname = format!("Iface_{}", san(&od.name));
                    let vt = format!("{}__{}__VT_", self.cg.class_name(&cd), san(&od.name));
                    return format!(
                        "(({}_VT*)&{})->{}({})",
                        iname,
                        vt,
                        san(&fname),
                        out.join(", ")
                    );
                }
                Some(i) => {
                    // interface 接收者（胖指针）：按**接收者的静态 interface** 取
                    // 虚表布局（方法可能来自其 extends 链，owner 与接收者不是同
                    // 一个 interface，布局不能混用），经 tv.vt 运行时派发
                    let rd = &self.cg.s.typedefs[i as usize];
                    let iname = format!("Iface_{}", san(&rd.name));
                    out[0] = format!("(void*){}.obj", tv);
                    return format!(
                        "(({}_VT*){}.vt)->{}({})",
                        iname,
                        tv,
                        san(&fname),
                        out.join(", ")
                    );
                }
                _ => return "0".to_string(),
            }
        }
        let vt_ty = format!("{}_VT", self.cg.class_name(&od));
        format!(
            "((({}*){}->vt)->{})({})",
            vt_ty,
            tv,
            san(&fname),
            out.join(", ")
        )
    }

    /// 表达式是否是一条以变量/this 为根的左值链（可安全取地址）。
    /// 只认不会产生临时对象的形态：临时对象的字段不是左值，C 里取址非法。
    fn is_lvalue(e: &'a ast::EPtr) -> bool {
        match &e.k {
            ast::Expr::Path { .. } | ast::Expr::This => true,
            ast::Expr::FieldAccess { recv, .. } => Self::is_lvalue(recv),
            _ => false,
        }
    }

    fn builtin_instance(&mut self, e: &'a ast::EPtr) -> String {
        let (recv, args): (&'a ast::EPtr, &'a [ast::N<ast::Arg>]) = match &e.k {
            Expr::MethodCall { recv, args, .. } => (recv, args),
            _ => unreachable!(),
        };
        let rv = self.gen(recv, None, false, false);
        let recv_ty = self.ty_or_int(recv.id);
        let tv = self.declare(&recv_ty, &rv);
        let b = self
            .cg
            .slot(&e.id)
            .and_then(|s| s.bname.clone())
            .unwrap_or_default();
        match b.as_str() {
            "row.empty" => format!("ax_row_empty({})", tv),
            "row.getString" => {
                let a = self.gen(&args[0].k.expr, None, false, false);
                format!("ax_row_get({}, {})", tv, a)
            }
            "row.getInt" => {
                let a = self.gen(&args[0].k.expr, None, false, false);
                format!("str_to_i64(ax_row_get({}, {}))", tv, a)
            }
            "row.getDecimal" => {
                let a = self.gen(&args[0].k.expr, None, false, false);
                format!(
                    "ax_dec_parse_or(ax_row_get({}, {}), ax_dec_from_i64(0))",
                    tv, a
                )
            }
            "file.close" => {
                self.emit(format!("ax_file_close({});", tv));
                "0".to_string()
            }
            "file.read" => {
                let tyc = self.cg.ct(&self.ty_or_int(e.id));
                format!(
                    "({{ {tyc} r_; ax_str o_; if (ax_file_read_all({tv}, &o_) == 0) {{ r_.tag = 1; r_.u.ok = o_; }} else {{ r_.tag = 0; r_.u.err = ax_err_new(4, AX_S(\"read failed\")); }} r_; }})",
                    tyc = tyc,
                    tv = tv
                )
            }
            "file.readLine" => {
                let tyc = self.cg.ct(&self.ty_or_int(e.id));
                format!(
                    "({{ {tyc} r_; ax_str o_; if (ax_file_read_line({tv}, &o_) == 0) {{ r_.has = 1; r_.v = o_; }} else {{ r_.has = 0; r_.v = ax_str_empty(); }} r_; }})",
                    tyc = tyc,
                    tv = tv
                )
            }
            "file.write" | "file.writeLine" => {
                let tyc = self.cg.ct(&self.ty_or_int(e.id));
                let a = self.gen(&args[0].k.expr, Some(&t_string()), false, false);
                let call = if b == "file.write" {
                    format!("ax_file_write({}, {})", tv, a)
                } else {
                    format!("ax_file_write_line({}, {})", tv, a)
                };
                format!(
                    "({{ int64_t n_ = {call}; {tyc} r_; if (n_ >= 0) {{ r_.tag = 1; r_.u.ok = n_; }} else {{ r_.tag = 0; r_.u.err = ax_err_new(4, AX_S(\"write failed\")); }} r_; }})",
                    call = call,
                    tyc = tyc
                )
            }
            /* ---------------- sync 模块方法（对标 Go sync，P2） ----------------
             * recv 在 C 层为 ax_mutex/ax_rwmutex/ax_waitgroup/ax_once 指针，直接传入。
             * 通过 ax_std_* 标准库 C 接口调用（由 axc/stdlib/sync.ax 预转译生成）。 */
            "mutex.lock" => { self.emit(format!("ax_std_mutex_lock(&{});", tv)); "0".to_string() }
            "mutex.unlock" => { self.emit(format!("ax_std_mutex_unlock(&{});", tv)); "0".to_string() }
            "rwmutex.lock" => { self.emit(format!("ax_std_rwmutex_lock(&{});", tv)); "0".to_string() }
            "rwmutex.unlock" => { self.emit(format!("ax_std_rwmutex_unlock(&{});", tv)); "0".to_string() }
            "rwmutex.rlock" => { self.emit(format!("ax_std_rwmutex_rlock(&{});", tv)); "0".to_string() }
            "rwmutex.runlock" => { self.emit(format!("ax_std_rwmutex_runlock(&{});", tv)); "0".to_string() }
            "waitgroup.add" => format!("ax_std_waitgroup_add(&{}, {})", tv, self.gen(&args[0].k.expr, Some(&t_int()), false, false)),
            "waitgroup.done" => { self.emit(format!("ax_std_waitgroup_done(&{});", tv)); "0".to_string() }
            "waitgroup.wait" => { self.emit(format!("ax_std_waitgroup_wait(&{});", tv)); "0".to_string() }
            "once.do" => { self.emit(format!("ax_std_once_do(&{});", tv)); "0".to_string() }
            _ => "0".to_string(),
        }
    }

    fn module_call(&mut self, e: &'a ast::EPtr) -> String {
        let (recv, name, args): (&'a ast::EPtr, &'a String, &'a [ast::N<ast::Arg>]) = match &e.k
        {
            Expr::MethodCall { recv, name, args } => (recv, name, args),
            _ => unreachable!(),
        };
        // 用户模块调用（文稿 9.2）：sema 已把符号解析为 func/ctor，
        // 这里按 slot 标记生成——顶层函数直接调用，类型走构造
        {
            let sl = self.cg.slot(&e.id).cloned().unwrap_or_default();
            if sl.ckind.as_deref() == Some("func") {
                if let Some(f) = sl.func {
                    let params = self.cg.s.funcs[f].params.clone();
                    let nm = self.cg.fn_name_idx(f);
                    let mut a: Vec<String> = Vec::new();
                    for (i, x) in args.iter().enumerate() {
                        let v = match params.get(i) {
                            Some(p) => self.gen(&x.k.expr, Some(&p.1), false, false),
                            None => self.gen(&x.k.expr, None, false, false),
                        };
                        a.push(v);
                    }
                    return format!("{}({})", nm, a.join(", "));
                }
            }
            if sl.ckind.as_deref() == Some("ctor") {
                let ty = self.ty_or_int(e.id);
                let d: u32 = match &*ty {
                    TyS::Named(DefId::Id(d), _) => *d,
                    TyS::Enum(d) => *d,
                    _ => 0,
                };
                let td = self.cg.s.typedefs[d as usize].clone();
                if td.kind == "struct" {
                    return self.struct_literal(&ty, &td, args);
                }
                let params: Vec<(String, Ty, Option<String>)> = match td.ctors.get("constructor").copied().or_else(|| td.ctors.values().next().copied()) {
                    Some(c) => self.cg.s.funcs[c].params.clone(),
                    None => Vec::new(),
                };
                let nm = match td.ctors.get("constructor").copied().or_else(|| td.ctors.values().next().copied()) {
                    Some(fid) => {
                        let fname = &self.cg.s.funcs[fid].name;
                        format!("{}__new_{}", self.cg.class_name(&td), fname)
                    }
                    None => format!("{}__new", self.cg.class_name(&td)),
                };
                let mut a: Vec<String> = Vec::new();
                for (i, x) in args.iter().enumerate() {
                    let v = match params.get(i) {
                        Some(p) => self.gen(&x.k.expr, Some(&p.1), false, false),
                        None => self.gen(&x.k.expr, None, false, false),
                    };
                    a.push(v);
                }
                return format!("{}({})", nm, a.join(", "));
            }
        }
        let module = match &recv.k {
            Expr::Path { segs, .. } => segs.join("."),
            _ => String::new(),
        };
        // `std.time.xxx()` / `std.string.xxx()` 等标准库多段名，需去掉 "std." 前缀
        // 后再匹配 codegen 内建模块签名（key 为 ("time", ...)、("string", ...) 等）。
        // 若 recv 是嵌套 MethodCall（如 (std.time).parse()），module 为空，则从 sema slot 的 bname 获取。
        let module = if module.is_empty() {
            self.cg
                .slot(&e.id)
                .and_then(|sl| sl.bname.clone())
                .unwrap_or_default()
        } else {
            module.strip_prefix("std.").unwrap_or(&module).to_string()
        };
        // fmt.Printf 在 sema 已被解析并核对，这里直接展开成插值式拼接——
        // 要赶在下面的通用实参 gen 循环之前，避免格式串字面量被多生成一份未使用的临时串
        if module == "fmt" && name == "printf" {
            return self.fmt_printf(e);
        }
        // fmt.Print / fmt.Println：sema 走的是 print 同款（多实参、空格分隔）
        if module == "fmt" && (name == "print" || name == "println") {
            self.emit_print(args, name == "println");
            return "0".to_string();
        }
        // fmt.Sprint：与 Print 同款实参拼串，但返回 ax_str 而非写 stdout
        if module == "fmt" && name == "sprint" {
            return self.fmt_sprint(args);
        }
        // fmt.Sprintf：与 Printf 同款格式解析，但返回 ax_str 而非写 stdout
        if module == "fmt" && name == "sprintf" {
            return self.fmt_sprintf(e);
        }
        // fmt.Fprintf：与 fmt_sprintf 同款格式解析，但将结果写入 ax_file*（Writer），返回写入字节数
        if module == "fmt" && name == "fprintf" {
            return self.fmt_fprintf(e);
        }
        // fmt.Errorf：与 fmt_sprintf 同款格式解析，但返回 ax_err（error），对标 Go fmt.Errorf / C 错误构造
        if module == "fmt" && name == "errorf" {
            return self.fmt_errorf(e);
        }
        // fmt.Scanf：从 stdin 按格式读取，返回读取的项数，对标 Go fmt.Scanf / C scanf
        if module == "fmt" && name == "scanf" {
            return self.fmt_scanf(e);
        }
        // log.Print / log.Fatal：对标 Go log.Print / log.Fatal，多实参，空格分隔，
        // Fatal 额外调用 os.Exit(1)。不加日期时间前缀（axlang 极简风格，用户可自行加前缀）。
        if module == "log" && (name == "print" || name == "fatal") {
            self.emit_print(args, false);
            if name == "fatal" {
                self.emit("ax_write_stdout(AX_S(\"\\n\"));".to_string());
                self.emit("ax_os_exit(1);".to_string());
            }
            return "0".to_string();
        }
        // log.Printf：对标 Go log.Printf，复用 fmt.Printf 的格式化（编译期核对占位符）。
        if module == "log" && name == "printf" {
            return self.fmt_printf(e);
        }
        let mut a: Vec<String> = Vec::new();
        // 实参按模块签名声明的形参类型转换（map → object 要装箱，等等）——
        // 与用户函数调用走同一套 coerce 规则
        let ptypes = crate::sema::module_sig(&module, &name).map(|(ps, _)| ps);
        for (i, x) in args.iter().enumerate() {
            let pt = ptypes.as_ref().and_then(|ps| ps.get(i).cloned().flatten());
            let v = match pt {
                Some(pt) => self.gen(&x.k.expr, Some(&pt), false, false),
                None => self.gen(&x.k.expr, None, false, false),
            };
            a.push(v);
        }
        match (module.as_str(), name.as_str()) {
            // 返回值走 axrt 的 out 参数形态：read_line 给 string?（EOF → has=0），
            // read_file 给 result<string>（失败 → fail）。原先按单值/无参生成，
            // 与 axrt.h 的签名不符，C 编译直接失败（此前没有用例覆盖）
            ("io", "readLine") => {
                let tyc = self.cg.ct(&self.ty_or_int(e.id));
                format!(
                    "({{ {tyc} r_; ax_str o_; if (ax_read_line(&o_) == 0) {{ r_.has = 1; r_.v = o_; }} else {{ r_.has = 0; r_.v = ax_str_empty(); }} r_; }})",
                    tyc = tyc
                )
            }
            ("io", "readFile") => {
                let tyc = self.cg.ct(&self.ty_or_int(e.id));
                format!(
                    "({{ {tyc} r_; ax_str o_; if (ax_read_file({p}, &o_) == 0) {{ r_.tag = 1; r_.u.ok = o_; }} else {{ r_.tag = 0; r_.u.err = ax_err_new(4, AX_S(\"read failed\")); }} r_; }})",
                    tyc = tyc,
                    p = a[0]
                )
            }
            ("io", "write") => {
                self.emit(format!("ax_write_stdout({});", a[0]));
                "0".to_string()
            }
            ("io", "writeLine") => {
                self.emit(format!("ax_write_stdout({});", a[0]));
                self.emit("ax_write_stdout(AX_S(\"\\n\"));".to_string());
                "0".to_string()
            }
            ("math", "equals") => {
                format!("(fabs(({}) - ({})) <= ({}))", a[0], a[1], a[2])
            }
            ("math", "compare") => format!(
                "((({0}) < ({1})) ? -1 : ((({0}) > ({1})) ? 1 : 0))",
                a[0], a[1]
            ),
            ("math", "abs") => format!("(fabs({}))", a[0]),
            ("math", "max") => format!("(fmax({}, {}))", a[0], a[1]),
            ("math", "min") => format!("(fmin({}, {}))", a[0], a[1]),
            ("math", "sqrt") => format!("(sqrt({}))", a[0]),
            ("math", "pow") => format!("(pow({}, {}))", a[0], a[1]),
            ("math", "exp") => format!("(exp({}))", a[0]),
            ("math", "log") => format!("(log({}))", a[0]),
            ("math", "sin") => format!("(sin({}))", a[0]),
            ("math", "cos") => format!("(cos({}))", a[0]),
            ("math", "tan") => format!("(tan({}))", a[0]),
            ("math", "ceil") => format!("(ceil({}))", a[0]),
            ("math", "floor") => format!("(floor({}))", a[0]),
            ("math", "round") => format!("(round({}))", a[0]),
            ("math", "sign") => format!("ax_math_sign({})", a[0]),
            ("math", "mod") => format!("ax_math_mod({}, {})", a[0], a[1]),
            ("math", "exp2") => format!("(exp2({}))", a[0]),
            ("math", "log2") => format!("(log2({}))", a[0]),
            ("math", "log10") => format!("(log10({}))", a[0]),
            ("math", "log1p") => format!("(log1p({}))", a[0]),
            ("math", "isNaN") => format!("ax_math_isnan({})", a[0]),
            ("math", "isInf") => format!("ax_math_isinf({}, {})", a[0], a[1]),
            ("math", "sinh") => format!("(sinh({}))", a[0]),
            ("math", "cosh") => format!("(cosh({}))", a[0]),
            ("math", "tanh") => format!("(tanh({}))", a[0]),
            ("math", "asin") => format!("(asin({}))", a[0]),
            ("math", "acos") => format!("(acos({}))", a[0]),
            ("math", "atan") => format!("(atan({}))", a[0]),
            ("math", "atan2") => format!("(atan2({}, {}))", a[0], a[1]),
            ("math", "hypot") => format!("(hypot({}, {}))", a[0], a[1]),
            ("math", "cbrt") => format!("(cbrt({}))", a[0]),
            ("math", "trunc") => format!("(trunc({}))", a[0]),
            ("db", "query") => {
                let arg = if a.len() > 1 {
                    a[1].clone()
                } else {
                    "AX_S(\"\")".to_string()
                };
                format!("ax_db_query_result({}, {})", a[0], arg)
            }
            ("json", "parse") => format!("ax_json_parse_result({})", a[0]),
            ("json", "stringify") => format!("ax_json_stringify({})", a[0]),
            ("json", "marshalIndent") => format!("ax_json_marshal_indent({}, {}, {})", a[0], a[1], a[2]),
            ("json", "valid") => format!("ax_json_valid({})", a[0]),
            /* file 模块（单段名，向后兼容；std.file 现已改为 axc/std/file.ax） */
            ("file", "open") => format!("ax_file_open_result({}, 0)", a[0]),
            ("file", "create") => format!("ax_file_open_result({}, 1)", a[0]),
            ("file", "append") => format!("ax_file_open_result({}, 2)", a[0]),
            ("file", "remove") => format!("ax_file_remove({})", a[0]),
            ("file", "rename") => format!("ax_file_rename({}, {})", a[0], a[1]),
            ("file", "exists") => format!("ax_file_exists({})", a[0]),
            ("file", "mkdir") => format!("ax_dir_create({})", a[0]),
            ("file", "isDir") => format!("ax_file_is_dir({})", a[0]),
            ("file", "isFile") => format!("ax_file_is_file({})", a[0]),
            ("file", "size") => format!("ax_file_size({})", a[0]),
            ("file", "modTime") => format!("ax_file_modtime({})", a[0]),
            ("file", "readAll") => {
                let tyc = self.cg.ct(&self.ty_or_int(e.id));
                format!(
                    "({{ {tyc} r_; ax_str o_; if (ax_read_file({p}, &o_) == 0) {{ r_.tag = 1; r_.u.ok = o_; }} else {{ r_.tag = 0; r_.u.err = ax_err_new(4, AX_S(\"read failed\")); }} r_; }})",
                    tyc = tyc,
                    p = a[0]
                )
            }
            ("file", "writeAll") => format!("ax_file_write_all({}, {})", a[0], a[1]),
            ("file", "chmod") => format!("ax_file_chmod({}, {})", a[0], a[1]),
            ("file", "chtimes") => format!("ax_file_chtimes({}, {}, {})", a[0], a[1], a[2]),
            ("file", "tempFile") => format!("ax_file_tempfile({}, {})", a[0], a[1]),
            ("time", "nowMs") => "ax_now_ms()".to_string(),
            ("time", "now") => "ax_now_ms()".to_string(),
            ("time", "fromUnixMs") => a[0].clone(),
            ("time", "sleep") => {
                self.emit(format!("ax_time_sleep({});", a[0]));
                "0".to_string()
            }
            ("time", "unix") => format!("ax_time_from_unix({}, {})", a[0], a[1]),
            ("time", "unixMilli") => format!("ax_time_from_unix_ms({})", a[0]),
            ("time", "unixMicro") => format!("ax_time_from_unix_us({})", a[0]),
            ("time", "unixNano") => format!("ax_time_from_unix_ns({})", a[0]),
            ("time", "since") => format!("ax_time_since({})", a[0]),
            ("time", "date") => format!("ax_time_date({}, {}, {}, {}, {}, {}, {})",
                                        a[0], a[1], a[2], a[3], a[4], a[5], a[6]),
            ("time", "until") => format!("ax_time_until({})", a[0]),
            ("time", "parse") => format!("ax_time_parse_result({}, {})", a[0], a[1]),
            /* ---------------- os 模块 ---------------- */
            ("os", "getenv") => format!("ax_os_getenv({})", a[0]),
            ("os", "exit") => {
                self.emit(format!("ax_os_exit({});", a[0]));
                "0".to_string()
            }
            ("os", "args") => "ax_os_args()".to_string(),
            ("os", "getwd") => "ax_os_getwd()".to_string(),
            ("os", "chdir") => format!("ax_os_chdir({})", a[0]),
            ("os", "getpid") => "ax_os_getpid()".to_string(),
            ("os", "getppid") => "ax_os_getppid()".to_string(),
            ("os", "hostname") => "ax_os_hostname()".to_string(),
            ("os", "environ") => "ax_os_environ()".to_string(),
            ("os", "setenv") => format!("ax_os_setenv({}, {})", a[0], a[1]),
            ("os", "unsetenv") => format!("ax_os_unsetenv({})", a[0]),
            ("os", "userHomeDir") => "ax_os_user_home_dir()".to_string(),
            /* atomic 模块（单段名，向后兼容；std.atomic 现已改为 axc/std/atomic.ax） */
            ("atomic", "addInt") => format!("ax_std_atomic_add_int(&{}, {})", a[0], a[1]),
            ("atomic", "loadInt") => format!("ax_std_atomic_load_int(&{})", a[0]),
            ("atomic", "storeInt") => {
                self.emit(format!("ax_std_atomic_store_int(&{}, {});", a[0], a[1]));
                "0".to_string()
            }
            ("atomic", "compareAndSwapInt") => {
                format!("ax_std_atomic_compare_and_swap_int(&{}, {}, {})", a[0], a[1], a[2])
            }
            /* ---------------- sort 模块 ---------------- */
            ("sort", "ints") => {
                let arr = self.declare(&t_array(t_int(), None), &a[0]);
                format!("ax_sort_int((int64_t*){}.data, {}.len)", arr, arr)
            }
            ("sort", "strings") => {
                let arr = self.declare(&t_array(t_string(), None), &a[0]);
                format!("ax_sort_str((ax_str*){}.data, {}.len)", arr, arr)
            }
            ("sort", "float64s") => {
                let arr = self.declare(&t_array(t_float(), None), &a[0]);
                format!("ax_sort_float((double*){}.data, {}.len)", arr, arr)
            }
            ("sort", "intsAreSorted") => {
                let arr = self.declare(&t_array(t_int(), None), &a[0]);
                format!("ax_sort_ints_are_sorted((int64_t*){}.data, {}.len)", arr, arr)
            }
            ("sort", "stringsAreSorted") => {
                let arr = self.declare(&t_array(t_string(), None), &a[0]);
                format!("ax_sort_strings_are_sorted((ax_str*){}.data, {}.len)", arr, arr)
            }
            ("sort", "searchInts") => {
                let arr = self.declare(&t_array(t_int(), None), &a[0]);
                format!("ax_sort_search_ints((int64_t*){}.data, {}.len, {})", arr, arr, a[1])
            }
            ("sort", "searchStrings") => {
                let arr = self.declare(&t_array(t_string(), None), &a[0]);
                format!("ax_sort_search_strings((ax_str*){}.data, {}.len, {})", arr, arr, a[1])
            }
            /* sort.slice / sort.slice_stable：按自定义比较函数对任意类型数组排序。
               底层复用 qsort + 文件级比较函数适配器（与 arr.sort(cmp) 相同）。 */
            ("sort", "slice") | ("sort", "sliceStable") => {
                let arr_ty = self.ty_or_int(args[0].k.expr.id);
                let et = match &*arr_ty {
                    TyS::Array(et, _) => et.clone(),
                    _ => t_object(),
                };
                let etc = self.cg.ct(&et);
                let tv = self.declare(&arr_ty, &a[0]);
                let uid = self.tmpn();
                let ctx = format!("axsort_ctx_{}", uid);
                let cmpfn = format!("axsort_cmp_{}", uid);
                let fnty_ty = t_fn(vec![et.clone(), et.clone()], t_int());
                self.cg.need(&fnty_ty);
                let fnty = format!("axfn_{}", self.cg.cn(&fnty_ty));
                self.cg
                    .fn_decls
                    .push(format!("static int {}(const void*, const void*);", cmpfn));
                self.cg
                    .fn_decls
                    .push(format!("static {} {};", fnty, ctx));
                self.cg.extra_fns.push(format!(
                    "static int {cmpfn}(const void* a_, const void* b_) {{\n  {etc} va_; {etc} vb_;\n  memcpy(&va_, a_, sizeof({etc}));\n  memcpy(&vb_, b_, sizeof({etc}));\n  return (int){ctx}(va_, vb_);\n}}",
                    cmpfn = cmpfn,
                    ctx = ctx,
                    etc = etc
                ));
                self.emit(format!("{ctx} = {cmp};", ctx = ctx, cmp = a[1]));
                self.emit(format!(
                    "qsort({0}.data, (size_t){0}.len, (size_t)sizeof({1}), {2});",
                    tv, etc, cmpfn
                ));
                "0".to_string()
            }
            ("reflect", "typeOf") => {
                // 编译期常量：直接生成字符串字面量，实参根本不出现在生成代码里
                let name = self
                    .cg
                    .slot(&e.id)
                    .and_then(|s| s.bname.clone())
                    .unwrap_or_default();
                format!("AX_S({})", cstr(&name))
            }
            ("reflect", "numField") => {
                // 编译期常量：直接生成整数字面量
                let n = self
                    .cg
                    .slot(&e.id)
                    .and_then(|s| s.const_value.clone())
                    .and_then(|cv| match cv {
                        ConstVal::Int(n) => Some(n),
                        _ => None,
                    })
                    .unwrap_or(0);
                format!("{}", n)
            }
            ("errors", "new") => format!("ax_err_new(1, {})", a[0]),
            // errors.Is(err, target) -> bool：对标 Go errors.Is，比较 code 和 msg 是否相等
            ("errors", "is") => format!("ax_err_is({}, {})", a[0], a[1]),
            // errors.As(err, target) -> bool：对标 Go errors.As，将 err 复制到 target（target 必须是 error 左值）
            ("errors", "as") => {
                let tgt = if Self::is_lvalue(&args[1].k.expr) {
                    format!("&{}", a[1])
                } else {
                    let tmp = self.declare(&t_error(), &a[1]);
                    format!("&{}", tmp)
                };
                format!("ax_err_as({}, {})", a[0], tgt)
            }
            /* ---------------- strconv 模块 ---------------- */
            ("strconv", "atoi") => {
                format!("({{ int64_t v_; ax_parse_i64({}, &v_) ? v_ : 0; }})", a[0])
            }
            ("strconv", "itoa") => format!("ax_str_of_int({})", a[0]),
            ("strconv", "parseFloat") => {
                format!("({{ double v_; ax_parse_f64({}, &v_) ? v_ : 0.0; }})", a[0])
            }
            ("strconv", "formatFloat") => format!("ax_str_of_f64({})", a[0]),
            ("strconv", "parseInt") => format!("ax_parse_int_base({}, {})", a[0], a[1]),
            ("strconv", "formatInt") => format!("ax_str_of_int_base({}, {})", a[0], a[1]),
            ("strconv", "parseBool") => format!("ax_parse_bool({})", a[0]),
            ("strconv", "formatBool") => format!("ax_str_of_bool({})", a[0]),
            ("strconv", "parseUint") => format!("ax_parse_uint_base({}, {})", a[0], a[1]),
            ("strconv", "formatUint") => format!("ax_str_of_uint_base({}, {})", a[0], a[1]),
            ("strconv", "canBackquote") => format!("ax_str_can_backquote({})", a[0]),
            ("strconv", "quote") => format!("ax_str_quote({})", a[0]),
            ("strconv", "unquote") => format!("ax_str_unquote({})", a[0]),
            /* string 模块（单段名，向后兼容；std.string 现已改为 axc/std/string.ax） */
            ("string", "compare") => format!("ax_str_cmp({}, {})", a[0], a[1]),
            ("string", "fields") => format!("ax_str_fields({})", a[0]),
            ("string", "to_upper_rune") => format!("ax_str_to_upper_rune({})", a[0]),
            ("string", "to_lower_rune") => format!("ax_str_to_lower_rune({})", a[0]),
            /* ---------------- filepath 模块 ---------------- */
            ("filepath", "join") => {
                let arr = &a[0];
                format!(
                    "ax_filepath_join({arr})",
                    arr = arr
                )
            }
            ("filepath", "dir") => format!("ax_filepath_dir({})", a[0]),
            ("filepath", "base") => format!("ax_filepath_base({})", a[0]),
            ("filepath", "ext") => format!("ax_filepath_ext({})", a[0]),
            ("filepath", "split") => format!("ax_filepath_split({})", a[0]),
            ("filepath", "clean") => format!("ax_filepath_clean({})", a[0]),
            ("filepath", "abs") => format!("ax_filepath_abs({})", a[0]),
            ("filepath", "isAbs") => format!("ax_filepath_is_abs({})", a[0]),
            ("filepath", "evalSymlinks") => format!("ax_filepath_eval_symlinks({})", a[0]),
            ("filepath", "glob") => format!("ax_filepath_glob({})", a[0]),
            ("filepath", "walk") => format!("ax_filepath_walk({})", a[0]),
            /* ---------------- maps 模块（对标 Go maps，K/V 由实参 Map 类型推导） ---------------- */
            // maps.Keys(m) -> K[]：返回键数组（按插入顺序）
            ("maps", "keys") => {
                let map_ty = self.ty_or_int(args[0].k.expr.id);
                let tv = self.declare(&map_ty, &a[0]);
                let n = self.tmpn();
                self.emit(format!("ax_arr {} = ax_map_keys(&{});", n, tv));
                n
            }
            // maps.Values(m) -> V[]：返回值数组（按插入顺序）
            ("maps", "values") => {
                let map_ty = self.ty_or_int(args[0].k.expr.id);
                let _vt = match &*map_ty {
                    TyS::Map(_, v) => v.clone(),
                    _ => t_string(),
                };
                let tv = self.declare(&map_ty, &a[0]);
                let n = self.tmpn();
                self.emit(format!("ax_arr {} = ax_map_values(&{});", n, tv));
                n
            }
            // maps.Clone(m) -> Map：返回独立副本
            ("maps", "clone") => {
                let map_ty = self.ty_or_int(args[0].k.expr.id);
                let tv = self.declare(&map_ty, &a[0]);
                let n = self.tmpn();
                self.emit(format!("ax_map {} = ax_map_clone(&{});", n, tv));
                n
            }
            // maps.Copy(dst, src) -> void：将 src 复制到 dst（dst 必须是左值才会生效）
            ("maps", "copy") => {
                let dst_ty = self.ty_or_int(args[0].k.expr.id);
                let src_ty = self.ty_or_int(args[1].k.expr.id);
                let dstv = if Self::is_lvalue(&args[0].k.expr) {
                    a[0].clone()
                } else {
                    self.declare(&dst_ty, &a[0])
                };
                let srcv = self.declare(&src_ty, &a[1]);
                self.emit(format!("ax_map_copy_into(&{}, &{});", dstv, srcv));
                "0".to_string()
            }
            // maps.Delete(m, key: K) -> void：从 m 中删除 key（m 必须是左值才会生效）
            ("maps", "delete") => {
                let map_ty = self.ty_or_int(args[0].k.expr.id);
                let kt = match &*map_ty {
                    TyS::Map(k, _) => k.clone(),
                    _ => t_int(),
                };
                let tv = if Self::is_lvalue(&args[0].k.expr) {
                    a[0].clone()
                } else {
                    self.declare(&map_ty, &a[0])
                };
                let kv = self.declare(&kt, &a[1]);
                self.emit(format!("ax_map_remove(&{}, &{});", tv, kv));
                "0".to_string()
            }
            // maps.DeleteFunc(m, f: func(K) bool) -> void：
            // 删除所有使 f(key) 为 true 的键值对。K 由第一个实参推导。
            ("maps", "deleteFunc") => {
                let map_ty = self.ty_or_int(args[0].k.expr.id);
                let kt = match &*map_ty {
                    TyS::Map(k, _) => k.clone(),
                    _ => t_int(),
                };
                let etc = self.cg.ct(&kt);
                let tv = if Self::is_lvalue(&args[0].k.expr) {
                    a[0].clone()
                } else {
                    self.declare(&map_ty, &a[0])
                };
                let uid = self.tmpn();
                let ctx = format!("axmap_delete_ctx_{}", uid);
                let cb = format!("axmap_delete_cb_{}", uid);
                let fnty = t_fn(vec![kt.clone()], t_bool());
                self.cg.need(&fnty);
                let fnty_name = format!("axfn_{}", self.cg.cn(&fnty));
                self.cg
                    .fn_decls
                    .push(format!("static int {}(const void*, void*);", cb));
                self.cg
                    .fn_decls
                    .push(format!("static {} {};", fnty_name, ctx));
                self.cg.extra_fns.push(format!(
                    "static int {cb}(const void* k_, void* ctx_) {{\n  {etc} k;\n  memcpy(&k, k_, sizeof({etc}));\n  return (int){ctx}(k);\n}}",
                    cb = cb, etc = etc, ctx = ctx
                ));
                self.emit(format!("{ctx} = {f};", ctx = ctx, f = a[1]));
                self.emit(format!(
                    "ax_map_delete_func(&{}, {}, NULL);",
                    tv, cb
                ));
                "0".to_string()
            }
            // maps.Equal(m1, m2) -> bool：判断两个 map 是否相等
            ("maps", "equal") => {
                let map1_ty = self.ty_or_int(args[0].k.expr.id);
                let map2_ty = self.ty_or_int(args[1].k.expr.id);
                let tv1 = if Self::is_lvalue(&args[0].k.expr) {
                    a[0].clone()
                } else {
                    self.declare(&map1_ty, &a[0])
                };
                let tv2 = if Self::is_lvalue(&args[1].k.expr) {
                    a[1].clone()
                } else {
                    self.declare(&map2_ty, &a[1])
                };
                format!("ax_map_equal(&{}, &{})", tv1, tv2)
            }
            /* ---------------- unique 模块（对标 Go unique，Handle[T] 装箱/解箱） ---------------- */
            // unique.Make(v: T) -> Handle[T]：将值装箱为 ax_box*，句柄即 box 指针（void*）
            ("unique", "make") => {
                let v_ty = self.ty_or_int(args[0].k.expr.id);
                let tv = self.declare(&v_ty, &a[0]);
                let tag = tag_of(&v_ty);
                let vtc = self.cg.ct(&v_ty);
                format!("ax_box_new({}u, &{}, (int64_t)sizeof({}))", tag, tv, vtc)
            }
            // unique.Value(h: Handle[T]) -> T：从句柄（ax_box*）解出原值
            ("unique", "value") => {
                let h_ty = self.ty_or_int(args[0].k.expr.id);
                let inner_ty = match &*h_ty {
                    TyS::Named(DefId::Name("Handle"), args) if args.len() == 1 => args[0].clone(),
                    _ => t_int(),
                };
                let tv = self.declare(&t_object(), &a[0]);
                let pa = self.pos_args(&e.tok);
                match &*inner_ty {
                    TyS::Int(..) => format!("ax_unbox_int({}, {})", tv, pa),
                    TyS::Float => format!("ax_unbox_f64({}, {})", tv, pa),
                    TyS::Decimal => format!("ax_unbox_dec({}, {})", tv, pa),
                    TyS::Str => format!("ax_unbox_str({}, {})", tv, pa),
                    TyS::Bool => format!("ax_unbox_bool({}, {})", tv, pa),
                    _ => {
                        let tag = tag_of(&inner_ty);
                        let innerc = self.cg.ct(&inner_ty);
                        format!("({})ax_unbox_ref({}, {}u, {})", innerc, tv, tag, pa)
                    }
                }
            }
            _ => "0".to_string(),
        }
    }

    /// `print` / `println` / `fmt.Print` / `fmt.Println` 的公共生成（0.0.27 起支持多实参）：
    /// 统一走字符串缓冲，实参之间以一个空格分隔；可插值的实参（sema 标在实参节点上的
    /// interp_arg）按 `"{x}"` 拼串，string 实参直接 push。0 个实参时输出空串（`println()` 打印空行）。
    fn emit_print(&mut self, args: &'a [ast::N<ast::Arg>], newline: bool) {
        let sb = self.tmpn();
        self.emit(format!("ax_sb {}; ax_sb_init(&{});", sb, sb));
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                self.emit(format!("ax_sb_push(&{}, AX_S(\" \"));", sb));
            }
            let interp = self
                .cg
                .slot(&arg.k.expr.id)
                .map(|s| s.interp_arg)
                .unwrap_or(false);
            if interp {
                let aty = self.ty_or_int(arg.k.expr.id);
                let av = self.gen(&arg.k.expr, None, false, false);
                self.push_interp(&sb, &av, &aty, None, &arg.k.expr);
            } else {
                let aty = self.ty_or_int(arg.k.expr.id);
                let av = self.gen(&arg.k.expr, None, false, false);
                self.push_one_to_sb(&sb, &av, &aty);
            }
        }
        let t = self.tmpn();
        self.emit(format!("ax_str {} = ax_sb_finish(&{});", t, sb));
        self.emit(format!("ax_write_stdout({});", t));
        if newline {
            self.emit("ax_write_stdout(AX_S(\"\\n\"));".to_string());
        }
    }

    /// `fmt.Printf("%d 件\n", n)`（文稿 11.2）：格式串在 sema 已被编译期解析并核对
    /// （个数/类型/精度，非法占位符 E4044），这里把占位符展开成与字符串插值完全相同的
    /// `ax_sb` 拼接——运行时没有任何格式串解析，`%.Nf` 复用插值的 `:.N` 生成。
    fn fmt_printf(&mut self, e: &'a ast::EPtr) -> String {
        let args: &'a [ast::N<ast::Arg>] = match &e.k {
            Expr::MethodCall { args, .. } => args,
            _ => return "0".to_string(),
        };
        if args.is_empty() {
            return "0".to_string();
        }
        let parts: &'a [SPart] = match &args[0].k.expr.k {
            Expr::StrLit { parts } => parts,
            _ => return "0".to_string(),
        };
        let fmt_str: String = parts
            .iter()
            .filter_map(|p| match p {
                SPart::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        let specs = crate::fmt_spec::parse(&fmt_str).unwrap_or_default();
        let sb = self.tmpn();
        self.emit(format!("ax_sb {}; ax_sb_init(&{});", sb, sb));
        let mut ai = 1;
        for sp in &specs {
            match sp {
                crate::fmt_spec::FmtPart::Text(t) => {
                    if !t.is_empty() {
                        let lit = format!("AX_S({})", cstr(t));
                        self.emit(format!("ax_sb_push(&{}, {});", sb, lit));
                    }
                }
                crate::fmt_spec::FmtPart::Pct => {
                    self.emit(format!("ax_sb_push(&{}, AX_S(\"%\"));", sb));
                }
                crate::fmt_spec::FmtPart::Arg { spec, prec } => {
                    if ai >= args.len() {
                        break;
                    }
                    let arg = &args[ai];
                    ai += 1;
                    let aty = self.ty_or_int(arg.k.expr.id);
                    let v = self.gen(&arg.k.expr, None, false, false);
                    // `%.Nf` → 插值的 `:.N`；其余占位符无精度走默认显示。
                    // sema 已核对类型与个数，push_interp 按类型分派即可。
                    let f = if *spec == 'f' { prec.or(Some(6)) } else { None };
                    self.push_interp(&sb, &v, &aty, f, &arg.k.expr);
                }
            }
        }
        let t = self.tmpn();
        self.emit(format!("ax_str {} = ax_sb_finish(&{});", t, sb));
        self.emit(format!("ax_write_stdout({});", t));
        "0".to_string()
    }

    /// `fmt.Sprint(args...)`：与 `emit_print` 同款空格分隔拼串，但返回 `ax_str` 而非写 stdout。
    fn fmt_sprint(&mut self, args: &'a [ast::N<ast::Arg>]) -> String {
        let sb = self.tmpn();
        self.emit(format!("ax_sb {}; ax_sb_init(&{});", sb, sb));
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                self.emit(format!("ax_sb_push(&{}, AX_S(\" \"));", sb));
            }
            let interp = self
                .cg
                .slot(&arg.k.expr.id)
                .map(|s| s.interp_arg)
                .unwrap_or(false);
            if interp {
                let aty = self.ty_or_int(arg.k.expr.id);
                let av = self.gen(&arg.k.expr, None, false, false);
                self.push_interp(&sb, &av, &aty, None, &arg.k.expr);
            } else {
                let aty = self.ty_or_int(arg.k.expr.id);
                let av = self.gen(&arg.k.expr, None, false, false);
                self.push_one_to_sb(&sb, &av, &aty);
            }
        }
        let t = self.tmpn();
        self.emit(format!("ax_str {} = ax_sb_finish(&{});", t, sb));
        t
    }

    /// `fmt.Sprintf(fmt, args...)`：与 `fmt_printf` 同款格式解析，但返回 `ax_str` 而非写 stdout。
    fn fmt_sprintf(&mut self, e: &'a ast::EPtr) -> String {
        let args: &'a [ast::N<ast::Arg>] = match &e.k {
            Expr::MethodCall { args, .. } => args,
            _ => return "AX_S(\"\")".to_string(),
        };
        if args.is_empty() {
            return "AX_S(\"\")".to_string();
        }
        let parts: &'a [SPart] = match &args[0].k.expr.k {
            Expr::StrLit { parts } => parts,
            _ => return "AX_S(\"\")".to_string(),
        };
        let fmt_str: String = parts
            .iter()
            .filter_map(|p| match p {
                SPart::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        let specs = crate::fmt_spec::parse(&fmt_str).unwrap_or_default();
        let sb = self.tmpn();
        self.emit(format!("ax_sb {}; ax_sb_init(&{});", sb, sb));
        let mut ai = 1;
        for sp in &specs {
            match sp {
                crate::fmt_spec::FmtPart::Text(t) => {
                    if !t.is_empty() {
                        let lit = format!("AX_S({})", cstr(t));
                        self.emit(format!("ax_sb_push(&{}, {});", sb, lit));
                    }
                }
                crate::fmt_spec::FmtPart::Pct => {
                    self.emit(format!("ax_sb_push(&{}, AX_S(\"%\"));", sb));
                }
                crate::fmt_spec::FmtPart::Arg { spec, prec } => {
                    if ai >= args.len() {
                        break;
                    }
                    let arg = &args[ai];
                    ai += 1;
                    let aty = self.ty_or_int(arg.k.expr.id);
                    let v = self.gen(&arg.k.expr, None, false, false);
                    let f = if *spec == 'f' { prec.or(Some(6)) } else { None };
                    self.push_interp(&sb, &v, &aty, f, &arg.k.expr);
                }
            }
        }
        let t = self.tmpn();
        self.emit(format!("ax_str {} = ax_sb_finish(&{});", t, sb));
        t
    }

    /// `fmt.Fprintf(w, fmt, args...)`：与 `fmt_sprintf` 同款格式解析，但将结果写入
    /// `ax_file*`（Writer），返回写入字节数。Writer 是 file.Open/Create/Append 得到的 File 值。
    fn fmt_fprintf(&mut self, e: &'a ast::EPtr) -> String {
        let args: &'a [ast::N<ast::Arg>] = match &e.k {
            Expr::MethodCall { args, .. } => args,
            _ => return "0".to_string(),
        };
        if args.len() < 2 {
            return "0".to_string();
        }
        // args[0] = Writer（File，C 层为 ax_file*），args[1] = 格式字符串，args[2..] = 占位符实参
        let wv = self.gen(&args[0].k.expr, None, false, false);
        let parts: &'a [SPart] = match &args[1].k.expr.k {
            Expr::StrLit { parts } => parts,
            _ => return "0".to_string(),
        };
        let fmt_str: String = parts
            .iter()
            .filter_map(|p| match p {
                SPart::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        let specs = crate::fmt_spec::parse(&fmt_str).unwrap_or_default();
        let sb = self.tmpn();
        self.emit(format!("ax_sb {}; ax_sb_init(&{});", sb, sb));
        let mut ai = 2;
        for sp in &specs {
            match sp {
                crate::fmt_spec::FmtPart::Text(t) => {
                    if !t.is_empty() {
                        let lit = format!("AX_S({})", cstr(t));
                        self.emit(format!("ax_sb_push(&{}, {});", sb, lit));
                    }
                }
                crate::fmt_spec::FmtPart::Pct => {
                    self.emit(format!("ax_sb_push(&{}, AX_S(\"%\"));", sb));
                }
                crate::fmt_spec::FmtPart::Arg { spec, prec } => {
                    if ai >= args.len() {
                        break;
                    }
                    let arg = &args[ai];
                    ai += 1;
                    let aty = self.ty_or_int(arg.k.expr.id);
                    let v = self.gen(&arg.k.expr, None, false, false);
                    let f = if *spec == 'f' { prec.or(Some(6)) } else { None };
                    self.push_interp(&sb, &v, &aty, f, &arg.k.expr);
                }
            }
        }
        let t = self.tmpn();
        self.emit(format!("ax_str {} = ax_sb_finish(&{});", t, sb));
        self.emit(format!("ax_file_write({}, {});", wv, t));
        "0".to_string()
    }

    /// `fmt.Errorf(format, args...)`：与 `fmt_sprintf` 同款格式解析，但返回 `ax_err`（error），
    /// 对标 Go `fmt.Errorf`。构造一个带错误码 1 的错误值，消息为格式化后的字符串。
    fn fmt_errorf(&mut self, e: &'a ast::EPtr) -> String {
        let args: &'a [ast::N<ast::Arg>] = match &e.k {
            Expr::MethodCall { args, .. } => args,
            _ => return "ax_err_new(1, AX_S(\"fmt.Errorf: no format\"))".to_string(),
        };
        if args.is_empty() {
            return "ax_err_new(1, AX_S(\"fmt.Errorf: no format\"))".to_string();
        }
        let parts: &'a [SPart] = match &args[0].k.expr.k {
            Expr::StrLit { parts } => parts,
            _ => return "ax_err_new(1, AX_S(\"fmt.Errorf: bad format\"))".to_string(),
        };
        let fmt_str: String = parts
            .iter()
            .filter_map(|p| match p {
                SPart::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        let specs = crate::fmt_spec::parse(&fmt_str).unwrap_or_default();
        let sb = self.tmpn();
        self.emit(format!("ax_sb {}; ax_sb_init(&{});", sb, sb));
        let mut ai = 1;
        for sp in &specs {
            match sp {
                crate::fmt_spec::FmtPart::Text(t) => {
                    if !t.is_empty() {
                        let lit = format!("AX_S({})", cstr(t));
                        self.emit(format!("ax_sb_push(&{}, {});", sb, lit));
                    }
                }
                crate::fmt_spec::FmtPart::Pct => {
                    self.emit(format!("ax_sb_push(&{}, AX_S(\"%\"));", sb));
                }
                crate::fmt_spec::FmtPart::Arg { spec, prec } => {
                    if ai >= args.len() {
                        break;
                    }
                    let arg = &args[ai];
                    ai += 1;
                    let aty = self.ty_or_int(arg.k.expr.id);
                    let v = self.gen(&arg.k.expr, None, false, false);
                    let f = if *spec == 'f' { prec.or(Some(6)) } else { None };
                    self.push_interp(&sb, &v, &aty, f, &arg.k.expr);
                }
            }
        }
        let t = self.tmpn();
        self.emit(format!("ax_str {} = ax_sb_finish(&{});", t, sb));
        // 返回 ax_err：使用 C11 复合字面量，code=1, msg=格式化后的字符串
        format!("(ax_err){{ .code = 1, .msg = {} }}", t)
    }

    /// `fmt.Scanf(format, args...)`：从 stdin 按格式读取，返回成功解析的项数。
    /// 对标 Go `fmt.Scanf` / C `scanf`。目前支持 `%d`、`%f`、`%s`，实参必须是 int / float / string 变量。
    fn fmt_scanf(&mut self, e: &'a ast::EPtr) -> String {
        let args: &'a [ast::N<ast::Arg>] = match &e.k {
            Expr::MethodCall { args, .. } => args,
            _ => return "0".to_string(),
        };
        if args.is_empty() {
            return "0".to_string();
        }
        let parts: &'a [SPart] = match &args[0].k.expr.k {
            Expr::StrLit { parts } => parts,
            _ => return "0".to_string(),
        };
        let fmt_str: String = parts
            .iter()
            .filter_map(|p| match p {
                SPart::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        // 读取一行输入到缓冲区
        let line = self.tmpn();
        self.emit(format!("ax_str {}; if (ax_read_line(&{}) != 0) {{ {} = ax_str_empty(); }}", line, line, line));
        // 按空白符分割输入行，得到 token 列表
        let tokens = self.tmpn();
        self.emit(format!("ax_arr {} = ax_str_fields({});", tokens, line));
        let count = self.tmpn();
        self.emit(format!("int64_t {} = 0;", count));
        let mut ti = 0; // token 下标
        let mut ai = 1; // args 下标（跳过格式串）
        let specs: Vec<crate::fmt_spec::FmtPart> = crate::fmt_spec::parse(&fmt_str).unwrap_or_default();
        for sp in &specs {
            match sp {
                crate::fmt_spec::FmtPart::Text(_) => {
                    // 文本部分在 scanf 中不消耗 token（可选：可做严格匹配，此处简化为跳过）
                }
                crate::fmt_spec::FmtPart::Pct => {
                    // %% 表示字面量 %，不消耗 token
                }
                crate::fmt_spec::FmtPart::Arg { spec, .. } => {
                    if ai >= args.len() {
                        break;
                    }
                    let arg = &args[ai];
                    ai += 1;
                    if ti >= 0 {
                        // 获取 token 数组长度
                        let len_var = self.tmpn();
                        self.emit(format!("int64_t {} = {}.len;", len_var, tokens));
                        // 检查是否还有 token
                        let ok_var = self.tmpn();
                        self.emit(format!("int {} = ({} < {}) ? 1 : 0;", ok_var, ti, len_var));
                        // 只有有 token 时才解析
                        self.emit(format!("if ({}) {{", ok_var));
                        // 获取 token 字符串
                        let tok_var = self.tmpn();
                        let tok_ptr = self.tmpn();
                        self.emit(format!("ax_str* {} = (ax_str*)ax_arr_at(&{}, {}, (int64_t)sizeof(ax_str), __FILE__, __LINE__, 0);", tok_ptr, tokens, ti));
                        self.emit(format!("ax_str {} = *{};", tok_var, tok_ptr));
                        // 根据 spec 解析
                        match spec {
                            'd' => {
                                let out_var = self.tmpn();
                                self.emit(format!("int64_t {} = 0;", out_var));
                                self.emit(format!("if (ax_parse_i64({}, &{})) {{", tok_var, out_var));
                                // 写回变量：arg.k.expr 是左值，gen 后得到 C 表达式
                                let lv = self.gen(&arg.k.expr, None, false, false);
                                self.emit(format!("{} = {};", lv, out_var));
                                self.emit(format!("{} += 1;", count));
                                self.emit("} else {".to_string());
                                self.emit(format!("{} = ax_str_empty();", tok_var));
                                self.emit("}".to_string());
                            }
                            'f' => {
                                let out_var = self.tmpn();
                                self.emit(format!("double {} = 0.0;", out_var));
                                self.emit(format!("if (ax_parse_f64({}, &{})) {{", tok_var, out_var));
                                let lv = self.gen(&arg.k.expr, None, false, false);
                                self.emit(format!("{} = {};", lv, out_var));
                                self.emit(format!("{} += 1;", count));
                                self.emit("} else {".to_string());
                                self.emit(format!("{} = ax_str_empty();", tok_var));
                                self.emit("}".to_string());
                            }
                            's' => {
                                let lv = self.gen(&arg.k.expr, None, false, false);
                                self.emit(format!("{} = {};", lv, tok_var));
                                self.emit(format!("{} += 1;", count));
                            }
                            _ => {
                                // 不支持的 spec，跳过
                            }
                        }
                        self.emit("}".to_string()); // if ok_var
                        ti += 1;
                    }
                }
            }
        }
        format!("(int){}", count)
    }

    fn intrinsic(&mut self, e: &'a ast::EPtr) -> String {
        let (recv, name, args): (&'a ast::EPtr, &'a String, &'a [ast::N<ast::Arg>]) = match &e.k
        {
            Expr::MethodCall { recv, name, args } => (recv, name, args),
            _ => unreachable!(),
        };
        // 用户调用可能是小写（如 to_string），intrinsic 表和 codegen 分支用 camelCase（toString）。
        let name_pc = camel_member(name);
        let rt = self.ty_or_int(recv.id);
        let rv = self.gen(recv, None, false, false);
        let mut a: Vec<String> = Vec::new();
        for x in args {
            let v = self.gen(&x.k.expr, None, false, false);
            a.push(v);
        }
        match &*rt {
            TyS::Str => {
                let tv = self.declare(&rt, &rv);
                match name_pc.as_str() {
                    "len" => format!("ax_str_len_cp({})", tv),
                    "byteLen" => format!("({}).len", tv),
                    "toString" => tv,
                    "trim" => format!("ax_str_trim({})", tv),
                    "trimLeft" => format!("ax_str_trim_left({}, {})", tv, a[0]),
                    "trimRight" => format!("ax_str_trim_right({}, {})", tv, a[0]),
                    "trimPrefix" => format!("ax_str_trim_prefix({}, {})", tv, a[0]),
                    "trimSuffix" => format!("ax_str_trim_suffix({}, {})", tv, a[0]),
                    "contains" => format!("ax_str_contains({}, {})", tv, a[0]),
                    "containsRune" => format!("ax_str_contains_rune({}, {})", tv, a[0]),
                    "containsAny" => format!("ax_str_contains_any({}, {})", tv, a[0]),
                    // 子串查找：按 UTF-8 码点索引返回，未找到返回 -1
                    "indexOf" => format!("ax_str_index_of({}, {})", tv, a[0]),
                    "indexAny" => format!("ax_str_index_any({}, {})", tv, a[0]),
                    "lastIndexOf" => format!("ax_str_last_index_of({}, {})", tv, a[0]),
                    "lastIndexAny" => format!("ax_str_last_index_any({}, {})", tv, a[0]),
                    "count" => format!("ax_str_count({}, {})", tv, a[0]),
                    // C# 风格字符串截取（按码点，安全处理越界，复用 ax_str_slice_cp）
                    "left" => {
                        let n = self.tmpn();
                        self.emit(format!(
                            "ax_str {} = ax_str_slice_cp({}, 0, {});",
                            n, tv, a[0]
                        ));
                        n
                    }
                    "right" => {
                        let len = self.tmpn();
                        let start = self.tmpn();
                        let n = self.tmpn();
                        self.emit(format!(
                            "int64_t {} = ax_str_len_cp({});",
                            len, tv
                        ));
                        self.emit(format!(
                            "int64_t {} = {} - {};",
                            start, len, a[0]
                        ));
                        self.emit(format!(
                            "ax_str {} = ax_str_slice_cp({}, {}, {});",
                            n, tv, start, len
                        ));
                        n
                    }
                    "mid" => {
                        let start = a[0].clone();
                        let count = a[1].clone();
                        let len = self.tmpn();
                        let end = self.tmpn();
                        let n = self.tmpn();
                        self.emit(format!(
                            "int64_t {} = ax_str_len_cp({});",
                            len, tv
                        ));
                        self.emit(format!(
                            "int64_t {} = {} + {};",
                            end, start, count
                        ));
                        self.emit(format!(
                            "if ({} > {}) {{ {} = {}; }}",
                            end, len, end, len
                        ));
                        self.emit(format!(
                            "ax_str {} = ax_str_slice_cp({}, {}, {});",
                            n, tv, start, end
                        ));
                        n
                    }
                    // 大小写转换
                    "upper" => format!("ax_str_to_upper({})", tv),
                    "lower" => format!("ax_str_to_lower({})", tv),
                    // 前缀/后缀判断
                    "startsWith" => format!("ax_str_starts_with({}, {})", tv, a[0]),
                    "endsWith" => format!("ax_str_ends_with({}, {})", tv, a[0]),
                    "equalFold" => format!("ax_str_equal_fold({}, {})", tv, a[0]),
                    // 按分隔符分割最多 n 份
                    "splitN" => {
                        let n = self.tmpn();
                        let cnt = self.tmpn();
                        self.emit(format!("int64_t {} = 0;", cnt));
                        self.emit(format!(
                            "ax_str* {0}_raw = ax_str_split_n({1}, {2}, {3}, &{4});",
                            n, tv, a[0], a[1], cnt
                        ));
                        self.emit(format!(
                            "ax_arr {0}; ax_arr_from(&{0}, {0}_raw, {1}, (int64_t)sizeof(ax_str));",
                            n, cnt
                        ));
                        n
                    }
                    // 分割并保留分隔符
                    "splitAfter" => {
                        let n = self.tmpn();
                        let cnt = self.tmpn();
                        self.emit(format!("int64_t {} = 0;", cnt));
                        self.emit(format!(
                            "ax_str* {0}_raw = ax_str_split_after({1}, {2}, &{3});",
                            n, tv, a[0], cnt
                        ));
                        self.emit(format!(
                            "ax_arr {0}; ax_arr_from(&{0}, {0}_raw, {1}, (int64_t)sizeof(ax_str));",
                            n, cnt
                        ));
                        n
                    }
                    // 每个单词首字母大写
                    "title" => format!("ax_str_title({})", tv),
                    // 字符映射
                    "map" => format!("ax_str_map({}, {})", tv, a[0]),
                    // 用 s 作为分隔符连接字符串数组
                    "join" => {
                        let arr_ty = t_array(t_string(), None);
                        let arr = self.declare(&arr_ty, &a[0]);
                        let result = self.tmpn();
                        self.emit(format!(
                            "ax_str {} = ax_str_join({}, (ax_str*){}.data, {}.len);",
                            result, tv, arr, arr
                        ));
                        result
                    }
                    // 替换所有出现
                    "replace" => format!("ax_str_replace({}, {}, {})", tv, a[0], a[1]),
                    // 重复 n 次
                    "repeat" => format!("ax_str_repeat({}, {})", tv, a[0]),
                    // 字符属性判断（对标 C <ctype.h>）
                    "isDigit" => format!("ax_str_is_digit({})", tv),
                    "isAlpha" => format!("ax_str_is_alpha({})", tv),
                    "isSpace" => format!("ax_str_is_space({})", tv),
                    "isLower" => format!("ax_str_is_lower({})", tv),
                    "isUpper" => format!("ax_str_is_upper({})", tv),
                    "isAlnum" => format!("ax_str_is_alnum({})", tv),
                    "at" => {
                        let n = self.tmpn();
                        let tmp = self.tmpn();
                        let ety = self.ty_or_int(e.id);
                        let tc = self.cg.ct(&ety);
                        self.emit(format!(
                            "{} {}; memset(&{}, 0, sizeof({}));",
                            tc, n, n, n
                        ));
                        self.emit(format!(
                            "ax_str {0}; if (ax_str_at({1}, {2}, &{0}) == 0) {{ {3}.has = 1; {3}.v = {0}; }}",
                            tmp, tv, a[0], n
                        ));
                        n
                    }
                    "codepointAt" => {
                        let n = self.tmpn();
                        let iv = self.tmpn();
                        let ety = self.ty_or_int(e.id);
                        let tc = self.cg.ct(&ety);
                        self.emit(format!(
                            "{} {}; memset(&{}, 0, sizeof({}));",
                            tc, n, n, n
                        ));
                        self.emit(format!("int32_t {};", iv));
                        self.emit(format!(
                            "if (ax_str_codepoint_at({1}, {2}, &{0}) == 0) {{ {3}.has = 1; {3}.v = (int64_t){0}; }}",
                            iv, tv, a[0], n
                        ));
                        n
                    }
                    "chars" => self.str_chars(&tv),
                    "split" => {
                        let n = self.tmpn();
                        let cnt = self.tmpn();
                        self.emit(format!("int64_t {} = 0;", cnt));
                        self.emit(format!(
                            "ax_str* {0}_raw = ax_str_split({1}, {2}, &{3});",
                            n, tv, a[0], cnt
                        ));
                        self.emit(format!(
                            "ax_arr {0}; ax_arr_from(&{0}, {0}_raw, {1}, (int64_t)sizeof(ax_str));",
                            n, cnt
                        ));
                        n
                    }
                    _ => "0".to_string(),
                }
            }
            TyS::Array(et, _) => {
                let et = et.clone();
                // 修改接收者的方法必须作用在左值上：接收者是左值链（变量 / this /
                // 字段访问）时直接寻址；否则 declare 出来的临时副本被改完即弃
                // （ax_arr 按值赋，改副本不影响原变量），表现为 push/pop 静默失效。
                let tv = if matches!(
                    name_pc.as_str(),
                    "push" | "pop" | "remove" | "delete" | "clear" | "sort" | "insert" | "fill" | "resize" | "merge"
                ) && Self::is_lvalue(recv)
                {
                    format!("({})", rv)
                } else {
                    self.declare(&rt, &rv)
                };
                match name_pc.as_str() {
                    // 定长数组没有 .len 成员：长度是类型的一部分，直接编译期常数
                    "len" => match &*rt {
                        TyS::Array(_, Some(n)) => format!("((int64_t){})", n),
                        _ => format!("{}.len", tv),
                    },
                    "push" => {
                        let v = self.declare(&et, &a[0]);
                        let etc = self.cg.ct(&et);
                        self.emit(format!(
                            "ax_arr_push(&{}, &{}, (int64_t)sizeof({}));",
                            tv, v, etc
                        ));
                        "0".to_string()
                    }
                    "pop" => {
                        let n = self.tmpn();
                        let etc = self.cg.ct(&et);
                        self.emit(format!("{} {};", etc, n));
                        let pa = self.pos_args(&e.tok);
                        self.emit(format!(
                            "ax_arr_pop(&{}, &{}, (int64_t)sizeof({}), {});",
                            tv, n, etc, pa
                        ));
                        n
                    }
                    "remove" => {
                        let etc = self.cg.ct(&et);
                        let pa = self.pos_args(&e.tok);
                        self.emit(format!(
                            "ax_arr_remove(&{}, {}, (int64_t)sizeof({}), {});",
                            tv, a[0], etc, pa
                        ));
                        "0".to_string()
                    }
                    "delete" => {
                        // 删除索引 [i, j) 范围内的元素，对标 Go slices.Delete
                        let etc = self.cg.ct(&et);
                        let pa = self.pos_args(&e.tok);
                        self.emit(format!(
                            "ax_arr_delete_range(&{}, {}, {}, (int64_t)sizeof({}), {});",
                            tv, a[0], a[1], etc, pa
                        ));
                        "0".to_string()
                    }
                    "clear" => {
                        self.emit(format!("ax_arr_clear(&{});", tv));
                        "0".to_string()
                    }
                    "contains" => {
                        let x = &a[0];
                        if matches!(&*et, TyS::Int(..)) {
                            format!("ax_slice_contains_int((const int64_t*){}.data, {}.len, {})", tv, tv, x)
                        } else if matches!(&*et, TyS::Str) {
                            format!("ax_slice_contains_string((const ax_str*){}.data, {}.len, {})", tv, tv, x)
                        } else {
                            "0".to_string()
                        }
                    }
                    "index" => {
                        let x = &a[0];
                        if matches!(&*et, TyS::Int(..)) {
                            format!("ax_slice_index_int((const int64_t*){}.data, {}.len, {})", tv, tv, x)
                        } else if matches!(&*et, TyS::Str) {
                            format!("ax_slice_index_string((const ax_str*){}.data, {}.len, {})", tv, tv, x)
                        } else {
                            "-1".to_string()
                        }
                    }
                    "sort" => {
                        if args.is_empty() {
                            // 无参数：默认升序排序（仅对 int[] / string[] 有默认序）
                            if matches!(&*et, TyS::Int(..)) {
                                self.emit(format!("ax_sort_int((int64_t*){}.data, {}.len);", tv, tv));
                            } else if matches!(&*et, TyS::Str) {
                                self.emit(format!("ax_sort_str((ax_str*){}.data, {}.len);", tv, tv));
                            }
                            "0".to_string()
                        } else {
                            // 带比较函数：GAP #10，qsort + 文件级适配器
                            let et = et.clone();
                            let etc = self.cg.ct(&et);
                            let uid = self.tmpn();
                            let ctx = format!("axsort_ctx_{}", uid);
                            let cmpfn = format!("axsort_cmp_{}", uid);
                            let fnty_ty = t_fn(vec![et.clone(), et.clone()], t_int());
                            self.cg.need(&fnty_ty);
                            let fnty = format!("axfn_{}", self.cg.cn(&fnty_ty));
                            self.cg
                                .fn_decls
                                .push(format!("static int {}(const void*, const void*);", cmpfn));
                            self.cg
                                .fn_decls
                                .push(format!("static {} {};", fnty, ctx));
                            self.cg.extra_fns.push(format!(
                                "static int {cmpfn}(const void* a_, const void* b_) {{\n  {etc} va_; {etc} vb_;\n  memcpy(&va_, a_, sizeof({etc}));\n  memcpy(&vb_, b_, sizeof({etc}));\n  return (int){ctx}(va_, vb_);\n}}",
                                cmpfn = cmpfn,
                                ctx = ctx,
                                etc = etc
                            ));
                            self.emit(format!(
                                "{ctx} = {cmp};",
                                ctx = ctx,
                                cmp = a[0]
                            ));
                            self.emit(format!(
                                "qsort({0}.data, (size_t){0}.len, (size_t)sizeof({1}), {2});",
                                tv, etc, cmpfn
                            ));
                            "0".to_string()
                        }
                    }
                    "reverse" => {
                        // 原地反转：对标 Go slices.Reverse，任意元素类型均可
                        let etc = self.cg.ct(&et);
                        self.emit(format!(
                            "ax_arr_reverse({}.data, (int64_t){}.len, (int64_t)sizeof({}));",
                            tv, tv, etc
                        ));
                        "0".to_string()
                    }
                    "clone" => {
                        // 深拷贝返回独立副本（对标 clone 语义，原数组不受影响）
                        let etc = self.cg.ct(&et);
                        let n = self.tmpn();
                        self.emit(format!(
                            "ax_arr {} = ax_arr_clone(&{}, (int64_t)sizeof({}));",
                            n, tv, etc
                        ));
                        n
                    }
                    "insert" => {
                        // 在索引 i 处插入元素 v，后续元素后移
                        let etc = self.cg.ct(&et);
                        let pa = self.pos_args(&e.tok);
                        let v = self.declare(&et, &a[1]);
                        self.emit(format!(
                            "ax_arr_insert(&{}, {}, &{}, (int64_t)sizeof({}), {});",
                            tv, a[0], v, etc, pa
                        ));
                        "0".to_string()
                    }
                    "fill" => {
                        // 用指定值 v 填充所有元素
                        let etc = self.cg.ct(&et);
                        let v = self.declare(&et, &a[0]);
                        self.emit(format!(
                            "ax_arr_fill(&{}, &{}, (int64_t)sizeof({}));",
                            tv, v, etc
                        ));
                        "0".to_string()
                    }
                    "resize" => {
                        // 调整长度为 n，不足时零初始化，超出时截断
                        let etc = self.cg.ct(&et);
                        self.emit(format!(
                            "ax_arr_resize(&{}, {}, (int64_t)sizeof({}));",
                            tv, a[0], etc
                        ));
                        "0".to_string()
                    }
                    "merge" => {
                        // 数组合并：将另一个同类型数组的所有元素追加到末尾
                        let etc = self.cg.ct(&et);
                        let b = self.declare(&rt, &a[0]);
                        self.emit(format!(
                            "ax_arr_merge(&{}, &{}, (int64_t)sizeof({}));",
                            tv, b, etc
                        ));
                        "0".to_string()
                    }
                    "min" => {
                        // int[]/float[] 聚合：求最小值，空数组返回 0
                        if matches!(&*et, TyS::Int(..)) {
                            format!("ax_arr_min_int((const int64_t*){}.data, {}.len)", tv, tv)
                        } else if matches!(&*et, TyS::Float) {
                            format!("ax_arr_min_float((const double*){}.data, {}.len)", tv, tv)
                        } else {
                            "0".to_string()
                        }
                    }
                    "max" => {
                        // int[]/float[] 聚合：求最大值，空数组返回 0
                        if matches!(&*et, TyS::Int(..)) {
                            format!("ax_arr_max_int((const int64_t*){}.data, {}.len)", tv, tv)
                        } else if matches!(&*et, TyS::Float) {
                            format!("ax_arr_max_float((const double*){}.data, {}.len)", tv, tv)
                        } else {
                            "0".to_string()
                        }
                    }
                    "sum" => {
                        // int[]/float[] 聚合：求和，空数组返回 0
                        if matches!(&*et, TyS::Int(..)) {
                            format!("ax_arr_sum_int((const int64_t*){}.data, {}.len)", tv, tv)
                        } else if matches!(&*et, TyS::Float) {
                            format!("ax_arr_sum_float((const double*){}.data, {}.len)", tv, tv)
                        } else {
                            "0".to_string()
                        }
                    }
                    "at" => {
                        let n = self.tmpn();
                        let ety = self.ty_or_int(e.id);
                        let tc = self.cg.ct(&ety);
                        let etc = self.cg.ct(&et);
                        self.emit(format!(
                            "{} {}; memset(&{}, 0, sizeof({}));",
                            tc, n, n, n
                        ));
                        self.emit(format!(
                            "{0}.has = ax_arr_try_at(&{1}, {2}, (int64_t)sizeof({3}), &{0}.v) == 0;",
                            n, tv, a[0], etc
                        ));
                        n
                    }
                    "toString" => "AX_S(\"[array]\")".to_string(),
                    _ => "0".to_string(),
                }
            }
            TyS::Map(kt, vt) => {
                let kt = kt.clone();
                let vt = vt.clone();
                // 修改接收者的方法必须作用在左值上（与数组方法保持一致），
                // 否则改副本不影响原变量。
                let tv = if matches!(
                    name.as_str(),
                    "remove" | "clear"
                ) && Self::is_lvalue(recv)
                {
                    format!("({})", rv)
                } else {
                    self.declare(&rt, &rv)
                };
                match name.as_str() {
                    "len" => format!("{}.len", tv),
                    "has" => {
                        let kv = self.declare(&kt, &a[0]);
                        format!("ax_map_has(&{}, &{})", tv, kv)
                    }
                    "get" => {
                        let kv = self.declare(&kt, &a[0]);
                        let n = self.tmpn();
                        let ety = self.ty_or_int(e.id);
                        let tc = self.cg.ct(&ety);
                        let vtc = self.cg.ct(&vt);
                        self.emit(format!(
                            "{} {}; memset(&{}, 0, sizeof({}));",
                            tc, n, n, n
                        ));
                        self.emit(format!(
                            "{{ void* p_ = ax_map_get(&{0}, &{1}); if (p_) {{ {2}.has = 1; memcpy(&{2}.v, p_, sizeof({3})); }} }}",
                            tv, kv, n, vtc
                        ));
                        n
                    }
                    "remove" => {
                        let kv = self.declare(&kt, &a[0]);
                        self.emit(format!(
                            "ax_map_remove(&{}, &{});",
                            tv, kv
                        ));
                        "0".to_string()
                    }
                    "keys" => {
                        let etc = self.cg.ct(&kt);
                        let n = self.tmpn();
                        self.emit(format!(
                            "ax_arr {} = ax_map_keys(&{});",
                            n, tv
                        ));
                        n
                    }
                    "clear" => {
                        self.emit(format!("ax_map_clear(&{});", tv));
                        "0".to_string()
                    }
                    _ => "0".to_string(),
                }
            }
            TyS::Decimal => {
                let tv = self.declare(&rt, &rv);
                match name_pc.as_str() {
                    "toString" => format!("ax_dec_to_str({})", tv),
                    "round" => format!(
                        "ax_dec_round({}, (int32_t)({}), (int)({}))",
                        tv, a[0], a[1]
                    ),
                    "divide" => format!(
                        "ax_dec_div_round({}, {}, (int32_t)({}), (int)({}))",
                        tv, a[0], a[1], a[2]
                    ),
                    "truncate" => {
                        let pa = self.pos_args(&e.tok);
                        format!(
                            "ax_narrow(ax_dec_to_i64({}, {}), (int)({}), 1, {})",
                            tv, pa, a[0], pa
                        )
                    }
                    _ => "0".to_string(),
                }
            }
            TyS::Int(..) => {
                let tv = self.declare(&rt, &rv);
                match name_pc.as_str() {
                    "toString" => format!("ax_str_of_int((int64_t){})", tv),
                    "wrappingAdd" => {
                        format!("(int64_t)((uint64_t){} + (uint64_t)({}))", tv, a[0])
                    }
                    "divide" => {
                        format!("({} / {})", tv, a[0])
                    }
                    _ => "0".to_string(),
                }
            }
            TyS::Float => {
                let tv = self.declare(&rt, &rv);
                match name_pc.as_str() {
                    "toString" => format!("ax_str_of_f64({})", tv),
                    "round" => {
                        format!("round({} * pow(10.0, {})) / pow(10.0, {})", tv, a[0], a[0])
                    }
                    "divide" => {
                        format!("({} / {})", tv, a[0])
                    }
                    _ => "0".to_string(),
                }
            }
            TyS::Bool => {
                let tv = self.declare(&rt, &rv);
                match name_pc.as_str() {
                    "toString" => format!("ax_str_of_bool({})", tv),
                    _ => "0".to_string(),
                }
            }
            TyS::Enum(_) => {
                let tv = self.declare(&rt, &rv);
                match name_pc.as_str() {
                    "toString" => self.enum_name_expr(recv, &tv),
                    _ => "0".to_string(),
                }
            }
            TyS::Named(DefId::Name("time"), _) => {
                let tv = self.declare(&rt, &rv);
                match name_pc.as_str() {
                    "unixMs" => tv,
                    "toString" => format!("ax_time_to_str({})", tv),
                    "format" => {
                        let fmt = self.declare(&t_string(), &a[0]);
                        format!("ax_time_format({}, (const char*){}.p)", tv, fmt)
                    }
                    "after" => format!("({} > {})", tv, a[0]),
                    "before" => format!("({} < {})", tv, a[0]),
                    "equal" => format!("({} == {})", tv, a[0]),
                    "compare" => format!("({} < {} ? -1 : {} > {} ? 1 : 0)", tv, a[0], tv, a[0]),
                    "sub" => format!("({} - {})", tv, a[0]),
                    "add" => format!("({} + {})", tv, a[0]),
                    "round" => format!("ax_time_round({}, {})", tv, a[0]),
                    "truncate" => format!("ax_time_truncate({}, {})", tv, a[0]),
                    _ => "0".to_string(),
                }
            }
            TyS::Named(DefId::Name("thread"), _) => {
                let tv = self.declare(&rt, &rv);
                match name.as_str() {
                    "join" => format!("ax_thread_join({})", tv),
                    _ => "0".to_string(),
                }
            }
            TyS::Named(DefId::Name("queue"), args) => {
                let tv = self.declare(&rt, &rv);
                let elem = args.first().cloned().unwrap_or_else(t_object);
                match name.as_str() {
                    "push" => {
                        let v = self.declare(&elem, &a[0]);
                        format!("ax_queue_push({}, &{})", tv, v)
                    }
                    "pop" => {
                        let out = self.tmpn();
                        let tmp = self.tmpn();
                        let elem_ty = args.first().cloned().unwrap_or_else(t_object);
                        let nullable_ty = t_nullable(elem_ty.clone());
                        let etc = self.cg.ct(&nullable_ty);
                        let etc_elem = self.cg.ct(&elem_ty);
                        self.emit(format!("{} {}; memset(&{}, 0, sizeof({}));", etc, out, out, out));
                        self.emit(format!("{} {};", etc_elem, tmp));
                        let pop_ok = self.tmpn();
                        self.emit(format!(
                            "int {} = ax_queue_pop({}, &{});",
                            pop_ok, tv, tmp
                        ));
                        self.emit(format!(
                            "if ({} == 0) {{ {}.has = 1; {}.v = {}; }}",
                            pop_ok, out, out, tmp
                        ));
                        out
                    }
                    "tryPop" => {
                        let out = self.tmpn();
                        let tmp = self.tmpn();
                        let elem_ty = args.first().cloned().unwrap_or_else(t_object);
                        let nullable_ty = t_nullable(elem_ty.clone());
                        let etc = self.cg.ct(&nullable_ty);
                        let etc_elem = self.cg.ct(&elem_ty);
                        self.emit(format!("{} {}; memset(&{}, 0, sizeof({}));", etc, out, out, out));
                        self.emit(format!("{} {};", etc_elem, tmp));
                        let try_ok = self.tmpn();
                        self.emit(format!(
                            "int {} = ax_queue_try_pop({}, &{});",
                            try_ok, tv, tmp
                        ));
                        self.emit(format!(
                            "if ({} == 0) {{ {}.has = 1; {}.v = {}; }}",
                            try_ok, out, out, tmp
                        ));
                        out
                    }
                    "Close" => format!("ax_queue_close({})", tv),
                    _ => "0".to_string(),
                }
            }
            // sync 内建句柄类型：通过 typedefs 注册，类型为 TyS::Named(DefId::Id(id), _)
            TyS::Named(DefId::Id(id), _) => {
                let td = self.cg.s.typedefs.get(*id as usize);
                let tv = self.declare(&rt, &rv);
                match td.map(|d| d.name.as_str()) {
                    Some("Mutex") => match name.as_str() {
                        "lock" => { self.emit(format!("ax_std_mutex_lock(&{});", tv)); "0".to_string() }
                        "unlock" => { self.emit(format!("ax_std_mutex_unlock(&{});", tv)); "0".to_string() }
                        _ => "0".to_string(),
                    },
                    Some("RWMutex") => match name.as_str() {
                        "lock" => { self.emit(format!("ax_std_rwmutex_lock(&{});", tv)); "0".to_string() }
                        "unlock" => { self.emit(format!("ax_std_rwmutex_unlock(&{});", tv)); "0".to_string() }
                        "rLock" => { self.emit(format!("ax_std_rwmutex_rlock(&{});", tv)); "0".to_string() }
                        "runlock" => { self.emit(format!("ax_std_rwmutex_runlock(&{});", tv)); "0".to_string() }
                        _ => "0".to_string(),
                    },
                    Some("WaitGroup") => match name.as_str() {
                        "add" => format!("ax_std_waitgroup_add(&{}, {})", tv, self.gen(&args[0].k.expr, Some(&t_int()), false, false)),
                        "done" => { self.emit(format!("ax_std_waitgroup_done(&{});", tv)); "0".to_string() }
                        "wait" => { self.emit(format!("ax_std_waitgroup_wait(&{});", tv)); "0".to_string() }
                        _ => "0".to_string(),
                    },
                    Some("Once") => match name.as_str() {
                        "do" => { self.emit(format!("ax_std_once_do(&{});", tv)); "0".to_string() }
                        _ => "0".to_string(),
                    },
                    _ => "0".to_string(),
                }
            }
            TyS::Range => {
                let tv = self.declare(&rt, &rv);
                match name.as_str() {
                    // Range 以 int64_t 表示，且只能出现在 for 头（parser 已转为 ForKind::Step），
                    // 作为独立表达式出现时 sema 会拦截（E4007），此分支仅防止 codegen 返回 "0" 占位符。
                    "step" => tv,
                    _ => "0".to_string(),
                }
            }
            _ => "0".to_string(),
        }
    }

    pub fn str_chars(&mut self, tv: &str) -> String {
        let n = self.tmpn();
        let i = self.tmpn();
        self.emit(format!("int64_t {} = ax_str_len_cp({});", i, tv));
        self.emit(format!("ax_arr {}; ax_arr_init(&{});", n, n));
        self.emit(format!(
            "for (int64_t k_ = 0; k_ < {0}; k_++) {{ ax_str c_; if (ax_str_at({1}, k_, &c_) == 0) ax_arr_push(&{2}, &c_, (int64_t)sizeof(ax_str)); }}",
            i, tv, n
        ));
        n
    }

    fn parse_call(&mut self, e: &'a ast::EPtr) -> String {
        let args: &'a [ast::N<ast::Arg>] = match &e.k {
            Expr::MethodCall { args, .. } => args,
            _ => &[],
        };
        let s = self.gen(&args[0].k.expr, Some(&t_string()), false, false);
        let sl = self.cg.slot(&e.id).cloned().unwrap_or_default();
        let target = sl.parse_to.unwrap_or_else(t_object);
        let rt = self.ty_or_int(e.id);
        let n = self.tmpn();
        let rtc = self.cg.ct(&rt);
        self.emit(format!("{} {};", rtc, n));
        let ov = self.tmpn();
        match &*target {
            TyS::Int(..) => {
                self.emit(format!("int64_t {} = 0;", ov));
                self.emit(format!(
                    "if (ax_parse_i64({1}, &{0})) {{ {2}.tag = 1; {2}.u.ok = {0}; }} else {{ {2}.tag = 0; {2}.u.err = ax_err_new(2, AX_S(\"parse failed\")); }}",
                    ov, s, n
                ));
            }
            TyS::Float => {
                self.emit(format!("double {} = 0;", ov));
                self.emit(format!(
                    "if (ax_parse_f64({1}, &{0})) {{ {2}.tag = 1; {2}.u.ok = {0}; }} else {{ {2}.tag = 0; {2}.u.err = ax_err_new(2, AX_S(\"parse failed\")); }}",
                    ov, s, n
                ));
            }
            _ => {
                let okv = self.tmpn();
                self.emit(format!(
                    "ax_dec {0} = ax_dec_from_i64(0); int {2} = 0; {0} = ax_dec_parse({1}, &{2});",
                    ov, s, okv
                ));
                self.emit(format!(
                    "if ({2}) {{ {0}.tag = 1; {0}.u.ok = {1}; }} else {{ {0}.tag = 0; {0}.u.err = ax_err_new(2, AX_S(\"parse failed\")); }}",
                    n, ov, okv
                ));
            }
        }
        n
    }

    fn from_codepoint(&mut self, e: &'a ast::EPtr) -> String {
        let args: &'a [ast::N<ast::Arg>] = match &e.k {
            Expr::MethodCall { args, .. } => args,
            _ => &[],
        };
        let v = self.gen(&args[0].k.expr, None, false, false);
        let n = self.tmpn();
        let ety = self.ty_or_int(e.id);
        let tc = self.cg.ct(&ety);
        self.emit(format!("{} {};", tc, n));
        self.emit(format!(
            "{{ ax_str s_; if (ax_str_from_codepoint((int32_t)({0}), &s_) == 0) {{ {1}.tag = 1; {1}.u.ok = s_; }} else {{ {1}.tag = 0; {1}.u.err = ax_err_new(2, AX_S(\"bad codepoint\")); }} }}",
            v, n
        ));
        n
    }

    // ---------- 字段 / 下标 / 安全访问 ----------

    fn field_access(&mut self, e: &'a ast::EPtr) -> String {
        let (recv, name): (&'a ast::EPtr, &'a String) = match &e.k {
            Expr::FieldAccess { recv, name } => (recv, name),
            _ => unreachable!(),
        };
        let res = self.res_of(e.id);
        match res {
            // 用户模块成员（文稿 9.2）：常量按值内联
            Some(crate::sema::Res::Const(c)) => {
                let value = c.value.borrow().clone();
                let ty = c.ty.borrow().clone();
                return match value {
                    Some(v) => self.cg.const_c(&v, &ty),
                    None => "0".to_string(),
                };
            }
            Some(crate::sema::Res::EnumVariant(d, var)) => {
                let idx = self
                    .cg
                    .s
                    .typedefs
                    .get(d as usize)
                    .and_then(|td| td.variants.iter().position(|v| v == &var))
                    .unwrap_or(0);
                return format!("{}u", idx);
            }
            // 函数引用（文稿 4.1）：`Type.static_method` / `mod.func` 不加括号
            Some(crate::sema::Res::Func(fid)) => {
                // 单态化实例：slot.func 指向实例，使用实例名（GX-04 泛型函数值实例化）
                if let Some(inst_fid) = self.cg.slot(&e.id).and_then(|sl| sl.func) {
                    return self.cg.fn_name_idx(inst_fid);
                }
                return self.cg.fn_name_idx(fid);
            }
            Some(crate::sema::Res::Rounding(n)) => {
                return match n.as_str() {
                    "halfUp" => "0".to_string(),
                    "halfEven" => "1".to_string(),
                    "down" => "2".to_string(),
                    _ => "0".to_string(),
                };
            }
            _ => {}
        }
        let rt = self.ty_or_int(recv.id);
        if matches!(&*rt, TyS::Error) {
            let (recv_c, deref) = self.recv_c(recv);
            let sep = if deref { "->" } else { "." };
            return format!("{}{}{}", recv_c, sep, name);
        }
        let (recv_c, deref) = self.recv_c(recv);
        let sep = if deref { "->" } else { "." };
        format!("{}{}f_{}", recv_c, sep, san(name))
    }

    fn safe_access(&mut self, e: &'a ast::EPtr) -> String {
        let (recv, name): (&'a ast::EPtr, &'a String) = match &e.k {
            Expr::SafeAccess { recv, name } => (recv, name),
            _ => unreachable!(),
        };
        let rt = self.ty_or_int(recv.id);
        let rv = self.gen(recv, None, false, false);
        let tv = self.declare(&rt, &rv);
        let n = self.tmpn();
        let ety = self.ty_or_int(e.id);
        let tc = self.cg.ct(&ety);
        self.emit(format!("{} {}; memset(&{}, 0, sizeof({}));", tc, n, n, n));
        self.emit(format!(
            "if ({0}.has) {{ {1}.has = 1; {1}.v = {0}.v.f_{2}; }}",
            tv,
            n,
            san(name)
        ));
        n
    }

    fn safe_call(&mut self) -> String {
        // 历史行为（测试钉死）：SafeCall 的生成恒返回 "0"，不做"顺手修正"
        "0".to_string()
    }

    fn index(&mut self, e: &'a ast::EPtr) -> String {
        let (recv, index): (&'a ast::EPtr, &'a ast::EPtr) = match &e.k {
            Expr::Index { recv, index } => (recv, index),
            _ => unreachable!(),
        };
        let rt = self.ty_or_int(recv.id);
        match &*rt {
            TyS::Record(_) => {
                let (recv_c, deref) = self.recv_c(recv);
                let key = self
                    .cg
                    .slot(&e.id)
                    .and_then(|s| s.key.clone())
                    .unwrap_or_default();
                let sep = if deref { "->" } else { "." };
                format!("{}{}f_{}", recv_c, sep, san(&key))
            }
            TyS::Map(kt, vt) => {
                let kt = kt.clone();
                let vt = vt.clone();
                let m = self.hoist(recv, &rt);
                let k = self.gen(index, Some(&kt), false, false);
                let kv = self.declare(&kt, &k);
                let pa = self.pos_args(&e.tok);
                let vtc = self.cg.ct(&vt);
                // keydesc：string 键传实际键值，使 R101 崩溃消息里显示真实键
                //（如 `key not found: b`）；其余类型保留类型描述，避免 %s 读
                // 非字符串内存。
                let keydesc = match &*kt {
                    TyS::Str => format!("(const char*){}.p", kv),
                    _ => cstr("key"),
                };
                format!(
                    "*({}*)ax_map_get_chk(&{}, &{}, {}, {})",
                    vtc,
                    m,
                    kv,
                    pa,
                    keydesc
                )
            }
            TyS::Array(et, len) => {
                let et = et.clone();
                match &index.k {
                    Expr::Range { lo, hi, inclusive } => {
                        // 数组切片 m[lo..hi] / m[lo..=hi] → 生成一个新的 ax_arr
                        // 动态数组：浅层共享底层数据（零复制）
                        // 定长数组 T[N]：深度复制到堆内存（避免悬垂指针，符合值语义）
                        let m = self.hoist(recv, &rt);
                        let lo_c = self.gen(lo, None, false, false);
                        let hi_c = self.gen(hi, None, false, false);
                        let lo_var = self.declare(&t_int(), &lo_c);
                        let hi_var = self.declare(&t_int(), &hi_c);
                        let etc = self.cg.ct(&et);
                        let slice_var = self.tmpn();
                        let eff_hi = if *inclusive {
                            format!("({} + 1)", hi_var)
                        } else {
                            hi_var.clone()
                        };
                        let slice_len = format!("({} - {})", eff_hi, lo_var);
                        let pa = self.pos_args(&e.tok);

                        // 运行期边界检查：动态数组用 m.len，定长数组用编译期常数 n
                        let bound_check = match &len {
                            Some(n) => {
                                format!("({} < 0 || {} > {})", lo_var, eff_hi, n)
                            }
                            None => format!("({} < 0 || {} > ({}).len)", lo_var, eff_hi, m),
                        };
                        self.emit(format!(
                            "if ({}) ax_trap(R_INDEX, {}, {}, {}, \"slice out of range\");",
                            bound_check, pa, pa, pa
                        ));

                        if len.is_some() {
                            // 定长数组 T[N]：深度复制到堆（ax_alloc + 逐元素复制）
                            // 避免栈上数组销毁后悬垂，符合值语义
                            self.emit(format!(
                                "ax_arr {}; ax_arr_init(&{});",
                                slice_var, slice_var
                            ));
                            self.emit(format!(
                                "{}.data = ax_alloc((int64_t){} * (int64_t)sizeof({}));",
                                slice_var, slice_len, etc
                            ));
                            self.emit(format!(
                                "{}.len = {}; {}.cap = {};",
                                slice_var, slice_len, slice_var, slice_len
                            ));
                            // 逐元素复制，避免 memcpy 的指针类型问题。
                            // 定长数组在 C 层是 `typedef struct { T v[N]; } axfix_<name>;`，
                            // 数据字段是 `.v[N]`，直接下标访问即可（m 已由 hoist 落为左值变量）。
                            self.emit(format!(
                                "for (int64_t i_ = 0; i_ < {}; i_++) {{ (({}*){}.data)[i_] = {}.v[{} + i_]; }}",
                                slice_len, etc, slice_var, m, lo_var
                            ));
                        } else {
                            // 动态数组 T[]：浅层共享底层数据（零复制）
                            self.emit(format!(
                                "ax_arr {}; ax_arr_init(&{});",
                                slice_var, slice_var
                            ));
                            self.emit(format!(
                                "{}.data = (uint8_t*)({}).data + (int64_t){} * (int64_t)sizeof({});",
                                slice_var, m, lo_var, etc
                            ));
                            self.emit(format!(
                                "{}.len = {}; {}.cap = {};",
                                slice_var, slice_len, slice_var, slice_len
                            ));
                        }
                        return format!("{}", slice_var);
                    }
                    _ => {}
                }
                if len.is_some() {
                    let (a, deref) = self.recv_c(recv);
                    let idx = self.gen(index, None, false, false);
                    // 定长数组在 C 里展开成 `T v_x[N]` 或 struct 成员 `.v[N]`；
                    // sep 已经带 `.`/`->`，模板里不能再写一个点
                    let sep = if deref { "->" } else { "." };
                    return format!("{}{}v[{}]", a, sep, idx);
                }
                let m = self.hoist(recv, &rt);
                let idx = self.gen(index, None, false, false);
                let iv = self.declare(&t_int(), &idx);
                let etc = self.cg.ct(&et);
                let pa = self.pos_args(&e.tok);
                format!(
                    "(*({0}*)ax_arr_at(&{1}, {2}, (int64_t)sizeof({0}), {3}))",
                    etc, m, iv, pa
                )
            }
            _ => "0".to_string(),
        }
    }

    /// struct 的位置构造 `Point(3, 4)` → C 复合字面量。
    /// struct 是值语义、没有出生逻辑（不像 class 要跑 constructor），所以不需要 `__new`
    /// 函数，按声明顺序做 designated initializer 即可（求值顺序与书写一致，文稿 8.6）。
    fn struct_literal(
        &mut self,
        ty: &Ty,
        td: &crate::sema::TypeDef,
        args: &'a [ast::N<ast::Arg>],
    ) -> String {
        let cty = self.cg.ct(ty);
        let mut parts: Vec<String> = Vec::new();
        for (i, (fname, fty, _, _)) in td.fields.iter().enumerate() {
            let v = match args.get(i) {
                Some(a) => self.gen(&a.k.expr, Some(fty), false, false),
                None => "0".to_string(),
            };
            parts.push(format!(".f_{} = {}", san(fname), v));
        }
        if parts.is_empty() {
            return format!("(({}){{ 0 }})", cty);
        }
        format!("(({}){{ {} }})", cty, parts.join(", "))
    }

    fn new_expr(&mut self, e: &'a ast::EPtr) -> String {
        let (args, path): (&'a [ast::N<ast::Arg>], &[String]) = match &e.k {
            Expr::New { path, args, .. } => (args, path.as_slice()),
            _ => (&[], &[]),
        };
        let ck = self
            .cg
            .slot(&e.id)
            .and_then(|s| s.ckind)
            .unwrap_or("");
        if ck == "arena" {
            // O-20：`new Arena()` 创建区域分配器；区域变量离开作用域时
            // 由块尾的 ax_arena_free 整体释放（codegen_fn 的 var_decl 挂钩）
            return "ax_arena_create()".to_string();
        }
        let ty = self.ty_or_int(e.id);
        let d: u32 = match &*ty {
            TyS::Named(DefId::Id(d), _) => *d,
            TyS::Enum(d) => *d,
            _ => 0,
        };
        let td = self.cg.s.typedefs[d as usize].clone();
        // 确定 constructor 名：path 最后一段若在 td.ctors 中，则为命名 constructor；
        // 否则默认 "constructor"（兼容 new ClassName() 与 new mod.Type()）。
        let ctor_name = if path.len() >= 2 && td.ctors.contains_key(&path[path.len() - 1]) {
            path[path.len() - 1].clone()
        } else {
            "constructor".to_string()
        };
        let ctor_fid = td.ctors.get(&ctor_name).copied();
        let nm = match ctor_fid {
            Some(fid) => {
                let fname = &self.cg.s.funcs[fid].name;
                format!("{}__new_{}", self.cg.class_name(&td), fname)
            }
            None => format!("{}__new", self.cg.class_name(&td)),
        };
        let mut out = Vec::new();
        for a in args {
            let v = self.gen(&a.k.expr, None, false, false);
            out.push(v);
        }
        format!("{}({})", nm, out.join(", "))
    }

    // ---------- match ----------

    pub fn match_stmt_stmt(&mut self, s: &'a ast::N<Stmt>) {
        let n = match &s.k {
            Stmt::Match(n) => n,
            _ => unreachable!(),
        };
        self.match_stmt(n, s.id, None, None);
    }

    pub fn match_stmt(
        &mut self,
        n: &'a ast::MatchNode,
        node_id: u32,
        want: Option<&Ty>,
        out_var: Option<&str>,
    ) {
        let st = self
            .cg
            .slot(&node_id)
            .and_then(|s| s.scrut_ty.clone())
            .unwrap_or_else(t_object);
        if let TyS::Enum(d_id) = &*st {
            let d_id = *d_id;
            let scrutinee = &n.scrutinee;
            let v = self.gen(scrutinee, None, false, false);
            let sv = if self.is_simple(scrutinee) {
                v
            } else {
                self.declare(&st, &v)
            };
            self.emit(format!("switch ({}) {{", sv));
            let d = self.cg.s.typedefs.get(d_id as usize).cloned();
            for arm in &n.arms {
                let p = &arm.pat;
                match &p.k {
                    Pat::Enum { variant, .. } => {
                        let idx = d
                            .as_ref()
                            .and_then(|td| td.variants.iter().position(|v| v == variant))
                            .unwrap_or(0);
                        self.emit(format!("case {}: {{", idx));
                    }
                    Pat::Multi { pats } => {
                        let enum_pats: Vec<&ast::N<ast::Pat>> = pats
                            .iter()
                            .filter(|sp| matches!(&sp.k, ast::Pat::Enum { .. }))
                            .collect();
                        if !enum_pats.is_empty() {
                            for sp in &enum_pats {
                                if let ast::Pat::Enum { variant, .. } = &sp.k {
                                    let idx = d
                                        .as_ref()
                                        .and_then(|td| td.variants.iter().position(|v| v == variant))
                                        .unwrap_or(0);
                                    self.emit(format!("case {}: ", idx));
                                }
                            }
                            self.emit("{".to_string());
                        } else {
                            self.emit("default: {".to_string());
                        }
                    }
                    _ => {
                        self.emit("default: {".to_string());
                    }
                }
                self.ind += 1;
                if let Some(ov) = out_var {
                    let val = self.gen(&arm.body, Some(want.unwrap()), false, false);
                    self.emit(format!("{} = {};", ov, val));
                } else {
                    // 臂体是表达式：纯表达式形态的调用（无 emit 副作用）必须
                    // 落成语句，否则整个调用被静默丢弃（与 Stmt::Simple 同理）
                    let v = self.gen(&arm.body, None, false, true);
                    if !v.is_empty() && v != "0" {
                        self.emit(format!("(void)({});", v));
                    }
                }
                self.emit("break;".to_string());
                self.ind -= 1;
                self.emit("}".to_string());
            }
            self.emit("}".to_string());
            return;
        }
        // 通用 if 链
        let mut sv: Option<String> = None;
        let mut chain_open = false;
        if matches!(
            &*st,
            TyS::Result(_)
                | TyS::Int(..)
                | TyS::Str
                | TyS::Bool
                | TyS::Nullable(_)
                | TyS::Object
                | TyS::Named(..)
        ) {
            let v = self.gen(&n.scrutinee, None, false, false);
            sv = Some(self.declare(&st, &v));
        }
        let svm = sv.clone().unwrap_or_default();
        for arm in &n.arms {
            let cond = self.pat_cond(&arm.pat, &svm, &st);
            let kw = match cond {
                None => {
                    if chain_open {
                        "else {".to_string()
                    } else {
                        "{".to_string()
                    }
                }
                Some(c) => {
                    if chain_open {
                        format!("else if ({}) {{", c)
                    } else {
                        format!("if ({}) {{", c)
                    }
                }
            };
            self.emit(kw);
            self.ind += 1;
            self.bind_pat(&arm.pat, &svm, &st);
            if let Some(ov) = out_var {
                let val = self.gen(&arm.body, Some(want.unwrap()), false, false);
                self.emit(format!("{} = {};", ov, val));
            } else {
                // 同上：纯表达式臂体必须落成语句
                let v = self.gen(&arm.body, None, false, true);
                if !v.is_empty() && v != "0" {
                    self.emit(format!("(void)({});", v));
                }
            }
            self.ind -= 1;
            self.emit("}".to_string());
            chain_open = true;
        }
    }

    fn pat_cond(&mut self, p: &'a ast::N<Pat>, sv: &str, st: &Ty) -> Option<String> {
        match &p.k {
            Pat::Wild | Pat::Bind { .. } => None,
            // enum 分支走 switch，不进 if 链
            Pat::Enum { .. } => None,
            Pat::Multi { pats } => {
                let conds: Vec<String> = pats
                    .iter()
                    .filter_map(|sp| self.pat_cond(sp, sv, st))
                    .collect();
                if conds.is_empty() {
                    None
                } else {
                    Some(conds.join(" || "))
                }
            }
            Pat::Lit { expr } => {
                let lit = self.gen(expr, None, false, false);
                if matches!(&**st, TyS::Str) {
                    Some(format!("ax_str_eq({}, {})", sv, lit))
                } else {
                    Some(format!("{} == {}", sv, lit))
                }
            }
            Pat::Range {
                lo,
                hi,
                inclusive,
            } => {
                let lo_c = self.gen(lo, None, false, false);
                let hi_c = self.gen(hi, None, false, false);
                let op = if *inclusive { "<=" } else { "<" };
                Some(format!("({} >= {} && {} {} {})", sv, lo_c, sv, op, hi_c))
            }
            Pat::Result { is_ok, .. } => Some(format!(
                "{}.tag == {}",
                sv,
                if *is_ok { AX_OK } else { AX_FAIL }
            )),
            Pat::Type { .. } => {
                let pty = self
                    .cg
                    .slot(&p.id)
                    .and_then(|s| s.ty.clone())
                    .unwrap_or_else(t_object);
                let tid = tag_of(&pty);
                // 两种来源的 tag 字段不同：`object` 是装箱句柄 `ax_box`（成员 `tag`），
                // 类引用是对象头 `T*`（成员 `__tag`，构造时写入真实类型标签）
                match &**st {
                    TyS::Object => Some(format!("({0} != NULL && {0}->tag == {1}u)", sv, tid)),
                    _ => Some(format!("({0} != NULL && {0}->__tag == {1}u)", sv, tid)),
                }
            }
        }
    }

    fn bind_pat(&mut self, p: &'a ast::N<Pat>, sv: &str, st: &Ty) {
        match &p.k {
            Pat::Result { is_ok, bind } => {
                if *is_ok {
                    let inner: Ty = match &**st {
                        TyS::Result(i) => i.clone(),
                        _ => t_object(),
                    };
                    let innerc = self.cg.ct(&inner);
                    self.emit(format!("{} {} = {}.u.ok;", innerc, self.lv(bind), sv));
                } else {
                    self.emit(format!("ax_err {} = {}.u.err;", self.lv(bind), sv));
                }
                // `fail(e) => print("...")` 这类不用载荷的分支，变量仍要声明；
                // 加 (void) 让 -Werror 下也能过
                self.emit(format!("(void){};", self.lv(bind)));
            }
            Pat::Type { bind, .. } => {
                let pty = self
                    .cg
                    .slot(&p.id)
                    .and_then(|s| s.ty.clone())
                    .unwrap_or_else(t_object);
                let tc = self.cg.ct(&pty);
                // `object` 的箱内 payload 分两种：class 装箱按 sizeof(T*) 存指针本身，
                // 标量装箱直接存值（按 ax_box_new 的 tag 校验解箱）
                match &**st {
                    TyS::Object => {
                        let pa = self.pos_args(&p.tok);
                        let init = match &*pty {
                            TyS::Bool => format!("ax_unbox_bool({}, {})", sv, pa),
                            TyS::Int(bits, signed) => {
                                let raw = format!("ax_unbox_int({}, {})", sv, pa);
                                if *bits == 64 {
                                    raw
                                } else {
                                    format!(
                                        "ax_narrow({}, {}, {}, {})",
                                        raw,
                                        bits,
                                        if *signed { 1 } else { 0 },
                                        pa
                                    )
                                }
                            }
                            TyS::Float => format!("ax_unbox_f64({}, {})", sv, pa),
                            TyS::Decimal => format!("ax_unbox_dec({}, {})", sv, pa),
                            TyS::Str => format!("ax_unbox_str({}, {})", sv, pa),
                            _ => format!("({})(*(void**)({}->payload))", tc, sv),
                        };
                        let b = self.lv(bind);
                        self.emit(format!("{} {} = {};", tc, b, init));
                    }
                    _ => self.emit(format!("{} {} = ({}){};", tc, self.lv(bind), tc, sv)),
                }
            }
            Pat::Bind { name } => {
                let stc = self.cg.ct(st);
                self.emit(format!("{} {} = {};", stc, self.lv(name), sv));
            }
            Pat::Multi { .. } => {
                // 多值模式不绑定变量（子模式仅限非绑定模式）
            }
            _ => {}
        }
    }

    fn is_simple(&self, e: &'a ast::EPtr) -> bool {
        matches!(
            &e.k,
            Expr::Path { .. } | Expr::IntLit { .. } | Expr::StrLit { .. }
        )
    }
}
