//! 语义检查——成员访问（`.field` / 下标）、方法调用与 vtable 解析、
//! match 模式、常量求值（eval_const）与二元常量折叠（const_binop）。
//! 与 `expr.rs` 的分界：走 `.` 与 `[` 的都归这里；`module_sig` 在 expr.rs。

use crate::frontend::ast;
use crate::frontend::ast::Expr;
use crate::sema::{const_binop, py_repr, ConstVal, ExportSym, Res, Sema};
use crate::sema::expr::module_sig;
use crate::types::*;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

impl<'a> Sema<'a> {
    // ---------------------------------------------------------------- 成员访问

    /// `ar.Alloc(x)`（O-17）：把 x move 进区域，返回 ArenaRef<T>（copy 语义）。
    fn check_arena_method(&mut self, e: &ast::EPtr, args: &[ast::N<ast::Arg>], name: &str) -> Ty {
        if name != "Alloc" {
            self.err(
                "E3003",
                &e.tok,
                "O-17",
                "`Arena` 没有 {0} 方法（只有 `Alloc`）",
                &[&name],
            );
            return t_object();
        }
        let recv = match &e.k {
            Expr::MethodCall { recv, .. } => recv,
            _ => unreachable!(),
        };
        let region = self.slot(&recv.id).and_then(|s| s.arena_region);
        if args.len() != 1 {
            self.err(
                "E4031",
                &e.tok,
                "O-17",
                "`arena.Alloc` 需要 1 个实参，得到 {0}",
                &[&args.len()],
            );
            return t_object();
        }
        let t = self.check_expr(&args[0].k.expr, None);
        // 只收养指针表示的类型：class 实例 / object 盒子。数组、map 在 C 层
        // 是值结构（ax_arr/ax_map），区域无法按指针收养。
        let adoptable = matches!(&*t, TyS::Object)
            || matches!(&*t, TyS::Named(DefId::Id(id), _)
                if self.typedefs.get(*id as usize).map_or(false, |d| d.kind == "class"));
        if !adoptable {
            self.err(
                "E7005",
                &args[0].k.expr.tok,
                "O-17",
                "`arena.Alloc` 只收引用类型（class 实例），得到 {0}",
                &[&self.ts(&t)],
            );
            return Rc::new(TyS::ArenaRef(t));
        }
        self.take(&args[0].k.expr);
        let s = self.sl(e.id);
        s.mkind = Some("arena_alloc");
        s.arena_region = region;
        Rc::new(TyS::ArenaRef(t))
    }

    pub fn check_field_access(&mut self, e: &ast::EPtr, expected: Option<&Ty>) -> Ty {
        let (recv, name) = match &e.k {
            Expr::FieldAccess { recv, name } => (recv, name.clone()),
            _ => unreachable!(),
        };
        let _ = expected;
        let errs_before = self.d.error_count;
        let rt = self.check_expr(recv, None);
        if let TyS::Meta(tname) = &*rt {
            // Status.Ok 枚举成员
            if let Some(d) = self.lookup_type_name(tname, &e.tok) {
                if self.typedefs[d as usize].kind == "enum" {
                    if !self.typedefs[d as usize].variants.contains(&name) {
                        self.err("E3003", &e.tok, "文稿 6.4", "enum `{0}` 没有成员 `{1}`", &[&tname, &name]);
                        return Rc::new(TyS::Enum(d));
                    }
                    self.sl(e.id).res = Some(Res::EnumVariant(d, name.clone()));
                    self.sl(e.id).enum_variant = Some((d, name.clone()));
                    return Rc::new(TyS::Enum(d));
                }
            }
            if tname == "Rounding" {
                if !matches!(name.as_str(), "HalfUp" | "HalfEven" | "Down") {
                    self.err("E3003", &e.tok, "N-17", "Rounding 没有成员 `{0}`", &[&name]);
                }
                self.sl(e.id).res = Some(Res::Rounding(name.clone()));
                return t_named(DefId::Name("Rounding"), Vec::new());
            }
            let row_member = tname == "Row"
                && matches!(
                    name.as_str(),
                    "Empty" | "GetString" | "GetInt" | "GetDecimal"
                );
            let file_member = tname == "File"
                && matches!(
                    name.as_str(),
                    "Read" | "ReadLine" | "Write" | "WriteLine" | "Close"
                );
            if row_member || file_member {
                self.sl(e.id).res = Some(Res::StaticBuiltin(tname.clone(), name.clone()));
                return t_object();
            }
            // 函数引用（文稿 4.1）：`Type.static_method` 不加括号 → 函数值
            if let Some(d) = self.lookup_type_name(tname, &e.tok) {
                if let Some(&m) = self.typedefs[d as usize].methods.get(&name) {
                    if self.funcs[m].is_static {
                        let ps: Vec<Ty> = self.funcs[m].params.iter().map(|p| p.1.clone()).collect();
                        let ret = self.funcs[m].ret.clone();
                        self.sl(e.id).res = Some(Res::Func(m));
                        return t_fn(ps, ret);
                    }
                }
            }
            self.err("E3003", &e.tok, "N-13", "`{0}` 没有静态成员 `{1}`", &[&tname, &name]);
            return t_object();
        }
        if let TyS::ModuleT(m) = &*rt {
            // 用户模块成员（文稿 9.2）：`mod.CONST` 取常量、`mod.Type` 取类型
            // （后者返回 Meta，供 `mod.Type.method(...)` 继续静态派发）
            if let Some((canon, _)) = self.view_canon_of(m) {
                if canon != "*builtin*" {
                    return self.user_mod_member(e, m, &canon, &name);
                }
            }
            self.err("E3003", &e.tok, "文稿 9.2", "模块 `{0}` 没有成员 `{1}`（模块函数要写成 `{2}.{3}(...)`）", &[&m, &name, &m, &name]);
            return t_object();
        }
        if let TyS::Nullable(inner) = &*rt {
            // T-20：可空值不能直接取成员，必须先收窄（判空 / `?.` / `??`）
            self.err("E4022", &e.tok, "T-20", "`{0}` 可能为 null，不能直接用 `.{1}`；请先判空收窄或用 `?.`", &[&self.ts(&rt), &name]);
            if let TyS::Named(DefId::Id(id), _) = &**inner {
                if let Some(d) = self.typedefs.get(*id as usize) {
                    let ft = self.find_field(d.id, &name);
                    if let Some(ft) = ft {
                        return ft;
                    }
                }
            }
            return t_unknown();
        }
        // ArenaRef<T> 解引用：字段访问落到 T 的字段（C 层 ArenaRef<T> 就是 T*，
        // 与 class 引用同形，字段读写生成不变）
        let rt = if let TyS::ArenaRef(inner) = &*rt {
            inner.clone()
        } else {
            rt
        };
        if let TyS::Named(DefId::Id(id), _) = &*rt {
            let d = match self.typedefs.get(*id as usize) {
                Some(d) => d.clone(),
                None => return t_object(),
            };
            if d.kind == "enum" {
                // Status.Ok：与 meta 分支同路径
                if !d.variants.contains(&name) {
                    self.err("E3003", &e.tok, "文稿 6.4", "enum `{0}` 没有成员 `{1}`", &[&d.name, &name]);
                    return Rc::new(TyS::Enum(d.id));
                }
                self.sl(e.id).enum_variant = Some((d.id, name.clone()));
                return Rc::new(TyS::Enum(d.id));
            }
            let fty = self.find_field(d.id, &name);
            match fty {
                None => {
                    if d.kind == "class" && d.methods.contains_key(&name) {
                        self.err("E4001", &e.tok, "N-13", "`{0}` 是方法，调用要写 `(...)`", &[&name]);
                    } else {
                        self.err("E3003", &e.tok, "N-7", "`{0}` 没有字段 `{1}`", &[&d.name, &name]);
                    }
                    t_object()
                }
                Some(fty) => {
                    self.check_field_vis(d.id, &name, &e.tok);
                    let owner = self.find_field_owner(d.id, &name);
                    let idx = self.field_index(owner, &name);
                    // O-18：经区域内对象读出的 ArenaRef 继承来源 region，
                    // 之后的使用点照样受逃逸检查约束
                    let rr = if matches!(&*fty, TyS::ArenaRef(_)) {
                        self.slot(&recv.id).and_then(|x| x.arena_region)
                    } else {
                        None
                    };
                    let s = self.sl(e.id);
                    s.field_owner = Some(owner);
                    s.field_idx = idx;
                    s.arena_region = rr;
                    fty
                }
            }
        } else if matches!(&*rt, TyS::Object) {
            // 接收者没解析成功时 `object` 只是兜底类型，真正的错因已在前面报过
            if self.d.error_count == errs_before {
                self.err("E4028", &e.tok, "文稿 6.7", "`object` 没有成员（文稿 6.7）；先解箱到具体类型", &[]);
            }
            t_object()
        } else if matches!(&*rt, TyS::BuiltinT(_)) {
            // 历史行为（测试钉死）：此处不再分支，落到最后统一报错
            self.err("E3003", &e.tok, "N-7", "{0} 没有字段 `{1}`", &[&self.ts(&rt), &name]);
            t_object()
        } else if matches!(
            &*rt,
            TyS::Named(DefId::Name("Row"), _) | TyS::Named(DefId::Name("File"), _)
        ) {
            t_object()
        } else if matches!(&*rt, TyS::Error) {
            match name.as_str() {
                "msg" => t_string(),
                "code" => t_int(),
                _ => {
                    self.err("E3003", &e.tok, "N-17", "error 只有 `msg` 字段", &[]);
                    t_string()
                }
            }
        } else {
            self.err("E3003", &e.tok, "N-7", "{0} 没有字段 `{1}`", &[&self.ts(&rt), &name]);
            t_object()
        }
    }

