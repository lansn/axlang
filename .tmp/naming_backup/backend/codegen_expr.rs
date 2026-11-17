//! FnGen 的表达式级 C 生成 + match 语句。
//!
//! 借用约定：所有节点都从 `&'a` 的 AST 借用（不 clone EPtr——clone 出来的
//! 盒子的生命周期只有函数栈帧，无法传给需要 `&'a` 的递归 gen）。

use crate::frontend::ast::{self, Expr, Pat, SPart, Stmt};
use crate::backend::{cstr, san, tag_of, AX_FAIL, AX_OK};
use crate::types::*;

use super::codegen_fn::{BinInfo, FnGen};

impl<'s, 'a, 'c> FnGen<'s, 'a, 'c> {
    // ---------- 统一出口 ----------

    /// 兼容入口：转换已统一到 gen 的出口，这里只做一次转发，不要重复 coerce。
    pub fn gen_to(&mut self, e: &'a ast::EPtr, want: &Ty) -> String {
        self.gen(e, Some(want), false, false)
    }

    fn coerce_c(&mut self, v: String, src: &Ty, dst: &Ty) -> String {
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
        v
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
                .s
                .slots
                .get(&e.id)
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
        self.coerce_c(v, &src, want)
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
            Expr::FloatLit { value, .. } => super::float_repr(*value),
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
                let r = self.gen(rhs, None, true, false);
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
                    .s
                    .slots
                    .get(&e.id)
                    .and_then(|sl| sl.func)
                    .unwrap_or(0);
                self.cg.fn_name_idx(fid)
            }
        }
    }

    pub fn path(&mut self, e: &'a ast::EPtr) -> String {
        let segs: &'a [String] = match &e.k {
            Expr::Path { segs, .. } => segs,
            _ => &[],
        };
        let res = self.res_of(e.id);
        match res {
            Some(crate::sema::Res::Local(_)) => {
                let (base, _v) = self.var_c(e);
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
                    "HalfUp" => "0".to_string(),
                    "HalfEven" => "1".to_string(),
                    "Down" => "2".to_string(),
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
                    match fmt {
                        Some(f) => match &*ty {
                            TyS::Decimal => {
                                self.emit(format!("ax_sb_push_dec_n(&{}, {}, {});", sb, val, f))
                            }
                            TyS::Float => {
                                self.emit(format!("ax_sb_push_f64_n(&{}, {}, {});", sb, val, f))
                            }
                            _ => self.emit(format!("ax_sb_push_int(&{}, {});", sb, val)),
                        },
                        None => match &*ty {
                            TyS::Str => self.emit(format!("ax_sb_push(&{}, {});", sb, val)),
                            TyS::Decimal => self.emit(format!(
                                "ax_sb_push(&{}, ax_dec_to_str({}));",
                                sb, val
                            )),
                            TyS::Float => {
                                self.emit(format!("ax_sb_push_f64(&{}, {});", sb, val))
                            }
                            TyS::Bool => self.emit(format!(
                                "ax_sb_push(&{}, ax_str_of_bool({}));",
                                sb, val
                            )),
                            TyS::Enum(_) => {
                                let en = self.enum_name_expr(expr, &val);
                                self.emit(format!("ax_sb_push(&{}, {});", sb, en))
                            }
                            TyS::Int(..) => self.emit(format!(
                                "ax_sb_push_int(&{}, (int64_t)({}));",
                                sb, val
                            )),
                            _ => self.emit(format!(
                                "ax_sb_push(&{}, ax_str_from_cstr(\"?\"));",
                                sb
                            )),
                        },
                    }
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
        self.cg.s.slots.get(&id).and_then(|s| s.folded.clone())
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
            dec_conv: self.cg.s.slots.get(&e.id).and_then(|s| s.dec_conv),
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
            .s
            .slots
            .get(&e.id)
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
            let nm = format!("{}__{}__new", self.cg.mod_name, self.cg.class_name(&td));
            let params: Vec<(String, Ty, Option<String>)> = match td.ctor {
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
                .s
                .slots
                .get(&e.id)
                .and_then(|s| s.parent_def)
                .unwrap_or(0);
            let ptd = self.cg.s.typedefs[pd as usize].clone();
            return format!(
                "{}__{}__init({})",
                self.cg.mod_name,
                self.cg.class_name(&ptd),
                parts.join(", ")
            );
        }
        if ck == "builtin" {
            let bname = self
                .cg
                .s
                .slots
                .get(&e.id)
                .and_then(|s| s.bname.clone())
                .unwrap_or_default();
            if bname == "print" {
                let v = self.gen(&args[0].k.expr, Some(&t_string()), false, false);
                self.emit(format!("ax_write_stdout({});", v));
                self.emit("ax_write_stdout(AX_S(\"\\n\"));".to_string());
                return "0".to_string();
            }
            return "0".to_string();
        }
        if ck == "func" {
            let fid = self
                .cg
                .s
                .slots
                .get(&e.id)
                .and_then(|s| s.func)
                .unwrap_or(0);
            let nm = self.cg.fn_name_idx(fid);
            let params = self.cg.s.funcs[fid].params.clone();
            let mut out = Vec::new();
            for (i, arg) in args.iter().enumerate() {
                let v = match params.get(i) {
                    Some(p) => self.gen(&arg.k.expr, Some(&p.1), false, false),
                    None => self.gen(&arg.k.expr, None, false, false),
                };
                out.push(v);
            }
            return format!("{}({})", nm, out.join(", "));
        }
        // 转换 / 构造外的情况
        self.convert_call(e)
    }

    fn convert_call(&mut self, e: &'a ast::EPtr) -> String {
        let args: &'a [ast::N<ast::Arg>] = match &e.k {
            Expr::Call { args, .. } => args,
            _ => &[],
        };
        let conv = self.cg.s.slots.get(&e.id).and_then(|s| s.conv);
        let arg = match args.first() {
            Some(a) => &a.k.expr,
            None => return "0".to_string(),
        };
        let src_ty = self.ty_or_int(arg.id);
        let cast_to = self
            .cg
            .s
            .slots
            .get(&e.id)
            .and_then(|s| s.cast_to.clone());
        let dst = cast_to.unwrap_or_else(|| self.ty_or_int(e.id));
        let v = match conv {
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
                let pa = self.pos_args(&e.tok);
                self.emit(format!(
                    "if ({} == NULL || {}->__tag != {}u) ax_trap(107, {}, \"downcast\");",
                    tv, tv, tid, pa
                ));
                let dstc = self.cg.ct(&dst);
                format!("({}){}", dstc, tv)
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
        let sl = self.cg.s.slots.get(&e.id).cloned().unwrap_or_default();
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
                "{}__{}__{}",
                self.cg.mod_name,
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
                "{}__{}__{}({})",
                self.cg.mod_name,
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
        if mk == "from_codepoint" {
            return self.from_codepoint(e);
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
        let vt_ty = format!("{}_VT", san(&od.name));
        format!(
            "((({}*){}->vt)->{})({})",
            vt_ty,
            tv,
            san(&fname),
            out.join(", ")
        )
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
            .s
            .slots
            .get(&e.id)
            .and_then(|s| s.bname.clone())
            .unwrap_or_default();
        match b.as_str() {
            "Row.empty" => format!("ax_row_empty({})", tv),
            "Row.get_string" => {
                let a = self.gen(&args[0].k.expr, None, false, false);
                format!("ax_row_get({}, {})", tv, a)
            }
            "Row.get_int" => {
                let a = self.gen(&args[0].k.expr, None, false, false);
                format!("str_to_i64(ax_row_get({}, {}))", tv, a)
            }
            "Row.get_decimal" => {
                let a = self.gen(&args[0].k.expr, None, false, false);
                format!(
                    "ax_dec_parse_or(ax_row_get({}, {}), ax_dec_from_i64(0))",
                    tv, a
                )
            }
            "File.close" => {
                self.emit("/* file.close */".to_string());
                "0".to_string()
            }
            "File.read_all" => {
                let ty = self.ty_or_int(e.id);
                let tyc = self.cg.ct(&ty);
                format!(
                    "(({}){{ .tag = 0, .u.err = ax_err_new(4, AX_S(\"file not supported\")) }})",
                    tyc
                )
            }
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
            let sl = self.cg.s.slots.get(&e.id).cloned().unwrap_or_default();
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
                let params: Vec<(String, Ty, Option<String>)> = match td.ctor {
                    Some(c) => self.cg.s.funcs[c].params.clone(),
                    None => Vec::new(),
                };
                let nm = format!(
                    "{}__{}__new",
                    self.cg.mod_name,
                    self.cg.class_name(&td)
                );
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
            Expr::Path { segs, .. } => segs.first().cloned().unwrap_or_default(),
            _ => String::new(),
        };
        let mut a: Vec<String> = Vec::new();
        for x in args {
            let v = self.gen(&x.k.expr, None, false, false);
            a.push(v);
        }
        match (module.as_str(), name.as_str()) {
            ("io", "read_line") => "ax_read_line()".to_string(),
            ("io", "write") => {
                self.emit(format!("ax_write_stdout({});", a[0]));
                "0".to_string()
            }
            ("io", "read_file") => format!("ax_read_file({})", a[0]),
            ("math", "equals") => {
                format!("(fabs(({}) - ({})) <= ({}))", a[0], a[1], a[2])
            }
            ("math", "compare") => format!(
                "((({0}) < ({1})) ? -1 : ((({0}) > ({1})) ? 1 : 0))",
                a[0], a[1]
            ),
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
            ("file", "open") => {
                let ty = self.ty_or_int(e.id);
                let tyc = self.cg.ct(&ty);
                format!(
                    "(({}){{ .tag = 0, .u.err = ax_err_new(4, AX_S(\"file not supported\")) }})",
                    tyc
                )
            }
            ("time", "now_ms") => "ax_now_ms()".to_string(),
            _ => "0".to_string(),
        }
    }

    fn intrinsic(&mut self, e: &'a ast::EPtr) -> String {
        let (recv, name, args): (&'a ast::EPtr, &'a String, &'a [ast::N<ast::Arg>]) = match &e.k
        {
            Expr::MethodCall { recv, name, args } => (recv, name, args),
            _ => unreachable!(),
        };
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
                match name.as_str() {
                    "len" => format!("ax_str_len_cp({})", tv),
                    "byte_len" => format!("({}).len", tv),
                    "to_string" => tv,
                    "trim" => format!("ax_str_trim({})", tv),
                    "contains" => format!("ax_str_contains({}, {})", tv, a[0]),
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
                    "codepoint_at" => {
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
                        let i = self.tmpn();
                        self.emit(format!("int64_t {} = 0;", i));
                        self.emit(format!(
                            "ax_str* {0}_raw = ax_str_split({1}, {2}, &{0});",
                            n, tv, a[0]
                        ));
                        self.emit(format!(
                            "ax_arr {0}; ax_arr_from(&{0}, {0}_raw, {1}, (int64_t)sizeof(ax_str));",
                            n, i
                        ));
                        n
                    }
                    _ => "0".to_string(),
                }
            }
            TyS::Array(et, _) => {
                let et = et.clone();
                let tv = self.declare(&rt, &rv);
                match name.as_str() {
                    "len" => format!("{}.len", tv),
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
                    "clear" => {
                        self.emit(format!("ax_arr_clear(&{});", tv));
                        "0".to_string()
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
                    "to_string" => "AX_S(\"[array]\")".to_string(),
                    _ => "0".to_string(),
                }
            }
            TyS::Map(kt, vt) => {
                let kt = kt.clone();
                let vt = vt.clone();
                let tv = self.declare(&rt, &rv);
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
                    _ => "0".to_string(),
                }
            }
            TyS::Decimal => {
                let tv = self.declare(&rt, &rv);
                match name.as_str() {
                    "to_string" => format!("ax_dec_to_str({})", tv),
                    "round" => format!(
                        "ax_dec_round({}, (int32_t)({}), (int)({}))",
                        tv, a[0], a[1]
                    ),
                    "divide" => format!(
                        "ax_dec_div_round({}, {}, (int32_t)({}), (int)({}))",
                        tv, a[0], a[1], a[2]
                    ),
                    _ => "0".to_string(),
                }
            }
            TyS::Int(..) => {
                let tv = self.declare(&rt, &rv);
                match name.as_str() {
                    "to_string" => format!("ax_str_of_int((int64_t){})", tv),
                    "wrapping_add" => {
                        format!("(int64_t)((uint64_t){} + (uint64_t)({}))", tv, a[0])
                    }
                    _ => "0".to_string(),
                }
            }
            TyS::Float => {
                let tv = self.declare(&rt, &rv);
                match name.as_str() {
                    "to_string" => format!("ax_str_of_f64({})", tv),
                    _ => "0".to_string(),
                }
            }
            TyS::Bool => {
                let tv = self.declare(&rt, &rv);
                match name.as_str() {
                    "to_string" => format!("ax_str_of_bool({})", tv),
                    _ => "0".to_string(),
                }
            }
            TyS::Enum(_) => {
                let tv = self.declare(&rt, &rv);
                match name.as_str() {
                    "to_string" => self.enum_name_expr(recv, &tv),
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
        let sl = self.cg.s.slots.get(&e.id).cloned().unwrap_or_default();
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
                self.emit(format!(
                    "ax_dec {0} = ax_dec_from_i64(0); int ok_ = 0; {0} = ax_dec_parse({1}, &ok_);",
                    ov, s
                ));
                self.emit(format!(
                    "if (ok_) {{ {0}.tag = 1; {0}.u.ok = {1}; }} else {{ {0}.tag = 0; {0}.u.err = ax_err_new(2, AX_S(\"parse failed\")); }}",
                    n, ov
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
                return self.cg.fn_name_idx(fid);
            }
            Some(crate::sema::Res::Rounding(n)) => {
                return match n.as_str() {
                    "HalfUp" => "0".to_string(),
                    "HalfEven" => "1".to_string(),
                    "Down" => "2".to_string(),
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
                    .s
                    .slots
                    .get(&e.id)
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
                format!(
                    "*({}*)ax_map_get_chk(&{}, &{}, {}, {})",
                    vtc,
                    m,
                    kv,
                    pa,
                    cstr("key")
                )
            }
            TyS::Array(et, len) => {
                let et = et.clone();
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
                    "*({0}*)ax_arr_at(&{1}, {2}, (int64_t)sizeof({0}), {3})",
                    etc, m, iv, pa
                )
            }
            _ => "0".to_string(),
        }
    }

    fn new_expr(&mut self, e: &'a ast::EPtr) -> String {
        let args: &'a [ast::N<ast::Arg>] = match &e.k {
            Expr::New { args, .. } => args,
            _ => &[],
        };
        let ck = self
            .cg
            .s
            .slots
            .get(&e.id)
            .and_then(|s| s.ckind)
            .unwrap_or("");
        if ck == "arena" {
            return "NULL".to_string();
        }
        let ty = self.ty_or_int(e.id);
        let d: u32 = match &*ty {
            TyS::Named(DefId::Id(d), _) => *d,
            TyS::Enum(d) => *d,
            _ => 0,
        };
        let td = self.cg.s.typedefs[d as usize].clone();
        let nm = format!("{}__{}__new", self.cg.mod_name, self.cg.class_name(&td));
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
            .s
            .slots
            .get(&node_id)
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
                    _ => {
                        self.emit("default: {".to_string());
                    }
                }
                self.ind += 1;
                if let Some(ov) = out_var {
                    let val = self.gen(&arm.body, Some(want.unwrap()), false, false);
                    self.emit(format!("{} = {};", ov, val));
                } else {
                    self.gen(&arm.body, None, false, true);
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
                self.gen(&arm.body, None, false, true);
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
                    .s
                    .slots
                    .get(&p.id)
                    .and_then(|s| s.ty.clone())
                    .unwrap_or_else(t_object);
                let tid = tag_of(&pty);
                Some(format!("({0} != NULL && {0}->__tag == {1}u)", sv, tid))
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
                    .s
                    .slots
                    .get(&p.id)
                    .and_then(|s| s.ty.clone())
                    .unwrap_or_else(t_object);
                let tc = self.cg.ct(&pty);
                self.emit(format!("{} {} = ({}){};", tc, self.lv(bind), tc, sv));
            }
            Pat::Bind { name } => {
                let stc = self.cg.ct(st);
                self.emit(format!("{} {} = {};", stc, self.lv(name), sv));
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
