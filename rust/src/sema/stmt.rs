//! 语义检查——语句级：作用域进出、变量声明/赋值与收窄、控制流（if/loop/while/for）、
//! defer、trap，以及 try/catch/finally（E6012 未捕获变体、E3027 兜底顺序、
//! E6013 break/continue 跨 try 边界——判据是 `loop_depth` 是否深于进入 try 时的
//! `try_loop_base`，不能用 loop_labels.len()）。
//! 每个入口把结果写进 `Sema::slots`（键 = 节点 id），backend 只读不查。

use crate::frontend::ast;
use crate::frontend::ast::{Expr, IfTail, Stmt};
use crate::sema::{ScopeNode, Sema, StmtUse, UseKind, VarInfo, VarState};
use crate::types::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

impl<'a> Sema<'a> {
    // ---------------------------------------------------------------- 作用域

    /// 当前 `break`/`continue` 是否会跨出某一层 try 块（从而跳过它的 finally）。
    fn crosses_try_boundary(&self) -> bool {
        match self.try_loop_base.last() {
            // 注意用 loop_depth：loop_labels 只在带标签的循环上入栈，无标签循环不占位
            Some(&base) => self.loop_depth <= base,
            None => false,
        }
    }

    pub fn push_scope(&mut self) {
        self.scope = Some(Rc::new(RefCell::new(ScopeNode {
            parent: self.scope.take(),
            names: HashMap::new(),
        })));
        self.narrow.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        let s = self.scope.take();
        self.scope = s.and_then(|s| s.borrow().parent.clone());
        self.narrow.pop();
    }

    pub fn declare(
        &mut self,
        name: &str,
        ty: Ty,
        is_const: bool,
        tok: &crate::frontend::lexer::Tok,
    ) -> Rc<RefCell<VarInfo>> {
        // 先查整条作用域链，命中即报遮蔽（decl_line 永不为 -1，必报）
        if let Some(v) = self.lookup_var(name) {
            let dl = v.borrow().decl_line;
            self.err("E3001", tok, "N-3", "变量 `{0}` 遮蔽了外层同名变量（第 {1} 行）", &[&name, &if dl == 0 { 1 } else { dl }]);
            return Rc::new(RefCell::new(VarInfo {
                name: name.to_string(),
                ty,
                is_const,
                state: VarState::Live,
                decl_line: tok.line,
                is_param: false,
                idx: 0,
                arena_region: None,
            }));
        }
        if self.lookup_type_name(name, tok).is_some() {
            self.err("E3018", tok, "N-16", "变量与类型同名 `{0}`，`T(x)` 无法判定是转换还是调用", &[&name]);
        }
        let v = Rc::new(RefCell::new(VarInfo {
            name: name.to_string(),
            ty,
            is_const,
            state: VarState::Live,
            decl_line: tok.line,
            is_param: false,
            idx: 0,
            arena_region: None,
        }));
        if let Some(s) = &self.scope {
            s.borrow_mut().names.insert(name.to_string(), v.clone());
        }
        v
    }

    pub fn lookup_var(&self, name: &str) -> Option<Rc<RefCell<VarInfo>>> {
        let mut cur = self.scope.clone();
        while let Some(s) = cur {
            if let Some(v) = s.borrow().names.get(name) {
                return Some(v.clone());
            }
            cur = s.borrow().parent.clone();
        }
        None
    }

    pub fn narrowed_ty_vi(&self, vi: &VarInfo) -> Ty {
        for frame in self.narrow.iter().rev() {
            if frame.get(&vi.name).copied().unwrap_or(false) {
                if let TyS::Nullable(inner) = &*vi.ty {
                    return inner.clone();
                }
                return vi.ty.clone();
            }
        }
        vi.ty.clone()
    }

    pub fn reset_narrow(&mut self, name: &str) {
        for frame in self.narrow.iter_mut() {
            frame.remove(name);
        }
    }

    // ------------------------------------------------------ 所有权状态合并（O-4/O-5）

    /// 变量状态快照：沿作用域链收集全部 (变量, 当前状态)。
    /// 分支/循环检查前拍快照，各路径从快照出发独立检查。
    pub fn var_states_snapshot(&self) -> Vec<(Rc<RefCell<VarInfo>>, VarState)> {
        let mut out = Vec::new();
        let mut cur = self.scope.clone();
        while let Some(s) = cur {
            for v in s.borrow().names.values() {
                out.push((v.clone(), v.borrow().state.clone()));
            }
            cur = s.borrow().parent.clone();
        }
        out
    }

    /// 把所有变量恢复到快照状态（分支检查互不泄漏）。
    pub fn restore_var_states(snap: &[(Rc<RefCell<VarInfo>>, VarState)]) {
        for (v, st) in snap {
            v.borrow_mut().state = st.clone();
        }
    }

    /// 一条路径结束时哪些变量处于 Moved（相对快照变量集）。
    /// 返回与快照同序的 bool 列表：true = 该路径把变量 move 了且未复活。
    fn moved_flags(snap: &[(Rc<RefCell<VarInfo>>, VarState)]) -> Vec<bool> {
        snap.iter()
            .map(|(v, _)| v.borrow().state == VarState::Moved)
            .collect()
    }

    /// 按最保守合并应用结果（GAP-13：任一可达路径把变量 move 了，
    /// 汇合点即视为 Moved；全部路径都复活才回到 Live）。
    /// `paths`：每个可达路径的 moved 标志列表（与快照同序同长）。
    fn apply_merged_states(snap: &[(Rc<RefCell<VarInfo>>, VarState)], paths: &[Vec<bool>]) {
        for (i, (v, _)) in snap.iter().enumerate() {
            let moved = paths.iter().any(|p| p.get(i).copied().unwrap_or(false));
            v.borrow_mut().state = if moved {
                VarState::Moved
            } else {
                VarState::Live
            };
        }
    }