    pub fn find_field(&self, d: u32, name: &str) -> Option<Ty> {
        let mut cur = Some(d);
        while let Some(c) = cur {
            if let Some(td) = self.typedefs.get(c as usize) {
                for (fn_, ft, _, _) in &td.fields {
                    if fn_ == name {
                        return Some(ft.clone());
                    }
                }
                cur = td.parent;
            } else {
                cur = None;
            }
        }
        None
    }

    pub fn find_field_owner(&self, d: u32, name: &str) -> u32 {
        let mut cur = Some(d);
        while let Some(c) = cur {
            if let Some(td) = self.typedefs.get(c as usize) {
                if td.fields.iter().any(|(fn_, _, _, _)| fn_ == name) {
                    return c;
                }
                cur = td.parent;
            } else {
                cur = None;
            }
        }
        d
    }

    pub fn field_index(&self, d: u32, name: &str) -> u32 {
        if let Some(td) = self.typedefs.get(d as usize) {
            for (i, (fn_, _, _, _)) in td.fields.iter().enumerate() {
                if fn_ == name {
                    return i as u32;
                }
            }
        }
        0
    }

    fn check_field_vis(&mut self, d: u32, name: &str, tok: &crate::frontend::lexer::Tok) {
        let owner = self.find_field_owner(d, name);
        let td = match self.typedefs.get(owner as usize) {
            Some(td) => td,
            None => return,
        };
        // struct 成员一律 public（连修饰词都不许写，E3011）——那是"公开的数据结构"，
        // 跨模块读字段不该被可见性拦下
        if td.kind == "struct" {
            return;
        }
        for (fn_, _, vis, _) in &td.fields {
            if fn_ != name {
                continue;
            }
            if vis == "public" {
                return;
            }
            if vis == "protected" {
                let ok = self
                    .cur_class
                    .map_or(false, |c| c == owner || self.is_subclass(c, owner));
                if ok {
                    return;
                }
                self.err("E3013", tok, "N-9", "`{0}` 是 protected，只能在本类或子类内访问", &[&name]);
                return;
            }
            let ok = self.cur_class.map_or(false, |c| c == owner);
            if !ok {
                self.err("E3013", tok, "N-9", "`{0}` 是私有的（只在本类内可见）", &[&name]);
            }
            return;
        }
    }

    pub fn check_safe_access(&mut self, e: &ast::EPtr, expected: Option<&Ty>) -> Ty {
        let (recv, name) = match &e.k {
            Expr::SafeAccess { recv, name } => (recv, name.clone()),
            _ => unreachable!(),
        };
        let rt = self.check_expr(recv, None);
        if !matches!(&*rt, TyS::Nullable(_)) {
            self.err("E4022", &recv.tok, "T-20", "`?.` 左侧必须是可空类型，得到 {0}", &[&self.ts(&rt)]);
            if matches!(&*rt, TyS::Named(DefId::Id(_), _)) {
                let tmp = self.mk_field_access(&e.tok, recv.clone(), &name);
                let ft = self.check_field_access(&tmp, expected);
                return t_nullable(ft);
            }
            return t_object();
        }
        let inner = match &*rt {
            TyS::Nullable(i) => i.clone(),
            _ => unreachable!(),
        };
        let tmp = self.mk_field_access(&e.tok, recv.clone(), &name);
        self.sl(e.id).inner_node = Some(tmp.clone());
        if matches!(&*inner, TyS::Named(DefId::Id(_), _)) {
            let ft = self.check_field_access(&tmp, None);
            return t_nullable(ft);
        }
        self.err("E3003", &e.tok, "N-7", "{0} 没有字段 `{1}`", &[&self.ts(&inner), &name]);
        t_object()
    }

    pub fn check_safe_call(&mut self, e: &ast::EPtr, _expected: Option<&Ty>) -> Ty {
        let (recv, name, args) = match &e.k {
            Expr::SafeCall { recv, name, args } => (recv, name.clone(), args.clone()),
            _ => unreachable!(),
        };
        let rt = self.check_expr(recv, None);
        if !matches!(&*rt, TyS::Nullable(_)) {
            self.err("E4022", &recv.tok, "T-20", "`?.` 左侧必须是可空类型，得到 {0}", &[&self.ts(&rt)]);
            return t_object();
        }
        let tmp = self.mk_method_call(&e.tok, recv.clone(), &name, args);
        self.sl(e.id).inner_node = Some(tmp.clone());
        let t = self.check_method_call(&tmp, None);
        t_nullable(t)
    }

    fn mk_field_access(
        &mut self,
        tok: &crate::frontend::lexer::Tok,
        recv: ast::EPtr,
        name: &str,
    ) -> ast::EPtr {
        Box::new(ast::N {
            id: self.new_node_id(),
            tok: tok.clone(),
            k: Expr::FieldAccess {
                recv,
                name: name.to_string(),
            },
        })
    }

