//! 语义检查——表达式级：check_expr 及下游、函数调用、coerce（含 try 块内
//! result<T> 的隐式展开）、常量求值入口与 `module_sig`（模块签名，供 member.rs 复用）。
//! 成员访问 / 方法调用 / match / eval_const 的实现在 `member.rs`。

use crate::frontend::ast;
use crate::frontend::ast::Expr;
use crate::sema::{is_builtin_type_name, is_reserved, ConstVal, ExportSym, Res, Sema, VarState};
use crate::types::*;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub fn module_sig(mod_: &str, name: &str) -> Option<(Vec<Option<Ty>>, Ty)> {
    let t = |k: TyS| -> Ty { Rc::new(k) };
    Some(match (mod_, name) {
        ("io", "ReadLine") => (vec![], t(TyS::Nullable(t_string()))),
        ("io", "ReadFile") => (vec![Some(t_string())], t_result(t_string())),
        ("math", "Equals") => (
            vec![Some(t_float()), Some(t_float()), Some(t_float())],
            t_bool(),
        ),
        ("math", "Compare") => (vec![Some(t_float()), Some(t_float())], t_int()),
        ("json", "Parse") => (
            vec![Some(t_string())],
            t_result(t_map(t_string(), t_object())),
        ),
        ("json", "stringify") => (vec![Some(t_object())], t_string()),
        ("db", "Query") => (
            vec![Some(t_string())],
            t_result(t_named(DefId::Name("Row"), Vec::new())),
        ),
        ("file", "Open") => (
            vec![Some(t_string())],
            t_result(t_named(DefId::Name("File"), Vec::new())),
        ),
        ("file", "Create") => (
            vec![Some(t_string())],
            t_result(t_named(DefId::Name("File"), Vec::new())),
        ),
        ("file", "Append") => (
            vec![Some(t_string())],
            t_result(t_named(DefId::Name("File"), Vec::new())),
        ),
        ("string", "FromCodepoint") => (vec![Some(t_int())], t_result(t_string())),
        ("time", "now_ms") => (vec![], t_int()),
        _ => return None,
    })
}

impl<'a> Sema<'a> {
    // ---------------------------------------------------------------- 入口

    pub fn check_expr(&mut self, e: &ast::EPtr, expected: Option<&Ty>) -> Ty {
        let ty = self.check_expr_inner(e, expected);
        self.sl(e.id).ty = Some(ty.clone());
        ty
    }

    fn check_expr_inner(&mut self, e: &ast::EPtr, expected: Option<&Ty>) -> Ty {
        match &e.k {
            Expr::IntLit { .. } => self.check_int_lit(e, expected),
            Expr::FloatLit { .. } => {
                if let Some(exp) = expected {
                    // 目标写 `decimal?` 时按内层 `decimal` 判：能不能落地与"可不可空"无关
                    let base = strip_nullable(exp);
                    if matches!(&**base, TyS::Decimal) {
                        self.err("E4003", &e.tok, "T-9", "float 字面量不能直接赋给 decimal（C-6）；金额请写 `19.99d`", &[]);
                        return exp.clone();
                    }
                    if !matches!(&**base, TyS::Float) {
                        if is_int(base) {
                            self.err("E4001", &e.tok, "T-5", "类型不匹配：期望 {0}，得到 float", &[&self.ts(exp)]);
                            return exp.clone();
                        }
                    }
                }
                t_float()
            }
            Expr::DecLit { .. } => {
                if let Some(exp) = expected {
                    if !matches!(&**strip_nullable(exp), TyS::Decimal) {
                        self.err("E4003", &e.tok, "T-9", "decimal 字面量只能赋给 decimal", &[]);
                        return exp.clone();
                    }
                }
                t_decimal()
            }
            Expr::BoolLit { .. } => t_bool(),
            Expr::NullLit => t_null(),
            Expr::StrLit { parts } => {
                for p in parts {
                    if let ast::SPart::Interp { expr, fmt } = p {
                        let errs_before = self.d.error_count;
                        let pt = self.check_expr(expr, None);
                        // 名字没解析成功（E3003 / E3030 已报）时不再追问「类型能不能插值」，
                        // 否则真正的错因被后一条诊断盖住（一处误用只报一条）。
                        if self.d.error_count > errs_before {
                            continue;
                        }
                        if let (Some(n), TyS::Nullable(_)) = (fmt, &*pt) {
                            // 可空数值本身是"数值或 null"，`:.N` 没有定义 null 怎么舍入——
                            // 提示先判空取出，而不是含糊地说"只能用于数值类型"。
                            self.err("E4030", &expr.tok, "文稿 11.2", "可空值 `{0}` 不能直接加格式说明符 `:.{1}`；先判空取出再格式化", &[&self.ts(&pt), &n]);
                        } else if fmt.is_some() && !is_numeric(&pt) {
                            self.err("E4030", &expr.tok, "文稿 11.2", "格式说明符 `:.{0}` 只能用于数值类型，得到 {1}", &[&fmt.unwrap(), &self.ts(&pt)]);
                        }
                        if matches!(&*pt, TyS::Object) {
                            self.err("E5002", &expr.tok, "T-10", "插值里不能直接用 object；先落成具体类型", &[]);
                        } else if !interp_supported(&pt) {
                            let t = self.ts(&pt);
                            self.err("E5006", &expr.tok, "文稿 11.2", "插值暂不支持 `{0}` 类型；先取其中的标量或自己拼接", &[&t]);
                        }
                    }
                }
                t_string()
            }
            Expr::Path { .. } => self.check_path(e),
            Expr::This => {
                let cc = match self.cur_class {
                    None => {
                        self.err("E3034", &e.tok, "G-8", "`this` 只能在 class/struct 方法体内使用", &[]);
                        return t_object();
                    }
                    Some(c) => c,
                };
                // 静态方法体内没有 this（文稿 6.1）：静态方法不属于任何对象。
                let in_static = self
                    .current_func
                    .map(|f| self.funcs[f].is_static)
                    .unwrap_or(false);
                if in_static {
                    self.err("E3038", &e.tok, "N-21", "静态方法体内没有 `this`（静态方法不属于任何对象）\n  = 访问实例成员请改为实例方法，或通过参数把实例传进来", &[]);
                    // 仍返回本类类型，避免下游级联报错（一处误用恰好一条诊断）
                    let gs = self.typedefs[cc as usize].generics.clone();
                    return t_named(
                        DefId::Id(cc),
                        gs.iter()
                            .map(|(g, _)| Rc::new(TyS::Param(g.clone())) as Ty)
                            .collect(),
                    );
                }
                let gs = self.typedefs[cc as usize].generics.clone();
                t_named(
                    DefId::Id(cc),
                    gs.iter()
                        .map(|(g, _)| Rc::new(TyS::Param(g.clone())) as Ty)
                        .collect(),
                )
            }
            Expr::Parent => {
                let p = match self.cur_class.and_then(|c| self.typedefs[c as usize].parent) {
                    None => {
                        self.err("E3034", &e.tok, "G-8", "`parent` 只能用于有父类的类内", &[]);
                        return t_object();
                    }
                    Some(p) => p,
                };
                t_named(DefId::Id(p), Vec::new())
            }
            Expr::TupleLit { elems } => Rc::new(TyS::Tuple(
                elems.iter().map(|x| self.check_expr(x, None)).collect(),
            )),
            Expr::ArrayLit { .. } => self.check_array_lit(e, expected),
            Expr::BraceLit { .. } => self.check_brace_lit(e, expected),
            Expr::Unary { .. } => self.check_unary(e, expected),
            Expr::Binary { .. } => {
                let (op, lhs, rhs) = match &e.k {
                    Expr::Binary { op, lhs, rhs } => (op.clone(), lhs, rhs),
                    _ => unreachable!(),
                };
                self.check_binary(e.id, &op, lhs, rhs)
            }
            Expr::Ternary { cond, then_, els } => {
                self.check_cond(cond, "三目");
                let tt = self.check_expr(then_, expected);
                let et = self.check_expr(els, expected);
                if tt != et && !matches!(&*tt, TyS::Never) && !matches!(&*et, TyS::Never) {
                    self.err("E4013", &e.tok, "文稿 5.1", "三目两分支类型不一致：{0} vs {1}", &[&self.ts(&tt), &self.ts(&et)]);
                    return tt;
                }
                if matches!(&*tt, TyS::Never) {
                    et
                } else {
                    tt
                }
            }
            Expr::Coalesce { lhs, rhs } => {
                let lt = self.check_expr(lhs, None);
                let inner = if matches!(&*lt, TyS::Nullable(_) | TyS::Null) {
                    match &*lt {
                        TyS::Nullable(i) => i.clone(),
                        _ => t_object(),
                    }
                } else {
                    self.err("E4022", &lhs.tok, "T-17", "`??` 左侧必须是可空值，得到 {0}", &[&self.ts(&lt)]);
                    lt.clone()
                };
                let rt = self.check_expr(rhs, Some(&inner));
                let r = assignable(&rt, &inner, self, true);
                if matches!(r, AssignResult::No(_)) {
                    self.err("E4015", &rhs.tok, "T-17", "`??` 右侧类型 {0} 不能赋给 {1}", &[&self.ts(&rt), &self.ts(&inner)]);
                }
                inner
            }
            Expr::Range { lo, hi, .. } => {
                let lt = self.check_expr(lo, Some(&t_int()));
                let ht = self.check_expr(hi, Some(&t_int()));
                if !is_int(&lt) || !is_int(&ht) {
                    self.err("E4009", &e.tok, "文稿 5.2", "range 两端必须是 int", &[]);
                }
                t_range()
            }
            Expr::Try { .. } => self.check_try(e),
            Expr::Call { .. } => self.check_call(e, expected),
            Expr::MethodCall { .. } => self.check_method_call(e, expected),
            Expr::FieldAccess { .. } => self.check_field_access(e, expected),
            Expr::SafeAccess { .. } => self.check_safe_access(e, expected),
            Expr::SafeCall { .. } => self.check_safe_call(e, expected),
            Expr::Index { .. } => self.check_index(e, expected),
            Expr::New { .. } => self.check_new(e),
            Expr::If(n) => {
                self.check_if(e.id, &e.tok, n, expected);
                return self.ty_of(e.id).unwrap_or_else(t_void);
            }
            Expr::Match(n) => {
                self.check_match(e.id, &e.tok, n, expected);
                return self.ty_of(e.id).unwrap_or_else(t_void);
            }
            Expr::AnonFunc { params, body } => {
                // 文稿 4.1：匿名函数是 func(...) 类型的值。需要上下文类型
                // （赋值目标 / 实参）来确定参数与返回类型，然后按普通函数
                // 完整检查函数体；codegen 为它生成一个合成的顶层函数。
                let (ps, ret) = match expected {
                    Some(t) if matches!(&**t, TyS::Func(..)) => match &**t {
                        TyS::Func(wps, wret) => (wps.clone(), wret.clone()),
                        _ => unreachable!(),
                    },
                    _ => {
                        self.err("E4043", &e.tok, "文稿 4.1", "匿名函数需要目标类型：先声明 `func(参数) 返回` 类型的变量或参数，再把匿名函数赋给它\n  = 例：`func(int) int f = (int x) { return x; }`", &[]);
                        return t_object();
                    }
                };
                if params.len() != ps.len() {
                    self.err("E4001", &e.tok, "文稿 4.1", "匿名函数有 {0} 个参数，目标类型需要 {1} 个", &[&params.len(), &ps.len()]);
                }
                // 合成 FuncInfo：不进 func_by_name（不可按名调用），进 anon_funcs。
                // body 从短命借用提升为 'a：clone 后 leak（编译器进程内可接受，
                // 总量受源程序规模约束）
                let fname = self.reg_name(&format!("anon{}", self.anon_funcs.len() + 1));
                let leaked: &'a ast::Block = Box::leak(Box::new(body.clone()));
                let f = self.funcs.len();
                self.funcs.push(crate::sema::FuncInfo {
                    name: fname,
                    tok: e.tok.clone(),
                    module: self.cur_mod_path.clone(),
                    owner: None,
                    params: params
                        .iter()
                        .zip(ps.iter())
                        .map(|(p, t)| (p.k.name.clone(), t.clone(), None))
                        .collect(),
                    ret: ret.clone(),
                    generics: Vec::new(),
                    vis: "private".to_string(),
                    is_static: false,
                    is_override: false,
                    implicit: false,
                    body: Some(leaked),
                });
                self.anon_funcs.push(f);
                self.check_func(f);
                let fret = self.funcs[f].ret.clone();
                self.sl(e.id).func = Some(f);
                t_fn(ps, fret)
            }
        }
    }