    /// 循环体两遍检查（O-5，规格书 11.2）：
    /// 第一遍 **probe**——抑制诊断，只收集"体内会把哪些外层变量 move 掉"；
    /// 第二遍把这些变量按 Moved 状态入场正式检查——下一轮迭代看到的
    /// 就是移走后的状态，use-after-move 在使用点报 E7001。
    /// 体尾永远离开循环（return/throw/break，continue 算回头）时不回卷，
    /// 跳过两遍。`loop_vars` 是循环头声明的迭代变量（每轮重新绑定，
    /// 不参与合并，否则 `for d in dogs { feed(d) }` 会被误报）。
    fn check_loop_body(&mut self, body: &ast::Block, loop_vars: &[String]) {
        if !self.block_leaves_loop(body) {
            let snap = self.var_states_snapshot();
            self.probe = true;
            self.check_block(body, true);
            self.probe = false;
            let flags = Self::moved_flags(&snap);
            Sema::restore_var_states(&snap);
            for (i, (v, _)) in snap.iter().enumerate() {
                if flags[i] && !loop_vars.contains(&v.borrow().name) {
                    v.borrow_mut().state = VarState::Moved;
                }
            }
        }
        self.check_block(body, true);
    }

    /// 循环体尾部是否必然离开循环（不回来）。`continue` 回到循环头，
    /// 不算离开；`return`/`throw`/`break` 离开。
    fn block_leaves_loop(&self, b: &ast::Block) -> bool {
        match b.stmts.last() {
            None => false,
            Some(l) => self.stmt_leaves_loop(l),
        }
    }

    fn stmt_leaves_loop(&self, s: &ast::SPtr) -> bool {
        match &s.k {
            Stmt::Return { .. } | Stmt::Throw { .. } | Stmt::Break { .. } => true,
            Stmt::Continue { .. } => false,
            Stmt::While { .. } => false,
            Stmt::Block(b) => self.block_leaves_loop(b),
            Stmt::If(n) => {
                let then_ok = self.block_leaves_loop(&n.then_);
                match &n.els {
                    Some(IfTail::Block(b)) => then_ok && self.block_leaves_loop(b),
                    Some(IfTail::If(inner)) => then_ok && self.ifnode_leaves_loop(&inner.k),
                    None => false,
                }
            }
            _ => false,
        }
    }

    fn ifnode_leaves_loop(&self, n: &ast::IfNode) -> bool {
        let els = match &n.els {
            None => return false,
            Some(e) => e,
        };
        let then_ok = self.block_leaves_loop(&n.then_);
        match els {
            IfTail::Block(b) => then_ok && self.block_leaves_loop(b),
            IfTail::If(inner) => then_ok && self.ifnode_leaves_loop(&inner.k),
        }
    }

    /// 分支汇合用"必然离开函数"判定（O-4）：只有 return/throw（及
    /// 两支都离开的 if/try）才算。**不含** `stmt_must_exit` 把任意调用
    /// 语句当作退出的历史近似——`sink(b)` 这类普通调用语句之后代码
    /// 照常执行，它的 move 必须参与合并。break/continue 跳去循环
    /// 边界，保守视为可达。
    fn block_leaves_func(&self, b: &ast::Block) -> bool {
        match b.stmts.last() {
            None => false,
            Some(l) => self.stmt_leaves_func(l),
        }
    }

    fn stmt_leaves_func(&self, s: &ast::SPtr) -> bool {
        match &s.k {
            Stmt::Return { .. } | Stmt::Throw { .. } => true,
            Stmt::Block(b) => self.block_leaves_func(b),
            Stmt::If(n) => self.ifnode_leaves_func(n),
            Stmt::TryCatch(n) => {
                if let Some(f) = &n.finally {
                    if self.block_leaves_func(f) {
                        return true;
                    }
                }
                self.block_leaves_func(&n.body)
                    && !n.catches.is_empty()
                    && n.catches.iter().all(|c| self.block_leaves_func(&c.body))
            }
            _ => false,
        }
    }

    fn ifnode_leaves_func(&self, n: &ast::IfNode) -> bool {
        let els = match &n.els {
            None => return false,
            Some(e) => e,
        };
        let then_ok = self.block_leaves_func(&n.then_);
        match els {
            IfTail::Block(b) => then_ok && self.block_leaves_func(b),
            IfTail::If(inner) => then_ok && self.ifnode_leaves_func(&inner.k),
        }
    }

    // ---------------------------------------------------------------- 语句

    pub fn check_block(&mut self, b: &ast::Block, new_scope: bool) {
        if new_scope {
            self.push_scope();
        }
        for s in &b.stmts {
            self.check_stmt(s);
        }
        if new_scope {
            self.pop_scope();
        }
    }