    fn mk_method_call(
        &mut self,
        tok: &crate::frontend::lexer::Tok,
        recv: ast::EPtr,
        name: &str,
        args: Vec<ast::N<ast::Arg>>,
    ) -> ast::EPtr {
        Box::new(ast::N {
            id: self.new_node_id(),
            tok: tok.clone(),
            k: Expr::MethodCall {
                recv,
                name: name.to_string(),
                args,
            },
        })
    }

    // ---------------------------------------------------------------- 下标

    pub fn check_index(&mut self, e: &ast::EPtr, _expected: Option<&Ty>) -> Ty {
        let (recv, index) = match &e.k {
            Expr::Index { recv, index } => (recv, index),
            _ => unreachable!(),
        };
        let rt = self.check_expr(recv, None);
        if let TyS::Record(fields) = &*rt {
            let key = match &index.k {
                Expr::StrLit { parts }
                    if parts.len() == 1 && matches!(&parts[0], ast::SPart::Text(_)) =>
                {
                    match &parts[0] {
                        ast::SPart::Text(t) => Some(t.clone()),
                        _ => None,
                    }
                }
                _ => None,
            };
            let key = match key {
                Some(k) => k,
                None => {
                    self.err("E4008", &index.tok, "T-26", "record 的下标必须是字符串字面量（键在类型里固定）", &[]);
                    return t_unknown();
                }
            };
            let fmap: HashMap<&String, &Ty> = fields.iter().map(|(k, t)| (k, t)).collect();
            match fmap.get(&key) {
                None => {
                    self.err("E4008", &index.tok, "T-26", "record 没有键 `{0}`（键名固定；要加键请用 map）", &[&key]);
                    t_unknown()
                }
                Some(ft) => {
                    self.sl(e.id).key = Some(key);
                    (*ft).clone()
                }
            }
        } else if let TyS::Map(kt, vt) = &*rt {
            let itt = self.check_expr(index, Some(kt));
            if itt != *kt {
                self.err("E4001", &index.tok, "T-5", "map 键类型是 {0}，得到 {1}", &[&self.ts(kt), &self.ts(&itt)]);
            }
            vt.clone()
        } else if let TyS::Array(et, len) = &*rt {
            let it = self.check_expr(index, Some(&t_int()));
            if !is_int(&it) {
                self.err("E4009", &index.tok, "文稿 3.3", "数组下标必须是 int，得到 {0}", &[&self.ts(&it)]);
            }
            if let (Some(n), Expr::IntLit { value, .. }) = (len, &index.k) {
                if *value < 0 || *value >= *n {
                    self.err("E4005", &index.tok, "S-1", "定长数组长度为 {0}，常量下标 {1} 越界（合法范围 0..{2}）", &[&n, &value, &(n - 1)]);
                }
            }
            et.clone()
        } else if matches!(&*rt, TyS::Str) {
            self.err("E4006", &e.tok, "文稿 11.2", "string 不支持 `s[i]`；请用 `s.at(i)`（返回 string?）或 `s[i..j]` 之外的切片方法", &[]);
            t_string()
        } else if matches!(&*rt, TyS::Object) {
            self.err("E4028", &e.tok, "文稿 6.7", "`object` 不能下标访问；先解箱到具体类型", &[]);
            t_object()
        } else {
            self.err("E4001", &e.tok, "文稿 3.3", "{0} 不能下标访问", &[&self.ts(&rt)]);
            t_object()
        }
    }

    // ---------------------------------------------------------------- 方法调用

    pub fn check_method_call(&mut self, e: &ast::EPtr, expected: Option<&Ty>) -> Ty {
        let (recv, name, args) = match &e.k {
            Expr::MethodCall { recv, name, args } => (recv, name.clone(), args),
            _ => unreachable!(),
        };
        let _ = expected;
        // parent.m(...)
        let parent_call = matches!(recv.k, Expr::Parent)
            || matches!(&recv.k, Expr::Path { segs, .. } if segs.len() == 1 && segs[0] == "parent");
        if parent_call {
            let pd = match self.cur_class.and_then(|c| self.typedefs[c as usize].parent) {
                None => {
                    self.err("E3034", &e.tok, "G-8", "`parent` 只能用于有父类的类内", &[]);
                    return t_object();
                }
                Some(p) => p,
            };
            let pd_name = self.typedefs[pd as usize].name.clone();
            let m = match self.find_method(pd, &name) {
                None => {
                    self.err("E3003", &e.tok, "N-7", "父类 `{0}` 没有方法 `{1}`", &[&pd_name, &name]);
                    return t_object();
                }
                Some(m) => m,
            };
            let ptypes: Vec<Ty> = self.funcs[m].params.iter().map(|p| p.1.clone()).collect();
            let borrows: Vec<Option<String>> =
                self.funcs[m].params.iter().map(|p| p.2.clone()).collect();
            self.check_call_args(args, Some(&ptypes), Some(&borrows), e.id);
            let ret = self.funcs[m].ret.clone();
            let s = self.sl(e.id);
            s.mkind = Some("super");
            s.method = Some(m);
            s.method_owner = Some(pd);
            return ret;
        }
        let errs_before = self.d.error_count;
        let rt = self.check_expr(recv, None);
        if let TyS::Meta(tname) = &*rt {
            return self.check_static_call(e, args, tname);
        }
        if let TyS::ModuleT(m) = &*rt {
            // 用户模块函数调用 / 类型构造（文稿 9.2）：`mod.f(...)`、`mod.Ctor(...)`
            if let Some((canon, exact)) = self.view_canon_of(m) {
                if canon != "*builtin*" && exact {
                    return self.user_mod_call(e, args, m, &canon);
                }
            }
            return self.check_module_call(e, args, m);
        }
        // 接收者本身没解析成功（E3003 / E3030 已报）时不再追问「object 没有成员方法」，
        // 后一条只会把真正的错因盖住（一处误用一条诊断）。
        if matches!(&*rt, TyS::Object) {
            if self.d.error_count == errs_before {
                self.err("E4028", &e.tok, "文稿 6.7", "`object` 没有成员方法；先解箱到具体类型", &[]);
            }
            return t_object();
        }
        let rt = if matches!(&*rt, TyS::Nullable(_)) {
            let rn = match &recv.k {
                Expr::Path { segs, .. } => segs[0].clone(),
                _ => "该值".to_string(),
            };
            self.err("E4022", &e.tok, "T-20", "`{0}` 可能为空，先判空或用 `?.`", &[&rn]);
            unnullable(&rt)
        } else {
            rt.clone()
        };
        // Arena 内建方法（O-17）：`ar.Alloc(x)` 把 x move 进区域
        if let TyS::Named(DefId::Name(nm), _) = &*rt {
            if *nm == "Arena" {
                return self.check_arena_method(e, args, &name);
            }
        }
        // ArenaRef<T> 解引用：方法调用落到 T 的方法（C 层 ArenaRef<T> 就是 T*，
        // 与 class 引用同形，虚派发生成不变）
        let rt = if let TyS::ArenaRef(inner) = &*rt {
            inner.clone()
        } else {
            rt
        };
        if let Some(intr) = self.intrinsic(e, args, &rt) {
            return intr;
        }
        if let TyS::Named(DefId::Name(n), _) = &*rt {
            if *n == "Row" || *n == "File" {
                return self.check_builtin_instance(e, args, n);
            }
        }
        if let TyS::Named(DefId::Id(id), _) = &*rt {
            if self.typedefs.get(*id as usize).is_none() {
                return t_object();
            }
            let d = *id;
            let m = match self.find_method(d, &name) {
                None => {
                    let dname = self.typedefs[d as usize].name.clone();
                    self.err("E3003", &e.tok, "N-7", "`{0}` 没有方法 `{1}`", &[&dname, &name]);
                    return t_object();
                }
                Some(m) => m,
            };
            if self.funcs[m].is_static {
                let dname = self.typedefs[d as usize].name.clone();
                self.err("E3013", &e.tok, "N-13", "`{0}` 是静态方法，只能用 `{1}.{2}(...)` 调用", &[&name, &dname, &name]);
            }
            self.check_member_vis(d, m, &e.tok);
            let ptypes: Vec<Ty> = self.funcs[m].params.iter().map(|p| p.1.clone()).collect();
            let borrows: Vec<Option<String>> =
                self.funcs[m].params.iter().map(|p| p.2.clone()).collect();
            self.check_call_args(args, Some(&ptypes), Some(&borrows), e.id);
            if args.len() != ptypes.len() {
                self.err("E4031", &e.tok, "N-20", "`{0}` 需要 {1} 个实参，得到 {2}", &[&name, &ptypes.len(), &args.len()]);
            }
            // 方法归属：class 方法落在声明的类上；interface 方法（含经
            // implement 回落的签名）落在 interface 上——codegen 据此选虚表
            let owner = self.funcs[m].owner.unwrap_or(d);
            let ret = self.funcs[m].ret.clone();
            let s = self.sl(e.id);
            s.mkind = Some("virtual");
            s.method = Some(m);
            s.method_owner = Some(owner);
            return ret;
        }
        self.err("E3003", &e.tok, "N-7", "{0} 没有方法 `{1}`", &[&self.ts(&rt), &name]);
        t_object()
    }