    fn check_int_lit(&mut self, e: &ast::EPtr, expected: Option<&Ty>) -> Ty {
        let v = match &e.k {
            Expr::IntLit { value, .. } => *value,
            _ => unreachable!(),
        };
        if let Some(exp) = expected {
            if is_int(exp) {
                let (lo, hi) = int_range(exp);
                let vv = v as i128;
                if !(lo <= vv && vv <= hi) {
                    self.err("E4003", &e.tok, "T-9", "字面量 {0} 超出 {1} 的范围 [{2}, {3}]", &[&v, &self.ts(exp), &lo, &hi]);
                }
                return exp.clone();
            }
            if matches!(&**exp, TyS::Decimal) {
                // T-9 的推广：整数字面量在需要 decimal 的位置直接落地为 decimal
                self.sl(e.id).as_dec = true;
                return t_decimal();
            }
            if matches!(&**exp, TyS::Float | TyS::Decimal) {
                self.err("E4003", &e.tok, "T-8", "需要显式转换：写 `{0}({1})`", &[&self.ts(exp), &v]);
                return exp.clone();
            }
        }
        let (lo, hi) = int_range(&t_int());
        let vv = v as i128;
        if !(lo <= vv && vv <= hi) {
            self.err("E4003", &e.tok, "T-9", "整数字面量 {0} 超出 int 范围", &[&v]);
        }
        t_int()
    }

    fn check_array_lit(&mut self, e: &ast::EPtr, expected: Option<&Ty>) -> Ty {
        let elems = match &e.k {
            Expr::ArrayLit { elems } => elems,
            _ => unreachable!(),
        };
        let mut elem_ty: Option<Ty> = None;
        let mut fixed: Option<i64> = None;
        if let Some(exp) = expected {
            if let TyS::Array(et, fl) = &**exp {
                elem_ty = Some(et.clone());
                fixed = *fl;
            }
        }
        if elems.is_empty() {
            return match elem_ty {
                None => {
                    self.err("E4007", &e.tok, "T-15", "空数组 `[]` 需要目标类型", &[]);
                    t_array(t_object(), None)
                }
                Some(et) => t_array(et, fixed),
            };
        }
        for x in elems {
            self.check_expr(x, elem_ty.as_ref());
        }
        match &elem_ty {
            None => {
                let first = self.ty_of(elems[0].id).unwrap_or_else(t_object);
                for x in &elems[1..] {
                    let xt = self.ty_of(x.id).unwrap_or_else(t_object);
                    let r = assignable(&xt, &first, self, true);
                    if matches!(r, AssignResult::No(_)) {
                        self.err("E4001", &x.tok, "T-15", "数组元素类型不一致：{0} vs {1}", &[&self.ts(&first), &self.ts(&xt)]);
                        break;
                    }
                }
                elem_ty = Some(first);
            }
            Some(et) => {
                for x in elems {
                    self.coerce(x, et, "数组元素");
                }
            }
        }
        let elem_ty = elem_ty.unwrap();
        if let Some(f) = fixed {
            if elems.len() as i64 != f {
                self.err("E4001", &e.tok, "T-15", "定长数组 `{0}` 需要 {1} 个元素，得到 {2}", &[&self.ts(&elem_ty), &f, &elems.len()]);
                return t_array(elem_ty, None);
            }
        }
        t_array(elem_ty, fixed)
    }

    fn check_brace_lit(&mut self, e: &ast::EPtr, expected: Option<&Ty>) -> Ty {
        let pairs = match &e.k {
            Expr::BraceLit { pairs } => pairs,
            _ => unreachable!(),
        };
        if pairs.is_empty() {
            return match expected {
                None => {
                    self.err("E4007", &e.tok, "T-14", "空 `{}` 需要目标类型", &[]);
                    Rc::new(TyS::Record(Vec::new()))
                }
                Some(exp) => {
                    if !matches!(&**exp, TyS::Map(..) | TyS::Record(_)) {
                        self.err("E4001", &e.tok, "T-14", "`{0}` 不能赋给 {1}", &[&"{}", &self.ts(exp)]);
                    }
                    exp.clone()
                }
            };
        }
        // 有目标类型：按目标逐值检查
        if let Some(exp) = expected {
            if let TyS::Map(_, vt) = &**exp {
                for (_, v) in pairs {
                    self.check_expr(v, Some(vt));
                    self.coerce(v, vt, "map 值");
                }
                return exp.clone();
            }
            if let TyS::Record(fields) = &**exp {
                let fmap: HashMap<&String, &Ty> = fields.iter().map(|(k, t)| (k, t)).collect();
                for (k, v) in pairs {
                    match fmap.get(k) {
                        None => {
                            self.err("E4008", &v.tok, "T-26", "record 没有键 `{0}`（键名固定；要加键请用 map）", &[&k]);
                            self.check_expr(v, None);
                        }
                        Some(ft) => {
                            let ft = (*ft).clone();
                            self.check_expr(v, Some(&ft));
                            self.coerce(v, &ft, "record 字段");
                        }
                    }
                }
                return exp.clone();
            }
        }
        let mut types: Vec<Ty> = Vec::new();
        for (_, v) in pairs {
            types.push(self.check_expr(v, None));
        }
        let first = types[0].clone();
        if types.iter().all(|t| *t == first) {
            for (_, v) in pairs {
                self.coerce(v, &first, "map 值");
            }
            return t_map(t_string(), first);
        }
        let mut fields: Vec<(String, Ty)> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for ((k, v), t) in pairs.iter().zip(types.iter()) {
            if seen.contains(k) {
                self.err("E4008", &v.tok, "T-26", "record 字面量重复键 `{0}`", &[&k]);
                continue;
            }
            seen.insert(k.clone());
            fields.push((k.clone(), t.clone()));
        }
        Rc::new(TyS::Record(fields))
    }