    pub fn check_stmt(&mut self, s: &ast::SPtr) {
        // O-7/O-12：单语句借用扫描。入口清空上一语句遗留的使用记录，
        // 出口按 place 分组互查；借用不跨语句，语句结束即失效。
        self.stmt_uses.clear();
        match &s.k {
            Stmt::Block(b) => self.check_block(b, true),
            Stmt::VarDecl { .. } => self.check_var_decl(s),
            Stmt::VarTuple { names, init } => {
                let t = self.check_expr(init, None);
                match &*t {
                    TyS::Tuple(elems) if elems.len() == names.len() => {
                        for (n, et) in names.iter().zip(elems.iter()) {
                            self.declare(n, et.clone(), false, &s.tok);
                        }
                    }
                    TyS::Tuple(elems) => {
                        self.err("E4027", &s.tok, "T-38", "元组元素个数不符：{0} 个变量，{1} 个元素", &[&names.len(), &elems.len()]);
                        for n in names {
                            self.declare(n, t_object(), false, &s.tok);
                        }
                    }
                    _ => {
                        self.err("E4027", &init.tok, "T-38", "解构右侧必须是元组", &[]);
                        for n in names {
                            self.declare(n, t_object(), false, &s.tok);
                        }
                    }
                }
            }
            Stmt::Simple { .. } => self.check_simple(s),
            Stmt::If(n) => {
                self.check_if(s.id, &s.tok, n, None);
            }
            Stmt::Match(n) => {
                self.check_match(s.id, &s.tok, n, None);
            }
            Stmt::For { .. } => self.check_for(s),
            Stmt::While { .. } => self.check_while(s),
            Stmt::Unsafe { body } => self.check_block(body, true),
            Stmt::Return { .. } => self.check_return(s),
            Stmt::Throw { .. } => self.check_throw(s),
            Stmt::TryCatch(_) => self.check_try_catch(s),
            Stmt::Defer { .. } => self.check_defer(s),
            Stmt::Break { label } => {
                if self.crosses_try_boundary() {
                    self.err("E6013", &s.tok, "文稿 10.2", "`break` 会跳出 try 块（finally 不会执行）；把循环移到 try 外面", &[]);
                } else if self.loop_depth == 0 {
                    self.err("E6009", &s.tok, "文稿 5.2", "`break` 不在循环内", &[]);
                } else if let Some(l) = label {
                    if !self.loop_labels.contains(l) {
                        self.err("E2014", &s.tok, "文稿 5.2", "没有名为 `{0}` 的循环标签", &[&l]);
                    }
                }
            }
            Stmt::Continue { label } => {
                if self.crosses_try_boundary() {
                    self.err("E6013", &s.tok, "文稿 10.2", "`continue` 会跳出 try 块（finally 不会执行）；把循环移到 try 外面", &[]);
                } else if self.loop_depth == 0 {
                    self.err("E6009", &s.tok, "文稿 5.2", "`continue` 不在循环内", &[]);
                } else if let Some(l) = label {
                    if !self.loop_labels.contains(l) {
                        self.err("E2014", &s.tok, "文稿 5.2", "没有名为 `{0}` 的循环标签", &[&l]);
                    }
                }
            }
            Stmt::Const { .. } => self.check_local_const(s),
            Stmt::FuncDecl { .. } => {
                // 名称已在 check_func 的 pre_register_nested_funcs 阶段注册到
                // funcs/func_by_name 与父函数的 nested 表；这里只检查函数体。
                // check_func 会替换 scope（parent: None），因此嵌套函数体内
                // 看不到父函数的局部变量（不捕获外部变量）。
                let name = match &s.k {
                    Stmt::FuncDecl { name, .. } => name.clone(),
                    _ => unreachable!(),
                };
                let cur = self.current_func.expect("嵌套函数只能在函数体内出现");
                let reg = format!("{}${name}", self.funcs[cur].name);
                let fid = self.func_by_name[&reg];
                self.check_func(fid);
            }
        }
        self.scan_stmt_uses();
    }

    fn check_local_const(&mut self, s: &ast::SPtr) {
        let (ty_node, name, init) = match &s.k {
            Stmt::Const { ty, name, init, .. } => (ty, name, init),
            _ => return,
        };
        let mut ty: Option<Ty> = None;
        if let Some(t) = ty_node {
            ty = Some(self.resolve_type(t));
        }
        let t = self.check_expr(init, ty.as_ref());
        if ty.is_none() {
            if matches!(&*t, TyS::Null | TyS::Void | TyS::Range) {
                self.err("E4007", &init.tok, "T-12", "无法推断 const 类型", &[]);
                ty = Some(t_int());
            } else {
                ty = Some(t);
            }
        } else {
            let dst = ty.clone().unwrap();
            self.coerce(init, &dst, "const 初始化");
        }
        let ty = ty.unwrap();
        let v = self.eval_const(init);
        if v.is_none() {
            self.err("E3003", &init.tok, "S-2", "const 初始化器必须是常量表达式", &[]);
        }
        self.sl(s.id).const_value = v;
        self.declare(name, ty, true, &s.tok);
    }

    fn check_var_decl(&mut self, s: &ast::SPtr) {
        let (explicit_ty, is_var, name, init) = match &s.k {
            Stmt::VarDecl { explicit_ty, is_var, name, init } => (explicit_ty, is_var, name, init),
            _ => return,
        };
        let mut ty: Option<Ty> = None;
        if let Some(t) = explicit_ty {
            ty = Some(self.resolve_type(t));
        }
        let init = match init {
            Some(i) => i,
            None => {
                self.err("E3002", &s.tok, "文稿 3.3", "变量声明必须有初值", &[]);
                let v = self.declare(name, ty.unwrap_or_else(t_object), false, &s.tok);
                self.sl(s.id).varinfo = Some(v);
                return;
            }
        };
        let t = self.check_expr(init, ty.as_ref());
        if ty.is_none() {
            ty = Some(match &*t {
                TyS::Null => {
                    self.err("E4007", &init.tok, "T-11", "`var` 不能从 null 推断类型", &[]);
                    t_object()
                }
                TyS::Void => {
                    self.err("E4007", &init.tok, "T-11", "`var` 不能从 void 表达式推断类型", &[]);
                    t_object()
                }
                TyS::Range => {
                    self.err("E4007", &init.tok, "T-11", "`var` 不能持有 range（range 只能用于 for 头）", &[]);
                    t_int()
                }
                TyS::Never => t_object(),
                _ => t,
            });
        } else {
            let dst = ty.clone().unwrap();
            self.coerce(init, &dst, "变量初始化");
        }
        let ty = ty.unwrap();
        if *is_var && self.current_func.is_none() {
            self.err("E3006", &s.tok, "T-13", "`var` 只能用于局部变量", &[]);
        }
        let v = self.declare(name, ty.clone(), false, &s.tok);
        self.sl(s.id).varinfo = Some(v.clone());
        if self.moves(&ty) {
            self.take(init);
        }
        // O-18：Arena / ArenaRef 变量绑定来源 region
        //（`new Arena()` → Arena 变量；`ar.Alloc(x)` → ArenaRef 变量）
        if let Some(r) = self.slot(&init.id).and_then(|sl| sl.arena_region) {
            v.borrow_mut().arena_region = Some(r);
        }
    }

    // ---------------------------------------------------------------- while

    fn check_while(&mut self, s: &ast::SPtr) {
        let (cond, body) = match &s.k {
            Stmt::While { cond, body } => (cond, body),
            _ => return,
        };
        self.check_cond(cond, "while");
        self.loop_depth += 1;
        self.check_loop_body(body, &[]);
        self.loop_depth -= 1;
    }