    fn check_builtin_instance(
        &mut self,
        e: &ast::EPtr,
        args: &[ast::N<ast::Arg>],
        name: &str,
    ) -> Ty {
        let mname = match &e.k {
            Expr::MethodCall { name, .. } => name.clone(),
            _ => unreachable!(),
        };
        let _t = |k: TyS| -> Ty { Rc::new(k) };
        let sig: Option<(Vec<Ty>, Ty)> = match (name, mname.as_str()) {
            ("Row", "Empty") => Some((vec![], t_bool())),
            ("Row", "GetString") => Some((vec![t_string()], t_string())),
            ("Row", "GetInt") => Some((vec![t_string()], t_int())),
            ("Row", "GetDecimal") => Some((vec![t_string()], t_decimal())),
            ("File", "Close") => Some((vec![], t_void())),
            ("File", "Read") => Some((vec![], t_result(t_string()))),
            ("File", "ReadLine") => Some((vec![], t_nullable(t_string()))),
            ("File", "Write") => Some((vec![t_string()], t_result(t_int()))),
            ("File", "WriteLine") => Some((vec![t_string()], t_result(t_int()))),
            _ => None,
        };
        let sig = match sig {
            Some(s) => s,
            None => {
                self.err("E3003", &e.tok, "N-7", "`{0}` 没有方法 `{1}`", &[&name, &mname]);
                return t_object();
            }
        };
        let (ptypes, ret) = sig;
        self.check_call_args(args, Some(&ptypes), None, e.id);
        if args.len() != ptypes.len() {
            self.err("E4031", &e.tok, "N-20", "`{0}` 需要 {1} 个实参，得到 {2}", &[&mname, &ptypes.len(), &args.len()]);
        }
        let s = self.sl(e.id);
        s.mkind = Some("builtin_instance");
        s.bname = Some(format!("{}.{}", name, mname));
        ret
    }

    pub fn method_owner(&self, d: u32, name: &str) -> u32 {
        let mut cur = Some(d);
        while let Some(c) = cur {
            if let Some(td) = self.typedefs.get(c as usize) {
                if td.methods.contains_key(name) {
                    return c;
                }
                cur = td.parent;
            } else {
                cur = None;
            }
        }
        d
    }

    fn check_member_vis(&mut self, d: u32, m: usize, tok: &crate::frontend::lexer::Tok) {
        let owner = self.method_owner(d, &self.funcs[m].name);
        let (owner_vis, m_vis, m_name) = {
            let o = &self.typedefs[owner as usize];
            (o.vis.clone(), self.funcs[m].vis.clone(), self.funcs[m].name.clone())
        };
        if owner_vis == "public" && m_vis == "public" {
            return;
        }
        if m_vis == "public" {
            return;
        }
        if m_vis == "protected" {
            let ok = self
                .cur_class
                .map_or(false, |c| c == owner || self.is_subclass(c, owner));
            if ok {
                return;
            }
            self.err("E3013", tok, "N-9", "`{0}` 是 protected，只能在本类或子类内调用", &[&m_name]);
            return;
        }
        let ok = self.cur_class.map_or(false, |c| c == owner);
        if !ok {
            self.err("E3013", tok, "N-9", "`{0}` 是私有的（只在本类内可见）", &[&m_name]);
        }
    }