    fn check_unary(&mut self, e: &ast::EPtr, expected: Option<&Ty>) -> Ty {
        let (op, operand) = match &e.k {
            Expr::Unary { op, operand } => (op.clone(), operand),
            _ => unreachable!(),
        };
        if op == "++" || op == "--" {
            self.err("E2010", &e.tok, "文稿 11.1", "不支持前缀 `{0}`", &[&op]);
            return t_int();
        }
        if op == "-" {
            if let Expr::IntLit { value, .. } = &operand.k {
                if let Some(exp) = expected {
                    if is_int(exp) {
                        let v = value.wrapping_neg();
                        self.sl(e.id).folded = Some(ConstVal::Int(v));
                        let (lo, hi) = int_range(exp);
                        let vv = v as i128;
                        if !(lo <= vv && vv <= hi) {
                            // 历史行为（测试钉死）：v == -2^63 时按空操作处理，错误照报
                            self.err("E4003", &e.tok, "T-9", "字面量 {0} 超出 {1} 的范围 [{2}, {3}]", &[&v, &self.ts(exp), &lo, &hi]);
                        }
                        return exp.clone();
                    }
                }
            }
            let t = self.check_expr(operand, None);
            if let TyS::Int(bits, signed) = &*t {
                if !*signed {
                    self.err("E4009", &e.tok, "文稿 11.1", "无符号整数不能取负", &[]);
                }
                let _ = bits;
                return t;
            }
            if matches!(&*t, TyS::Float | TyS::Decimal) {
                return t;
            }
            self.err("E4009", &e.tok, "文稿 11.1", "`-` 只能用于数值，得到 {0}", &[&self.ts(&t)]);
            return t_int();
        }
        if op == "!" {
            let t = self.check_expr(operand, Some(&t_bool()));
            if t != t_bool() {
                self.err("E4009", &e.tok, "文稿 11.1", "`!` 只能用于 bool，得到 {0}", &[&self.ts(&t)]);
            }
            return t_bool();
        }
        if op == "~" {
            let t = self.check_expr(operand, None);
            if !is_int(&t) {
                self.err("E4009", &e.tok, "文稿 11.1", "`~` 只能用于整数，得到 {0}", &[&self.ts(&t)]);
                return t_int();
            }
            return t;
        }
        t_int()
    }

    pub fn check_binary(&mut self, eid: u32, op: &str, lhs: &ast::EPtr, rhs: &ast::EPtr) -> Ty {
        if op == "&&" || op == "||" {
            let lt = self.check_expr(lhs, Some(&t_bool()));
            if matches!(&*lt, TyS::Object) {
                self.err("E5002", &lhs.tok, "T-10", "`object` 不能直接参与逻辑运算；先落到 bool", &[]);
            } else if lt != t_bool() {
                self.err("E4002", &lhs.tok, "文稿 11.1", "`{0}` 的操作数必须是 bool，得到 {1}", &[&op, &self.ts(&lt)]);
            }
            let saved = self.cond_narrow(lhs, op == "&&");
            self.narrow.push(saved);
            let rt = self.check_expr(rhs, Some(&t_bool()));
            self.narrow.pop();
            if matches!(&*rt, TyS::Object) {
                self.err("E5002", &rhs.tok, "T-10", "`object` 不能直接参与逻辑运算", &[]);
            } else if rt != t_bool() {
                self.err("E4002", &rhs.tok, "文稿 11.1", "`{0}` 的操作数必须是 bool，得到 {1}", &[&op, &self.ts(&rt)]);
            }
            return t_bool();
        }
        let lt = self.check_expr(lhs, None);
        let rt = self.check_expr(
            rhs,
            if op == "==" || op == "!=" {
                Some(&lt)
            } else {
                None
            },
        );
        if matches!(&*lt, TyS::Object) {
            self.err("E5002", &lhs.tok, "T-10", "`object` 不能直接参与运算；先落到具体类型的变量，或写 `int(o)`", &[]);
            return t_object();
        }
        if matches!(&*rt, TyS::Object) {
            self.err("E5002", &rhs.tok, "T-10", "`object` 不能直接参与运算；先落到具体类型的变量，或写 `int(o)`", &[]);
            return t_object();
        }
        if op == "==" || op == "!=" {
            if matches!(&*lt, TyS::Null) || matches!(&*rt, TyS::Null) {
                let other = if matches!(&*lt, TyS::Null) { rt.clone() } else { lt.clone() };
                let ok = matches!(&*other, TyS::Nullable(_) | TyS::Object) || self.is_ref(&other);
                if !ok {
                    self.err("E4001", &lhs.tok, "T-19", "`null` 只能与可空类型或引用类型比较", &[]);
                }
                return t_bool();
            }
            if matches!(&*lt, TyS::Float) || matches!(&*rt, TyS::Float) {
                self.err("E4011", &lhs.tok, "文稿 3.7", "`float` 不能用 `{0}`（浮点比较请用 math.equals）", &[&op]);
                return t_bool();
            }
            if lt != rt {
                let r = assignable(&rt, &lt, self, false);
                if matches!(r, AssignResult::No(_)) {
                    self.err("E4001", &lhs.tok, "T-5", "`{0}` 两边类型不一致：{1} vs {2}", &[&op, &self.ts(&lt), &self.ts(&rt)]);
                }
                return t_bool();
            }
            if matches!(&*lt, TyS::Tuple(_)) {
                self.err("E4020", &lhs.tok, "T-37", "元组不能用 `{0}`", &[&op]);
            }
            if matches!(&*lt, TyS::Func(..)) {
                self.err("E4024", &lhs.tok, "文稿 4.1", "函数值不能用 `{0}`", &[&op]);
            }
            return t_bool();
        }
        if matches!(op, "<" | "<=" | ">" | ">=") {
            if lt != rt {
                self.err("E4001", &lhs.tok, "T-5", "比较两边类型不一致：{0} vs {1}", &[&self.ts(&lt), &self.ts(&rt)]);
                return t_bool();
            }
            if matches!(&*lt, TyS::Float) {
                self.err("E4017", &lhs.tok, "文稿 7.1", "`float` 不实现 Comparable（浮点比较请用 math.compare）", &[]);
            } else if !matches!(&*lt, TyS::Int(..) | TyS::Decimal | TyS::Str) {
                self.err("E4017", &lhs.tok, "文稿 7.1", "{0} 不实现 Comparable", &[&self.ts(&lt)]);
            } else if let TyS::Param(pn) = &*lt {
                let cons = self.generic_cons.get(pn).cloned().unwrap_or_default();
                if !cons.iter().any(|c| c == "Comparable") {
                    self.err("E4036", &lhs.tok, "T-32", "类型参数 `{0}` 缺少 `Comparable` 约束", &[&pn]);
                }
            }
            return t_bool();
        }
        if op == "+" {
            if matches!(&*lt, TyS::Str) || matches!(&*rt, TyS::Str) {
                if matches!(&*lt, TyS::Param(_)) || matches!(&*rt, TyS::Param(_)) {
                    return t_string();
                }
                if !(matches!(&*lt, TyS::Str) && matches!(&*rt, TyS::Str)) {
                    self.err("E4010", &lhs.tok, "文稿 11.2", "`+` 两边必须都是 string（拼接）；数值请用 `{x}{y}` 插值", &[]);
                }
                return t_string();
            }
        }
        if matches!(op, "+" | "-" | "*" | "/" | "%") {
            if is_int(&lt) && is_int(&rt) {
                if lt != rt {
                    self.err("E4003", &lhs.tok, "T-8", "整数类型不一致：{0} 与 {1}（无隐式转换，请写 `{2}(x)`）", &[&self.ts(&lt), &self.ts(&rt), &self.ts(&lt)]);
                }
                if (op == "/" || op == "%")
                    && matches!(&rhs.k, Expr::IntLit { value: 0, .. })
                {
                    self.err("E4009", &lhs.tok, "S-3", "常量除零", &[]);
                }
                return lt;
            }
            if matches!(&*lt, TyS::Float) && matches!(&*rt, TyS::Float) {
                if op == "%" {
                    self.err("E4009", &lhs.tok, "文稿 11.1", "`%` 只能用于整数", &[]);
                }
                return t_float();
            }
            if matches!(&*lt, TyS::Decimal) && matches!(&*rt, TyS::Decimal) {
                if op == "%" {
                    self.err("E4009", &lhs.tok, "文稿 3.2", "`decimal` 不支持 `%`（除不尽会崩），请用 `a - trunc(a/b)*b`", &[]);
                }
                if op == "/" {
                    if let Expr::DecLit { coeff, .. } = &rhs.k {
                        if *coeff == 0 {
                            self.err("E4009", &lhs.tok, "S-3", "常量除零", &[]);
                        }
                    }
                }
                return t_decimal();
            }
            // decimal 与整数混算：按 C-3「整数 → decimal 允许（无损）」处理
            let dec_int = (matches!(&*lt, TyS::Decimal) && is_int(&rt))
                || (is_int(&lt) && matches!(&*rt, TyS::Decimal));
            if dec_int {
                if op == "%" {
                    self.err("E4009", &lhs.tok, "文稿 3.2", "`decimal` 不支持 `%`（除不尽会崩），请用 `a - trunc(a/b)*b`", &[]);
                    return t_decimal();
                }
                self.sl(eid).dec_conv = Some(if matches!(&*lt, TyS::Decimal) {
                    "rhs"
                } else {
                    "lhs"
                });
                return t_decimal();
            }
            if matches!(&*lt, TyS::Param(_)) || matches!(&*rt, TyS::Param(_)) {
                return if matches!(&*lt, TyS::Param(_)) { lt } else { rt };
            }
            self.err("E4009", &lhs.tok, "文稿 11.1", "`{0}` 的操作数类型不允许：{1} 与 {2}", &[&op, &self.ts(&lt), &self.ts(&rt)]);
            return lt;
        }
        if matches!(op, "&" | "|" | "^") {
            if !(is_int(&lt) && is_int(&rt)) {
                self.err("E4009", &lhs.tok, "文稿 11.1", "`{0}` 只能用于整数，得到 {1} 与 {2}", &[&op, &self.ts(&lt), &self.ts(&rt)]);
                return t_int();
            }
            if lt != rt {
                self.err("E4003", &lhs.tok, "T-8", "整数类型不一致：{0} 与 {1}", &[&self.ts(&lt), &self.ts(&rt)]);
            }
            return lt;
        }
        if matches!(op, "<<" | ">>") {
            if !is_int(&lt) || !is_int(&rt) {
                self.err("E4009", &lhs.tok, "文稿 11.1", "移位只能用于整数", &[]);
                return t_int();
            }
            if let Expr::IntLit { value, .. } = &rhs.k {
                let bits = match &*lt {
                    TyS::Int(b, _) => *b as i64,
                    _ => 64,
                };
                if *value < 0 || *value >= bits {
                    self.err("E4012", &lhs.tok, "S-1", "常量移位量 {0} 越界（0 <= n < {1}）", &[&value, &bits]);
                }
            }
            return lt;
        }
        self.err("E4009", &lhs.tok, "文稿 11.1", "未知运算符 `{0}`", &[&op]);
        t_int()
    }