    pub fn check_simple(&mut self, s: &ast::SPtr) {
        let (lhs, op, rhs, incdec) = match &s.k {
            Stmt::Simple { lhs, op, rhs, incdec } => (lhs, op, rhs, incdec),
            _ => return,
        };
        if let Some(d) = incdec {
            let t = self.check_expr(lhs, None);
            if !(is_int(&t) || matches!(&*t, TyS::Float | TyS::Decimal)) {
                self.err("E4009", &lhs.tok, "文稿 11.1", "`{0}` 只能用于数值", &[&d]);
            }
            if let Expr::Path { segs, .. } = &lhs.k {
                if let Some(v) = self.lookup_var(&segs[0]) {
                    if v.borrow().is_const {
                        let vn = v.borrow().name.clone();
                        self.err("E3005", &lhs.tok, "文稿 3.3", "不能给 const `{0}` 赋值", &[&vn]);
                    }
                    v.borrow_mut().state = VarState::Live;
                    self.reset_narrow(&segs[0]);
                }
            } else if !self.is_place(lhs) {
                self.err("E4001", &lhs.tok, "文稿 11.1", "`{0}` 左侧必须是可写位置", &[&d]);
            }
            return;
        }
        // O-3：重新赋值可复活。对直接变量左值先放回 Live 再取类型——
        // 否则"读左值"本身就触发 E7001，而 `b = new Buf(4)` 恰是复活动作。
        // 字段/元素左值不在此列：对已 move 的对象写字段仍是 use-after-move。
        if op.as_deref() == Some("=") {
            if let Expr::Path { segs, .. } = &lhs.k {
                if let Some(v) = self.lookup_var(&segs[0]) {
                    if v.borrow().state == VarState::Moved {
                        v.borrow_mut().state = VarState::Live;
                    }
                }
            }
        }
        let t = self.check_expr(lhs, None);
        let op = match op {
            None => {
                self.sl(s.id).ty = Some(t);
                return;
            }
            Some(o) => o.clone(),
        };
        if !self.is_place(lhs) && op == "=" {
            self.err("E4001", &lhs.tok, "文稿 11.1", "赋值左侧必须是可写位置", &[]);
        }
        // const 不可写
        if let Expr::Path { segs, .. } = &lhs.k {
            if let Some(v) = self.lookup_var(&segs[0]) {
                if v.borrow().is_const {
                    let vn = v.borrow().name.clone();
                    self.err("E3005", &lhs.tok, "文稿 3.3", "不能给 const `{0}` 赋值", &[&vn]);
                }
            }
        }
        let rhs = rhs.as_ref().unwrap();
        let rhs_exp = if op == "=" { Some(t.clone()) } else { None };
        self.check_expr(rhs, rhs_exp.as_ref());
        if op == "=" {
            self.coerce(rhs, &t, "赋值");
            if self.moves(&t) {
                self.take(rhs);
            }
            // 写入也是独占使用（O-7：`f(&x, x = 1)` 一类冲突由语句扫描覆盖）
            if self.is_place(lhs) {
                if let Some(place) = self.place_key(lhs) {
                    self.stmt_uses.push(StmtUse {
                        place,
                        kind: UseKind::Write,
                        tok: lhs.tok.clone(),
                    });
                }
            }
            // O-18：ArenaRef 赋值的 region 绑定与逃逸检测
            if matches!(&*t, TyS::ArenaRef(_)) {
                let rr = self.slot(&rhs.id).and_then(|sl| sl.arena_region);
                match &lhs.k {
                    Expr::Path { segs, .. } => {
                        if let Some(v) = self.lookup_var(&segs[0]) {
                            let lvr = v.borrow().arena_region;
                            match (lvr, rr) {
                                (Some(r2), Some(r)) if r2 == r => {} // 同区域别名，合法
                                (Some(_), Some(_)) => {
                                    self.err(
                                        "E7006",
                                        &rhs.tok,
                                        "O-18",
                                        "`ArenaRef` 不能逃出 arena 的作用域（{0}）",
                                        &[&"两个变量来自不同的 arena"],
                                    );
                                }
                                (None, Some(r)) => {
                                    if self.region_in_scope(r) {
                                        // 赋值点在区域内：绑定来源 region，之后
                                        // 在块外的使用点会触发 E7006
                                        v.borrow_mut().arena_region = Some(r);
                                    } else {
                                        let vn = v.borrow().name.clone();
                                        let detail =
                                            format!("赋给 arena 块外的变量 `{0}`", vn);
                                        self.err(
                                            "E7006",
                                            &rhs.tok,
                                            "O-18",
                                            "`ArenaRef` 不能逃出 arena 的作用域（{0}）",
                                            &[&detail],
                                        );
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Expr::FieldAccess { recv, .. } => {
                        // 存进"区域内对象"（ArenaRef 解引用后的字段）合法——
                        // 这是图/双向链表的写法（文稿 8.4）；存进普通对象
                        // 字段 → 逃逸。
                        let rt = self.ty_of(recv.id);
                        let in_region = matches!(
                            rt.as_deref(),
                            Some(TyS::ArenaRef(_))
                        );
                        if !in_region {
                            self.err(
                                "E7006",
                                &rhs.tok,
                                "O-18",
                                "`ArenaRef` 不能逃出 arena 的作用域（{0}）",
                                &[&"存进 arena 区域外的对象字段"],
                            );
                        }
                    }
                    Expr::Index { .. } => {
                        self.err(
                            "E7006",
                            &rhs.tok,
                            "O-18",
                            "`ArenaRef` 不能逃出 arena 的作用域（{0}）",
                            &[&"存进容器（数组/map）"],
                        );
                    }
                    _ => {}
                }
            }
            if let Expr::Path { segs, .. } = &lhs.k {
                if let Some(v) = self.lookup_var(&segs[0]) {
                    v.borrow_mut().state = VarState::Live;
                    self.reset_narrow(&segs[0]);
                }
            }
            self.sl(s.id).ty = Some(t);
            return;
        }
        // 组合赋值：按等价的二元运算检查，转换信息随之丢弃（历史行为，测试钉死）
        let op0 = &op[0..1];
        let junk = u32::MAX;
        let _ = self.check_binary(junk, op0, lhs, rhs);
        self.coerce(rhs, &t, "组合赋值");
        self.sl(s.id).ty = Some(t);
        if let Expr::Path { segs, .. } = &lhs.k {
            if self.lookup_var(&segs[0]).is_some() {
                self.reset_narrow(&segs[0]);
            }
        }
    }

    pub fn is_place(&self, e: &ast::EPtr) -> bool {
        match &e.k {
            Expr::Path { segs, .. } => self.lookup_var(&segs[0]).is_some(),
            Expr::FieldAccess { .. } | Expr::Index { .. } => true,
            _ => false,
        }
    }

    // ---------------------------------------------------------------- if

    pub fn check_if(
        &mut self,
        sid: u32,
        tok: &crate::frontend::lexer::Tok,
        n: &ast::IfNode,
        expected: Option<&Ty>,
    ) {
        self.check_cond(&n.cond, "if");
        let then_narrow = self.cond_narrow(&n.cond, true);
        let else_narrow = self.cond_narrow(&n.cond, false);
        // O-4 分支汇合：两支各自从进入前的状态快照出发独立检查
        // （否则 then 里的 move 会泄漏进 else，造成误报 E7001）；
        // 结束后按最保守合并——任一可达路径把变量 move 了，汇合点即 Moved。
        // 永远离开（return/throw/break）的分支不可达，不参与合并。
        let snap = self.var_states_snapshot();
        self.narrow.push(then_narrow);
        self.check_block(&n.then_, true);
        self.narrow.pop();
        let then_exits = self.block_leaves_func(&n.then_);
        let mut paths: Vec<Vec<bool>> = Vec::new();
        if !then_exits {
            paths.push(Self::moved_flags(&snap));
        }
        Sema::restore_var_states(&snap);
        match &n.els {
            Some(els) => {
                self.narrow.push(else_narrow);
                match els {
                    IfTail::Block(b) => self.check_block(b, true),
                    IfTail::If(inner) => self.check_if(inner.id, &inner.tok, &inner.k, None),
                }
                self.narrow.pop();
                let else_exits = match els {
                    IfTail::Block(b) => self.block_leaves_func(b),
                    IfTail::If(inner) => self.ifnode_leaves_func(&inner.k),
                };
                if !else_exits {
                    paths.push(Self::moved_flags(&snap));
                }
            }
            None => {
                // 没有 else：跳过整个 if 是一条可达路径，状态即快照
                paths.push(snap.iter().map(|(_, st)| *st == VarState::Moved).collect());
                if expected.map_or(false, |t| !matches!(&**t, TyS::Void)) {
                    self.err("E2005", tok, "G-1", "`if` 用作表达式时必须有 `else`", &[]);
                }
                if then_exits {
                    let frame = self.narrow.last_mut().unwrap();
                    for k in else_narrow.keys() {
                        frame.insert(k.clone(), true);
                    }
                }
            }
        }
        // O-4：汇合点状态 = 各可达路径的保守合并
        Sema::apply_merged_states(&snap, &paths);
        // 类型
        let is_expr = expected.map_or(false, |t| !matches!(&**t, TyS::Void));
        if is_expr && n.els.is_some() {
            let tt = self.block_value_type(&n.then_);
            let et = match &n.els {
                Some(IfTail::Block(b)) => self.block_value_type(b),
                Some(IfTail::If(inner)) => self.ty_of(inner.id),
                None => None,
            };
            if let (Some(tt), Some(et)) = (&tt, &et) {
                if tt != et && !matches!(&**tt, TyS::Never) && !matches!(&**et, TyS::Never) {
                    self.err("E4013", tok, "文稿 5.1", "`if` 两分支类型不一致：{0} vs {1}", &[&self.ts(tt), &self.ts(et)]);
                }
            }
            let ty = tt.unwrap_or_else(|| expected.unwrap().clone());
            self.sl(sid).ty = Some(ty);
        } else if is_expr {
            // 缺 else：E2005 已报，不再追加"两分支类型不一致"的假错误
            self.sl(sid).ty = Some(expected.unwrap().clone());
        } else {
            self.sl(sid).ty = Some(t_void());
        }
    }

    pub fn block_must_exit_block(&self, b: &ast::Block) -> bool {
        match b.stmts.last() {
            None => false,
            Some(last) => self.stmt_must_exit(last),
        }
    }

    pub fn stmt_must_exit(&self, s: &ast::SPtr) -> bool {
        match &s.k {
            Stmt::Return { .. } | Stmt::Throw { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => true,
            Stmt::Block(b) => self.block_must_exit_block(b),
            Stmt::If(n) => self.ifnode_must_exit(n),
            Stmt::Match(n) => {
                !n.arms.is_empty() && n.arms.iter().all(|a| self.expr_must_exit(&a.body))
            }
            Stmt::Simple { lhs, .. } => matches!(&lhs.k, Expr::Call { .. }),
            // try/catch 一定离开函数，当且仅当：finally 自己离开，或
            // 「没有 finally」且 try 体与每个 catch 体都离开
            //（未匹配的 catch 会按原路径 return fail，也算离开）。
            Stmt::TryCatch(n) => {
                if let Some(f) = &n.finally {
                    if self.block_must_exit_block(f) {
                        return true;
                    }
                }
                self.block_must_exit_block(&n.body)
                    && !n.catches.is_empty()
                    && n.catches.iter().all(|c| self.block_must_exit_block(&c.body))
            }
            _ => false,
        }
    }

    fn ifnode_must_exit(&self, n: &ast::IfNode) -> bool {
        let els = match &n.els {
            None => return false,
            Some(e) => e,
        };
        let then_ok = self.block_must_exit_block(&n.then_);
        match els {
            IfTail::Block(b) => then_ok && self.block_must_exit_block(b),
            IfTail::If(inner) => then_ok && self.ifnode_must_exit(&inner.k),
        }
    }

    pub fn expr_must_exit(&self, e: &ast::EPtr) -> bool {
        match &e.k {
            Expr::Call { .. } => true,
            Expr::If(n) => self.ifnode_must_exit(n),
            Expr::Match(n) => {
                !n.arms.is_empty() && n.arms.iter().all(|a| self.expr_must_exit(&a.body))
            }
            _ => false,
        }
    }

    pub fn block_returns(&self, b: Option<&ast::Block>) -> bool {
        match b {
            None => false,
            Some(b) => self.block_must_exit_block(b),
        }
    }

    pub fn block_value_type(&self, b: &ast::Block) -> Option<Ty> {
        let last = b.stmts.last()?;
        match &last.k {
            Stmt::Simple {
                lhs,
                op: None,
                rhs: None,
                incdec: None,
            } => self.ty_of(lhs.id),
            Stmt::If(..) | Stmt::Match(..) => self.ty_of(last.id),
            _ => Some(t_void()),
        }
    }

    pub fn cond_narrow(&self, cond: &ast::EPtr, truth: bool) -> HashMap<String, bool> {
        let mut out = HashMap::new();
        match &cond.k {
            Expr::Binary { op, lhs, rhs } if op == "==" || op == "!=" => {
                if let (Expr::Path { segs, .. }, Expr::NullLit) = (&lhs.k, &rhs.k) {
                    if (op == "==" && !truth) || (op == "!=" && truth) {
                        out.insert(segs[0].clone(), true);
                    }
                }
                if let (Expr::Path { segs, .. }, Expr::NullLit) = (&rhs.k, &lhs.k) {
                    if (op == "==" && !truth) || (op == "!=" && truth) {
                        out.insert(segs[0].clone(), true);
                    }
                }
            }
            Expr::Unary { op, operand } if op == "!" => {
                return self.cond_narrow(operand, !truth);
            }
            _ => {}
        }
        out
    }

    pub fn check_cond(&mut self, e: &ast::EPtr, what: &str) {
        let t = self.check_expr(e, Some(&t_bool()));
        if matches!(&*t, TyS::Object) {
            self.err("E5002", &e.tok, "T-10", "`object` 不能直接作为条件；先落到 bool 变量或写 `bool(o)`", &[]);
        } else if t != t_bool() {
            self.err("E4002", &e.tok, "文稿 5.1", "`{0}` 的条件必须是 bool，得到 {1}", &[&what, &self.ts(&t)]);
        }
    }

    // ---------------------------------------------------------------- for

    fn check_for(&mut self, s: &ast::SPtr) {
        let (label, kind, body) = match &s.k {
            Stmt::For { label, kind, body } => (label, kind, body),
            _ => return,
        };
        match kind {
            ast::ForKind::Cond => {
                self.loop_depth += 1;
                if let Some(l) = label {
                    if self.loop_labels.contains(l) {
                        self.err("E6008", &s.tok, "文稿 5.2", "循环标签 `{0}` 重复", &[&l]);
                    }
                    self.loop_labels.push(l.clone());
                }
                self.check_loop_body(body, &[]);
                if label.is_some() {
                    self.loop_labels.pop();
                }
                self.loop_depth -= 1;
            }
            ast::ForKind::Range { var, lo, hi, .. } => {
                self.check_expr(lo, Some(&t_int()));
                self.check_expr(hi, Some(&t_int()));
                let lo_t = self.ty_of(lo.id).unwrap_or_else(t_object);
                let hi_t = self.ty_of(hi.id).unwrap_or_else(t_object);
                if !(is_int(&lo_t) && is_int(&hi_t)) {
                    self.err("E4009", &s.tok, "文稿 5.2", "range 两端必须是 int", &[]);
                }
                self.loop_depth += 1;
                if let Some(l) = label {
                    if self.loop_labels.contains(l) {
                        self.err("E6008", &s.tok, "文稿 5.2", "循环标签 `{0}` 重复", &[&l]);
                    }
                    self.loop_labels.push(l.clone());
                }
                self.push_scope();
                self.declare(var, t_int(), false, &s.tok);
                self.check_loop_body(body, std::slice::from_ref(var));
                self.pop_scope();
                if label.is_some() {
                    self.loop_labels.pop();
                }
                self.loop_depth -= 1;
            }
            ast::ForKind::Step { var, lo, hi, step, inclusive: _ } => {
                self.check_expr(lo, Some(&t_int()));
                self.check_expr(hi, Some(&t_int()));
                self.check_expr(step, Some(&t_int()));
                let lo_t = self.ty_of(lo.id).unwrap_or_else(t_object);
                let hi_t = self.ty_of(hi.id).unwrap_or_else(t_object);
                let step_t = self.ty_of(step.id).unwrap_or_else(t_object);
                if !(is_int(&lo_t) && is_int(&hi_t) && is_int(&step_t)) {
                    self.err("E4009", &s.tok, "文稿 5.2", "range.Step 的两端与步长必须是 int", &[]);
                }
                self.loop_depth += 1;
                if let Some(l) = label {
                    if self.loop_labels.contains(l) {
                        self.err("E6008", &s.tok, "文稿 5.2", "循环标签 `{0}` 重复", &[&l]);
                    }
                    self.loop_labels.push(l.clone());
                }
                self.push_scope();
                self.declare(var, t_int(), false, &s.tok);
                self.check_loop_body(body, std::slice::from_ref(var));
                self.pop_scope();
                if label.is_some() {
                    self.loop_labels.pop();
                }
                self.loop_depth -= 1;
            }
            ast::ForKind::Iter { var, iterable } | ast::ForKind::Kv { var, iterable, .. } => {
                let kv = matches!(kind, ast::ForKind::Kv { .. });
                let t = self.check_expr(iterable, None);
                let elem: Ty;
                let mut elem2: Ty = t_int();
                if kv {
                    match &*t {
                        TyS::Map(k, v) => {
                            elem = k.clone();
                            elem2.clone_from(v);
                        }
                        _ => {
                            self.err("E4025", &iterable.tok, "文稿 5.2", "`(k, v) in` 只能遍历 map，得到 {0}", &[&self.ts(&t)]);
                            elem = t_int();
                        }
                    }
                } else {
                    elem = match &*t {
                        TyS::Array(e, _) => e.clone(),
                        TyS::Str => t_string(),
                        TyS::Map(k, _) => k.clone(),
                        TyS::Range => t_int(),
                        _ => {
                            self.err("E4025", &iterable.tok, "文稿 5.2", "{0} 不能遍历（只支持数组、map、string、range）", &[&self.ts(&t)]);
                            t_object()
                        }
                    };
                }
                let target = match &iterable.k {
                    Expr::Path { segs, .. } => Some(segs[0].clone()),
                    _ => None,
                };
                if let Some(tg) = &target {
                    self.iter_targets.push(tg.clone());
                }
                self.loop_depth += 1;
                if let Some(l) = label {
                    if self.loop_labels.contains(l) {
                        self.err("E6008", &s.tok, "文稿 5.2", "循环标签 `{0}` 重复", &[&l]);
                    }
                    self.loop_labels.push(l.clone());
                }
                self.push_scope();
                if kv {
                    let (v1, v2) = match kind {
                        ast::ForKind::Kv { var, var2, .. } => (var, var2),
                        _ => unreachable!(),
                    };
                    self.declare(v1, elem, false, &s.tok);
                    self.declare(v2, elem2, false, &s.tok);
                } else {
                    self.declare(var, elem, false, &s.tok);
                }
                let mut lv: Vec<String> = Vec::new();
                if kv {
                    if let ast::ForKind::Kv { var, var2, .. } = kind {
                        lv.push(var.clone());
                        lv.push(var2.clone());
                    }
                } else {
                    lv.push(var.clone());
                }
                self.check_loop_body(body, &lv);
                self.pop_scope();
                if label.is_some() {
                    self.loop_labels.pop();
                }
                self.loop_depth -= 1;
                if target.is_some() {
                    self.iter_targets.pop();
                }
            }
        }
    }

    // ---------------------------------------------------------------- return/throw/defer

    fn check_return(&mut self, s: &ast::SPtr) {
        let value = match &s.k {
            Stmt::Return { value } => value,
            _ => return,
        };
        let ret = match self.current_func {
            Some(f) => self.funcs[f].ret.clone(),
            None => t_void(),
        };
        let value = match value {
            None => {
                if !matches!(&*ret, TyS::Void) && !self.in_main {
                    self.err("E4001", &s.tok, "T-5", "函数需要返回 {0}，但 `return` 没有值", &[&self.ts(&ret)]);
                }
                return;
            }
            Some(v) => v,
        };
        let exp = if matches!(&*ret, TyS::Void) {
            None
        } else {
            Some(ret.clone())
        };
        let t = self.check_expr(value, exp.as_ref());
        // O-18：ArenaRef 不能作为返回值逃出函数
        if matches!(&*t, TyS::ArenaRef(_)) {
            self.err(
                "E7006",
                &value.tok,
                "O-18",
                "`ArenaRef` 不能逃出 arena 的作用域（{0}）",
                &[&"作为返回值"],
            );
        }
        if matches!(&*ret, TyS::Void) {
            if self.in_main {
                if t != t_int() {
                    self.coerce(value, &t_int(), "main 返回值");
                }
            } else if !matches!(&*t, TyS::Void | TyS::Never) {
                self.err("E4001", &value.tok, "T-5", "void 函数不能返回值", &[]);
            }
            return;
        }
        if self.in_main && ret != t_int() && t == t_int() {
            return;
        }
        self.coerce(value, &ret, "return");
        if self.moves(&ret) {
            self.take(value);
        }
    }

    fn check_throw(&mut self, s: &ast::SPtr) {
        let (variant, msg) = match &s.k {
            Stmt::Throw { variant, msg } => (variant, msg),
            _ => return,
        };
        if !self.in_result && !self.in_main && self.try_depth == 0 {
            self.err("E6010", &s.tok, "文稿 10.1", "`throw` 只能出现在返回 result<...> 的函数内或 try 块内（main 除外）", &[]);
        }
        let t = self.check_expr(msg, Some(&t_string()));
        if t != t_string() {
            self.err("E4001", &msg.tok, "T-5", "`throw` 的消息必须是 string", &[]);
        }
        if !matches!(variant.as_str(), "NotFound" | "Invalid" | "Money" | "IO") {
            self.err("E3027", &s.tok, "文稿 10.1", "未知的错误变体 `error.{0}`（可用：NotFound/Invalid/Money/IO）", &[&variant]);
        }
        self.sl(s.id).ty = Some(t_never());
    }

    /// `try { } catch (error e) { } finally { }`
    ///
    /// try 体内 `try_depth` +1：`throw` 与失败展开有就地捕获目标，
    /// 不再要求所在函数返回 `result<...>`。
    /// catch 体按**外层**的 `try_depth` 检查 —— catch 里的 `throw` 不会被子句重新捕获
    /// （否则 `catch (error e) { throw e }` 会自己吃自己），只能交给外层或函数出口。
    fn check_try_catch(&mut self, s: &ast::SPtr) {
        let n = match &s.k {
            Stmt::TryCatch(n) => n,
            _ => return,
        };
        self.sl(s.id).ty = Some(t_void());

        self.push_scope();
        self.try_depth += 1;
        self.try_loop_base.push(self.loop_depth);
        for st in &n.body.stmts {
            self.check_stmt(st);
        }
        self.try_loop_base.pop();
        self.try_depth -= 1;
        self.pop_scope();

        let mut seen: Vec<String> = Vec::new();
        let mut fallback_at: Option<usize> = None;
        for (i, c) in n.catches.iter().enumerate() {
            if let Some(k) = fallback_at {
                self.err("E6012", &c.variant_tok, "文稿 10.2", "`catch (error e)` 已捕获全部错误，后面的 catch 永远不会执行", &[]);
                let _ = k;
            }
            match &c.variant {
                None => fallback_at = Some(i),
                Some(v) => {
                    if !matches!(v.as_str(), "NotFound" | "Invalid" | "Money" | "IO") {
                        self.err("E3027", &c.variant_tok, "文稿 10.1", "未知的错误变体 `error.{0}`（可用：NotFound/Invalid/Money/IO）", &[&v]);
                    } else if seen.iter().any(|x| x == v) {
                        self.err("E6012", &c.variant_tok, "文稿 10.2", "`catch (error.{0} ...)` 重复，前一个已经接住这类错误", &[&v]);
                    } else {
                        seen.push(v.clone());
                    }
                }
            }
            self.push_scope();
            self.declare(&c.name, t_error(), true, &c.name_tok);
            self.try_loop_base.push(self.loop_depth);
            for st in &c.body.stmts {
                self.check_stmt(st);
            }
            self.try_loop_base.pop();
            self.pop_scope();
        }

        if let Some(f) = &n.finally {
            self.try_loop_base.push(self.loop_depth);
            self.check_block(f, true);
            self.try_loop_base.pop();
        }
    }

    fn check_defer(&mut self, s: &ast::SPtr) {
        let call = match &s.k {
            Stmt::Defer { call } => call,
            _ => return,
        };
        if !matches!(&call.k, Expr::Call { .. } | Expr::MethodCall { .. } | Expr::SafeCall { .. }) {
            self.err("E2013", &call.tok, "文稿 5.4", "`defer` 后必须是一次调用", &[]);
        }
        self.check_expr(call, None);
        self.sl(s.id).ty = Some(t_void());
        // E7008：此检查自首次发布即不可达（见 sema/mod.rs 行为契约），保留不触发
    }

    // ---------------------------------------------------------------- coerce / take

    pub fn coerce(&mut self, node: &ast::EPtr, dst: &Ty, what: &str) {
        let src = match self.ty_of(node.id) {
            Some(t) => t,
            None => return,
        };
        if matches!(&**dst, TyS::Void) {
            return;
        }
        if matches!(&*src, TyS::Never) {
            return;
        }
        // try 块内的隐式展开：`int x = f()` 中 f 返回 result<int>，
        // 等价于自动补了 `try` —— 失败跳到本块的 catch。
        if self.try_depth > 0 {
            if let TyS::Result(inner) = &*src {
                let inner = inner.clone();
                if !matches!(&*inner, TyS::Void) {
                    let r2 = assignable(&inner, dst, self, true);
                    if !matches!(r2, AssignResult::No(_)) {
                        let s = self.sl(node.id);
                        s.ty = Some(dst.clone());
                        s.implicit_try = true;
                        if let AssignResult::Conv(kind) = r2 {
                            s.conv = Some(kind);
                            s.conv_to = Some(dst.clone());
                        }
                        return;
                    }
                }
            }
        }
        let r = assignable(&src, dst, self, true);
        match r {
            AssignResult::Yes => {
                if matches!(&*src, TyS::Nullable(_)) && matches!(&**dst, TyS::Nullable(_)) && src != *dst {
                    self.sl(node.id).conv = Some("nullwrap");
                }
            }
            AssignResult::Conv(kind) => {
                let s = self.sl(node.id);
                s.conv = Some(kind);
                s.conv_to = Some(dst.clone());
            }
            AssignResult::No(_) => {
                if matches!(&*src, TyS::Record(_)) && matches!(&**dst, TyS::Object) {
                    self.err("E4029", &node.tok, "T-28", "record 不能装箱到 object", &[]);
                    return;
                }
                if matches!(&*src, TyS::Record(_)) {
                    self.err("E4034", &node.tok, "T-27", "record 不能作为参数/返回值/赋给 {0}；请定义命名 struct", &[&self.ts(dst)]);
                    return;
                }
                if matches!(&*src, TyS::Nullable(_)) {
                    self.err("E4001", &node.tok, "T-19", "可空值 `{0}` 不能直接赋给 `{1}`，请先判空", &[&self.ts(&src), &self.ts(dst)]);
                    return;
                }
                if matches!(&*src, TyS::Object) && !matches!(&**dst, TyS::Object) {
                    self.err("E5002", &node.tok, "T-10", "`object` 不能直接当 {0} 用；先落到具体类型的变量，或写 `{1}(o)`", &[&self.ts(dst), &self.ts(dst)]);
                    return;
                }
                if is_numeric(&src) && is_numeric(dst) && src != *dst {
                    self.err("E4003", &node.tok, "T-8", "{0} 与 {1} 之间没有隐式转换，请写 `{2}(x)`", &[&self.ts(&src), &self.ts(dst), &self.ts(dst)]);
                    return;
                }
                self.err("E4001", &node.tok, "T-5", "类型不匹配：期望 {0}，得到 {1}（{2}）", &[&self.ts(dst), &self.ts(&src), &what]);
            }
        }
    }

    pub fn take(&mut self, node: &ast::EPtr) {
        match &node.k {
            Expr::Path { segs, .. } => {
                if let Some(v) = self.lookup_var(&segs[0]) {
                    v.borrow_mut().state = VarState::Moved;
                }
                self.sl(node.id).is_move = true;
            }
            Expr::FieldAccess { .. } | Expr::Index { .. } => {
                self.sl(node.id).is_move = true;
            }
            _ => {}
        }
        // O-12：move 视作独占使用，参与单语句借用冲突扫描
        if let Some(place) = self.place_key(node) {
            self.stmt_uses.push(StmtUse {
                place,
                kind: UseKind::Move,
                tok: node.tok.clone(),
            });
        }
    }

    pub fn moves(&self, t: &Ty) -> bool {
        matches!(&**t, TyS::Object)
            || matches!(&**t, TyS::Named(DefId::Name("Arena"), _))
            || self.is_ref(t)
    }

    pub fn is_ref(&self, t: &Ty) -> bool {
        match &**t {
            TyS::Array(..) | TyS::Map(..) => true,
            TyS::Named(DefId::Id(id), _) => self
                .typedefs
                .get(*id as usize)
                .map_or(false, |d| d.kind == "class"),
            _ => false,
        }
    }
}