    fn check_static_call(
        &mut self,
        e: &ast::EPtr,
        args: &[ast::N<ast::Arg>],
        type_name: &str,
    ) -> Ty {
        let name = match &e.k {
            Expr::MethodCall { name, .. } => name.clone(),
            _ => unreachable!(),
        };
        if matches!(
            type_name,
            "int" | "float" | "decimal" | "string" | "int8" | "int16" | "int32"
        ) {
            return self.check_builtin_static(e, args, type_name);
        }
        if type_name == "Rounding" {
            if !matches!(name.as_str(), "HalfUp" | "HalfEven" | "Down") {
                self.err("E3003", &e.tok, "N-17", "Rounding 没有成员 `{0}`", &[&name]);
            }
            return t_named(DefId::Name("Rounding"), Vec::new());
        }
        if type_name == "Row" || type_name == "File" {
            let _t = |k: TyS| -> Ty { Rc::new(k) };
            let (ptypes, ret): (Vec<Ty>, Ty) = match name.as_str() {
                "Empty" => (vec![], t_bool()),
                "GetString" => (vec![t_string()], t_string()),
                "GetInt" => (vec![t_string()], t_int()),
                "GetDecimal" => (vec![t_string()], t_decimal()),
                "Close" => (vec![], t_void()),
                "Read" => (vec![], t_result(t_string())),
                "ReadLine" => (vec![], t_nullable(t_string())),
                "Write" => (vec![t_string()], t_result(t_int())),
                "WriteLine" => (vec![t_string()], t_result(t_int())),
                _ => {
                    self.err("E3003", &e.tok, "N-3", "`{0}` 没有静态方法 `{1}`", &[&type_name, &name]);
                    return t_object();
                }
            };
            self.check_call_args(args, Some(&ptypes), None, e.id);
            return ret;
        }
        // Meta 里的名字可能是裸名（`Product.Sample()`）或注册名（`product__Product`），
        // lookup_type_name 两档都能落定
        let d = match self.lookup_type_name(type_name, &e.tok) {
            None => {
                self.err("E3003", &e.tok, "N-7", "未定义的类型 `{0}`", &[&type_name]);
                return t_object();
            }
            Some(d) => d,
        };
        let mut m = self.typedefs[d as usize].methods.get(&name).copied();
        if m.is_none() && self.typedefs[d as usize].kind == "interface" {
            m = self
                .all_iface_methods(d)
                .into_iter()
                .find(|(n, _)| *n == name)
                .map(|(_, m)| m);
        }
        let m = match m {
            None => {
                self.err("E3003", &e.tok, "N-7", "`{0}` 没有方法 `{1}`", &[&type_name, &name]);
                return t_object();
            }
            Some(m) => m,
        };
        if !self.funcs[m].is_static && self.typedefs[d as usize].kind == "class" {
            self.err("E3013", &e.tok, "N-13", "`{0}` 不是静态方法，请用实例调用", &[&name]);
        }
        self.check_member_vis(d, m, &e.tok);
        let ptypes: Vec<Ty> = self.funcs[m].params.iter().map(|p| p.1.clone()).collect();
        let borrows: Vec<Option<String>> =
            self.funcs[m].params.iter().map(|p| p.2.clone()).collect();
        self.check_call_args(args, Some(&ptypes), Some(&borrows), e.id);
        if args.len() != self.funcs[m].params.len() {
            self.err("E4031", &e.tok, "N-20", "`{0}` 需要 {1} 个实参，得到 {2}", &[&name, &self.funcs[m].params.len(), &args.len()]);
        }
        let ret = self.funcs[m].ret.clone();
        let s = self.sl(e.id);
        s.mkind = Some("static");
        s.method = Some(m);
        ret
    }

    fn check_builtin_static(
        &mut self,
        e: &ast::EPtr,
        args: &[ast::N<ast::Arg>],
        type_name: &str,
    ) -> Ty {
        let name = match &e.k {
            Expr::MethodCall { name, .. } => name.clone(),
            _ => unreachable!(),
        };
        if name == "Parse" {
            self.check_call_args(args, Some(&[t_string()]), None, e.id);
            if type_name == "string" {
                self.err("E3003", &e.tok, "N-17", "string 没有 parse", &[]);
                return t_string();
            }
            let tgt = builtin_scalar(type_name).unwrap_or_else(t_int);
            let s = self.sl(e.id);
            s.mkind = Some("Parse");
            s.parse_to = Some(tgt.clone());
            return t_result(tgt);
        }
        if name == "Truncate" && matches!(type_name, "int8" | "int16" | "int32") {
            self.check_call_args(args, Some(&[t_int()]), None, e.id);
            let tgt = builtin_scalar(type_name).unwrap();
            let s = self.sl(e.id);
            s.mkind = Some("Truncate");
            s.trunc_to = Some(tgt.clone());
            return tgt;
        }
        if name == "FromCodepoint" && type_name == "string" {
            self.check_call_args(args, Some(&[t_int()]), None, e.id);
            self.sl(e.id).mkind = Some("FromCodepoint");
            return t_result(t_string());
        }
        self.err("E3003", &e.tok, "N-17", "`{0}.{1}` 不存在", &[&type_name, &name]);
        t_object()
    }

    fn check_module_call(
        &mut self,
        e: &ast::EPtr,
        args: &[ast::N<ast::Arg>],
        m: &str,
    ) -> Ty {
        let name = match &e.k {
            Expr::MethodCall { name, .. } => name.clone(),
            _ => unreachable!(),
        };
        self.check_call_args(args, None, None, e.id);
        let sig = match module_sig(m, &name) {
            None => {
                self.err("E3003", &e.tok, "文稿 9.2", "模块 `{0}` 没有符号 `{1}`", &[&m, &name]);
                return t_object();
            }
            Some(s) => s,
        };
        let (ptypes, ret) = sig;
        for (i, a) in args.iter().enumerate() {
            if let Some(Some(pt)) = ptypes.get(i).map(|x| x.clone()) {
                self.coerce(&a.k.expr, &pt, "实参");
            }
        }
        self.sl(e.id).mkind = Some("module");
        ret
    }

    // ------------------------------------------------------ 用户模块（文稿 9.2）

    /// 具名导入白名单校验；不通过时报 E3030。
    fn user_export_gate(&mut self, e: &ast::EPtr, view: &str, name: &str) -> bool {
        if self.export_allowed(view, name) {
            return true;
        }
        // 报错用 import 原文（短名视图下 `http` 其实来自 `import net.http`）
        let src = self.view_display(view);
        self.err("E3030", &e.tok, "文稿 9.2", "`{0}` 不在 `import {1}.{{...}}` 的具名导入清单里", &[&name, &src]);
        false
    }

    /// 用户模块的成员访问（非调用）：`mod.CONST` / `mod.Type`。
    fn user_mod_member(&mut self, e: &ast::EPtr, view: &str, canon: &str, name: &str) -> Ty {
        // 点号模块路径下钻：`import net.http` 后，`net.http` 整体是一个视图，
        // 继续返回 ModuleT 让外层 `.get()` 在完整视图上分派
        let full = format!("{}.{}", view, name);
        // `net.http.server.xxx`：`net` / `net.http` 都不是模块本身，只是已导入视图的前缀，
        // 所以按"前缀"判定而不是精确匹配，否则 3 段以上的模块路径会在这里断掉
        let is_view = {
            let pref = format!("{}.", full);
            self.active_views.contains_key(&full)
                || self.active_views.keys().any(|v| v.starts_with(&pref))
        };
        if is_view {
            self.sl(e.id).res = Some(Res::Module(full.clone()));
            return Rc::new(TyS::ModuleT(full));
        }
        if !self.user_export_gate(e, view, name) {
            return t_object();
        }
        let sym = self
            .module_exports
            .get(canon)
            .and_then(|m| m.get(name))
            .cloned();
        match sym {
            Some(ExportSym::Const) => {
                let reg = format!("{}__{}", canon, name);
                match self.consts.get(&reg).cloned() {
                    Some(c) => {
                        self.sl(e.id).res = Some(Res::Const(c.clone()));
                        let t = c.ty.borrow().clone();
                        t
                    }
                    None => t_object(),
                }
            }
            Some(ExportSym::Type(d)) => {
                let tname = self.typedefs[d as usize].name.clone();
                self.sl(e.id).res = Some(Res::Type(d));
                Rc::new(TyS::Meta(tname))
            }
            Some(ExportSym::Func(f)) => {
                // 函数引用（不加括号）→ 函数值（文稿 4.1）
                self.sl(e.id).res = Some(Res::Func(f));
                let fi = &self.funcs[f];
                t_fn(
                    fi.params.iter().map(|p| p.1.clone()).collect(),
                    fi.ret.clone(),
                )
            }
            None => {
                self.err("E3030", &e.tok, "文稿 9.1", "模块 `{0}` 没有公开成员 `{1}`（文件内符号默认私有，导出需写 `public`）", &[&view, &name]);
                t_object()
            }
        }
    }

