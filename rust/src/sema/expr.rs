//! 语义检查——表达式级：check_expr 及下游、函数调用、coerce（含 try 块内
//! result<T> 的隐式展开）、常量求值入口与 `module_sig`（模块签名，供 member.rs 复用）。
//! 成员访问 / 方法调用 / match / eval_const 的实现在 `member.rs`。

use crate::frontend::ast;
use crate::frontend::ast::Expr;
use crate::sema::{BUILTIN_MODULES, is_builtin_type_name, is_reserved, ConstVal, ExportSym, Res, Sema, StmtUse, UseKind, VarState};
use crate::types::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

pub fn module_sig(mod_: &str, name: &str) -> Option<(Vec<Option<Ty>>, Ty)> {
    let t = |k: TyS| -> Ty { Rc::new(k) };
    Some(match (mod_, name) {
        ("io", "readLine") => (vec![], t(TyS::Nullable(t_string()))),
        ("io", "readFile") => (vec![Some(t_string())], t_result(t_string())),
        ("io", "write") => (vec![Some(t_string())], t_void()),
        ("io", "writeLine") => (vec![Some(t_string())], t_void()),
        ("math", "equals") => (
            vec![Some(t_float()), Some(t_float()), Some(t_float())],
            t_bool(),
        ),
        ("math", "compare") => (vec![Some(t_float()), Some(t_float())], t_int()),
        ("math", "abs") => (vec![Some(t_float())], t_float()),
        ("math", "max") => (vec![Some(t_float()), Some(t_float())], t_float()),
        ("math", "min") => (vec![Some(t_float()), Some(t_float())], t_float()),
        ("math", "sqrt") => (vec![Some(t_float())], t_float()),
        ("math", "pow") => (vec![Some(t_float()), Some(t_float())], t_float()),
        ("math", "exp") => (vec![Some(t_float())], t_float()),
        ("math", "log") => (vec![Some(t_float())], t_float()),
        ("math", "sin") => (vec![Some(t_float())], t_float()),
        ("math", "cos") => (vec![Some(t_float())], t_float()),
        ("math", "tan") => (vec![Some(t_float())], t_float()),
        ("math", "ceil") => (vec![Some(t_float())], t_float()),
        ("math", "floor") => (vec![Some(t_float())], t_float()),
        ("math", "round") => (vec![Some(t_float())], t_float()),
        ("math", "sign") => (vec![Some(t_float())], t_int()),
        ("math", "mod") => (vec![Some(t_float()), Some(t_float())], t_float()),
        ("math", "exp2") => (vec![Some(t_float())], t_float()),
        ("math", "log2") => (vec![Some(t_float())], t_float()),
        ("math", "log10") => (vec![Some(t_float())], t_float()),
        ("math", "log1p") => (vec![Some(t_float())], t_float()),
        ("math", "is_nan") => (vec![Some(t_float())], t_bool()),
        ("math", "is_inf") => (vec![Some(t_float()), Some(t_int())], t_bool()),
        ("math", "sinh") => (vec![Some(t_float())], t_float()),
        ("math", "cosh") => (vec![Some(t_float())], t_float()),
        ("math", "tanh") => (vec![Some(t_float())], t_float()),
        ("math", "asin") => (vec![Some(t_float())], t_float()),
        ("math", "acos") => (vec![Some(t_float())], t_float()),
        ("math", "atan") => (vec![Some(t_float())], t_float()),
        ("math", "atan2") => (vec![Some(t_float()), Some(t_float())], t_float()),
        ("math", "hypot") => (vec![Some(t_float()), Some(t_float())], t_float()),
        ("math", "cbrt") => (vec![Some(t_float())], t_float()),
        ("math", "trunc") => (vec![Some(t_float())], t_float()),
        ("json", "parse") => (
            vec![Some(t_string())],
            t_result(t_map(t_string(), t_object())),
        ),
        ("json", "stringify") => (vec![Some(t_object())], t_string()),
        ("json", "marshalIndent") => (
            vec![Some(t_object()), Some(t_string()), Some(t_string())],
            t_string(),
        ),
        ("json", "valid") => (vec![Some(t_string())], t_bool()),
        ("db", "query") => (
            vec![Some(t_string())],
            t_result(t_named(DefId::Name("Row"), Vec::new())),
        ),
        /* file 模块（单段名，向后兼容；std.file 现已改为 axc/std/file.ax） */
        ("file", "open") => (
            vec![Some(t_string())],
            t_result(t_named(DefId::Name("File"), Vec::new())),
        ),
        ("file", "create") => (
            vec![Some(t_string())],
            t_result(t_named(DefId::Name("File"), Vec::new())),
        ),
        ("file", "append") => (
            vec![Some(t_string())],
            t_result(t_named(DefId::Name("File"), Vec::new())),
        ),
        ("file", "remove") => (vec![Some(t_string())], t_int()),
        ("file", "rename") => (vec![Some(t_string()), Some(t_string())], t_int()),
        ("file", "exists") => (vec![Some(t_string())], t_int()),
        ("file", "mkdir") => (vec![Some(t_string())], t_int()),
        ("file", "isDir") => (vec![Some(t_string())], t_int()),
        ("file", "isFile") => (vec![Some(t_string())], t_int()),
        ("file", "size") => (vec![Some(t_string())], t_int()),
        ("file", "modTime") => (vec![Some(t_string())], t_int()),
        ("file", "readAll") => (vec![Some(t_string())], t_result(t_string())),
        ("file", "writeAll") => (vec![Some(t_string()), Some(t_string())], t_int()),
        ("file", "chmod") => (vec![Some(t_string()), Some(t_int())], t_int()),
        ("file", "chtimes") => (vec![Some(t_string()), Some(t_int()), Some(t_int())], t_int()),
        ("file", "tempFile") => (vec![Some(t_string()), Some(t_string())], t_result(t_named(DefId::Name("File"), Vec::new()))),
        /* string 模块（单段名，向后兼容；std.string 现已改为 axc/std/string.ax） */
        ("string", "from_codepoint") => (vec![Some(t_int())], t_result(t_string())),
        ("string", "compare") => (vec![Some(t_string()), Some(t_string())], t_int()),
        ("string", "fields") => (vec![Some(t_string())], t_array(t_string(), None)),
        ("string", "to_upper_rune") => (vec![Some(t_int())], t_int()),
        ("string", "to_lower_rune") => (vec![Some(t_int())], t_int()),
        ("time", "now_ms") => (vec![], t_int()),
        ("time", "now") => (vec![], t_named(DefId::Name("time"), Vec::new())),
        ("time", "from_unix_ms") => (vec![Some(t_int())], t_named(DefId::Name("time"), Vec::new())),
        ("time", "sleep") => (vec![Some(t_int())], t_void()),
        ("time", "unix") => (vec![Some(t_int()), Some(t_int())], t_named(DefId::Name("time"), Vec::new())),
        ("time", "unix_milli") => (vec![Some(t_int())], t_named(DefId::Name("time"), Vec::new())),
        ("time", "unix_micro") => (vec![Some(t_int())], t_named(DefId::Name("time"), Vec::new())),
        ("time", "unix_nano") => (vec![Some(t_int())], t_named(DefId::Name("time"), Vec::new())),
        ("time", "since") => (vec![Some(t_named(DefId::Name("time"), Vec::new()))], t_int()),
        ("time", "date") => (vec![
            Some(t_int()), Some(t_int()), Some(t_int()),
            Some(t_int()), Some(t_int()), Some(t_int()), Some(t_int()),
        ], t_named(DefId::Name("time"), Vec::new())),
        ("time", "until") => (vec![Some(t_named(DefId::Name("time"), Vec::new()))], t_int()),
        ("time", "parse") => (
            vec![Some(t_string()), Some(t_string())],
            t_result(t_named(DefId::Name("time"), Vec::new())),
        ),
        /* os 模块 */
        ("os", "getenv") => (vec![Some(t_string())], t_string()),
        ("os", "exit") => (vec![Some(t_int())], t_void()),
        ("os", "args") => (vec![], t_array(t_string(), None)),
        ("os", "getwd") => (vec![], t_string()),
        ("os", "chdir") => (vec![Some(t_string())], t_int()),
        ("os", "getpid") => (vec![], t_int()),
        ("os", "getppid") => (vec![], t_int()),
        ("os", "hostname") => (vec![], t_string()),
        ("os", "environ") => (vec![], t_array(t_string(), None)),
        ("os", "setenv") => (vec![Some(t_string()), Some(t_string())], t_int()),
        ("os", "unsetenv") => (vec![Some(t_string())], t_int()),
        ("os", "userHomeDir") => (vec![], t_string()),
        /* atomic 模块（单段名，向后兼容；std.atomic 现已改为 axc/std/atomic.ax） */
        ("atomic", "addInt") => (vec![Some(t_int()), Some(t_int())], t_int()),
        ("atomic", "loadInt") => (vec![Some(t_int())], t_int()),
        ("atomic", "storeInt") => (vec![Some(t_int()), Some(t_int())], t_void()),
        ("atomic", "compareAndSwapInt") => (vec![Some(t_int()), Some(t_int()), Some(t_int())], t_bool()),
        /* sort 模块 */
        ("sort", "ints") => (vec![Some(t_array(t_int(), None))], t_void()),
        ("sort", "strings") => (vec![Some(t_array(t_string(), None))], t_void()),
        ("sort", "float64s") => (vec![Some(t_array(t_float(), None))], t_void()),
        ("sort", "intsAreSorted") => (vec![Some(t_array(t_int(), None))], t_bool()),
        ("sort", "stringsAreSorted") => (vec![Some(t_array(t_string(), None))], t_bool()),
        ("sort", "searchInts") => (vec![Some(t_array(t_int(), None)), Some(t_int())], t_int()),
        ("sort", "searchStrings") => (vec![Some(t_array(t_string(), None)), Some(t_string())], t_int()),
        ("sort", "slice") => (
            vec![
                Some(t_array(t_object(), None)),
                Some(t_fn(vec![t_int(), t_int()], t_int())),
            ],
            t_void(),
        ),
        ("sort", "sliceStable") => (
            vec![
                Some(t_array(t_object(), None)),
                Some(t_fn(vec![t_int(), t_int()], t_int())),
            ],
            t_void(),
        ),
        ("errors", "new") => (vec![Some(t_string())], t_error()),
        ("errors", "is") => (vec![Some(t_error()), Some(t_error())], t_bool()),
        ("errors", "as") => (vec![Some(t_error()), Some(t_error())], t_bool()),
        /* log 模块（对标 Go log，2026-11-23；P2。Print/Fatal 走多实参 print，Printf 复用 fmt.Printf） */
        ("log", "print") => (vec![None], t_void()),
        ("log", "printf") => (vec![Some(t_string()), None], t_void()),
        ("log", "fatal") => (vec![None], t_void()),
        /* strconv 模块 */
        ("strconv", "atoi") => (vec![Some(t_string())], t_int()),
        ("strconv", "itoa") => (vec![Some(t_int())], t_string()),
        ("strconv", "parseFloat") => (vec![Some(t_string())], t_float()),
        ("strconv", "formatFloat") => (vec![Some(t_float())], t_string()),
        ("strconv", "parseInt") => (vec![Some(t_string()), Some(t_int())], t_int()),
        ("strconv", "formatInt") => (vec![Some(t_int()), Some(t_int())], t_string()),
        ("strconv", "parseBool") => (vec![Some(t_string())], t_bool()),
        ("strconv", "formatBool") => (vec![Some(t_bool())], t_string()),
        ("strconv", "parseUint") => (vec![Some(t_string()), Some(t_int())], t_int()),
        ("strconv", "formatUint") => (vec![Some(t_int()), Some(t_int())], t_string()),
        ("strconv", "canBackquote") => (vec![Some(t_string())], t_bool()),
        ("strconv", "quote") => (vec![Some(t_string())], t_string()),
        ("strconv", "unquote") => (vec![Some(t_string())], t_string()),
        /* filepath 模块 */
        ("filepath", "join") => (vec![Some(t_array(t_string(), None))], t_string()),
        ("filepath", "dir") => (vec![Some(t_string())], t_string()),
        ("filepath", "base") => (vec![Some(t_string())], t_string()),
        ("filepath", "ext") => (vec![Some(t_string())], t_string()),
        ("filepath", "split") => (vec![Some(t_string())], t_array(t_string(), None)),
        ("filepath", "clean") => (vec![Some(t_string())], t_string()),
        ("filepath", "abs") => (vec![Some(t_string())], t_string()),
        ("filepath", "isAbs") => (vec![Some(t_string())], t_bool()),
        ("filepath", "evalSymlinks") => (vec![Some(t_string())], t_string()),
        ("filepath", "glob") => (vec![Some(t_string())], t_array(t_string(), None)),
        ("filepath", "walk") => (vec![Some(t_string())], t_array(t_string(), None)),
        /* reflect 模块：编译期类型查询，零运行时开销 */
        ("reflect", "typeOf") => (vec![Some(t_object())], t_string()),
        ("reflect", "numField") => (vec![Some(t_object())], t_int()),
        _ => return None,
    })
}