    fn check_try(&mut self, e: &ast::EPtr) -> Ty {
        let inner = match &e.k {
            Expr::Try { inner } => inner,
            _ => unreachable!(),
        };
        let t = self.check_expr(inner, None);
        match &*t {
            TyS::Result(i) => {
                if !self.in_result && !self.in_main && self.try_depth == 0 {
                    self.err("E6010", &e.tok, "文稿 10.1", "`try` 只能出现在返回 result<...> 的函数内或 try 块内（main 除外）", &[]);
                }
                i.clone()
            }
            _ => {
                self.err("E6011", &inner.tok, "文稿 10.1", "`try` 只能作用于 result<T>，得到 {0}", &[&self.ts(&t)]);
                t_object()
            }
        }
    }

    // ---------------------------------------------------------------- Path

    fn check_path(&mut self, e: &ast::EPtr) -> Ty {
        let segs = match &e.k {
            Expr::Path { segs, .. } => segs,
            _ => unreachable!(),
        };
        let name = &segs[0];
        if let Some(v) = self.lookup_var(name) {
            if v.borrow().state == VarState::Moved {
                self.err("E7001", &e.tok, "O-2", "`{0}` 已被移走，此处不可再用（重新赋一个值可复活）", &[&name]);
            }
            self.sl(e.id).res = Some(Res::Local(v.clone()));
            let vi = v.borrow();
            return self.narrowed_ty_vi(&vi);
        }
        if let Some(c) = self.qual_lookup(&self.consts, name) {
            let c = c.clone();
            self.sl(e.id).res = Some(Res::Const(c.clone()));
            return c.ty.borrow().clone();
        }
        if let Some(&f) = self.qual_lookup(&self.func_by_name, name) {
            let f: usize = f;
            let (vis, module) = {
                let fi = &self.funcs[f];
                (fi.vis.clone(), fi.module.clone())
            };
            if vis != "public" && module != self.cur_mod_path {
                self.err("E3013", &e.tok, "N-8", "`{0}` 不是 public，不能跨模块使用", &[&name]);
            }
            self.sl(e.id).res = Some(Res::Func(f));
            let fi = &self.funcs[f];
            return t_fn(fi.params.iter().map(|p| p.1.clone()).collect(), fi.ret.clone());
        }
        if let Some(d) = self.lookup_type_name(name, &e.tok) {
            self.sl(e.id).res = Some(Res::Type(d));
            return Rc::new(TyS::Meta(name.clone()));
        }
        if is_builtin_type_name(name) {
            self.sl(e.id).res = Some(Res::BuiltinType(name.clone()));
            return Rc::new(TyS::Meta(name.clone()));
        }
        if name == "Row" || name == "File" {
            self.sl(e.id).res = Some(Res::BuiltinType(name.clone()));
            return Rc::new(TyS::Meta(name.clone()));
        }
        // 用户模块成员访问（文稿 9.2）：`mod.func` / `mod.CONST` / `mod.Type` /
        // `mod.Type.static_method`（经 Meta 走 check_static_call）。
        // 视图最长匹配：`import a.b` 后 `a.b.foo` 的前两段是模块视图。
        if segs.len() >= 2 {
            if let Some((k, canon)) = self.view_of(segs) {
                if canon != "*builtin*" {
                    let view = segs[..k].join(".");
                    let rest = &segs[k..];
                    if rest.len() > 2 {
                        self.err("E3003", &e.tok, "文稿 9.2", "`{0}` 之后不能继续取成员（`模块.类型.方法(...)` 最多三层）", &[&segs[..k + 2].join(".")]);
                        return t_object();
                    }
                    let export = &rest[0];
                    if !self.export_allowed(&view, export) {
                        self.err("E3030", &e.tok, "文稿 9.2", "`{0}` 不在 `import {1}.{{...}}` 的具名导入清单里", &[&export, &view]);
                        return t_object();
                    }
                    let sym = self
                        .module_exports
                        .get(&canon)
                        .and_then(|m| m.get(export))
                        .cloned();
                    return match sym {
                        Some(ExportSym::Const) => {
                            let reg = format!("{}__{}", canon, export);
                            let c = self.consts.get(&reg).cloned().unwrap();
                            self.sl(e.id).res = Some(Res::Const(c.clone()));
                            let t = c.ty.borrow().clone();
                            t
                        }
                        Some(ExportSym::Func(f)) => {
                            self.sl(e.id).res = Some(Res::Func(f));
                            let fi = &self.funcs[f];
                            t_fn(
                                fi.params.iter().map(|p| p.1.clone()).collect(),
                                fi.ret.clone(),
                            )
                        }
                        Some(ExportSym::Type(d)) => {
                            if rest.len() == 2 {
                                // `mod.Type.method`：让接收者解析成限定类型名，
                                // 静态派发交给 check_static_call
                                let tname = self.typedefs[d as usize].name.clone();
                                Rc::new(TyS::Meta(tname))
                            } else {
                                self.sl(e.id).res = Some(Res::Type(d));
                                Rc::new(TyS::Meta(self.typedefs[d as usize].name.clone()))
                            }
                        }
                        None => {
                            self.err("E3030", &e.tok, "文稿 9.1", "模块 `{0}` 没有公开成员 `{1}`（文件内符号默认私有，导出需写 `public`）", &[&view, &export]);
                            t_object()
                        }
                    };
                }
            }
        }
        // 模块视图（含点号路径的中间段）：`import net.http` 后，`net.http.get()`
        // 的接收者逐段解析——`net` 是前缀视图，`net.http` 才是完整视图
        if self
            .active_views
            .keys()
            .any(|v| v == name || v.starts_with(&format!("{}.", name)))
        {
            self.sl(e.id).res = Some(Res::Module(name.clone()));
            return Rc::new(TyS::ModuleT(name.clone()));
        }
        if name == "Print" || name == "Println" {
            self.sl(e.id).res = Some(Res::Builtin(name.clone()));
            return t_fn(vec![t_string()], t_void());
        }
        if name == "print" {
            // 旧名迁移提示（v0.33）：输出拆成"不换行 / 换行"两个名字，名字本身说明行为
            self.err("E3003", &e.tok, "文稿 11.2", "`print` 已拆分为 `Print`（不换行）与 `Println`（换行）", &[]);
            return t_fn(vec![t_string()], t_void());
        }
        if name == "ok" || name == "fail" {
            self.sl(e.id).res = Some(Res::Builtin(name.clone()));
            return Rc::new(TyS::BuiltinT(name.clone()));
        }
        if name == "parent" {
            self.sl(e.id).res = Some(Res::Parent);
            return Rc::new(TyS::ParentT);
        }
        if name == "Self" {
            return Rc::new(TyS::Meta(name.clone()));
        }
        self.err("E3003", &e.tok, "N-5", "未定义的标识符 `{0}`", &[&name]);
        t_object()
    }