    /// 用户模块的调用：`mod.f(...)` 函数调用 / `mod.Ctor(...)` 构造。
    fn user_mod_call(
        &mut self,
        e: &ast::EPtr,
        args: &[ast::N<ast::Arg>],
        view: &str,
        canon: &str,
    ) -> Ty {
        let name = match &e.k {
            Expr::MethodCall { name, .. } => name.clone(),
            _ => unreachable!(),
        };
        if !self.user_export_gate(e, view, &name) {
            return t_object();
        }
        let sym = self
            .module_exports
            .get(canon)
            .and_then(|m| m.get(&name))
            .cloned();
        // codegen 按 mkind=="module" 路由到 module_call，再按 ckind 生成 func/ctor
        self.sl(e.id).mkind = Some("module");
        match sym {
            Some(ExportSym::Func(f)) => self.call_func(e, f, args, &[]),
            Some(ExportSym::Type(d)) => self.conversion_named(e, args, Some(d), None),
            Some(ExportSym::Const) => {
                self.err("E3003", &e.tok, "文稿 9.2", "`{0}.{1}` 是常量，不可调用", &[&view, &name]);
                t_object()
            }
            None => {
                self.err("E3030", &e.tok, "文稿 9.1", "模块 `{0}` 没有公开成员 `{1}`（文件内符号默认私有，导出需写 `public`）", &[&view, &name]);
                t_object()
            }
        }
    }

    // ---------------------------------------------------------------- 内建方法

    fn intrinsic(
        &mut self,
        e: &ast::EPtr,
        args: &[ast::N<ast::Arg>],
        rt: &Ty,
    ) -> Option<Ty> {
        let name = match &e.k {
            Expr::MethodCall { name, .. } => name.clone(),
            _ => return None,
        };
        let _t = |k: TyS| -> Ty { Rc::new(k) };
        let table: Option<Vec<(&str, Vec<Ty>, Ty)>> = match &**rt {
            TyS::Str => Some(vec![
                ("Len", vec![], t_int()),
                ("ByteLen", vec![], t_int()),
                ("At", vec![t_int()], t_nullable(t_string())),
                ("Chars", vec![], t_array(t_string(), None)),
                ("CodepointAt", vec![t_int()], t_nullable(t_int())),
                ("Contains", vec![t_string()], t_bool()),
                ("Split", vec![t_string()], t_array(t_string(), None)),
                ("ToString", vec![], t_string()),
                ("Trim", vec![], t_string()),
            ]),
            TyS::Array(elem, _) => {
                let el = elem.clone();
                Some(vec![
                    ("Len", vec![], t_int()),
                    ("Push", vec![el.clone()], t_void()),
                    ("Pop", vec![], el.clone()),
                    ("Remove", vec![t_int()], t_void()),
                    ("Clear", vec![], t_void()),
                    ("At", vec![t_int()], t_nullable(el.clone())),
                    (
                        "Sort",
                        vec![t_fn(vec![el.clone(), el.clone()], t_int())],
                        t_void(),
                    ),
                    ("ToString", vec![], t_string()),
                ])
            }
            TyS::Map(k, v) => {
                let (k1, v1) = (k.clone(), v.clone());
                Some(vec![
                    ("Len", vec![], t_int()),
                    ("Get", vec![k1.clone()], t_nullable(v1.clone())),
                    ("Keys", vec![], t_array(k1.clone(), None)),
                    ("Remove", vec![k1.clone()], t_void()),
                    ("Has", vec![k1.clone()], t_bool()),
                ])
            }
            TyS::Decimal => Some(vec![
                (
                    "Round",
                    vec![t_int(), t_named(DefId::Name("Rounding"), Vec::new())],
                    t_decimal(),
                ),
                (
                    "Divide",
                    vec![
                        t_decimal(),
                        t_int(),
                        t_named(DefId::Name("Rounding"), Vec::new()),
                    ],
                    t_decimal(),
                ),
                ("ToString", vec![], t_string()),
            ]),
            TyS::Int(..) | TyS::Float => {
                if name == "ToString" {
                    Some(vec![("ToString", vec![], t_string())])
                } else if name == "WrappingAdd" && matches!(&**rt, TyS::Int(..)) {
                    Some(vec![("WrappingAdd", vec![rt.clone()], rt.clone())])
                } else {
                    None
                }
            }
            TyS::Bool | TyS::Enum(..) => {
                if name == "ToString" {
                    Some(vec![("ToString", vec![], t_string())])
                } else {
                    None
                }
            }
            TyS::Named(DefId::Name("Rounding"), _) => None,
            _ => None,
        };
        let table = match table {
            Some(t) => t,
            None => return None,
        };
        let sig = table.into_iter().find(|(n, _, _)| *n == name);
        let (_, ptypes, ret) = match sig {
            Some(s) => s,
            None => return None,
        };
        Some(self._intr(e, args, rt, &ptypes, &ret))
    }

    fn _intr(
        &mut self,
        e: &ast::EPtr,
        args: &[ast::N<ast::Arg>],
        rt: &Ty,
        ptypes: &[Ty],
        ret: &Ty,
    ) -> Ty {
        let name = match &e.k {
            Expr::MethodCall { name, .. } => name.clone(),
            _ => unreachable!(),
        };
        self.check_call_args(args, Some(ptypes), None, e.id);
        if args.len() != ptypes.len() {
            self.err("E4031", &e.tok, "N-20", "`{0}` 需要 {1} 个实参，得到 {2}", &[&name, &ptypes.len(), &args.len()]);
        }
        {
            let s = self.sl(e.id);
            s.mkind = Some("intrinsic");
            s.recv_ty = Some(rt.clone());
        }
        // 遍历期修改长度
        let tgt = self.iter_targets.last().cloned();
        if let Some(tg) = tgt {
            if matches!(name.as_str(), "Push" | "Pop" | "Remove" | "Clear") {
                if let Expr::Path { segs, .. } = &e.k {
                    if segs[0] == tg {
                        self.err("E4026", &e.tok, "文稿 5.2", "不能在遍历 `{0}` 期间修改它的长度（`{1}`）", &[&tg, &name]);
                    }
                } else if let Expr::MethodCall { recv, .. } = &e.k {
                    if let Expr::Path { segs, .. } = &recv.k {
                        if segs[0] == tg {
                            self.err("E4026", &e.tok, "文稿 5.2", "不能在遍历 `{0}` 期间修改它的长度（`{1}`）", &[&tg, &name]);
                        }
                    }
                }
            }
        }
        if name == "Push" && !args.is_empty() {
            self.take(&args[0].k.expr);
        }
        ret.clone()
    }

    // ---------------------------------------------------------------- match