/// `read_line` → `ReadLine`、`now_ms` → `NowMs`：只用于给旧写法一条精确的
/// 迁移提示（文稿 2.3「凡带 `.` 的调用一律 PascalCase」），不参与解析。
pub fn camel_member(name: &str) -> String {
    let mut out = String::new();
    let segs: Vec<&str> = name.split('_').filter(|s| !s.is_empty()).collect();
    for (i, seg) in segs.iter().enumerate() {
        let mut chars = seg.chars();
        if let Some(c) = chars.next() {
            if i == 0 {
                // 第一个单词首字母小写
                out.extend(c.to_lowercase());
            } else {
                // 后续单词首字母大写
                out.extend(c.to_uppercase());
            }
            out.push_str(chars.as_str());
        }
    }
    out
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
            Expr::FloatLit { is_f32, .. } => {
                if let Some(exp) = expected {
                    // 目标写 `decimal?` 时按内层 `decimal` 判：能不能落地与"可不可空"无关
                    let base = strip_nullable(exp);
                    if matches!(&**base, TyS::Decimal) {
                        self.err("E4003", &e.tok, "T-9", "float 字面量不能直接赋给 decimal（C-6）；金额请写 `19.99d`", &[]);
                        return exp.clone();
                    }
                    let lit_ty = if *is_f32 { t_float32() } else { t_float() };
                    if !matches!(&**base, TyS::Float | TyS::Float32) {
                        if is_int(base) {
                            self.err("E4001", &e.tok, "T-5", "类型不匹配：期望 {0}，得到 {1}", &[&self.ts(exp), &self.ts(&lit_ty)]);
                            return exp.clone();
                        }
                    }
                    return lit_ty;
                }
                if *is_f32 { t_float32() } else { t_float() }
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
                        } else if !interp_supported(&pt) && !self.implements_printable(&pt) {
                            let t = self.ts(&pt);
                            self.err("E5006", &expr.tok, "文稿 11.2", "插值暂不支持 `{0}` 类型；先取其中的标量或自己拼接（实现 `Printable.ToString` 后即可直接插值）", &[&t]);
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
                    // 返回 object 类型，避免下游对本类字段的查找再触发 E3003（一处误用恰好一条诊断）
                    return t_object();
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
            Expr::New { .. } => self.check_new(e, expected),
            Expr::IsType { .. } => self.check_is_type(e),
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
                    is_abstract: false,
                    is_override: false,
                    is_pure: false,
                    is_nothrow: false,
                    is_ref_return: false,
                    is_nogc: false,
                    is_deprecated: false,
                    implicit: false,
                gdepth: 0,
                mangle: None,
                is_extern: false,
                is_export: false,
                parent: None,
                nested: BTreeMap::new(),
                    body: Some(leaked),
                });
                self.anon_funcs.push(f);
                // 实例体内合成的匿名函数继承当前实例号：其体已按实例的槽键检查，
                // codegen 生成它时要取到同一批槽
                if self.cur_inst > 0 {
                    let iid = self.cur_inst;
                    self.inst_slot_of.insert(f, iid);
                }
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
            if matches!(&*t, TyS::Float | TyS::Float32 | TyS::Decimal) {
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
        if op == "&" {
            // &x：x 必须是 place（变量、字段、元素），否则 E7007
            let t = self.check_expr(operand, None);
            // E7009 前置于 E7007：const 不能可变借用
            if let Some(res) = self.res_of(operand.id) {
                if matches!(res, Res::Const(_)) {
                    self.err("E7009", &e.tok, "O-16", "`const` 不能可变借用，应写 `&const`", &[]);
                    return t;
                }
            }
            if !self.is_place(operand) {
                self.err("E7007", &e.tok, "O-8", "`&` 只能作用于可写位置（变量、字段、元素）", &[]);
            } else {
                // 仅在操作数为 place 时标记为借用表达式；非-place 已报 E7007，不再重复报 E7003
                self.sl(e.id).is_borrow = true;
            }
            self.sl(e.id).ty = Some(t.clone());
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
            if matches!(&*lt, TyS::Float | TyS::Float32) || matches!(&*rt, TyS::Float | TyS::Float32) {
                self.err("E4011", &lhs.tok, "文稿 3.7", "`float` 不能用 `{0}`（浮点比较请用 math.Equals）", &[&op]);
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
            if matches!(&*lt, TyS::Float | TyS::Float32) {
                self.err("E4017", &lhs.tok, "文稿 7.1", "`float` 不实现 Comparable（浮点比较请用 math.Compare）", &[]);
            } else if let TyS::Param(pn) = &*lt {
                let cons = self.generic_cons.get(pn).cloned().unwrap_or_default();
                if !cons.iter().any(|c| c == "Comparable") {
                    self.err("E4036", &lhs.tok, "T-32", "类型参数 `{0}` 缺少 `Comparable` 约束", &[&pn]);
                }
            } else if matches!(&*lt, TyS::Int(..) | TyS::Decimal | TyS::Str | TyS::Named(DefId::Name("time"), _)) {
                // 内建可比类型直接放行（time 按 Unix 毫秒数值比较）
            } else if let TyS::Named(DefId::Id(d), _) = &*lt {
                // 实现 Comparable 的类：`a > b` 按 `a.Compare(b) > 0` 生成
                let ok = self.types.contains_key("Comparable")
                    && self.implements(*d, self.types["Comparable"]);
                if !ok {
                    self.err("E4017", &lhs.tok, "文稿 7.1", "{0} 不实现 Comparable", &[&self.ts(&lt)]);
                }
            } else {
                self.err("E4017", &lhs.tok, "文稿 7.1", "{0} 不实现 Comparable", &[&self.ts(&lt)]);
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
            // float32 + float32 → float32
            if matches!(&*lt, TyS::Float32) && matches!(&*rt, TyS::Float32) {
                if op == "%" {
                    self.err("E4009", &lhs.tok, "文稿 11.1", "`%` 只能用于整数", &[]);
                }
                return t_float32();
            }
            // float32 与 float 混算：float32 隐式扩大为 float（无损）
            if (matches!(&*lt, TyS::Float32) && matches!(&*rt, TyS::Float))
                || (matches!(&*lt, TyS::Float) && matches!(&*rt, TyS::Float32))
            {
                if op == "%" {
                    self.err("E4009", &lhs.tok, "文稿 11.1", "`%` 只能用于整数", &[]);
                }
                return t_float();
            }
            // float32 与 int 混算：int 隐式转为 float32（无损）
            if (matches!(&*lt, TyS::Float32) && is_int(&rt))
                || (is_int(&lt) && matches!(&*rt, TyS::Float32))
            {
                if op == "%" {
                    self.err("E4009", &lhs.tok, "文稿 11.1", "`%` 只能用于整数", &[]);
                }
                return t_float32();
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
            // 标记变量已被读取（用于 W1005 未使用变量警告）
            v.borrow_mut().is_used = true;
            if v.borrow().state == VarState::Moved {
                self.err("E7001", &e.tok, "O-2", "`{0}` 已被移走，此处不可再用（重新赋一个值可复活）", &[&name]);
            }
            // O-18：ArenaRef 的所有使用点必须在 arena 声明块内（含嵌套块）
            if let Some(r) = v.borrow().arena_region {
                if !self.region_in_scope(r) {
                    let detail = format!("变量 `{0}` 在 arena 块外使用", name);
                    self.err(
                        "E7006",
                        &e.tok,
                        "O-18",
                        "`ArenaRef` 不能逃出 arena 的作用域（{0}）",
                        &[&detail],
                    );
                }
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
        // 具名导入兜底：`import m.{c}` 之后裸 `c` 也要能解析；多模块同名时报 E3045（文稿 9.2，#15）。
        else if let Some(canon) = self.named_import_canon(name, &e.tok) {
            if let Some(c) = self.consts.get(&format!("{}__{}", canon, name)) {
                let c = c.clone();
                self.sl(e.id).res = Some(Res::Const(c.clone()));
                return c.ty.borrow().clone();
            }
        }
        if let Some(&f) = self.qual_lookup(&self.func_by_name, name) {
            let f: usize = f;
            let (vis, module, is_deprecated, params, ret) = {
                let fi = &self.funcs[f];
                (
                    fi.vis.clone(),
                    fi.module.clone(),
                    fi.is_deprecated,
                    fi.params.clone(),
                    fi.ret.clone(),
                )
            };
            if vis != "public" && module != self.cur_mod_path {
                self.err("E3013", &e.tok, "N-8", "`{0}` 不是 public，不能跨模块使用", &[&name]);
            }
            self.sl(e.id).res = Some(Res::Func(f));
            if is_deprecated {
                self.warn("W1004", &e.tok, "文稿 9.3", "符号 `{0}` 已弃用（`@deprecated`），建议不再使用", &[&name]);
            }
            return t_fn(params.iter().map(|p| p.1.clone()).collect(), ret);
        }
        // 具名导入兜底：`import m.{f}` 之后裸 `f` 也要能解析；多模块同名时报 E3045（文稿 9.2，#15）。
        else if let Some(canon) = self.named_import_canon(name, &e.tok) {
            if let Some(&f) = self.func_by_name.get(&format!("{}__{}", canon, name)) {
                let f: usize = f;
                let (vis, module, is_deprecated, params, ret) = {
                    let fi = &self.funcs[f];
                    (
                        fi.vis.clone(),
                        fi.module.clone(),
                        fi.is_deprecated,
                        fi.params.clone(),
                        fi.ret.clone(),
                    )
                };
                if vis != "public" && module != self.cur_mod_path {
                    self.err("E3013", &e.tok, "N-8", "`{0}` 不是 public，不能跨模块使用", &[&name]);
                }
                self.sl(e.id).res = Some(Res::Func(f));
                if is_deprecated {
                    self.warn("W1004", &e.tok, "文稿 9.3", "符号 `{0}` 已弃用（`@deprecated`），建议不再使用", &[&name]);
                }
                return t_fn(params.iter().map(|p| p.1.clone()).collect(), ret);
            }
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
                            // 回退到 codegen 内建模块签名：time.now / time.parse 等
                            // 仍由 codegen 硬编码处理，key 为 ("time", ...) 而非 ("std.time", ...)
                            // canon 可能是 "std.time"（标准库多段名）或 "time"（单段内建模块名），
                            // 需要去掉 "std." 前缀后再匹配 module_sig。
                            let sig_mod = canon.strip_prefix("std.").unwrap_or(&canon);
                            if let Some((_ptypes, ret)) = module_sig(sig_mod, export) {
                                self.sl(e.id).mkind = Some("module");
                                self.sl(e.id).ckind = Some("module");
                                return ret;
                            }
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
        // 内建模块（db/file/io/json/math/string/time 等）无需 import 即可作为模块名使用，
        // 如 `time.Now()` / `math.Compare(...)`；与裸类型名 `time` 的声明位置用法不冲突。
        if BUILTIN_MODULES.contains(&name.as_str()) {
            self.sl(e.id).res = Some(Res::Module(name.clone()));
            return Rc::new(TyS::ModuleT(name.clone()));
        }
        if name == "print" || name == "println" {
            self.sl(e.id).res = Some(Res::Builtin(name.clone()));
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
        if name == "typeof" {
            self.err(
                "E3003",
                &e.tok,
                "文稿 11.2",
                "`typeof` 是编译期函数，要写成 `typeof(表达式)`",
                &[],
            );
            return t_string();
        }
        if name == "sizeof" {
            self.err(
                "E3003",
                &e.tok,
                "文稿 11.2",
                "`sizeof` 是编译期函数，要写成 `sizeof(表达式)`",
                &[],
            );
            return t_int();
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
            // `queue<T>()`：创建元素类型为 T 的阻塞队列，返回 queue<T>（C 层 ax_queue*）。
            if segs.len() == 1
                && name == "queue"
                && targs.len() == 1
                && args.is_empty()
            {
                let elem_ty = self.resolve_type(&targs[0]);
                let qty = t_named(DefId::Name("queue"), vec![elem_ty.clone()]);
                self.sl(e.id).ty = Some(qty.clone());
                self.sl(e.id).ckind = Some("queue_create");
                return qty;
            }
            // 本模块限定名兜底：用户模块体内裸名调用自身的类型构造 / 顶层函数。
            // 零实参调用（如 Mutex()、RWMutex()、WaitGroup()、Once()）不走 conversion，
            // 以免被 E4031“缺少实参”拦截；应落到 res_of 匹配，由 conversion_named→construct
            // 处理零字段/非泛型结构体的零实参构造。
            let treg = self
                .lookup_type_name(name, &callee.tok)
                .map(|_| name.clone());
            if segs.len() == 1
                && !args.is_empty()
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
                // 具名导入兜底：`import m.{f}` 之后裸 `f` 也要能解析；多模块同名时报 E3045（文稿 9.2，#15）。
                if let Some(canon) = self.named_import_canon(name, &callee.tok) {
                    if let Some(&f) = self.func_by_name.get(&format!("{}__{}", canon, name)) {
                        return self.call_func(e, f, args, targs);
                    }
                }
            }
            if segs.len() == 1 && (name == "ok" || name == "fail") {
                return self.call_ok_fail(e, args, name);
            }
            if segs.len() == 1 && name == "typeof" {
                return self.call_typeof(e, args);
            }
            if segs.len() == 1 && name == "sizeof" {
                return self.call_sizeof(e, args);
            }
            if segs.len() == 1 && (name == "print" || name == "println") {
                return self.call_print(e, args, name);
            }
            if segs.len() == 1 && name == "parent" {
                return self.call_parent_ctor(e, args);
            }
            if segs.len() == 1 && name == "thread" {
                return self.call_thread(e, args);
            }
            match self.res_of(callee.id) {
                Some(Res::Type(d)) => {
                    return self.conversion_named(e, args, Some(d), expected)
                }
                Some(Res::BuiltinType(bn)) => {
                    return self.conversion(e, args, &bn, expected)
                }
                _ => {}
            }
        }
        let errs_before = self.d.error_count;
        let t = self.check_expr(callee, None);
        if let TyS::Func(ps, ret) = &*t {
            self.check_call_args(args, Some(ps.as_slice()), None, e.id);
            return ret.clone();
        }
        // 名字本身没解析成功时，check_expr 已经报过 E3003；不要再连着报「不是可调用对象」
        // （一处误用只报一条，否则真正的错因会被后面这条盖住）。
        if self.d.error_count > errs_before {
            return t_object();
        }
        // 类型名作为可调用目标：`Mutex()`、`RWMutex()`、`WaitGroup()`、`Once()` 等
        // 零实参构造调用。check_expr 已把 callee 的 res 设为 Res::Type(d)，返回 TyS::Meta(name)。
        if let TyS::Meta(name) = &*t {
            if let Some(d) = self.lookup_type_name(name, &callee.tok) {
                return self.conversion_named(e, args, Some(d), expected);
            }
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
        borrows: Option<&[Option<String>]>,
        _eid: u32,
    ) {
        for (i, a) in args.iter().enumerate() {
            let exp = want.and_then(|w| w.get(i)).cloned();
            self.check_expr(&a.k.expr, exp.as_ref());
            let rejected = self.reject_arena_ref_arg(&a.k.expr);
            if a.k.borrow {
                // 借用强度由被调函数的形参声明决定（签名负责契约）；签名
                // 不可知（函数值调用等）时保守按可变借用处理
                let mutable = borrows
                    .and_then(|b| b.get(i))
                    .map(|b| b.as_deref() == Some("mut"))
                    .unwrap_or(true);
                self.check_borrow(&a.k.expr, mutable);
                self.record_borrow_use(&a.k.expr, mutable);
            } else if let Some(e) = &exp {
                if !rejected {
                    self.coerce(&a.k.expr, e, "实参");
                    if self.moves(e) {
                        self.take(&a.k.expr);
                    }
                }
            }
            let at = self.ty_of(a.k.expr.id);
            self.sl(a.id).ty = at;
        }
    }

    fn check_borrow(&mut self, e: &ast::EPtr, mutable: bool) {
        match &e.k {
            Expr::Path { segs, .. } => {
                if let Some(v) = self.lookup_var(&segs[0]) {
                    let (is_const, name, ty) = {
                        let b = v.borrow();
                        (b.is_const, b.name.clone(), b.ty.clone())
                    };
                    if is_const && mutable {
                        self.err("E7009", &e.tok, "O-9", "不能对 const `{0}` 取可变借用（写 `&const`）", &[&name]);
                    }
                    // O-10：对 copy 类型借用合法但无收益，等价传值 → W1003 提示
                    if Self::is_copy_ty(&ty) {
                        self.warn("W1003", &e.tok, "O-10", "对 copy 类型 `{0}` 写借用等价于传值，`&` 是多余的", &[&self.ts(&ty)]);
                    }
                }
            }
            Expr::FieldAccess { .. } | Expr::Index { .. } => {}
            _ => {
                self.err("E7007", &e.tok, "O-8", "`&` 只能作用于可写位置（变量、字段、元素）", &[]);
            }
        }
    }

    /// O-10 判据：赋值即整份复制、不走 move 的类型（规格书 11.1 表）。
    /// 堆对象（class / T[] / map / object / ArenaRef）不在其列。
    fn is_copy_ty(t: &Ty) -> bool {
        matches!(
            &**t,
            TyS::Bool
                | TyS::Str
                | TyS::Float
                | TyS::Decimal
                | TyS::Int(..)
                | TyS::Tuple(_)
                | TyS::Func(..)
                | TyS::Record(_)
                | TyS::Enum(_)
                | TyS::Range
        )
    }

    /// region 的 arena 声明作用域是否仍在当前作用域链上
    ///（使用点在声明块内或其嵌套块 → 合法；否则逃逸，O-18）。
    pub fn region_in_scope(&self, r: u32) -> bool {
        let target = match self.arena_regions.get(&r) {
            None => return true, // region 来源不可知（如函数形参），不强查
            Some(s) => s,
        };
        let mut cur = self.scope.clone();
        while let Some(s) = cur {
            if Rc::ptr_eq(&s, target) {
                return true;
            }
            cur = s.borrow().parent.clone();
        }
        false
    }

    /// O-18：ArenaRef 不能作为实参传给任何调用——"会存起来的位置"
    /// 语法上无法穷举，按规格保守全部拦截（区域内的字段赋值走
    /// check_simple，不经这里）。返回 true 表示已拦截，调用方跳过
    /// coerce（ArenaRef 与形参类型不匹配的 E4001 不再叠加）。
    fn reject_arena_ref_arg(&mut self, e: &ast::EPtr) -> bool {
        if let Some(t) = self.ty_of(e.id) {
            if matches!(&*t, TyS::ArenaRef(_)) {
                self.err(
                    "E7006",
                    &e.tok,
                    "O-18",
                    "`ArenaRef` 不能逃出 arena 的作用域（{0}）",
                    &[&"作为实参传出调用"],
                );
                return true;
            }
        }
        false
    }

    /// 记录一条借用使用（O-7）：强度来自被调函数的形参声明——
    /// 借用强度由签名负责，调用点不重复标注（文稿 8.2）。
    fn record_borrow_use(&mut self, e: &ast::EPtr, mutable: bool) {
        if let Some(place) = self.place_key(e) {
            self.stmt_uses.push(StmtUse {
                place,
                kind: if mutable {
                    UseKind::BorrowMut
                } else {
                    UseKind::BorrowConst
                },
                tok: e.tok.clone(),
            });
        }
    }

    /// 语句出口扫描（O-7/O-12，规格书 11.4 的单语句一遍扫描）：
    /// 同一 place 上借用互斥——多个 `&const` 合法共存；出现可变借用、
    /// move 或写即冲突（move 视作独占使用）。借用不跨语句：
    /// 扫描完语句即失效，这是语言设计（借用不可存储）的直接结果。
    pub fn scan_stmt_uses(&mut self) {
        let uses = std::mem::take(&mut self.stmt_uses);
        if uses.len() < 2 {
            return;
        }
        for i in 0..uses.len() {
            for j in (i + 1)..uses.len() {
                let a = &uses[i];
                let b = &uses[j];
                if !Self::places_conflict(&a.place, &b.place) {
                    continue;
                }
                let ab = matches!(a.kind, UseKind::BorrowConst | UseKind::BorrowMut);
                let bb = matches!(b.kind, UseKind::BorrowConst | UseKind::BorrowMut);
                if ab && bb {
                    if a.kind == UseKind::BorrowConst && b.kind == UseKind::BorrowConst {
                        continue; // 多只读借用共存（O-7 明确允许）
                    }
                    self.err(
                        "E7002",
                        &b.tok,
                        "O-7",
                        "同一语句内对 `{0}` 有多个借用（至少一个是可变的）",
                        &[&a.place],
                    );
                    return;
                }
                if ab || bb {
                    // 借用期间所有者被 move / 写（O-12）
                    let place = if ab { &a.place } else { &b.place };
                    self.err(
                        "E7004",
                        &b.tok,
                        "O-12",
                        "`{0}` 正在被借用，同一语句内不能移动或写入它",
                        &[place],
                    );
                    return;
                }
                // Move+Move：状态机已对第二个使用报 E7001，不在此重复
            }
        }
    }

    /// 两个 place 是否指到同一存储：精确相等，或一个是另一个的
    /// 字段/元素前缀（借容器与写元素互斥：`&a` 与 `a[0] = 1` 冲突）。
    fn places_conflict(a: &str, b: &str) -> bool {
        if a == b {
            return true;
        }
        let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
        long.starts_with(short)
            && matches!(long[short.len()..].chars().next(), Some('.') | Some('['))
    }

    /// 位置键（O-7 借用扫描用）：`x` / `x.f` / `xs[]`——
    /// 根变量名 + 字段/元素链，同根即可能同存储。
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

    /// `sizeof(表达式)`（文稿 11.2）：编译期生成 C 的 `sizeof`，返回
    /// **实参类型在 C 里的存储大小**（`int`）——`int` = 8、`uint8` = 1、
    /// `string` = `sizeof(ax_str)`、`int[]` = `sizeof(ax_arr)`、class 按引用
    /// 语义给**指针大小**（与赋值语义一致，对象本身的大小不给）。值 sema
    /// 不算，交给 C 编译器，所以不能用于 `const`（声明会报"必须是常量表达式"）。
    fn call_sizeof(&mut self, e: &ast::EPtr, args: &[ast::N<ast::Arg>]) -> Ty {
        if args.len() != 1 {
            self.err(
                "E4031",
                &e.tok,
                "文稿 11.2",
                "`sizeof` 需要 1 个实参（表达式），得到 {1}",
                &[&args.len()],
            );
            return t_int();
        }
        let arg = &args[0].k.expr;
        let errs_before = self.d.error_count;
        let t = self.check_expr(arg, None);
        // 实参本身有错时不再追问类型，避免一处误用两条诊断
        if self.d.error_count > errs_before {
            return t_int();
        }
        if matches!(&*t, TyS::Void | TyS::Never) {
            self.err("E4001", &arg.tok, "文稿 11.2", "`sizeof` 的实参不能是 `void`", &[]);
            return t_int();
        }
        self.sl(args[0].id).ty = Some(t);
        self.sl(e.id).ckind = Some("sizeof");
        t_int()
    }

    /// `thread(func, arg)`：在新线程中并发执行 `func(arg)`，返回线程句柄。
    /// `func` 必须是 `func(void*)` 类型（或可隐式转为该签名），`arg` 作为唯一实参传入。
    fn call_thread(&mut self, e: &ast::EPtr, args: &[ast::N<ast::Arg>]) -> Ty {
        if args.len() != 2 {
            self.err(
                "E4031",
                &e.tok,
                "文稿 14.1",
                "`thread` 需要 2 个实参（函数, 上下文），得到 {1}",
                &[&args.len()],
            );
            return t_named(DefId::Name("thread"), Vec::new());
        }
        let farg = &args[0].k.expr;
        let errs_before = self.d.error_count;
        let ft = self.check_expr(farg, None);
        if self.d.error_count > errs_before {
            return t_named(DefId::Name("thread"), Vec::new());
        }
        if !matches!(&*ft, TyS::Func(..)) {
            self.err(
                "E4001",
                &farg.tok,
                "文稿 14.1",
                "`thread` 的第一个实参必须是函数类型，得到 {0}",
                &[&self.ts(&ft)],
            );
            return t_named(DefId::Name("thread"), Vec::new());
        }
        // 上下文参数，类型不限，作为 void* 传递；仍需 check_expr 以便
        // 为变量引用设置 res（否则 codegen 回落到源码名，而非 C 变量名 v_x）。
        self.check_expr(&args[1].k.expr, None);
        self.sl(e.id).ckind = Some("thread");
        t_named(DefId::Name("thread"), Vec::new())
    }

    /// `typeof(表达式)`（文稿 11.2）：**编译期常量**，返回实参静态类型的显示名
    /// （`int` / `int8` / `string` / `Dog` / `int[]` / `int?` / `map<string, object>`）。
    /// 不是运行期查询——codegen 直接生成字符串字面量，零运行时开销；
    /// 运行期判别仍走 `match` 类型模式（文稿 5.3），两者不重叠。
    fn call_typeof(&mut self, e: &ast::EPtr, args: &[ast::N<ast::Arg>]) -> Ty {
        if args.len() != 1 {
            self.err(
                "E4031",
                &e.tok,
                "文稿 11.2",
                "`typeof` 需要 1 个实参（表达式），得到 {1}",
                &[&args.len()],
            );
            return t_string();
        }
        let arg = &args[0].k.expr;
        let errs_before = self.d.error_count;
        let t = self.check_expr(arg, None);
        // 实参本身有错时不再追问类型，避免一处误用两条诊断
        if self.d.error_count > errs_before {
            return t_string();
        }
        let name = self.ts(&t);
        self.sl(args[0].id).ty = Some(t);
        self.sl(e.id).ckind = Some("typeof");
        self.sl(e.id).bname = Some(name.clone());
        self.sl(e.id).const_value = Some(ConstVal::Str(name));
        t_string()
    }

    /// `is(T == Type)` / `is(T : Interface)` / `is(typeof(Expr) == Type)`：编译期类型查询，
    /// 返回 bool 常量，零运行期代价。
    fn check_is_type(&mut self, e: &ast::EPtr) -> Ty {
        let (kind, ty_node, target_node, expr_node, iface_path) = match &e.k {
            Expr::IsType { kind, ty, target, expr, interface } => {
                (kind.clone(), ty.clone(), target.clone(), expr.clone(), interface.clone())
            }
            _ => return t_bool(),
        };

        let result = match kind.as_str() {
            "eq" => {
                let lt = self.resolve_type(ty_node.as_ref().unwrap());
                let rt = self.resolve_type(target_node.as_ref().unwrap());
                ConstVal::Bool(lt == rt)
            }
            "impl" => {
                let lt = self.resolve_type(ty_node.as_ref().unwrap());
                let iface_name = iface_path.as_ref().unwrap().join(".");
                // 查找 interface 定义，检查 lt 是否实现了该 interface
                let mut found = false;
                if let Some(d) = self.lookup_type_name(&iface_name, &e.tok) {
                    let td = &self.typedefs[d as usize];
                    if td.kind == "interface" {
                        // 检查 lt 是否是 class/struct 且其 impls 包含该 interface
                        let type_name = self.ts(&lt);
                        if let Some(td_id) = self.lookup_type_name(&type_name, &e.tok) {
                            let src_td = &self.typedefs[td_id as usize];
                            found = src_td.impls.contains(&d);
                        }
                    }
                }
                ConstVal::Bool(found)
            }
            "typeof_eq" => {
                let et = self.check_expr(expr_node.as_ref().unwrap(), None);
                let rt = self.resolve_type(target_node.as_ref().unwrap());
                ConstVal::Bool(et == rt)
            }
            _ => ConstVal::Bool(false),
        };

        self.sl(e.id).ty = Some(t_bool());
        self.sl(e.id).const_value = Some(result);
        t_bool()
    }

    /// `print` / `println`（文稿 11.2）：签名是 `void print(string)`，但允许
    /// 「可直接插值的单个值」直接传——语义等价于 `print("{x}")`，只是省掉花括号。
    /// 仍然只有一种格式化能力（插值），占位符与多实参依旧不支持。
    /// `print` / `println`（以及等价入口 `fmt.Print` / `fmt.Println`）。
    ///
    /// 文稿 11.2（0.0.27 起）：**多实参合法**，实参之间以一个空格分隔输出
    /// （FreeBASIC 风格的方便写法；分隔符固定为空格，写死在规则里就不再"没约定就不猜"）。
    /// 每个实参独立走原先单实参的规则：string 直接输出、可插值的值按插值显示、
    /// 复合类型（数组/map/元组等）仍报 E4001。0 个实参合法（`println()` 打印一个空行）。
    pub(crate) fn call_print(
        &mut self,
        e: &ast::EPtr,
        args: &[ast::N<ast::Arg>],
        name: &str,
    ) -> Ty {
        for a in args {
            if a.k.borrow {
                // `print(&x)` 沿用旧路径：按 string 强转
                let t = self.check_expr(&a.k.expr, None);
                if matches!(&*t, TyS::Str) {
                    self.sl(a.k.expr.id).ty = Some(t);
                } else {
                    self.coerce(&a.k.expr, &t_string(), "实参");
                }
                continue;
            }
            let errs_before = self.d.error_count;
            let t = self.check_expr(&a.k.expr, None);
            // 实参本身有错（E3003 等）时不再追问类型，避免一处误用两条诊断
            if self.d.error_count > errs_before {
                continue;
            }
            if matches!(&*t, TyS::Str) {
                // 常规形态：本来就是字符串，直接输出
                self.sl(a.k.expr.id).ty = Some(t);
                continue;
            }
            let interpable = !matches!(&*t, TyS::Object | TyS::Void)
                && (interp_supported(&t) || self.implements_printable(&t));
            if interpable {
                // 非 string 但可插值：标在实参节点上，codegen 按 `"{x}"` 拼串输出
                self.sl(a.k.expr.id).ty = Some(t);
                self.sl(a.k.expr.id).interp_arg = true;
                continue;
            }
            self.err(
                "E4001",
                &a.tok,
                "文稿 11.2",
                "`{0}` 的实参必须是 string，或能直接插值的值（数字 / 布尔 / 字符串 / 枚举 / 实现了 Printable 的类），得到 {1}",
                &[&name, &self.ts(&t)],
            );
        }
        self.sl(e.id).ckind = Some("builtin");
        // `fmt.Print` / `fmt.Println` 与 `print` / `println` 同实现，codegen 按小写基名分派
        let base = match name {
            "fmt.Print" => "print",
            "fmt.Println" => "println",
            other => other,
        };
        self.sl(e.id).bname = Some(base.to_string());
        t_void()
    }

    fn call_plain(
        &mut self,
        e: &ast::EPtr,
        args: &[ast::N<ast::Arg>],
        name: &str,
        ptypes: Vec<Ty>,
        ret: Ty,
    ) -> Ty {
        self.check_call_args(args, Some(&ptypes), None, e.id);
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
        let (fname, generics, is_pure, _is_nothrow) = {
            let fi = &self.funcs[f];
            (fi.name.clone(), fi.generics.clone(), fi.is_pure, fi.is_nothrow)
        };
        // pure 函数内不能调用非纯函数
        if let Some(cur_f) = self.current_func {
            if self.funcs[cur_f].is_pure && !is_pure {
                let cur_name = self.funcs[cur_f].name.clone();
                self.err("E6015", &e.tok, "文稿 10.1", "`pure` 函数 `{0}` 不能调用非纯函数 `{1}`", &[&cur_name, &fname]);
            }
        }
        let mut generic_args: Vec<Ty> = Vec::new();
        // 推断失败（E4037）时不做实例化——object 兜底实参不该生成实例
        let mut infer_failed = false;
        // 推断路径下实参先按无期望类型预查一遍（infer_generics 依赖实参类型，
        // 而主循环的 check_expr 用替换后的形参类型作期望——顺序必须如此）
        let mut prechecked = false;
        if !callee_targs.is_empty() {
            if callee_targs.len() != generics.len() {
                self.err("E4035", &e.tok, "G-1", "`{0}` 需要 {1} 个类型实参", &[&fname, &generics.len()]);
            }
            let mut subst: HashMap<String, Ty> = HashMap::new();
            for ((g, _), a) in generics.iter().zip(callee_targs.iter()) {
                subst.insert(g.clone(), self.resolve_type(a));
            }
            if !self.check_generic_constraints(&generics, &subst, &e.tok) {
                // 约束不满足：不实例化（E4036 已报），调用点按未实例化处理
                self.sl(e.id).ckind = Some("func");
                self.sl(e.id).func = Some(f);
                return t_object();
            }
            generic_args = generics
                .iter()
                .map(|(g, _)| subst.get(g).cloned().unwrap_or_else(t_object))
                .collect();
        } else if !generics.is_empty() {
            let mut arg_tys: Vec<Ty> = Vec::with_capacity(args.len());
            for a in args {
                // 匿名函数没有目标类型就报 E4043：推断期还没有形参类型可当
                // 期望，跳过它，留到主循环按替换后的形参类型检查
                if matches!(a.k.expr.k, Expr::AnonFunc { .. }) {
                    arg_tys.push(t_object());
                    continue;
                }
                arg_tys.push(self.check_expr(&a.k.expr, None));
            }
            match self.infer_generics(f, &arg_tys) {
                None => {
                    self.err("E4037", &e.tok, "T-16", "无法推断 `{0}` 的类型实参，请显式写 `{1}<...>(...)`", &[&fname, &fname]);
                    generic_args = vec![t_object(); generics.len()];
                    infer_failed = true;
                }
                Some(inf) => {
                    generic_args = inf;
                    let subst: HashMap<String, Ty> = generics
                        .iter()
                        .zip(generic_args.iter())
                        .map(|((g, _), t)| (g.clone(), t.clone()))
                        .collect();
                    if !self.check_generic_constraints(&generics, &subst, &e.tok) {
                        self.sl(e.id).ckind = Some("func");
                        self.sl(e.id).func = Some(f);
                        return t_object();
                    }
                }
            }
            prechecked = true;
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
                .map(|(n, t, b)| {
                    let t2 = self.subst_ty(&t, &subst);
                    (n, self.concretize(&t2, &e.tok), b)
                })
                .collect();
            let r2 = self.subst_ty(&self.funcs[f].ret, &subst);
            ret = self.concretize(&r2, &e.tok);
        } else {
            ret = self.funcs[f].ret.clone();
        }
        // —— 单态化（GAP 泛型 #1，GX-01/02/09）：每个 (函数, 类型实参) 组合
        // 生成一份 FuncInfo 实例（参数/返回已替换、体共享 AST），实例体在
        // drain_insts 里按具体类型重查；调用点指向实例，codegen 只生成实例。
        let mut target_f = f;
        if !generics.is_empty() && !generic_args.is_empty() && !infer_failed {
            let mangle = self.mangle_inst(&fname, &generic_args);
            if let Some(&inst) = self.inst_registry.get(&mangle) {
                target_f = inst;
            } else {
                let depth = self.cur_gdepth + 1;
                // 深度超限：报一次 E4038，实例照常物化（签名具体、调用点类型
                // 正确、后续收敛不级联），只是不再查它的函数体
                let check_body = depth <= 64;
                if !check_body && !self.depth_err_reported {
                    self.err(
                        "E4038",
                        &e.tok,
                        "T-37",
                        "泛型实例化递归超过 64 层：`{0}`",
                        &[&mangle],
                    );
                    self.depth_err_reported = true;
                }
                if self.inst_registry.contains_key(&mangle) {
                    target_f = self.inst_registry[&mangle];
                } else {
                    let fi = &self.funcs[f];
                    let inst_fi = crate::sema::FuncInfo {
                        name: fi.name.clone(),
                        tok: fi.tok.clone(),
                        module: fi.module.clone(),
                        owner: fi.owner,
                        params: params.clone(),
                        ret: ret.clone(),
                        generics: Vec::new(),
                        vis: fi.vis.clone(),
                        is_static: fi.is_static,
                        is_override: fi.is_override,
                        is_abstract: fi.is_abstract,
                        is_pure: fi.is_pure,
                        is_nothrow: fi.is_nothrow,
                        is_ref_return: fi.is_ref_return,
                        is_nogc: fi.is_nogc,
                        is_deprecated: fi.is_deprecated,
                        body: fi.body,
                        implicit: false,
                        gdepth: depth,
                        mangle: Some(mangle.clone()),
                        is_extern: fi.is_extern,
                        is_export: fi.is_export,
                        parent: None,
                        nested: BTreeMap::new(),
                    };
                    let inst = self.funcs.len();
                    self.funcs.push(inst_fi);
                    let iid = self.next_inst;
                    self.next_inst += 1;
                    self.inst_registry.insert(mangle, inst);
                    self.inst_slot_of.insert(inst, iid);
                    let env: HashMap<String, Ty> = generics
                        .iter()
                        .zip(generic_args.iter())
                        .map(|((g, _), t)| (g.clone(), t.clone()))
                        .collect();
                    self.inst_env.insert(inst, env);
                    if check_body {
                        self.pending_insts.push(inst);
                    }
                    target_f = inst;
                }
            }
        }
        // 实参个数
        if args.len() != params.len() {
            self.err("E4031", &e.tok, "N-20", "`{0}` 需要 {1} 个实参，得到 {2}", &[&fname, &params.len(), &args.len()]);
        }
        for (i, a) in args.iter().enumerate() {
            let exp = params.get(i).map(|p| p.1.clone());
            if !prechecked {
                self.check_expr(&a.k.expr, exp.as_ref());
            } else if matches!(a.k.expr.k, Expr::AnonFunc { .. }) {
                self.check_expr(&a.k.expr, exp.as_ref());
            }
            let rejected = self.reject_arena_ref_arg(&a.k.expr);
            if a.k.borrow {
                let mutable = params.get(i).map(|p| p.2.as_deref() == Some("mut")).unwrap_or(true);
                self.check_borrow(&a.k.expr, mutable);
                self.record_borrow_use(&a.k.expr, mutable);
            } else if let Some(ex) = &exp {
                if !rejected {
                    self.coerce(&a.k.expr, ex, "实参");
                    if self.moves(ex) {
                        self.take(&a.k.expr);
                    }
                }
            }
            let at = self.ty_of(a.k.expr.id);
            let s = self.sl(a.id);
            s.ty = at;
            s.decl_ty = exp;
        }
        let s = self.sl(e.id);
        s.ckind = Some("func");
        s.func = Some(target_f);
        ret
    }

    /// 实例 mangle 名：`函数名$类型mangle…`。稳定（同名同实参必同串）、
    /// 去 C 标识符以外字符，用于实例注册表去重与 C 函数名。
    pub(crate) fn mangle_inst(&self, fname: &str, args: &[Ty]) -> String {
        let parts: Vec<String> = args.iter().map(|t| self.mangle_ty(t)).collect();
        format!("{}${}", fname, parts.join("_"))
    }

    /// 类型 → C 安全的 mangle 片段（`int` / `uint8` / `map_string_int` /
    /// `Dog_Animal` / `opt_int` …）。与显示名分离：显示名给人看，这个给 C。
    fn mangle_ty(&self, t: &Ty) -> String {
        match &**t {
            TyS::Bool => "bool".to_string(),
            TyS::Str => "string".to_string(),
            TyS::Float => "float".to_string(),
            TyS::Decimal => "decimal".to_string(),
            TyS::Object => "object".to_string(),
            TyS::Null => "null".to_string(),
            TyS::Int(w, s) => format!("{}int{}", if *s { "" } else { "u" }, w),
            TyS::Array(e, None) => format!("arr_{}", self.mangle_ty(e)),
            TyS::Array(e, Some(n)) => format!("fix{}_{}", n, self.mangle_ty(e)),
            TyS::Map(k, v) => format!("map_{}_{}", self.mangle_ty(k), self.mangle_ty(v)),
            TyS::Nullable(i) => format!("opt_{}", self.mangle_ty(i)),
            TyS::Result(r) => format!("res_{}", self.mangle_ty(r)),
            TyS::Tuple(elems) => {
                let ps: Vec<String> = elems.iter().map(|t| self.mangle_ty(t)).collect();
                format!("tuple_{}", ps.join("_"))
            }
            TyS::Func(ps, r) => {
                let mut all: Vec<String> = ps.iter().map(|t| self.mangle_ty(t)).collect();
                all.push(self.mangle_ty(r));
                format!("fn_{}", all.join("_"))
            }
            TyS::Named(DefId::Id(id), as_) => {
                let base = self
                    .typedefs
                    .get(*id as usize)
                    .map(|d| d.name.clone())
                    .or_else(|| self.inst_names.get(id).cloned())
                    .unwrap_or_else(|| "t".to_string());
                if as_.is_empty() {
                    base
                } else {
                    let ps: Vec<String> = as_.iter().map(|t| self.mangle_ty(t)).collect();
                    format!("{}_{}", base, ps.join("_"))
                }
            }
            TyS::Named(DefId::Name(n), as_) => {
                if as_.is_empty() {
                    n.to_string()
                } else {
                    let ps: Vec<String> = as_.iter().map(|t| self.mangle_ty(t)).collect();
                    format!("{}_{}", n, ps.join("_"))
                }
            }
            TyS::Enum(id) => self
                .typedefs
                .get(*id as usize)
                .map(|d| d.name.clone())
                .unwrap_or_else(|| "enum".to_string()),
            TyS::Param(n) => format!("T_{}", n),
            _ => {
                let d = self.ts(t);
                d.chars()
                    .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
                    .collect()
            }
        }
    }

    /// 校验类型实参是否满足约束；返回 false 表示有 `E4036`（调用方据此
    /// 跳过实例化——类型不合法的实例不该生成，也不该再查它的体）。
    pub(crate) fn check_generic_constraints(
        &mut self,
        generics: &[(String, Vec<String>)],
        subst: &HashMap<String, Ty>,
        tok: &crate::frontend::lexer::Tok,
    ) -> bool {
        let mut ok = true;
        for (name, ty) in subst {
            let cons = generics
                .iter()
                .find(|(g, _)| g == name)
                .map(|(_, c)| c.clone())
                .unwrap_or_default();
            for c in &cons {
                let is_builtin = matches!(c.as_str(), "Comparable" | "Hashable" | "Printable");
                // 约束必须是 interface 名：内置约束或可解析的类型（含跨模块导入）
                let constraint_def = if !is_builtin {
                    self.lookup_type_name(c, tok)
                } else {
                    None
                };
                if !is_builtin && constraint_def.is_none() {
                    self.err("E4036", tok, "T-31", "约束 `{0}` 必须是 interface", &[&c]);
                    ok = false;
                    continue;
                }
                if c == "Comparable" && matches!(&**ty, TyS::Float) {
                    self.err("E4036", tok, "T-33", "`float` 不实现 Comparable", &[]);
                    ok = false;
                }
                if matches!(&**ty, TyS::Param(_)) {
                    continue;
                }
                if let Some(ctid) = constraint_def {
                    if let TyS::Named(DefId::Id(id), _) = &**ty {
                        let impl_ok = match self.typedefs.get(*id as usize) {
                            Some(d) => self.implements(d.id, ctid),
                            None => false,
                        };
                        if !impl_ok {
                            self.err("E4036", tok, "T-36", "{0} 不满足约束 `{1}`", &[&self.ts(ty), &c]);
                            ok = false;
                        }
                    }
                }
            }
        }
        ok
    }

    fn infer_generics(&mut self, f: usize, arg_tys: &[Ty]) -> Option<Vec<Ty>> {
        let names: Vec<String> = self.funcs[f].generics.iter().map(|g| g.0.clone()).collect();
        let params = self.funcs[f].params.clone();
        let mut subst: HashMap<String, Ty> = HashMap::new();
        for (i, aty) in arg_tys.iter().enumerate() {
            if i >= params.len() {
                break;
            }
            let pty = params[i].1.clone();
            self.unify(&pty, aty, &names, &mut subst);
        }
        if subst.len() != names.len() {
            return None;
        }
        Some(names.iter().map(|n| subst[n].clone()).collect())
    }

    pub(crate) fn unify(&self, p: &Ty, a: &Ty, names: &[String], subst: &mut HashMap<String, Ty>) {
        if let TyS::Param(pn) = &**p {
            if names.contains(pn) {
                subst.entry(pn.clone()).or_insert_with(|| a.clone());
                return;
            }
        }
        match (&**p, &**a) {
            // 泛型类形参 `Box<T>` 对实例实参 `Box$int`：形参 id 是模板、实参
            // id 是实例，按 mat_of 回到模板再逐个统一类型实参
            (
                TyS::Named(DefId::Id(pid), pargs),
                TyS::Named(DefId::Id(aid), aargs),
            ) if *pid != *aid => {
                let same_tmpl = self.mat_of.get(aid).map(|(t, _)| *t == *pid);
                let inst_targs = self.mat_of.get(aid).map(|(_, t)| t.clone());
                if same_tmpl == Some(true) && pargs.len() == inst_targs.as_ref().map(|t| t.len()).unwrap_or(0) {
                    let it = inst_targs.unwrap();
                    for (pp, aa) in pargs.iter().zip(it.iter()) {
                        self.unify(pp, aa, names, subst);
                    }
                }
            }
            (TyS::Named(DefId::Id(pid), pargs), TyS::Named(DefId::Id(aid), aargs))
                if *pid == *aid =>
            {
                for (pp, aa) in pargs.iter().zip(aargs.iter()) {
                    self.unify(pp, aa, names, subst);
                }
            }
            (TyS::Array(p1, _), TyS::Array(a1, _)) => self.unify(p1, a1, names, subst),
            (TyS::Map(p1, p2), TyS::Map(a1, a2)) => {
                self.unify(p1, a1, names, subst);
                self.unify(p2, a2, names, subst);
            }
            (TyS::Nullable(p1), TyS::Nullable(a1)) => self.unify(p1, a1, names, subst),
            (TyS::Result(p1), TyS::Result(a1)) => self.unify(p1, a1, names, subst),
            (TyS::Func(ps, r), TyS::Func(ps2, r2)) => {
                for (p, a) in ps.iter().zip(ps2.iter()) {
                    self.unify(p, a, names, subst);
                }
                self.unify(r, r2, names, subst);
            }
            _ => {}
        }
    }

    /// 类型树里是否还有未替换的 `Param`（深查——`Box<Box<T>>` 的外层实参
    /// 是 Named 而内层才是 Param，浅查会把模板误物化）
    pub fn ty_contains_param(&self, t: &Ty) -> bool {
        match &**t {
            TyS::Param(_) => true,
            TyS::Named(_, args) => args.iter().any(|a| self.ty_contains_param(a)),
            TyS::Array(e, _) => self.ty_contains_param(e),
            TyS::Map(k, v) => self.ty_contains_param(k) || self.ty_contains_param(v),
            TyS::Nullable(i) => self.ty_contains_param(i),
            TyS::Result(i) => self.ty_contains_param(i),
            TyS::Tuple(elems) => elems.iter().any(|a| self.ty_contains_param(a)),
            TyS::Func(ps, r) => {
                ps.iter().any(|a| self.ty_contains_param(a)) || self.ty_contains_param(r)
            }
            _ => false,
        }
    }

    /// 把类型里所有"泛型模板 + 具体实参"的 Named 落成实例（GX-04）。
    /// subst_ty 只做词法替换，替换结果仍指向模板；实例化统一在这里收口。
    pub fn concretize(&mut self, t: &Ty, tok: &crate::frontend::lexer::Tok) -> Ty {
        match &**t {
            TyS::Named(DefId::Id(id), args) if !args.is_empty() => {
                let tmpl_gen = self.typedefs.get(*id as usize).map(|d| d.generics.len());
                let kind = self.typedefs.get(*id as usize).map(|d| d.kind);
                if tmpl_gen == Some(args.len())
                    && matches!(kind, Some("class") | Some("struct"))
                    && !args.iter().any(|a| self.ty_contains_param(a))
                {
                    let args2: Vec<Ty> =
                        args.iter().map(|a| self.concretize(a, tok)).collect();
                    let inst = self.materialize(*id, &args2, tok);
                    return t_named(DefId::Id(inst), Vec::new());
                }
                let args2: Vec<Ty> = args.iter().map(|a| self.concretize(a, tok)).collect();
                t_named(DefId::Id(*id), args2)
            }
            TyS::Named(DefId::Id(_), _) => t.clone(),
            TyS::Named(DefId::Name(n), args) => {
                let args2: Vec<Ty> = args.iter().map(|a| self.concretize(a, tok)).collect();
                t_named(DefId::Name(*n), args2)
            }
            TyS::Array(e, l) => t_array(self.concretize(e, tok), *l),
            TyS::Map(k, v) => t_map(self.concretize(k, tok), self.concretize(v, tok)),
            TyS::Nullable(i) => t_nullable(self.concretize(i, tok)),
            TyS::Result(i) => t_result(self.concretize(i, tok)),
            _ => t.clone(),
        }
    }

    /// 泛型 class/struct 单态化：以模板为蓝本生成实例 typedef（字段、ctor、
    /// 方法签名全部替换为具体类型），方法体入队按实例重查。同名同实参只
    /// 物化一次（注册表去重），递归深度超 64 报 `E4038`（GX-09）。
    pub fn materialize(&mut self, tmpl_id: u32, targs: &[Ty], tok: &crate::frontend::lexer::Tok) -> u32 {
        let tmpl = self.typedefs[tmpl_id as usize].clone();
        let parts: Vec<String> = targs.iter().map(|t| self.mangle_ty(t)).collect();
        let mangle = format!("{}${}", tmpl.name, parts.join("_"));
        if let Some(&inst) = self.mat_registry.get(&mangle) {
            return inst;
        }
        // 链深取两者较大：物化递归期间看 cur_mat_depth；实例体检查期间
        // cur_mat_depth 已还原，看当前实例的 gdepth（check_func 会带上）。
        // 深度超限：报一次 E4038，实例照常物化（签名具体、类型不级联），
        // 只是方法体不再按实例重查。
        let depth = self.cur_mat_depth.max(self.cur_gdepth) + 1;
        let check_body = depth <= 64;
        if !check_body && !self.depth_err_reported {
            self.err("E4038", tok, "T-37", "泛型实例化递归超过 64 层：`{0}`", &[&mangle]);
            self.depth_err_reported = true;
        }
        let saved_depth = self.cur_mat_depth;
        self.cur_mat_depth = depth;
        // 实例号先占位登记：构造字段/方法签名时的自引用（`Node<Node<T>>` 的
        // 内层就是自己）直接命中注册表返回本实例，不再入递归
        let inst = self.typedefs.len() as u32;
        self.mat_registry.insert(mangle.clone(), inst);
        self.inst_names.insert(inst, mangle.clone());
        // 骨架 typedef 立即入表：构造期间的递归物化（自引用/嵌套实参）再算
        // typedefs.len() 时不会撞上同一个 id
        self.typedefs.push(crate::sema::TypeDef {
            id: inst,
            name: mangle.clone(),
            kind: tmpl.kind,
            tok: tmpl.tok.clone(),
            vis: tmpl.vis.clone(),
            generics: Vec::new(),
            is_abstract: false,
            is_final: false,
            fields: Vec::new(),
            methods: std::collections::BTreeMap::new(),
            ctors: HashMap::new(),
            parent: None,
            impls: Vec::new(),
            extends: Vec::new(),
            variants: Vec::new(),
            alias_target: None,
            alias_node: None,
            nested: std::collections::BTreeMap::new(),
            expanding: false,
        });
        let tmod0 = self.typedef_mod.get(tmpl_id as usize).cloned().flatten();
        self.typedef_mod.push(tmod0);
        let subst: HashMap<String, Ty> = tmpl
            .generics
            .iter()
            .zip(targs.iter())
            .map(|((g, _), t)| (g.clone(), t.clone()))
            .collect();
        let env: HashMap<String, Ty> = subst.clone();
        // 实例号：该类实例的所有方法共享一个槽隔离空间
        let iid = self.next_inst;
        self.next_inst += 1;
        let mut fields = Vec::new();
        for (fn_, ft, vis, idx) in &tmpl.fields {
            let ft2 = self.subst_ty(ft, &subst);
            let ft2 = self.concretize(&ft2, tok);
            fields.push((fn_.clone(), ft2, vis.clone(), *idx));
        }
        // 方法 / ctor：签名替换，体共享 AST，入队按实例重查
        let mut methods = tmpl.methods.clone();
        for (mn, fid) in tmpl.methods.iter() {
            let (fname, ftok, fmod, fparams, fret, fvis, fstatic, foverride, fbody, fpure, fnothrow, frefret, fnogc, fdeprecated) = {
                let fi = &self.funcs[*fid];
                (
                    fi.name.clone(),
                    fi.tok.clone(),
                    fi.module.clone(),
                    fi.params.clone(),
                    fi.ret.clone(),
                    fi.vis.clone(),
                    fi.is_static,
                    fi.is_override,
                    fi.body,
                    fi.is_pure,
                    fi.is_nothrow,
                    fi.is_ref_return,
                    fi.is_nogc,
                    fi.is_deprecated,
                )
            };
            let mut params = fparams;
            for p in params.iter_mut() {
                p.1 = self.subst_ty(&p.1, &subst);
                p.1 = self.concretize(&p.1, tok);
            }
            let ret = self.subst_ty(&fret, &subst);
            let ret = self.concretize(&ret, tok);
            let nfid = self.funcs.len();
            self.funcs.push(crate::sema::FuncInfo {
                name: fname,
                tok: ftok,
                module: fmod,
                owner: None, // 先占位，实例 typedef 建好后再改指
                params,
                ret,
                generics: Vec::new(),
                vis: fvis,
                is_static: fstatic,
                is_override: foverride,
                is_abstract: false,
                is_pure: fpure,
                is_nothrow: fnothrow,
                is_ref_return: frefret,
                is_nogc: fnogc,
                is_deprecated: fdeprecated,
                body: fbody,
                implicit: false,
                gdepth: depth,
                mangle: None,
                is_extern: false,
                is_export: false,
                parent: None,
                nested: BTreeMap::new(),
            });
            self.inst_slot_of.insert(nfid, iid);
            self.inst_env.insert(nfid, env.clone());
            if check_body {
                self.pending_insts.push(nfid);
            }
            methods.insert(mn.clone(), nfid);
        }
        let mut ctors = HashMap::new();
        for (cname, &cf) in &tmpl.ctors {
            let (fname, ftok, fmod, fparams, fret, fvis, fstatic, foverride, fbody, fpure, fnothrow, frefret, fnogc, fdeprecated) = {
                let fi = &self.funcs[cf];
                (
                    fi.name.clone(),
                    fi.tok.clone(),
                    fi.module.clone(),
                    fi.params.clone(),
                    fi.ret.clone(),
                    fi.vis.clone(),
                    fi.is_static,
                    fi.is_override,
                    fi.body,
                    fi.is_pure,
                    fi.is_nothrow,
                    fi.is_ref_return,
                    fi.is_nogc,
                    fi.is_deprecated,
                )
            };
            let mut params = fparams;
            for p in params.iter_mut() {
                p.1 = self.subst_ty(&p.1, &subst);
                p.1 = self.concretize(&p.1, tok);
            }
            let nfid = self.funcs.len();
            self.funcs.push(crate::sema::FuncInfo {
                name: fname,
                tok: ftok,
                module: fmod,
                owner: None,
                params,
                ret: fret,
                generics: Vec::new(),
                vis: fvis,
                is_static: fstatic,
                is_override: foverride,
                is_abstract: false,
                is_pure: fpure,
                is_nothrow: fnothrow,
                is_ref_return: frefret,
                is_nogc: fnogc,
                is_deprecated: fdeprecated,
                body: fbody,
                implicit: false,
                gdepth: depth,
                mangle: None,
                is_extern: false,
                is_export: false,
                parent: None,
                nested: BTreeMap::new(),
            });
            self.inst_slot_of.insert(nfid, iid);
            self.inst_env.insert(nfid, env.clone());
            if check_body {
                self.pending_insts.push(nfid);
            }
            ctors.insert(cname.clone(), nfid);
        }
        // 父类：泛型父类按同一组实参实例化（`Box<T> extend Container<T>`）
        let parent = match tmpl.parent {
            Some(p) => {
                let pgen = self.typedefs.get(p as usize).map(|d| d.generics.len());
                if pgen == Some(0) {
                    Some(p)
                } else if pgen.is_some() && pgen == Some(targs.len()) {
                    let pi = self.materialize(p, targs, tok);
                    Some(pi)
                } else {
                    Some(p)
                }
            }
            None => None,
        };
        let method_fids: Vec<usize> = methods.values().copied().collect();
        for nfid in &method_fids {
            self.funcs[*nfid].owner = Some(inst);
        }
        // 为所有单态化后的 constructor 设置 owner
        for (_cname, &nfid) in &ctors {
            self.funcs[nfid].owner = Some(inst);
        }
        {
            let d = &mut self.typedefs[inst as usize];
            d.fields = fields;
            d.methods = methods;
            d.ctors = ctors;
            d.parent = parent;
            d.impls = tmpl.impls.clone();
            d.extends = tmpl.extends.clone();
        }
        self.mat_of.insert(inst, (tmpl_id, targs.to_vec()));
        self.cur_mat_depth = saved_depth;
        inst
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
        let has_ctor = self.typedefs[pd as usize].ctors.get("constructor").is_some();
        let params: Vec<(String, Ty, Option<String>)> = match self.typedefs[pd as usize].ctors.get("constructor").copied() {
            Some(c) => self.funcs[c].params.clone(),
            None => Vec::new(),
        };
        let ptypes: Vec<Ty> = params.iter().map(|p| p.1.clone()).collect();
        let borrows: Vec<Option<String>> = params.iter().map(|p| p.2.clone()).collect();
        self.check_call_args(args, Some(&ptypes), Some(&borrows), e.id);
        // 只有父类有 constructor 时才校验实参个数；无 constructor 时 parent() 本身不应被调用，
        // 由上层检查（如 E3017 override 缺失）负责报错，避免级联 E4031。
        if has_ctor && args.len() != params.len() {
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
                    let src_kind = self.typedefs[*src as usize].kind;
                    if self.is_subclass(*src, d) {
                        // 源是目标的子类：这其实是上转，值已是目标类型，恒成立
                        let s = self.sl(e.id);
                        s.conv = Some("upcast");
                        s.cast_to = Some(t_named(DefId::Id(d), Vec::new()));
                        return t_named(DefId::Id(d), Vec::new());
                    }
                    // 合法下转 = 源是目标的父类，或源是 interface（文稿 3.1：
                    // 父类/interface → 派生类写 `Dog(a)`，运行时检查）。
                    // 互不为父子的两个 class 下转不可能成立，编译期就拒绝。
                    let impossible = src_kind == "class" && !self.is_subclass(d, *src);
                    if impossible {
                        self.err("E5003", &e.tok, "C-14", "向下转型的目标必须是 `{0}` 的子类（`{1}` 与它没有继承关系）", &[&src_name, &dname]);
                    }
                    let s = self.sl(e.id);
                    s.conv = Some("downcast");
                    s.cast_to = Some(t_named(DefId::Id(d), Vec::new()));
                    return t_named(DefId::Id(d), Vec::new());
                }
                }
            }
            return self.construct(e, args, d, "constructor");
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
            // 泛型 struct：从实参按位对应字段推断类型实参（GX-04）
            let d = self.infer_struct_targs(e, d, args);
            return self.construct(e, args, d, "constructor");
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

    fn construct(&mut self, e: &ast::EPtr, args: &[ast::N<ast::Arg>], d: u32, ctor_name: &str) -> Ty {
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
        let has_ctor = !td.ctors.is_empty();
        let has_fields = !td.fields.is_empty();
        if is_class && !has_ctor && !has_fields {
            self.err("E4042", &e.tok, "N-20", "`{0}` 没有 constructor，不能 new\n  = 它的静态方法请直接用类名调用：`{1}.xxx(...)`\n  = 如果确实需要实例，请先为 `{2}` 添加 constructor", &[&dname, &dname, &dname]);
            return t_object();
        }
        if is_class {
            // 只有 class 才需要查找/检查 constructor
            // self.err 会产生可变借用，td 的不可变借用在此点结束；下面重新获取
            let td = &self.typedefs[d as usize];
            let ctor_fid = match td.ctors.get(ctor_name) {
                Some(&fid) => fid,
                None => {
                    // 没有任何 constructor：E3008（有字段）或 E4042（零字段）已在别处报告
                    if !has_ctor {
                        return t_object();
                    }
                    // 有 constructor 但名字不匹配：报 E4043
                    self.err("E4043", &e.tok, "N-20", "类 `{0}` 没有名为 `{1}` 的 constructor", &[&dname, &ctor_name]);
                    return t_object();
                }
            };
            let params: Vec<(String, Ty, Option<String>)> = self.funcs[ctor_fid].params.clone();
            if args.len() != params.len() {
                self.err("E4031", &e.tok, "N-20", "`{0}` 的 constructor `{1}` 需要 {2} 个实参，得到 {3}", &[&dname, &ctor_name, &params.len(), &args.len()]);
            }
            for (i, a) in args.iter().enumerate() {
                let exp = params.get(i).map(|p| p.1.clone());
                self.check_expr(&a.k.expr, exp.as_ref());
                if let Some(ex) = &exp {
                    self.coerce(&a.k.expr, ex, "构造实参");
                }
            }
        } else {
            // struct/enum：只需给每个实参做类型检查，以便后续 struct_literal 能正确生成嵌套初始化
            for a in args {
                self.check_expr(&a.k.expr, None);
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

    fn check_new(&mut self, e: &ast::EPtr, expected: Option<&Ty>) -> Ty {
        // @nogc：函数体内不得出现 `new` 表达式（堆分配）
        if let Some(f) = self.current_func {
            if self.funcs[f].is_nogc {
                let fname = self.funcs[f].name.clone();
                self.err(
                    "E6017",
                    &e.tok,
                    "文稿 9.3",
                    "`@nogc` 函数 `{0}` 内不得出现 `new` 表达式（堆分配）",
                    &[&fname],
                );
                return t_object();
            }
        }
        let (path, args) = match &e.k {
            Expr::New { path, args } => (path, args),
            _ => unreachable!(),
        };
        // 从 path 中提取类型名和 constructor 名：
        //   new ClassName()          → type="ClassName", ctor="constructor"
        //   new ClassName.from_json() → type="ClassName", ctor="from_json"
        //   new mod.Type()           → type="Type" (module qualified), ctor="constructor"
        //   new mod.Type.from_json()  → type="Type", ctor="from_json"
        let mut ctor_name = "constructor".to_string();
        let type_name = if path.len() == 1 {
            path[0].clone()
        } else {
            // 多段路径：默认最后一段为类型名（兼容 mod.Type / Outer.Inner）。
            // 若后续类型查找失败，再回退：把最后一段当 constructor 名，
            // path[..len-1] 当类型路径。
            path.last().unwrap().clone()
        };
        let mut type_path = path.clone();

        // 1) 模块限定：`new mod.Type(...)`（文稿 9.2）
        // 模块路径可多段（`new shop.model.Product(...)`），末段才是类型名
        let mut d: Option<u32> = None;
        let mut module_qualified = false;
        if type_path.len() >= 2 {
            if let Some((k, canon)) = self.view_of(&type_path) {
                if canon != "*builtin*" {
                    if k + 1 == type_path.len() {
                        // `new mod.Type(...)`：最后一段是类型名
                        module_qualified = true;
                        let view = type_path[..k].join(".");
                        let export = &type_path[k];
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
                    } else if k + 1 < type_path.len() {
                        // `new mod.Outer.Inner(...)`：模块类型 + 嵌套类型
                        // 或者 `new mod.ClassName.ctorName(...)`：模块类型 + 命名 constructor
                        // 先按嵌套类型处理；若失败，再按命名 constructor 处理
                        let view = type_path[..k].join(".");
                        let export = &type_path[k];
                        if !self.export_allowed(&view, export) {
                            let src = self.view_display(&view);
                            self.err("E3030", &e.tok, "文稿 9.2", "`{0}` 不在 `import {1}.{{...}}` 的具名导入清单里", &[&export, &src]);
                            return t_object();
                        }
                        let sym = self
                            .module_exports
                            .get(&canon)
                            .and_then(|m| m.get(export));
                        match sym {
                            Some(ExportSym::Type(d0)) => {
                                module_qualified = true;
                                // 沿 nested 表逐段查找嵌套类型
                                let mut cur = *d0;
                                let mut found = true;
                                for i in (k + 1)..type_path.len() {
                                    let inner = self.typedefs[cur as usize].nested.get(&type_path[i]).copied();
                                    match inner {
                                        Some(iid) => cur = iid,
                                        None => {
                                            found = false;
                                            break;
                                        }
                                    }
                                }
                                if found {
                                    d = Some(cur);
                                } else {
                                    // 回退：把最后一段当 constructor 名
                                    if type_path.len() >= 3 {
                                        ctor_name = type_path[type_path.len() - 1].clone();
                                        type_path = type_path[..type_path.len() - 1].to_vec();
                                        // 重新用 type_path 做模块限定查找
                                        if let Some((k2, canon2)) = self.view_of(&type_path) {
                                            if canon2 != "*builtin*" {
                                                let view2 = type_path[..k2].join(".");
                                                let export2 = &type_path[k2];
                                                if self.export_allowed(&view2, export2) {
                                                    if let Some(sym2) = self.module_exports.get(&canon2).and_then(|m| m.get(export2)) {
                                                        match sym2 {
                                                            ExportSym::Type(d0) => {
                                                                let mut cur = *d0;
                                                                let mut found2 = true;
                                                                for i in (k2 + 1)..type_path.len() {
                                                                    let inner = self.typedefs[cur as usize].nested.get(&type_path[i]).copied();
                                                                    match inner {
                                                                        Some(iid) => cur = iid,
                                                                        None => { found2 = false; break; }
                                                                    }
                                                                }
                                                                if found2 {
                                                                    d = Some(cur);
                                                                }
                                                            }
                                                            _ => {}
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            _ => {
                                self.err("E3030", &e.tok, "文稿 9.1", "模块 `{0}` 没有公开类型 `{1}`", &[&view, &export]);
                                return t_object();
                            }
                        }
                    }
                }
            }
        }
        // 2) 裸名 / 本模块限定名
        if d.is_none() {
            d = self.lookup_type_name(&type_name, &e.tok);
        }
        if type_path.len() > 1 && !module_qualified && d.is_none() {
            // 尝试嵌套类型：Outer.Inner
            d = self
                .types
                .get(&type_path[0])
                .and_then(|o| self.typedefs[*o as usize].nested.get(&type_path[1]).copied());
        }
        // 3) 若仍未找到类型，且 path 有 2+ 段，回退：最后一段为 constructor 名
        if d.is_none() && path.len() >= 2 {
            ctor_name = path[path.len() - 1].clone();
            let type_path_trimmed = &path[..path.len() - 1];
            if type_path_trimmed.len() == 1 {
                d = self.lookup_type_name(&type_path_trimmed[0], &e.tok);
            } else {
                d = self
                    .types
                    .get(&type_path_trimmed[0])
                    .and_then(|o| self.typedefs[*o as usize].nested.get(&type_path_trimmed[1]).copied());
            }
        }
        if type_name == "Arena" {
            for a in args {
                self.check_expr(&a.k.expr, None);
            }
            // O-20：`new Arena()` 创建区域，region 绑定当前作用域——
            // 区域内 ArenaRef 的所有使用点都不得逃出这个块（O-18）
            let region = self.next_region;
            self.next_region += 1;
            if let Some(sc) = self.scope.clone() {
                self.arena_regions.insert(region, sc);
            }
            self.sl(e.id).ckind = Some("arena");
            self.sl(e.id).arena_region = Some(region);
            return t_named(DefId::Name("Arena"), Vec::new());
        }
        let d = match d {
            None => {
                if is_reserved(&type_name) {
                    self.err("E2012", &e.tok, "G-9", "`new` 不能用于内建类型 `{0}`", &[&type_name]);
                } else {
                    self.err("E3003", &e.tok, "N-7", "未定义的类型 `{0}`", &[&type_name]);
                }
                return t_object();
            }
            Some(d) => d,
        };
        if self.typedefs[d as usize].kind != "class" {
            self.err("E2012", &e.tok, "G-9", "`new` 只能用于 class（struct/enum/interface 不需要 new）", &[]);
            return t_object();
        }
        if self.typedefs[d as usize].is_abstract {
            let cls_name = self.typedefs[d as usize].name.clone();
            self.err("E4001", &e.tok, "T-5", "抽象类 `{0}` 不能直接 `new` 实例化，只能作为基类被继承", &[&cls_name]);
            return t_object();
        }
        // 泛型类：`new` 语法带不了类型实参，从期望类型或 ctor 实参推断（GX-04）
        let d = self.infer_new_targs(e, d, args, expected);
        self.construct(e, args, d, &ctor_name)
    }

    /// 泛型 struct 的构造推断：实参按位对应模板字段，统一出 `T` → 实例。
    fn infer_struct_targs(
        &mut self,
        e: &ast::EPtr,
        d: u32,
        args: &[ast::N<ast::Arg>],
    ) -> u32 {
        let ngen = self.typedefs[d as usize].generics.len();
        if ngen == 0 {
            return d;
        }
        let fields: Vec<Ty> = self.typedefs[d as usize]
            .fields
            .iter()
            .map(|f| f.1.clone())
            .collect();
        let names: Vec<String> = self.typedefs[d as usize]
            .generics
            .iter()
            .map(|g| g.0.clone())
            .collect();
        let mut arg_tys: Vec<Ty> = Vec::with_capacity(args.len());
        for a in args {
            if matches!(a.k.expr.k, Expr::AnonFunc { .. }) {
                arg_tys.push(t_object());
                continue;
            }
            arg_tys.push(self.check_expr(&a.k.expr, None));
        }
        let mut subst: HashMap<String, Ty> = HashMap::new();
        for (i, aty) in arg_tys.iter().enumerate() {
            if let Some(fty) = fields.get(i) {
                self.unify(fty, aty, &names, &mut subst);
            }
        }
        let targs: Vec<Ty> = names
            .iter()
            .map(|n| subst.get(n).cloned().unwrap_or_else(t_object))
            .collect();
        if targs.iter().any(|t| matches!(&**t, TyS::Object)) {
            let dname = self.typedefs[d as usize].name.clone();
            self.err(
                "E4037",
                &e.tok,
                "T-16",
                "无法推断 `{0}` 的类型实参；给变量标注类型，或把构造实参写成具体类型",
                &[&dname],
            );
            return d;
        }
        self.materialize(d, &targs, &e.tok)
    }

    /// 泛型类的 `new` 目标解析：期望类型命中实例 → 直接用；否则用 ctor
    /// 形参（含 `Param`）对实参做统一；推不出报 `E4037`。
    fn infer_new_targs(
        &mut self,
        e: &ast::EPtr,
        d: u32,
        args: &[ast::N<ast::Arg>],
        expected: Option<&Ty>,
    ) -> u32 {
        let ngen = self.typedefs[d as usize].generics.len();
        if ngen == 0 {
            return d;
        }
        // 1) ctor 形参统一推断（匿名函数没有目标类型会报 E4043，推断期跳过）。
        //    实参推断优先于期望类型：`Box<int> b = new Box(new Box(1))` 这类
        //    写错实参的代码要报"实参与 T 不符"，而不是被期望类型悄悄带过。
        // 2) 推不出再看期望类型是否已是本模板的实例（零参构造等场景）。
        let ctor_params: Vec<(String, Ty, Option<String>)> = match self.typedefs[d as usize].ctors.get("constructor").copied() {
            Some(c) => self.funcs[c].params.clone(),
            None => Vec::new(),
        };
        let mut arg_tys: Vec<Ty> = Vec::with_capacity(args.len());
        for a in args {
            if matches!(a.k.expr.k, Expr::AnonFunc { .. }) {
                arg_tys.push(t_object());
                continue;
            }
            arg_tys.push(self.check_expr(&a.k.expr, None));
        }
        let names: Vec<String> = self.typedefs[d as usize]
            .generics
            .iter()
            .map(|g| g.0.clone())
            .collect();
        let mut subst: HashMap<String, Ty> = HashMap::new();
        for (i, aty) in arg_tys.iter().enumerate() {
            if let Some(p) = ctor_params.get(i) {
                self.unify(&p.1, aty, &names, &mut subst);
            }
        }
        let targs: Vec<Ty> = names
            .iter()
            .map(|n| subst.get(n).cloned().unwrap_or_else(t_object))
            .collect();
        if !targs.iter().any(|t| matches!(&**t, TyS::Object)) {
            return self.materialize(d, &targs, &e.tok);
        }
        // 2) 期望类型兜底
        if let Some(t) = expected {
            if let TyS::Named(DefId::Id(id), _) = &**t {
                if let Some((tmpl, _)) = self.mat_of.get(id) {
                    if *tmpl == d {
                        return *id;
                    }
                }
            }
        }
        let dname = self.typedefs[d as usize].name.clone();
        self.err(
            "E4037",
            &e.tok,
            "T-16",
            "无法推断 `{0}` 的类型实参；给变量标注类型，或把构造实参写成具体类型",
            &[&dname],
        );
        d
    }
}