    // ---------------------------------------------------------------- 调用

    fn check_call(&mut self, e: &ast::EPtr, expected: Option<&Ty>) -> Ty {
        let (callee, args) = match &e.k {
            Expr::Call { callee, args } => (callee, args),
            _ => unreachable!(),
        };
        if matches!(callee.k, Expr::Parent) {
            return self.call_parent_ctor(e, args);
        }
        if let Expr::Path { segs, args: targs } = &callee.k {
            let name = &segs[0];
            // 本模块限定名兜底：用户模块体内裸名调用自身的类型构造 / 顶层函数
            let treg = self
                .lookup_type_name(name, &callee.tok)
                .map(|_| name.clone());
            if segs.len() == 1
                && (is_builtin_scalar(name)
                    || treg.is_some()
                    || matches!(name.as_str(), "object" | "result" | "error" | "Rounding")
                    || name == "Row"
                    || name == "File")
            {
                return self.conversion(e, args, name, expected);
            }
            if segs.len() == 1 {
                if let Some(&f) = self.qual_lookup(&self.func_by_name, name) {
                    return self.call_func(e, f, args, targs);
                }
            }
            if segs.len() == 1 && (name == "ok" || name == "fail") {
                return self.call_ok_fail(e, args, name);
            }
            if segs.len() == 1 && (name == "Print" || name == "Println") {
                return self.call_plain(e, args, name, vec![t_string()], t_void());
            }
            if segs.len() == 1 && name == "parent" {
                return self.call_parent_ctor(e, args);
            }
            match self.res_of(callee.id) {
                Some(Res::Type(d)) => return self.conversion_named(e, args, Some(d), expected),
                Some(Res::BuiltinType(bn)) => {
                    return self.conversion(e, args, &bn, expected)
                }
                _ => {}
            }
        }
        let errs_before = self.d.error_count;
        let t = self.check_expr(callee, None);
        if let TyS::Func(ps, ret) = &*t {
            self.check_call_args(args, Some(ps.as_slice()), e.id);
            return ret.clone();
        }
        // 名字本身没解析成功时，check_expr 已经报过 E3003；不要再连着报「不是可调用对象」
        // （一处误用只报一条，否则真正的错因会被后面这条盖住）。
        if self.d.error_count > errs_before {
            return t_object();
        }
        if matches!(
            &*t,
            TyS::Meta(_) | TyS::ModuleT(_) | TyS::BuiltinT(_) | TyS::ParentT
        ) {
            self.err("E5004", &e.tok, "N-20", "调用目标不是函数", &[]);
            return t_object();
        }
        self.err("E5004", &e.tok, "N-20", "`{0}` 不是可调用对象", &[&self.ts(&t)]);
        t_object()
    }

    pub fn check_call_args(
        &mut self,
        args: &[ast::N<ast::Arg>],
        want: Option<&[Ty]>,
        eid: u32,
    ) {
        for (i, a) in args.iter().enumerate() {
            let exp = want.and_then(|w| w.get(i)).cloned();
            self.check_expr(&a.k.expr, exp.as_ref());
            if a.k.borrow {
                self.check_borrow(&a.k.expr, true);
            } else if let Some(e) = &exp {
                self.coerce(&a.k.expr, e, "实参");
                if self.moves(e) {
                    self.take(&a.k.expr);
                }
            }
            let at = self.ty_of(a.k.expr.id);
            self.sl(a.id).ty = at;
        }
        self.check_borrow_conflicts(args, eid);
    }

    fn check_borrow(&mut self, e: &ast::EPtr, mutable: bool) {
        match &e.k {
            Expr::Path { segs, .. } => {
                if let Some(v) = self.lookup_var(&segs[0]) {
                    let (is_const, name) = {
                        let b = v.borrow();
                        (b.is_const, b.name.clone())
                    };
                    if is_const && mutable {
                        self.err("E7009", &e.tok, "O-9", "不能对 const `{0}` 取可变借用（写 `&const`）", &[&name]);
                    }
                }
            }
            Expr::FieldAccess { .. } | Expr::Index { .. } => {}
            _ => {
                self.err("E7007", &e.tok, "O-8", "`&` 只能作用于可写位置（变量、字段、元素）", &[]);
            }
        }
    }

    fn check_borrow_conflicts(&mut self, args: &[ast::N<ast::Arg>], _eid: u32) {
        let mut places: Vec<(String, Vec<&ast::N<ast::Arg>>)> = Vec::new();
        for a in args {
            self.walk_arg(a, &mut places);
        }
        for (key, uses) in &places {
            if uses.len() > 1 {
                let tok = uses[1].tok.clone();
                self.err("E7002", &tok, "O-7", "同一语句内对 `{0}` 有多个借用（至少一个是可变的）", &[&key]);
            }
        }
    }

    /// 借用检查 walk 的 Arg 分支：可借用实参记录位置键，
    /// 然后继续沿表达式下行走嵌套调用里的借用。
    fn walk_arg<'m>(
        &self,
        a: &'m ast::N<ast::Arg>,
        places: &mut Vec<(String, Vec<&'m ast::N<ast::Arg>>)>,
    ) {
        if a.k.borrow {
            if let Some(key) = self.place_key(&a.k.expr) {
                match places.iter_mut().find(|(k, _)| *k == key) {
                    Some((_, v)) => v.push(a),
                    None => places.push((key, vec![a])),
                }
            }
        }
        self.walk_expr(&a.k.expr, places);
    }

    /// 借用检查 walk 的通用分支：沿 AST 子节点下行，找嵌套的 Arg 节点。
    fn walk_expr<'m>(
        &self,
        e: &'m ast::EPtr,
        places: &mut Vec<(String, Vec<&'m ast::N<ast::Arg>>)>,
    ) {
        let mut kids: Vec<&'m ast::EPtr> = Vec::new();
        match &e.k {
            Expr::Call { callee, args } => {
                kids.push(callee);
                for a in args {
                    self.walk_arg(a, places);
                }
            }
            Expr::MethodCall { recv, args, .. } | Expr::SafeCall { recv, args, .. } => {
                kids.push(recv);
                for a in args {
                    self.walk_arg(a, places);
                }
            }
            Expr::New { args, .. } => {
                for a in args {
                    self.walk_arg(a, places);
                }
            }
            Expr::Unary { operand, .. } => kids.push(operand),
            Expr::Binary { lhs, rhs, .. } => {
                kids.push(lhs);
                kids.push(rhs);
            }
            Expr::Ternary { cond, then_, els } => {
                kids.push(cond);
                kids.push(then_);
                kids.push(els);
            }
            Expr::Coalesce { lhs, rhs } => {
                kids.push(lhs);
                kids.push(rhs);
            }
            Expr::Range { lo, hi, .. } => {
                kids.push(lo);
                kids.push(hi);
            }
            Expr::Try { inner } => kids.push(inner),
            Expr::Index { recv, index } => {
                kids.push(recv);
                kids.push(index);
            }
            Expr::FieldAccess { recv, .. } | Expr::SafeAccess { recv, .. } => kids.push(recv),
            Expr::TupleLit { elems } | Expr::ArrayLit { elems } => {
                for x in elems {
                    kids.push(x);
                }
            }
            Expr::BraceLit { pairs } => {
                for (_, v) in pairs {
                    kids.push(v);
                }
            }
            Expr::StrLit { parts } => {
                for p in parts {
                    if let ast::SPart::Interp { expr, .. } = p {
                        kids.push(expr);
                    }
                }
            }
            Expr::If(n) => {
                kids.push(&n.cond);
                for s in &n.then_.stmts {
                    self.walk_stmt(s, places);
                }
                match &n.els {
                    Some(ast::IfTail::Block(b)) => {
                        for s in &b.stmts {
                            self.walk_stmt(s, places);
                        }
                    }
                    Some(ast::IfTail::If(inner)) => {
                        // 包装成临时遍历：直接走 IfNode
                        kids.push(&inner.k.cond);
                        for s in &inner.k.then_.stmts {
                            self.walk_stmt(s, places);
                        }
                    }
                    None => {}
                }
            }
            Expr::Match(n) => {
                kids.push(&n.scrutinee);
                for a in &n.arms {
                    kids.push(&a.body);
                }
            }
            Expr::AnonFunc { body, .. } => {
                for s in &body.stmts {
                    self.walk_stmt(s, places);
                }
            }
            _ => {}
        }
        for k in kids {
            self.walk_expr(k, places);
        }
    }