    pub fn check_match(
        &mut self,
        mid: u32,
        tok: &crate::frontend::lexer::Tok,
        n: &ast::MatchNode,
        expected: Option<&Ty>,
    ) {
        let st = self.check_expr(&n.scrutinee, None);
        self.sl(mid).scrut_ty = Some(st.clone());
        let mut seen_variants: HashSet<String> = HashSet::new();
        let mut seen_lits: HashSet<String> = HashSet::new();
        let mut has_wild = false;
        let mut arm_tys: Vec<Ty> = Vec::new();
        for arm in &n.arms {
            self.push_scope();
            self.check_pattern(&arm.pat, &st, &mut seen_variants, &mut seen_lits, has_wild);
            if matches!(arm.pat.k, ast::Pat::Wild | ast::Pat::Bind { .. }) {
                has_wild = true;
            }
            let t = self.check_expr(&arm.body, expected);
            // 注意：不能把臂值类型写回 `arm.pat.id`——类型模式（`Dog d`）已在
            // check_pattern 里把该节点的 ty 存成被匹配的目标类型，臂值类型是
            // 另一件事。写回会把它覆盖成 void/臂类型，codegen 判 tag 时全错。
            arm_tys.push(t);
            self.pop_scope();
        }
        // 穷尽
        if let TyS::Enum(did) = &*st {
            let d = match self.typedefs.get(*did as usize) {
                Some(d) => d.clone(),
                None => return,
            };
            if !has_wild {
                let missing: Vec<String> = d
                    .variants
                    .iter()
                    .filter(|v| !seen_variants.contains(&format!("{}#{}", d.id, v)))
                    .cloned()
                    .collect();
                if !missing.is_empty() {
                    self.err("E6001", tok, "文稿 5.3", "`match` 非穷尽：缺少 {0}", &[&missing.join(", ")]);
                }
            }
        } else if let TyS::Result(_) = &*st {
            if !has_wild {
                let has_ok = seen_variants.contains("ok");
                let has_fail = seen_variants.contains("fail");
                if !has_ok && !has_fail {
                    self.err("E6001", tok, "文稿 5.3", "`match` 非穷尽：缺少 ok 与 fail", &[]);
                } else if !has_ok {
                    self.err("E6001", tok, "文稿 5.3", "`match` 非穷尽：缺少 ok(...)", &[]);
                } else if !has_fail {
                    self.err("E6001", tok, "文稿 5.3", "`match` 非穷尽：缺少 fail(...)", &[]);
                }
            }
        } else if matches!(&*st, TyS::Object) {
            if !has_wild {
                self.err("E6002", tok, "文稿 5.3", "对 `object` 的 `match` 必须有 `_` 分支", &[]);
            }
        } else if !has_wild {
            self.err("E6002", tok, "文稿 5.3", "对 {0} 的 `match` 必须有 `_` 分支", &[&self.ts(&st)]);
        }
        if let Some(exp) = expected {
            if !matches!(&**exp, TyS::Void) {
                let mut base: Option<Ty> = None;
                for t in &arm_tys {
                    if matches!(&**t, TyS::Never | TyS::Void) {
                        continue;
                    }
                    match &base {
                        None => base = Some(t.clone()),
                        Some(b) => {
                            if t != b {
                                self.err("E4014", tok, "文稿 5.3", "`match` 分支类型不一致：{0} vs {1}", &[&self.ts(b), &self.ts(t)]);
                                break;
                            }
                        }
                    }
                }
                if base.is_some() && arm_tys.iter().any(|t| matches!(&**t, TyS::Void)) {
                    self.err("E6007", tok, "文稿 5.3", "`match` 作表达式时分支不能是 void", &[]);
                }
                let ty = base.unwrap_or_else(|| exp.clone());
                self.sl(mid).ty = Some(ty);
                return;
            }
        }
        self.sl(mid).ty = Some(t_void());
    }

    fn check_pattern(
        &mut self,
        p: &ast::N<ast::Pat>,
        st: &Ty,
        seen_variants: &mut HashSet<String>,
        seen_lits: &mut HashSet<String>,
        has_wild: bool,
    ) {
        match &p.k {
            ast::Pat::Wild => {}
            ast::Pat::Lit { expr } => {
                let exp = if matches!(&**st, TyS::Object) {
                    None
                } else {
                    Some(st.clone())
                };
                let t = self.check_expr(expr, exp.as_ref());
                if !matches!(&**st, TyS::Object) && t != *st {
                    let r = assignable(&t, st, self, false);
                    if matches!(r, AssignResult::No(_)) {
                        self.err("E4001", &expr.tok, "文稿 5.3", "模式类型 {0} 与 match 值 {1} 不一致", &[&self.ts(&t), &self.ts(st)]);
                    }
                }
                if let Some(v) = self.eval_const(expr) {
                    let key = py_repr(&v);
                    if seen_lits.contains(&key) {
                        self.err("E6006", &p.tok, "文稿 5.3", "该分支不可达（重复的字面量 {0}）", &[&key]);
                    }
                    seen_lits.insert(key);
                }
                if has_wild {
                    self.err("E6006", &p.tok, "文稿 5.3", "`_` 之后还有分支，不可达", &[]);
                }
            }
            ast::Pat::Range { lo, hi, .. } => {
                let exp = if matches!(&**st, TyS::Int(..)) {
                    Some(st.clone())
                } else {
                    None
                };
                self.check_expr(lo, exp.as_ref());
                self.check_expr(hi, exp.as_ref());
                if !is_int(st) {
                    self.err("E4001", &p.tok, "文稿 5.3", "区间模式只能用于 int", &[]);
                }
                let lov = self.eval_const(lo);
                let hiv = self.eval_const(hi);
                match (lov, hiv) {
                    (Some(ConstVal::Int(a)), Some(ConstVal::Int(b))) => {
                        if a > b {
                            self.err("E4001", &p.tok, "S-1", "区间模式下界大于上界", &[]);
                        }
                    }
                    _ => {
                        self.err("E4001", &p.tok, "S-1", "区间模式两端必须是常量", &[]);
                    }
                }
                if has_wild {
                    self.err("E6006", &p.tok, "文稿 5.3", "`_` 之后还有分支，不可达", &[]);
                }
            }
            ast::Pat::Enum { ty, variant } => {
                let name = ty.join(".");
                let d = match self.lookup_type_name(&name, &p.tok) {
                    Some(d) if self.typedefs[d as usize].kind == "enum" => d,
                    _ => {
                        self.err("E3003", &p.tok, "N-18", "`{0}` 不是 enum", &[&name]);
                        return;
                    }
                };
                if !self.typedefs[d as usize].variants.contains(variant) {
                    self.err("E3003", &p.tok, "文稿 6.4", "enum `{0}` 没有成员 `{1}`", &[&name, &variant]);
                    return;
                }
                let st_ok = match &**st {
                    TyS::Enum(id) => *id == d,
                    _ => false,
                };
                if !st_ok {
                    self.err("E4001", &p.tok, "文稿 5.3", "模式 `{0}.{1}` 与 match 值类型 {2} 不一致", &[&name, &variant, &self.ts(st)]);
                }
                let key = format!("{}#{}", d, variant);
                if seen_variants.contains(&key) {
                    self.err("E6006", &p.tok, "文稿 5.3", "该分支不可达（`{0}.{1}` 已覆盖）", &[&name, &variant]);
                }
                seen_variants.insert(key);
                if has_wild {
                    self.err("E6006", &p.tok, "文稿 5.3", "`_` 之后还有分支，不可达", &[]);
                }
            }
            ast::Pat::Result { is_ok, bind } => {
                if !matches!(&**st, TyS::Result(_)) {
                    self.err("E6004", &p.tok, "文稿 10.1", "`ok(...)`/`fail(...)` 模式只能用于 result，得到 {0}", &[&self.ts(st)]);
                    self.declare(bind, t_object(), false, &p.tok);
                    return;
                }
                let key = if *is_ok { "ok" } else { "fail" };
                if seen_variants.contains(key) {
                    self.err("E6006", &p.tok, "文稿 5.3", "该分支不可达（`{0}` 已覆盖）", &[&key]);
                }
                seen_variants.insert(key.to_string());
                if *is_ok {
                    let inner = match &**st {
                        TyS::Result(i) => i.clone(),
                        _ => unreachable!(),
                    };
                    self.declare(bind, inner, false, &p.tok);
                } else {
                    self.declare(bind, t_error(), false, &p.tok);
                }
                if has_wild {
                    self.err("E6006", &p.tok, "文稿 5.3", "`_` 之后还有分支，不可达", &[]);
                }
            }
            ast::Pat::Type { ty, bind } => {
                let t = self.resolve_type(ty);
                // 语义类型挂到模式节点上供 codegen 收窄用（存 resolved Ty 而非
                // AST 类型节点，后端无法直接用；诊断行为不受影响）
                self.sl(p.id).ty = Some(t.clone());
                if !matches!(
                    &*t,
                    TyS::Named(..)
                        | TyS::Bool
                        | TyS::Int(..)
                        | TyS::Float
                        | TyS::Decimal
                        | TyS::Str
                ) {
                    self.err("E4001", &p.tok, "文稿 5.3", "类型模式必须是 class 或标量类型", &[]);
                }
                if !matches!(&**st, TyS::Object | TyS::Named(..)) {
                    self.err("E4001", &p.tok, "文稿 5.3", "类型模式只能用于 object 或类引用，得到 {0}", &[&self.ts(st)]);
                }
                if let TyS::Named(DefId::Id(id), _) = &*t {
                    if let Some(d) = self.typedefs.get(*id as usize) {
                        if d.kind == "interface" {
                            self.err("E4001", &p.tok, "文稿 5.3", "类型模式不能是 interface（用具体类）", &[]);
                        }
                    }
                }
                self.declare(bind, t, false, &p.tok);
                if has_wild {
                    self.err("E6006", &p.tok, "文稿 5.3", "`_` 之后还有分支，不可达", &[]);
                }
            }
            ast::Pat::Bind { name } => {
                if self.lookup_type_name(name, &p.tok).is_some() {
                    self.err("E6005", &p.tok, "G-11", "类型模式缺少绑定名（写 `{0} x`）", &[&name]);
                    return;
                }
                self.declare(name, st.clone(), false, &p.tok);
            }
        }
    }