    fn walk_stmt<'m>(
        &self,
        s: &'m ast::SPtr,
        places: &mut Vec<(String, Vec<&'m ast::N<ast::Arg>>)>,
    ) {
        let mut kids: Vec<&'m ast::EPtr> = Vec::new();
        match &s.k {
            ast::Stmt::Simple { lhs, rhs, .. } => {
                kids.push(lhs);
                if let Some(r) = rhs {
                    kids.push(r);
                }
            }
            ast::Stmt::If(n) => {
                kids.push(&n.cond);
                for s2 in &n.then_.stmts {
                    self.walk_stmt(s2, places);
                }
                if let Some(ast::IfTail::Block(b)) = &n.els {
                    for s2 in &b.stmts {
                        self.walk_stmt(s2, places);
                    }
                }
            }
            ast::Stmt::Match(n) => {
                kids.push(&n.scrutinee);
                for a in &n.arms {
                    kids.push(&a.body);
                }
            }
            ast::Stmt::Block(b) => {
                for s2 in &b.stmts {
                    self.walk_stmt(s2, places);
                }
            }
            ast::Stmt::For { kind, body, .. } => {
                match kind {
                    ast::ForKind::Range { lo, hi, .. } => {
                        kids.push(lo);
                        kids.push(hi);
                    }
                    ast::ForKind::Iter { iterable, .. } | ast::ForKind::Kv { iterable, .. } => {
                        kids.push(iterable);
                    }
                    ast::ForKind::Cond => {}
                }
                for s2 in &body.stmts {
                    self.walk_stmt(s2, places);
                }
            }
            ast::Stmt::Return { value } => {
                if let Some(v) = value {
                    kids.push(v);
                }
            }
            ast::Stmt::Throw { msg, .. } => kids.push(msg),
            ast::Stmt::TryCatch(n) => {
                for s2 in &n.body.stmts {
                    self.walk_stmt(s2, places);
                }
                for c in &n.catches {
                    for s2 in &c.body.stmts {
                        self.walk_stmt(s2, places);
                    }
                }
                if let Some(f) = &n.finally {
                    for s2 in &f.stmts {
                        self.walk_stmt(s2, places);
                    }
                }
            }
            ast::Stmt::Defer { call } => kids.push(call),
            ast::Stmt::VarDecl { init, .. } => {
                if let Some(v) = init {
                    kids.push(v);
                }
            }
            ast::Stmt::VarTuple { init, .. } => kids.push(init),
            ast::Stmt::Const { init, .. } => kids.push(init),
            ast::Stmt::Break { .. } | ast::Stmt::Continue { .. } => {}
        }
        for k in kids {
            self.walk_expr(k, places);
        }
    }

    pub fn place_key(&self, e: &ast::EPtr) -> Option<String> {
        match &e.k {
            Expr::Path { segs, .. } => Some(segs[0].clone()),
            Expr::FieldAccess { recv, name } => {
                self.place_key(recv).map(|b| b + "." + name)
            }
            Expr::Index { recv, .. } => self.place_key(recv).map(|b| b + "[]"),
            _ => None,
        }
    }

    fn call_plain(
        &mut self,
        e: &ast::EPtr,
        args: &[ast::N<ast::Arg>],
        name: &str,
        ptypes: Vec<Ty>,
        ret: Ty,
    ) -> Ty {
        self.check_call_args(args, Some(&ptypes), e.id);
        if args.len() != ptypes.len() {
            self.err("E4031", &e.tok, "N-20", "`{0}` 需要 {1} 个实参，得到 {2}", &[&name, &ptypes.len(), &args.len()]);
        }
        self.sl(e.id).ckind = Some("builtin");
        self.sl(e.id).bname = Some(name.to_string());
        ret
    }

    fn call_ok_fail(&mut self, e: &ast::EPtr, args: &[ast::N<ast::Arg>], which: &str) -> Ty {
        if !self.in_result && !self.in_main {
            self.err("E6010", &e.tok, "文稿 10.1", "`{0}(...)` 只能用于返回 result<...> 的函数内（main 除外）", &[&which]);
        }
        if which == "ok" {
            if args.len() != 1 {
                self.err("E4031", &e.tok, "文稿 10.1", "`ok` 需要 1 个实参", &[]);
                return t_result(t_object());
            }
            let t = self.check_expr(&args[0].k.expr, None);
            self.sl(e.id).ckind = Some("ok");
            return t_result(t);
        }
        if args.len() != 1 {
            self.err("E4031", &e.tok, "文稿 10.1", "`fail` 需要 1 个实参", &[]);
            return t_result(t_object());
        }
        let t = self.check_expr(&args[0].k.expr, None);
        if t != t_error() {
            self.err("E4001", &args[0].k.expr.tok, "文稿 10.1", "`fail` 的实参必须是 error，得到 {0}", &[&self.ts(&t)]);
        }
        self.sl(e.id).ckind = Some("fail");
        let inner = match self.current_func {
            Some(f) if matches!(&*self.funcs[f].ret, TyS::Result(_)) => {
                match &*self.funcs[f].ret {
                    TyS::Result(i) => i.clone(),
                    _ => t_object(),
                }
            }
            _ => t_object(),
        };
        t_result(inner)
    }

    pub(crate) fn call_func(
        &mut self,
        e: &ast::EPtr,
        f: usize,
        args: &[ast::N<ast::Arg>],
        callee_targs: &[ast::TPtr],
    ) -> Ty {
        let (fname, generics) = {
            let fi = &self.funcs[f];
            (fi.name.clone(), fi.generics.clone())
        };
        let mut generic_args: Vec<Ty> = Vec::new();
        if !callee_targs.is_empty() {
            if callee_targs.len() != generics.len() {
                self.err("E4035", &e.tok, "G-1", "`{0}` 需要 {1} 个类型实参", &[&fname, &generics.len()]);
            }
            let mut subst: HashMap<String, Ty> = HashMap::new();
            for ((g, _), a) in generics.iter().zip(callee_targs.iter()) {
                subst.insert(g.clone(), self.resolve_type(a));
            }
            self.check_generic_constraints(&generics, &subst, &e.tok);
            generic_args = generics
                .iter()
                .map(|(g, _)| subst.get(g).cloned().unwrap_or_else(t_object))
                .collect();
        } else {
            if !generics.is_empty() {
                match self.infer_generics(f, args) {
                    None => {
                        self.err("E4037", &e.tok, "T-16", "无法推断 `{0}` 的类型实参，请显式写 `{1}<...>(...)`", &[&fname, &fname]);
                        generic_args = vec![t_object(); generics.len()];
                    }
                    Some(inf) => {
                        generic_args = inf;
                        let subst: HashMap<String, Ty> = generics
                            .iter()
                            .zip(generic_args.iter())
                            .map(|((g, _), t)| (g.clone(), t.clone()))
                            .collect();
                        self.check_generic_constraints(&generics, &subst, &e.tok);
                    }
                }
            }
        }
        self.sl(e.id).generic_args = generic_args.clone();
        // 参数替换
        let mut params = self.funcs[f].params.clone();
        let ret;
        if !generics.is_empty() && !generic_args.is_empty() {
            let subst: HashMap<String, Ty> = generics
                .iter()
                .zip(generic_args.iter())
                .map(|((g, _), t)| (g.clone(), t.clone()))
                .collect();
            params = params
                .into_iter()
                .map(|(n, t, b)| (n, self.subst_ty(&t, &subst), b))
                .collect();
            ret = self.subst_ty(&self.funcs[f].ret, &subst);
        } else {
            ret = self.funcs[f].ret.clone();
        }
        // 实参个数
        if args.len() != params.len() {
            self.err("E4031", &e.tok, "N-20", "`{0}` 需要 {1} 个实参，得到 {2}", &[&fname, &params.len(), &args.len()]);
        }
        for (i, a) in args.iter().enumerate() {
            let exp = params.get(i).map(|p| p.1.clone());
            self.check_expr(&a.k.expr, exp.as_ref());
            if a.k.borrow {
                self.check_borrow(&a.k.expr, true);
            } else if let Some(ex) = &exp {
                self.coerce(&a.k.expr, ex, "实参");
                if self.moves(ex) {
                    self.take(&a.k.expr);
                }
            }
            let at = self.ty_of(a.k.expr.id);
            let s = self.sl(a.id);
            s.ty = at;
            s.decl_ty = exp;
        }
        self.check_borrow_conflicts(args, e.id);
        let s = self.sl(e.id);
        s.ckind = Some("func");
        s.func = Some(f);
        ret
    }

    fn check_generic_constraints(
        &mut self,
        generics: &[(String, Vec<String>)],
        subst: &HashMap<String, Ty>,
        tok: &crate::frontend::lexer::Tok,
    ) {
        for (name, ty) in subst {
            let cons = generics
                .iter()
                .find(|(g, _)| g == name)
                .map(|(_, c)| c.clone())
                .unwrap_or_default();
            for c in &cons {
                if !matches!(c.as_str(), "Comparable" | "Hashable" | "Printable")
                    && !self.types.contains_key(c)
                {
                    self.err("E4036", tok, "T-31", "约束 `{0}` 必须是 interface", &[&c]);
                    continue;
                }
                if c == "Comparable" && matches!(&**ty, TyS::Float) {
                    self.err("E4036", tok, "T-33", "`float` 不实现 Comparable", &[]);
                }
                if matches!(&**ty, TyS::Param(_)) {
                    continue;
                }
                if self.types.contains_key(c) {
                    if let TyS::Named(DefId::Id(id), _) = &**ty {
                        let ok = match self.typedefs.get(*id as usize) {
                            Some(d) => self.implements(d.id, self.types[c]),
                            None => false,
                        };
                        if !ok {
                            self.err("E4036", tok, "T-36", "{0} 不满足约束 `{1}`", &[&self.ts(ty), &c]);
                        }
                    }
                }
            }
        }
    }

    fn infer_generics(&mut self, f: usize, args: &[ast::N<ast::Arg>]) -> Option<Vec<Ty>> {
        let names: Vec<String> = self.funcs[f].generics.iter().map(|g| g.0.clone()).collect();
        let params = self.funcs[f].params.clone();
        let mut subst: HashMap<String, Ty> = HashMap::new();
        for (i, a) in args.iter().enumerate() {
            if i >= params.len() {
                break;
            }
            let pty = params[i].1.clone();
            let aty = self.ty_of(a.k.expr.id);
            let aty = match aty {
                Some(t) => t,
                None => continue,
            };
            self.unify(&pty, &aty, &names, &mut subst);
        }
        if subst.len() != names.len() {
            return None;
        }
        Some(names.iter().map(|n| subst[n].clone()).collect())
    }

    fn unify(&self, p: &Ty, a: &Ty, names: &[String], subst: &mut HashMap<String, Ty>) {
        if let TyS::Param(pn) = &**p {
            if names.contains(pn) {
                subst.entry(pn.clone()).or_insert_with(|| a.clone());
                return;
            }
        }
        match (&**p, &**a) {
            (TyS::Array(p1, _), TyS::Array(a1, _)) => self.unify(p1, a1, names, subst),
            (TyS::Map(p1, p2), TyS::Map(a1, a2)) => {
                self.unify(p1, a1, names, subst);
                self.unify(p2, a2, names, subst);
            }
            (TyS::Nullable(p1), TyS::Nullable(a1)) => self.unify(p1, a1, names, subst),
            (TyS::Result(p1), TyS::Result(a1)) => self.unify(p1, a1, names, subst),
            _ => {}
        }
    }

    pub fn subst_ty(&self, t: &Ty, subst: &HashMap<String, Ty>) -> Ty {
        match &**t {
            TyS::Param(pn) => subst.get(pn).cloned().unwrap_or_else(|| t.clone()),
            TyS::Array(e, l) => t_array(self.subst_ty(e, subst), *l),
            TyS::Map(k, v) => t_map(self.subst_ty(k, subst), self.subst_ty(v, subst)),
            TyS::Nullable(i) => t_nullable(self.subst_ty(i, subst)),
            TyS::Result(i) => t_result(self.subst_ty(i, subst)),
            TyS::Tuple(elems) => Rc::new(TyS::Tuple(
                elems.iter().map(|x| self.subst_ty(x, subst)).collect(),
            )),
            TyS::Func(ps, r) => t_fn(
                ps.iter().map(|x| self.subst_ty(x, subst)).collect(),
                self.subst_ty(r, subst),
            ),
            TyS::Named(id, args) => t_named(
                id.clone(),
                args.iter().map(|x| self.subst_ty(x, subst)).collect(),
            ),
            TyS::Record(fields) => Rc::new(TyS::Record(
                fields
                    .iter()
                    .map(|(n, x)| (n.clone(), self.subst_ty(x, subst)))
                    .collect(),
            )),
            TyS::ArenaRef(i) => Rc::new(TyS::ArenaRef(self.subst_ty(i, subst))),
            _ => t.clone(),
        }
    }

    fn call_parent_ctor(&mut self, e: &ast::EPtr, args: &[ast::N<ast::Arg>]) -> Ty {
        let pd = match self.cur_class.and_then(|c| self.typedefs[c as usize].parent) {
            None => {
                self.err("E3034", &e.tok, "G-8", "`parent(...)` 只能在子类 constructor 内使用", &[]);
                return t_void();
            }
            Some(p) => p,
        };
        let params: Vec<(String, Ty, Option<String>)> = match self.typedefs[pd as usize].ctor {
            Some(c) => self.funcs[c].params.clone(),
            None => Vec::new(),
        };
        let ptypes: Vec<Ty> = params.iter().map(|p| p.1.clone()).collect();
        self.check_call_args(args, Some(&ptypes), e.id);
        if args.len() != params.len() {
            self.err("E4031", &e.tok, "文稿 6.2", "`parent(...)` 需要 {0} 个实参，得到 {1}", &[&params.len(), &args.len()]);
        }
        let s = self.sl(e.id);
        s.ckind = Some("parent_ctor");
        s.parent_def = Some(pd);
        t_void()
    }

    // ---------------------------------------------------------------- 转换/构造

    fn conversion(
        &mut self,
        e: &ast::EPtr,
        args: &[ast::N<ast::Arg>],
        name: &str,
        expected: Option<&Ty>,
    ) -> Ty {
        if args.is_empty() {
            self.err("E4031", &e.tok, "N-20", "`{0}(...)` 缺少实参", &[&name]);
            return t_object();
        }
        if args.len() != 1 {
            // 构造（class/struct）走 conversion_named
            if let Some(d) = self.lookup_type_name(name, &e.tok) {
                return self.conversion_named(e, args, Some(d), expected);
            }
            self.err("E5005", &e.tok, "C-1", "`{0}(x)` 只能有一个实参（构造请用构造调用）", &[&name]);
            for a in args {
                self.check_expr(&a.k.expr, None);
            }
            return t_object();
        }
        let arg = &args[0].k.expr;
        let at = self.check_expr(arg, None);
        if let Some(dst) = builtin_scalar(name) {
            return self.numeric_conv(e, arg, &at, &dst, name);
        }
        match name {
            "object" => {
                let dst = t_object();
                let r = assignable(&at, &dst, self, false);
                if matches!(r, AssignResult::No(_)) {
                    self.err("E4029", &e.tok, "T-6", "{0} 不能装箱到 object", &[&self.ts(&at)]);
                } else {
                    self.sl(e.id).conv = Some("box");
                }
                dst
            }
            "string" => {
                self.err("E5001", &e.tok, "C-8", "不能用 `string(x)` 转字符串；请写 `x.ToString()`，码点用 `string.FromCodepoint`", &[]);
                t_string()
            }
            "Row" | "File" => self.conversion_named(e, args, None, expected),
            "Rounding" => {
                self.err("E4019", &e.tok, "C-17", "Rounding 是 enum，不能从数值转换", &[]);
                t_named(DefId::Name("Rounding"), Vec::new())
            }
            _ => {
                if let Some(&d) = self.types.get(name) {
                    return self.conversion_named(e, args, Some(d), expected);
                }
                self.err("E5004", &e.tok, "N-20", "`{0}` 不是类型", &[&name]);
                at
            }
        }
    }

    fn numeric_conv(&mut self, e: &ast::EPtr, _arg: &ast::EPtr, at: &Ty, dst: &Ty, name: &str) -> Ty {
        {
            let s = self.sl(e.id);
            s.conv = Some("cast");
            s.cast_to = Some(dst.clone());
        }
        if matches!(&**at, TyS::Object) {
            self.sl(e.id).conv = Some("unbox");
            return dst.clone();
        }
        if matches!(&**at, TyS::Str) {
            self.err("E5001", &e.tok, "C-9", "不能用 `{0}(x)` 解析字符串；请用 `{1}.Parse(s)`（返回 result）", &[&name, &name]);
            return dst.clone();
        }
        if matches!(&**at, TyS::Bool) {
            self.err("E5001", &e.tok, "C-10", "bool 与其他类型之间没有转换", &[]);
            return dst.clone();
        }
        if matches!(&**at, TyS::Enum(..)) {
            self.err("E4019", &e.tok, "C-17", "enum 不能转成整数", &[]);
            return dst.clone();
        }
        if matches!(&**at, TyS::Float) && matches!(dst.as_ref(), TyS::Decimal) {
            self.err("E5001", &e.tok, "C-6", "float 不能直接转 decimal（C-6）；请用 `decimal.Parse(s)`", &[]);
            return dst.clone();
        }
        if matches!(dst.as_ref(), TyS::Str) {
            self.err("E5001", &e.tok, "C-8", "不能用 `string(x)` 转字符串", &[]);
            return dst.clone();
        }
        if !is_numeric(at) || !is_numeric(dst) {
            self.err("E5004", &e.tok, "C-1", "`{0}(x)` 的实参类型 {1} 不能转换", &[&name, &self.ts(at)]);
            return dst.clone();
        }
        if at == dst {
            self.sl(e.id).conv = None;
        }
        dst.clone()
    }