    // ---------------------------------------------------------------- 常量求值

    pub fn eval_const(&self, e: &ast::EPtr) -> Option<ConstVal> {        match &e.k {
            Expr::IntLit { value, .. } => Some(ConstVal::Int(*value)),
            Expr::FloatLit { value, .. } => Some(ConstVal::Float(*value)),
            Expr::DecLit { coeff, scale, .. } => Some(ConstVal::Dec(*coeff, *scale)),
            Expr::BoolLit { value } => Some(ConstVal::Bool(*value)),
            Expr::StrLit { parts } => {
                if parts.is_empty() {
                    return Some(ConstVal::Str(String::new()));
                }
                if parts.len() == 1 {
                    if let ast::SPart::Text(t) = &parts[0] {
                        return Some(ConstVal::Str(t.clone()));
                    }
                }
                None
            }
            Expr::NullLit => Some(ConstVal::Null),
            Expr::Path { .. } => match self.res_of(e.id) {
                Some(Res::Const(c)) => c.value.borrow().clone(),
                _ => None,
            },
            Expr::FieldAccess { .. } => {
                // Rounding / enum_variant 记录在 res/enum_variant 槽
                if let Some(slot) = self.slot(&e.id) {
                    if let Some((d, v)) = &slot.enum_variant {
                        return Some(ConstVal::Enum(*d, v.clone()));
                    }
                    if let Some(Res::Rounding(n)) = &slot.res {
                        return Some(ConstVal::Rounding(n.clone()));
                    }
                }
                None
            }
            Expr::Unary { op, operand, .. } => {
                if let Some(slot) = self.slot(&e.id) {
                    if let Some(f) = &slot.folded {
                        return Some(f.clone());
                    }
                }
                let v = self.eval_const(operand)?;
                match (op.as_str(), &v) {
                    ("-", ConstVal::Int(i)) => Some(ConstVal::Int(i.wrapping_neg())),
                    ("-", ConstVal::Float(f)) => Some(ConstVal::Float(-f)),
                    ("!", ConstVal::Bool(b)) => Some(ConstVal::Bool(!b)),
                    ("~", ConstVal::Int(i)) => Some(ConstVal::Int(!i)),
                    _ => None,
                }
            }
            Expr::Binary { op, lhs, rhs } => {
                let a = self.eval_const(lhs)?;
                let b = self.eval_const(rhs)?;
                const_binop(op, &a, &b)
            }
            Expr::Ternary { cond, then_, els } => {
                let c = self.eval_const(cond)?;
                let b = matches!(c, ConstVal::Bool(true));
                if b {
                    self.eval_const(then_)
                } else {
                    self.eval_const(els)
                }
            }
            Expr::Coalesce { lhs, rhs } => {
                let a = self.eval_const(lhs)?;
                if matches!(a, ConstVal::Null) {
                    return self.eval_const(rhs);
                }
                Some(a)
            }
            Expr::Call { callee, args } => {
                // `typeof(x)` 的结果在 check_call 时已作为常量存进槽（编译期字符串）
                if let Some(slot) = self.slot(&e.id) {
                    if let Some(v) = &slot.const_value {
                        return Some(v.clone());
                    }
                }
                let name = match &callee.k {
                    Expr::Path { segs, .. } if segs.len() == 1 => &segs[0],
                    _ => return None,
                };
                if let Some(tgt) = builtin_scalar(name) {
                    if args.len() != 1 {
                        return None;
                    }
                    let v = self.eval_const(&args[0].k.expr)?;
                    return match (tgt.as_ref(), &v) {
                        (TyS::Int(..), ConstVal::Bool(_)) => None,
                        (TyS::Int(..), ConstVal::Int(i)) => Some(ConstVal::Int(*i)),
                        (TyS::Int(..), ConstVal::Float(f)) => Some(ConstVal::Int(*f as i64)),
                        (TyS::Float, ConstVal::Int(i)) => Some(ConstVal::Float(*i as f64)),
                        (TyS::Float, ConstVal::Float(f)) => Some(ConstVal::Float(*f)),
                        (TyS::Decimal, ConstVal::Dec(c, s)) => Some(ConstVal::Dec(*c, *s)),
                        (TyS::Decimal, ConstVal::Int(i)) => Some(ConstVal::Dec(*i as i128, 0)),
                        _ => None,
                    };
                }
                None
            }
            Expr::MethodCall { name, recv, .. } => {
                if name == "Len" || name == "ByteLen" {
                    if let Some(ConstVal::Str(s)) = self.eval_const(recv) {
                        return Some(ConstVal::Int(if name == "Len" {
                            s.chars().count() as i64
                        } else {
                            s.len() as i64
                        }));
                    }
                }
                None
            }
            _ => None,
        }
    }
}