    pub(crate) fn conversion_named(
        &mut self,
        e: &ast::EPtr,
        args: &[ast::N<ast::Arg>],
        d: Option<u32>,
        _expected: Option<&Ty>,
    ) -> Ty {
        let d = match d {
            Some(d) => d,
            None => {
                self.err("E5004", &e.tok, "N-20", "未知类型转换", &[]);
                return t_object();
            }
        };
        let (kind, dname) = {
            let td = &self.typedefs[d as usize];
            (td.kind, td.name.clone())
        };
        if kind == "class" {
            if args.len() == 1 {
                let at = self.check_expr(&args[0].k.expr, None);
                if let TyS::Named(DefId::Id(src), _) = &*at {
                    let src_known = self
                        .typedefs
                        .get(*src as usize)
                        .map_or(false, |sd| sd.kind == "class" || sd.kind == "interface");
                    if src_known && *src != d {
                        let src_name = self.typedefs[*src as usize].name.clone();
                        if self.is_subclass(d, *src) || self.implements(d, *src) {
                            self.err("E5003", &e.tok, "C-14", "向下转型的目标必须是子类（`{0}` 不是 `{1}` 的子类）", &[&dname, &src_name]);
                        }
                        let s = self.sl(e.id);
                        s.conv = Some("downcast");
                        s.cast_to = Some(t_named(DefId::Id(d), Vec::new()));
                        return t_named(DefId::Id(d), Vec::new());
                    }
                }
            }
            return self.construct(e, args, d);
        }
        if kind == "struct" {
            if args.len() == 1 {
                let at = self.check_expr(&args[0].k.expr, None);
                if let TyS::Record(fields) = &*at {
                    let fmap: HashMap<&String, &Ty> =
                        fields.iter().map(|(k, t)| (k, t)).collect();
                    let need: Vec<(String, Ty)> = self.typedefs[d as usize]
                        .fields
                        .iter()
                        .map(|f| (f.0.clone(), f.1.clone()))
                        .collect();
                    let need_keys: HashSet<&String> = need.iter().map(|(k, _)| k).collect();
                    let fmap_keys: HashSet<&String> = fmap.keys().copied().collect();
                    if need_keys != fmap_keys {
                        let mut need_sorted: Vec<String> =
                            need.iter().map(|(k, _)| k.clone()).collect();
                        need_sorted.sort();
                        let mut got_sorted: Vec<String> =
                            fmap.keys().map(|k| (*k).clone()).collect();
                        got_sorted.sort();
                        self.err(
                            "E4034",
                            &e.tok,
                            "C-15",
                            "record 与 struct `{0}` 字段不匹配（需要 {1}，得到 {2}）",
                            &[
                                &dname,
                                &format!("{:?}", need_sorted),
                                &format!("{:?}", got_sorted),
                            ],
                        );
                    } else {
                        for (n, t) in &need {
                            let got = fmap.get(n).unwrap();
                            if **got != *t {
                                self.err("E4034", &e.tok, "C-15", "字段 `{0}` 类型不匹配：{1} vs {2}", &[&n, &self.ts(t), &self.ts(got)]);
                            }
                        }
                    }
                    self.sl(e.id).conv = Some("record2struct");
                    return t_named(DefId::Id(d), Vec::new());
                }
            }
            return self.construct(e, args, d);
        }
        if kind == "enum" {
            self.err("E4019", &e.tok, "C-17", "enum 不能从整数/其他类型转换", &[]);
            return Rc::new(TyS::Enum(d));
        }
        if kind == "interface" {
            if args.len() == 1 {
                let at = self.check_expr(&args[0].k.expr, None);
                if let TyS::Named(DefId::Id(src), _) = &*at {
                    if self.implements(*src, d) {
                        self.sl(e.id).conv = Some("upcast");
                        return t_named(DefId::Id(d), Vec::new());
                    }
                }
            }
            self.err("E5003", &e.tok, "C-14", "interface 只能从实现它的类构造", &[]);
            return t_named(DefId::Id(d), Vec::new());
        }
        self.err("E5004", &e.tok, "N-20", "`{0}` 不是可构造类型", &[&dname]);
        t_object()
    }

    fn construct(&mut self, e: &ast::EPtr, args: &[ast::N<ast::Arg>], d: u32) -> Ty {
        let (dname, dkind, dgenerics) = {
            let td = &self.typedefs[d as usize];
            (td.name.clone(), td.kind, td.generics.clone())
        };
        // constructor 是实例化的唯一通行证：没有声明 constructor 的类不可 new。
        // 与 E3008（有字段必须有 constructor）分工：有字段没构造 → 只报 E3008（在声明处，
        // 教用户补构造）；零字段没构造 → 只报 E4042（在 new 处，教用户改用静态调用）。
        // 一处误用恰好一条诊断，两条规则合起来"有 constructor ⇔ 可 new"。
        // 只对 class 生效——struct/enum 没有 constructor 概念，维持原判定。
        let td = &self.typedefs[d as usize];
        let is_class = td.kind == "class";
        let has_ctor = td.ctor.is_some();
        let has_fields = !td.fields.is_empty();
        if is_class && !has_ctor && !has_fields {
            self.err("E4042", &e.tok, "N-20", "`{0}` 没有 constructor，不能 new\n  = 它的静态方法请直接用类名调用：`{1}.xxx(...)`\n  = 如果确实需要实例，请先为 `{2}` 添加 constructor", &[&dname, &dname, &dname]);
        }
        let params: Vec<(String, Ty, Option<String>)> = match self.typedefs[d as usize].ctor {
            Some(c) => self.funcs[c].params.clone(),
            None => Vec::new(),
        };
        if has_ctor && args.len() != params.len() {
            self.err("E4031", &e.tok, "N-20", "`{0}` 的 constructor 需要 {1} 个实参，得到 {2}", &[&dname, &params.len(), &args.len()]);
        }
        for (i, a) in args.iter().enumerate() {
            let exp = params.get(i).map(|p| p.1.clone());
            self.check_expr(&a.k.expr, exp.as_ref());
            if let Some(ex) = &exp {
                self.coerce(&a.k.expr, ex, "构造实参");
            }
        }
        let s = self.sl(e.id);
        s.ckind = Some("ctor");
        if dkind == "enum" {
            return Rc::new(TyS::Enum(d));
        }
        t_named(
            DefId::Id(d),
            dgenerics
                .iter()
                .map(|(g, _)| Rc::new(TyS::Param(g.clone())) as Ty)
                .collect(),
        )
    }

    fn check_new(&mut self, e: &ast::EPtr) -> Ty {
        let (path, args) = match &e.k {
            Expr::New { path, args } => (path, args),
            _ => unreachable!(),
        };
        let name = path.last().unwrap();
        // 1) 模块限定：`new mod.Type(...)`（文稿 9.2）
        // 模块路径可多段（`new shop.model.Product(...)`），末段才是类型名
        let mut d: Option<u32> = None;
        let mut module_qualified = false;
        if path.len() >= 2 {
            if let Some((k, canon)) = self.view_of(path) {
                if canon != "*builtin*" && k + 1 == path.len() {
                    module_qualified = true;
                    let view = path[..k].join(".");
                    let export = &path[k];
                    if !self.export_allowed(&view, export) {
                        let src = self.view_display(&view);
                        self.err("E3030", &e.tok, "文稿 9.2", "`{0}` 不在 `import {1}.{{...}}` 的具名导入清单里", &[&export, &src]);
                        return t_object();
                    }
                    d = self
                        .module_exports
                        .get(&canon)
                        .and_then(|m| m.get(export))
                        .and_then(|s| match s {
                            ExportSym::Type(d) => Some(*d),
                            _ => None,
                        });
                    if d.is_none() {
                        self.err("E3030", &e.tok, "文稿 9.1", "模块 `{0}` 没有公开类型 `{1}`", &[&view, &export]);
                        return t_object();
                    }
                }
            }
        }
        // 2) 裸名 / 本模块限定名
        if d.is_none() {
            d = self.lookup_type_name(&path[0], &e.tok);
        }
        if path.len() > 1 && !module_qualified {
            d = self
                .types
                .get(&path[0])
                .and_then(|o| self.typedefs[*o as usize].nested.get(&path[1]).copied());
        }
        if name == "Arena" {
            for a in args {
                self.check_expr(&a.k.expr, None);
            }
            self.sl(e.id).ckind = Some("arena");
            return t_named(DefId::Name("Arena"), Vec::new());
        }
        let d = match d {
            None => {
                if is_reserved(name) {
                    self.err("E2012", &e.tok, "G-9", "`new` 不能用于内建类型 `{0}`", &[&name]);
                } else {
                    self.err("E3003", &e.tok, "N-7", "未定义的类型 `{0}`", &[&name]);
                }
                return t_object();
            }
            Some(d) => d,
        };
        if self.typedefs[d as usize].kind != "class" {
            self.err("E2012", &e.tok, "G-9", "`new` 只能用于 class（struct/enum/interface 不需要 new）", &[]);
            return t_object();
        }
        self.construct(e, args, d)
    }
}
