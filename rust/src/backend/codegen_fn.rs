//! FnGen：单个函数的语句级 C 生成——控制流、defer、trap，以及
//! try/catch/finally 的展开（`try_stack` 记录错误变量与跳转标签；
//! return 前先按内→外跑 finally 链；只写 finally 也生成 rethrow 段，不吞错误）。
//! 语义槽统一读 `Sema::slots`（键 = 节点 id）。

use crate::frontend::ast::{self, Expr, Stmt};
use crate::backend::{cstr, san, AX_FAIL};
use crate::frontend::lexer::Tok;
use crate::sema::{Res, VarInfo};
use crate::types::*;
use std::cell::RefCell;
use std::rc::Rc;

use super::CodeGen;

/// binop_c 需要的节点最小信息集（tok + 转换标记），避免向深处传整个 AST 节点。
pub struct BinInfo<'t> {
    pub tok: &'t Tok,
    pub dec_conv: Option<&'static str>,
    pub null_lhs: bool,
    pub null_rhs: bool,
}

/// 一层 `try` 块的生成上下文。
///
/// `target` 是块内 `throw` / 展开失败要跳去的标签：try 体内指向 catch 链入口，
/// catch 体内指向 finally 入口（catch 里再抛不该被自己重新捕获）。
pub struct TryCtx<'a> {
    pub err_var: String,
    pub target: String,
    /// 进入 try 块前的 `defers.len()`：跳转时只展开块内的 defer，块外的留给函数出口。
    pub defers_depth: usize,
    /// 本层的 finally 体：`return` 离开函数前要按内→外补跑一遍。
    pub finally: Option<&'a ast::Block>,
}

/// 单个函数的 C 生成器。`defers` / `loop_labels` / `try_stack` 三个栈分别
/// 对应 defer、带标签循环、try 块的嵌套展开——跳转与 return 前要按栈序
/// 补跑 finally / defer；`try_seq` 给标签与临时量编号去重。
pub struct FnGen<'s, 'a, 'c> {
    pub cg: &'c mut CodeGen<'s, 'a>,
    pub f: &'s crate::sema::FuncInfo<'a>,
    pub cname: String,
    pub owner: Option<u32>,
    pub lines: Vec<String>,
    pub ind: usize,
    pub defers: Vec<Vec<&'a ast::EPtr>>,
    pub loop_labels: Vec<(Option<String>, usize, String)>,
    pub try_stack: Vec<TryCtx<'a>>,
    pub try_seq: usize,
    /// 生成隐式展开时置位，避免同一节点反复进入隐式处理分支。
    pub suppress_implicit: bool,
    pub is_ctor: bool,
    pub is_main: bool,
}

impl<'s, 'a, 'c> FnGen<'s, 'a, 'c> {
    pub fn new(
        cg: &'c mut CodeGen<'s, 'a>,
        f: &'s crate::sema::FuncInfo<'a>,
        cname: String,
        owner: Option<u32>,
        is_ctor: bool,
    ) -> Self {
        let is_main = !is_ctor && f.name == "main";
        FnGen {
            cg,
            f,
            cname,
            owner,
            lines: Vec::new(),
            ind: 1,
            defers: Vec::new(),
            loop_labels: Vec::new(),
            try_stack: Vec::new(),
            try_seq: 0,
            suppress_implicit: false,
            is_ctor,
            is_main,
        }
    }

    // ---------- 语义槽快捷访问 ----------

    pub fn s(&self) -> &'s crate::sema::Sema<'a> {
        self.cg.s
    }

    pub fn ty_opt(&self, id: u32) -> Option<Ty> {
        self.slot(id).and_then(|s| s.ty.clone())
    }

    /// 节点没有类型结论时按 int 兜底（如已报错的表达式）。
    pub fn ty_or_int(&self, id: u32) -> Ty {
        self.ty_opt(id).unwrap_or_else(t_int)
    }

    pub fn res_of(&self, id: u32) -> Option<Res> {
        self.slot(id).and_then(|s| s.res.clone())
    }

    fn slot(&self, id: u32) -> Option<&crate::sema::Slot> {
        self.cg.slot(&id)
    }

    pub fn is_move(&self, id: u32) -> bool {
        self.slot(id).map_or(false, |s| s.is_move)
    }

    // ---------- 输出 ----------

    pub fn emit(&mut self, line: String) {
        self.lines.push(format!("{}{}", "  ".repeat(self.ind), line));
    }

    pub fn raw(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }

    pub fn tmpn(&mut self) -> String {
        self.cg.tmp_n += 1;
        format!("t{}", self.cg.tmp_n)
    }

    pub fn declare(&mut self, ty: &Ty, expr: &str) -> String {
        let n = self.tmpn();
        let ct = self.cg.ct(ty);
        self.emit(format!("{} {} = {};", ct, n, expr));
        n
    }

    pub fn pos_args(&self, tok: &Tok) -> String {
        format!("{}, {}, {}", cstr(&self.cg.path), tok.line, tok.col)
    }

    // ---------- 签名 / 函数体 ----------

    pub fn decl(&mut self) -> String {
        let ret = self.f.ret.clone();
        let ret = self.cg.ct(&ret);
        let mut ps: Vec<String> = Vec::new();
        let mut self_param: Option<String> = None;
        if self.owner.is_some() && !self.f.is_static {
            self_param = Some("void* self_".to_string());
        }
        for (name, ty, _borrow) in self.f.params.clone() {
            ps.push(format!("{} {}", self.cg.ct(&ty), self.pv(&name)));
        }
        if let Some(sp) = self_param {
            ps.insert(0, sp);
        }
        if ps.is_empty() {
            ps.push("void".to_string());
        }
        format!("static {} {}({});", ret, self.cname, ps.join(", "))
    }

    pub fn signature(&mut self) -> String {
        let ret = self.f.ret.clone();
        let ret = self.cg.ct(&ret);
        let mut ps: Vec<String> = Vec::new();
        if self.owner.is_some() && !self.f.is_static {
            ps.push("void* self_".to_string());
        }
        for (name, ty, _borrow) in self.f.params.clone() {
            ps.push(format!("{} {}", self.cg.ct(&ty), self.pv(&name)));
        }
        if ps.is_empty() {
            ps.push("void".to_string());
        }
        format!("static {} {}({}) {{", ret, self.cname, ps.join(", "))
    }

    pub fn pv(&self, name: &str) -> String {
        self.cg.pv(name)
    }

    pub fn lv(&self, name: &str) -> String {
        self.cg.lv(name)
    }

    pub fn body(&mut self, _want_ret: bool) -> String {
        self.lines = vec![self.signature()];
        self.ind = 1;
        if self.owner.is_some() && !self.f.is_static {
            // interface 默认方法没有 this（semantics：签名落到 funcs，body 不引用字段），
            // 只有 class 才做 self_ → 具体类型指针的收窄
            let od = &self.s().typedefs[self.owner.unwrap() as usize];
            if od.kind == "class" {
                let cn = san(&od.name);
                self.emit(format!(
                    "{0}* self = ({0}*)self_; (void)self;",
                    cn
                ));
            } else {
                self.emit("(void)self_;".to_string());
            }
        }
        let body = self.f.body;
        self.block(body, None, None);
        // 只有 main 允许末尾省略 `return`（文稿 9.4）——等价 `return 0`。
        // 少了这一句，C 的函数会"掉出末尾"，返回值是寄存器里的残留，退出码随机。
        if self.is_main && self.f.ret == t_int() {
            let ends_with_return = matches!(
                body.and_then(|b| b.stmts.last()).map(|sp| &sp.k),
                Some(Stmt::Return { .. })
            );
            if !ends_with_return {
                self.emit("return 0;".to_string());
            }
        }
        self.raw("}");
        self.lines.join("\n")
    }

    pub fn ctor_body(&mut self, d: &crate::sema::TypeDef, _nm: &str) -> String {
        self.lines = vec![self.signature()];
        self.ind = 1;
        let cn = san(&d.name);
        self.emit(format!("{0}* self = ({0}*)self_; (void)self;", cn));
        if let Some(b) = d.ctor.and_then(|c| self.cg.s.funcs[c].body) {
            self.block(Some(b), None, None);
        } else {
            self.block(None, None, None);
        }
        self.raw("}");
        self.lines.join("\n")
    }

    // ---------- 语句 ----------

    pub fn block(
        &mut self,
        b: Option<&'a ast::Block>,
        want_value: Option<&Ty>,
        out_var: Option<&str>,
    ) {
        self.emit("{".to_string());
        self.ind += 1;
        self.defers.push(Vec::new());
        let stmts: &[std::boxed::Box<ast::N<Stmt>>] = match b {
            Some(bb) => &bb.stmts,
            None => &[],
        };
        let n = stmts.len();
        for (i, sp) in stmts.iter().enumerate() {
            let last = i == n - 1;
            if last && want_value.is_some() && out_var.is_some() {
                let is_plain_simple = matches!(
                    &sp.k,
                    Stmt::Simple {
                        op: None,
                        incdec: None,
                        ..
                    }
                );
                if is_plain_simple {
                    let lhs = match &sp.k {
                        Stmt::Simple { lhs, .. } => lhs,
                        _ => unreachable!(),
                    };
                    let v = self.gen(lhs, want_value, false, false);
                    self.emit(format!("{} = {};", out_var.unwrap(), v));
                    continue;
                }
                if matches!(&sp.k, Stmt::If(_) | Stmt::Match(_)) {
                    self.gen_value_stmt(sp, want_value.unwrap(), out_var.unwrap());
                    continue;
                }
            }
            self.stmt(sp);
        }
        let frame: Vec<&'a ast::EPtr> = self.defers.pop().unwrap_or_default();
        for call in frame.iter().rev() {
            self.emit_defer_call(call);
        }
        self.ind -= 1;
        self.emit("}".to_string());
    }

    fn gen_value_stmt(&mut self, s: &'a ast::N<Stmt>, want: &Ty, out_var: &str) {
        match &s.k {
            Stmt::If(n) => self.if_stmt(n, s.id, Some(want), Some(out_var)),
            Stmt::Match(n) => self.match_stmt(n, s.id, Some(want), Some(out_var)),
            _ => unreachable!(),
        }
    }

    pub fn stmt(&mut self, s: &'a ast::N<Stmt>) {
        match &s.k {
            Stmt::Block(_) => {
                let b = match &s.k {
                    Stmt::Block(b) => b,
                    _ => unreachable!(),
                };
                self.block(Some(b), None, None);
            }
            Stmt::VarDecl { .. } => self.var_decl(s),
            Stmt::VarTuple { .. } => self.var_tuple(s),
            Stmt::Simple { .. } => self.simple(s),
            Stmt::If(_) => self.if_stmt_stmt(s),
            Stmt::Match(_) => self.match_stmt_stmt(s),
            Stmt::For { .. } => self.for_stmt(s),
            Stmt::Return { .. } => self.return_stmt(s),
            Stmt::Throw { .. } => self.throw_stmt(s),
            Stmt::TryCatch(_) => self.try_catch_stmt(s),
            Stmt::Defer { call } => {
                self.defers.last_mut().unwrap().push(call);
            }
            Stmt::Break { .. } => self.break_stmt(s, true),
            Stmt::Continue { .. } => self.break_stmt(s, false),
            Stmt::Const { .. } => self.var_decl(s),
        }
    }

    fn var_decl(&mut self, s: &'a ast::N<Stmt>) {
        // 局部 `const` 与 `var` 同一条生成路径：const 在 sema 已 declare 成
        // 只读变量，使用处按 `v_<name>` 引用，这里不生成声明就会 undeclared
        let (name, init): (&String, Option<&'a ast::EPtr>) = match &s.k {
            Stmt::VarDecl { name, init, .. } => (name, init.as_ref()),
            Stmt::Const { name, init, .. } => (name, Some(init)),
            _ => unreachable!(),
        };
        let varinfo = self.slot(s.id).and_then(|sl| sl.varinfo.clone());
        let ty: Ty = match varinfo {
            Some(v) => v.borrow().ty.clone(),
            None => match init {
                Some(i) => self.ty_or_int(i.id),
                None => t_int(),
            },
        };
        let expr = self.gen_to(init.as_ref().unwrap(), &ty);
        let lv = self.lv(name);
        let ct = self.cg.ct(&ty);
        self.emit(format!("{} {} = {};", ct, lv, expr));
        if let Some(i) = init {
            if self.is_move(i.id) && matches!(&i.k, Expr::Path { .. }) {
                self.emit_zero(i, &ty);
            }
        }
    }

    /// move 之后把源变量置空（drop 容空，消除 double free 一类问题）。
    fn emit_zero(&mut self, node: &'a ast::EPtr, ty: &Ty) {
        if let Expr::Path { segs, .. } = &node.k {
            // 参数被 move 时置空要写参数名 p_x，不是局部变量名 v_x
            let (_, v) = self.var_c(node);
            let is_param = v.map(|x| x.borrow().is_param).unwrap_or(false);
            let name = if is_param {
                self.pv(&segs[0])
            } else {
                self.lv(&segs[0])
            };
            match &**ty {
                TyS::Array(..) => {
                    self.emit(format!(
                        "{0}.len = 0; {0}.cap = 0; {0}.data = NULL;",
                        name
                    ));
                }
                TyS::Map(..) => {
                    self.emit(format!("{}.len = 0;", name));
                }
                TyS::Object => {
                    self.emit(format!("{} = NULL;", name));
                }
                TyS::Named(did, _) => {
                    let is_class = self
                        .cg
                        .def_of(did)
                        .map_or(false, |d| d.kind == "class");
                    if is_class {
                        self.emit(format!("{} = NULL;", name));
                    } else if self
                        .cg
                        .def_of(did)
                        .map_or(false, |d| d.kind == "interface")
                    {
                        // interface 是胖指针值，move 后整体清零
                        let ct = self.cg.ct(&ty);
                        self.emit(format!("{} = ({}){{0}};", name, ct));
                    }
                }
                _ => {}
            }
        }
    }

    fn var_tuple(&mut self, s: &'a ast::N<Stmt>) {
        let (names, init) = match &s.k {
            Stmt::VarTuple { names, init } => (names, init),
            _ => unreachable!(),
        };
        let t = self.gen(init, None, false, false);
        let n = self.tmpn();
        let ty = self.ty_or_int(init.id);
        let ct = self.cg.ct(&ty);
        self.emit(format!("{} {} = {};", ct, n, t));
        for (i, name) in names.iter().enumerate() {
            let et: Ty = match &*ty {
                TyS::Tuple(elems) => elems[i].clone(),
                _ => t_int(),
            };
            let etc = self.cg.ct(&et);
            let lv = self.lv(name);
            self.emit(format!("{} {} = {}.f{};", etc, lv, n, i));
        }
    }

    fn simple(&mut self, s: &'a ast::N<Stmt>) {
        let (lhs, op, rhs, incdec) = match &s.k {
            Stmt::Simple {
                lhs,
                op,
                rhs,
                incdec,
            } => (lhs, op, rhs, incdec),
            _ => unreachable!(),
        };
        if incdec.is_some() {
            self.incdec(s);
            return;
        }
        if op.is_none() {
            let v = self.gen(lhs, None, false, true);
            // 有些"表达式"其实是语句级副作用（parent(...) 的父类构造调用、
            // 返回结构体的方法调用）——它们的值必须落成一条语句，不能丢。
            if !v.is_empty() && v != "0" {
                // `(void)` 抑制"值未被使用"的 C 警告：语句位置的 `expr try` 会展开成
                // `tN.u.ok`，直接落成语句会触发 -Wunused-value。
                self.emit(format!("(void)({});", v));
            }
            return;
        }
        let lty = self.ty_or_int(lhs.id);
        let opc = op.as_ref().unwrap().as_bytes()[0] as char;
        if opc == '=' && op.as_ref().unwrap().len() == 1 {
            let r = rhs.as_ref().unwrap();
            let rv = self.gen(r, Some(&lty), false, false);
            self.assign(lhs, rv, &lty);
            if self.is_move(r.id) && matches!(&r.k, Expr::Path { .. }) {
                self.emit_zero(r, &lty);
            }
            return;
        }
        if opc == '+' && matches!(&*lty, TyS::Str) {
            let cur = self.gen(lhs, None, false, false);
            let r = rhs.as_ref().unwrap();
            let rv = self.gen(r, None, false, false);
            let concat = format!("ax_str_concat({}, {})", cur, rv);
            self.assign(lhs, concat, &lty);
            return;
        }
        let cur = self.gen(lhs, None, false, false);
        let r = rhs.as_ref().unwrap();
        let rv = self.gen(r, Some(&lty), false, false);
        let info = self.bin_info(s.id, &s.tok, Some(lhs), Some(r));
        let val = self.binop_c(&opc.to_string(), cur, rv, &lty, &info, None);
        self.assign(lhs, val, &lty);
    }

    /// 汇总 binop_c 需要的节点信息（dec_conv 槽 + NullLit 判断）。
    fn bin_info(
        &self,
        id: u32,
        tok: &'a Tok,
        lhs: Option<&'a ast::EPtr>,
        rhs: Option<&'a ast::EPtr>,
    ) -> BinInfo<'a> {
        BinInfo {
            tok,
            dec_conv: self.slot(id).and_then(|s| s.dec_conv),
            null_lhs: lhs.map_or(false, |e| matches!(&e.k, Expr::NullLit)),
            null_rhs: rhs.map_or(false, |e| matches!(&e.k, Expr::NullLit)),
        }
    }

    fn incdec(&mut self, s: &'a ast::N<Stmt>) {
        let lhs = match &s.k {
            Stmt::Simple { lhs, incdec, .. } => {
                let _ = incdec;
                lhs
            }
            _ => unreachable!(),
        };
        let dir = match &s.k {
            Stmt::Simple { incdec, .. } => incdec.clone().unwrap(),
            _ => unreachable!(),
        };
        let lty = self.ty_or_int(lhs.id);
        let cur = self.gen(lhs, None, false, false);
        let info = self.bin_info(s.id, &s.tok, Some(lhs), None);
        let val = self.binop_c(
            if dir == "++" { "+" } else { "-" },
            cur,
            "1".to_string(),
            &lty,
            &info,
            None,
        );
        self.assign(lhs, val, &lty);
    }

    fn assign(&mut self, lhs: &'a ast::EPtr, rhs: String, lty: &Ty) {
        match &lhs.k {
            Expr::Path { segs, .. } => {
                let res = self.res_of(lhs.id);
                let v: Option<Rc<RefCell<VarInfo>>> = match res {
                    Some(Res::Local(v)) => Some(v),
                    _ => None,
                };
                let (is_param, v_nullable) = match &v {
                    Some(v) => {
                        let b = v.borrow();
                        (b.is_param, matches!(&*b.ty, TyS::Nullable(_)))
                    }
                    None => (false, false),
                };
                let name = if v.is_some() && is_param {
                    self.pv(&segs[0])
                } else {
                    self.lv(&segs[0])
                };
                let lty_nonopt = !matches!(&**lty, TyS::Nullable(_));
                if v.is_some() && v_nullable && lty_nonopt {
                    self.emit(format!("{}.v = {}; {}.has = 1;", name, rhs, name));
                } else {
                    self.emit(format!("{} = {};", name, rhs));
                }
            }
            Expr::FieldAccess { recv, name } => {
                let (recv_c, deref) = self.recv_c(recv);
                let sep = if deref { "->" } else { "." };
                self.emit(format!("{}{}f_{} = {};", recv_c, sep, san(name), rhs));
            }
            Expr::Index {
                recv,
                index,
            } => {
                let rt = self.ty_or_int(recv.id);
                match &*rt {
                    TyS::Record(_) => {
                        let (recv_c, deref) = self.recv_c(recv);
                        let key = self
                            .slot(lhs.id)
                            .and_then(|s| s.key.clone())
                            .unwrap_or_default();
                        let sep = if deref { "->" } else { "." };
                        self.emit(format!("{}{}f_{} = {};", recv_c, sep, san(&key), rhs));
                    }
                    TyS::Map(kt, vt) => {
                        let kt = kt.clone();
                        let vt = vt.clone();
                        let m = self.hoist(recv, &rt);
                        let k = self.gen(index, None, false, false);
                        let kv = self.declare(&kt, &k);
                        let vt_v = self.declare(&vt, &rhs);
                        self.emit(format!("ax_map_put(&{}, &{}, &{});", m, kv, vt_v));
                    }
                    TyS::Array(_, len) => {
                        if len.is_some() {
                            let (a, deref) = self.recv_c(recv);
                            let idx = self.gen(index, None, false, false);
                            let sep = if deref { "->" } else { "." };
                            // sep 已含 `.` 或 `->`，后面不能再补点
                            self.emit(format!("{}{}v[{}] = {};", a, sep, idx, rhs));
                            return;
                        }
                        let m = self.hoist(recv, &rt);
                        let idx = self.gen(index, None, false, false);
                        let iv = self.declare(&t_int(), &idx);
                        let et = match &*rt {
                            TyS::Array(e, _) => e.clone(),
                            _ => t_int(),
                        };
                        let etc = self.cg.ct(&et);
                        let pa = self.pos_args(&lhs.tok);
                        self.emit(format!(
                            "*({0}*)ax_arr_at(&{1}, {2}, (int64_t)sizeof({0}), {3}) = {4};",
                            etc, m, iv, pa, rhs
                        ));
                    }
                    _ => {
                        self.emit("/* unsupported assign */".to_string());
                    }
                }
            }
            _ => {
                self.emit("/* unsupported assign */".to_string());
            }
        }
    }

    /// 把可能带副作用的接收者落成临时变量，返回变量名（值语义）。
    pub fn hoist(&mut self, node: &'a ast::EPtr, ty: &Ty) -> String {
        if matches!(&node.k, Expr::Path { .. }) {
            return self.path(node);
        }
        let expr = self.gen(node, None, true, false);
        self.declare(ty, &expr)
    }

    /// 把 Path 节点落到 C 变量名，返回 (名字, VarInfo 或 None)。
    pub fn var_c(&mut self, node: &'a ast::EPtr) -> (String, Option<Rc<RefCell<VarInfo>>>) {
        let res = self.res_of(node.id);
        let v = match res {
            Some(Res::Local(v)) => Some(v),
            _ => None,
        };
        let name = match &node.k {
            Expr::Path { segs, .. } => segs[0].clone(),
            _ => String::new(),
        };
        let is_param = v
            .as_ref()
            .map(|v| v.borrow().is_param)
            .unwrap_or(false);
        let mut base = if v.is_some() && is_param {
            self.pv(&name)
        } else {
            self.lv(&name)
        };
        let v_nullable = v
            .as_ref()
            .map(|v| matches!(&*v.borrow().ty, TyS::Nullable(_)))
            .unwrap_or(false);
        let node_nullable = self
            .ty_opt(node.id)
            .map(|t| matches!(&*t, TyS::Nullable(_)))
            .unwrap_or(false);
        if v.is_some() && v_nullable && !node_nullable {
            base += ".v";
        }
        (base, v)
    }

    /// 返回 (C 表达式, 是否指针)。
    pub fn recv_c(&mut self, node: &'a ast::EPtr) -> (String, bool) {
        match &node.k {
            Expr::Path { .. } => {
                let (name, _v) = self.var_c(node);
                let ty = self.ty_or_int(node.id);
                match &*ty {
                    TyS::Nullable(_) => (name, false),
                    TyS::Named(did, _) => {
                        let is_class = self.cg.def_of(did).map_or(false, |d| d.kind == "class");
                        if is_class {
                            (name, true)
                        } else {
                            (name, false)
                        }
                    }
                    TyS::Object => (name, true),
                    _ => (name, false),
                }
            }
            Expr::This => ("self".to_string(), true),
            _ => {
                let ty = self.ty_or_int(node.id);
                // 复杂接收者（下标、字段链、调用结果……）落临时变量再取成员：
                // 裸表达式（如 `*(T**)ax_arr_at(...)`）直接接 `->field` 会被 C
                // 解析成 `*(...->field)`——一元 `*` 低于后缀 `->`。
                let expr = self.hoist(node, &ty);
                if let TyS::Named(did, _) = &*ty {
                    let is_class = self.cg.def_of(did).map_or(false, |d| d.kind == "class");
                    if is_class {
                        return (expr, true);
                    }
                }
                (expr, false)
            }
        }
    }

    pub fn if_stmt(&mut self, n: &'a ast::IfNode, _id: u32, want: Option<&Ty>, out_var: Option<&str>) {
        let cond = self.cond(&n.cond);
        self.emit(format!("if ({}) {{", cond));
        self.ind += 1;
        let then_ = &n.then_;
        self.block(Some(then_), want, out_var);
        self.ind -= 1;
        match &n.els {
            Some(ast::IfTail::Block(b)) => {
                self.emit("} else {".to_string());
                self.ind += 1;
                self.block(Some(b), want, out_var);
                self.ind -= 1;
                self.emit("}".to_string());
            }
            Some(ast::IfTail::If(inner)) => {
                self.emit("} else {".to_string());
                self.ind += 1;
                self.if_stmt(&inner.k, inner.id, want, out_var);
                self.ind -= 1;
                self.emit("}".to_string());
            }
            None => {
                self.emit("}".to_string());
            }
        }
    }

    fn if_stmt_stmt(&mut self, s: &'a ast::N<Stmt>) {
        let n = match &s.k {
            Stmt::If(n) => n,
            _ => unreachable!(),
        };
        self.if_stmt(n, s.id, None, None);
    }

    fn cond(&mut self, e: &'a ast::EPtr) -> String {
        let t = self.ty_or_int(e.id);
        if matches!(&*t, TyS::Nullable(_)) {
            let v = self.gen(e, None, false, false);
            return format!("{}.has", v);
        }
        self.gen(e, None, false, false)
    }

    fn return_stmt(&mut self, s: &'a ast::N<Stmt>) {
        let value = match &s.k {
            Stmt::Return { value } => value,
            _ => unreachable!(),
        };
        let ret = self.f.ret.clone();
        // 在 try 块里 return：先把值算出来落到临时变量（finally 看不到"求值被推迟"），
        // 再按内→外补跑各层 finally，最后才是 defer 与 return —— 与 PHP 的
        // `try { return v } finally { ... }` 一致。
        let saved = if !self.try_stack.is_empty() {
            match value {
                Some(v) => {
                    let vt = if matches!(&*ret, TyS::Void) {
                        t_int()
                    } else {
                        ret.clone()
                    };
                    let vc = if matches!(&*ret, TyS::Void) {
                        self.gen(v, None, false, false)
                    } else {
                        self.gen(v, Some(&ret), false, false)
                    };
                    Some(self.declare(&vt, &vc))
                }
                None => None,
            }
        } else {
            None
        };
        self.emit_finally_chain();
        self.emit_defers(None);
        if let Some(tmp) = saved {
            if self.is_main && ret != t_int() {
                self.emit(format!("return (int64_t)({});", tmp));
            } else if matches!(&*ret, TyS::Void) {
                self.emit(format!("(void)({});", tmp));
                self.emit("return;".to_string());
            } else {
                self.emit(format!("return {};", tmp));
            }
            return;
        }
        if value.is_none() {
            if self.is_main {
                self.emit("return 0;".to_string());
            } else {
                self.emit("return;".to_string());
            }
            return;
        }
        let v = if !matches!(&*ret, TyS::Void) {
            self.gen(value.as_ref().unwrap(), Some(&ret), false, false)
        } else {
            self.gen(value.as_ref().unwrap(), None, false, false)
        };
        if self.is_main && ret != t_int() {
            self.emit(format!("return (int64_t)({});", v));
            return;
        }
        if matches!(&*ret, TyS::Void) {
            self.emit(format!("(void)({});", v));
            self.emit("return;".to_string());
            return;
        }
        self.emit(format!("return {};", v));
    }

    /// 按内→外生成各层 finally 的代码（`return` 离开函数时补跑）。
    /// 生成期间临时清空 try 栈 —— finally 里的 `throw` 属于外层，不该被本层 catch 接住。
    fn emit_finally_chain(&mut self) {
        if self.try_stack.is_empty() {
            return;
        }
        let fins: Vec<Option<&'a ast::Block>> =
            self.try_stack.iter().rev().map(|c| c.finally).collect();
        let saved = std::mem::take(&mut self.try_stack);
        for f in fins.into_iter().flatten() {
            self.block(Some(f), None, None);
        }
        self.try_stack = saved;
    }

    fn throw_stmt(&mut self, s: &'a ast::N<Stmt>) {
        let (variant, msg) = match &s.k {
            Stmt::Throw { variant, msg } => (variant, msg),
            _ => unreachable!(),
        };
        let code: u32 = err_kind_code(variant);
        let msg_c = self.gen(msg, Some(&t_string()), false, false);
        self.emit("{".to_string());
        self.ind += 1;
        self.emit(format!("ax_err e_ = ax_err_new({}, {});", code, msg_c));
        if let Some((err_var, target, depth)) = self
            .try_stack
            .last()
            .map(|c| (c.err_var.clone(), c.target.clone(), c.defers_depth))
        {
            // 有就地捕获目标：把错误交给本块的 catch / finally，不离开函数
            self.emit_defers_from(depth);
            self.emit(format!("{} = e_;", err_var));
            self.emit(format!("goto {};", target));
        } else if self.is_main {
            self.emit_defers(None);
            self.emit("ax_write_stderr(e_.msg); ax_write_stderr(AX_S(\"\\n\"));".to_string());
            self.emit("return 1;".to_string());
        } else {
            self.emit_defers(None);
            let rt = self.f.ret.clone();
            let rtc = self.cg.ct(&rt);
            self.emit(format!("{} r_ = {{0}};", rtc));
            self.emit(format!("r_.tag = {}; r_.u.err = e_;", AX_FAIL));
            self.emit("return r_;".to_string());
        }
        self.ind -= 1;
        self.emit("}".to_string());
    }

    /// `try { } catch (error e) { } catch (error.NotFound e) { } finally { }`
    ///
    /// 生成形态（`ax_err` 的 `code == 0` 表示"无错"）：
    ///
    /// ```c
    /// { ax_err t_err_N = {0};
    ///   <try 体，throw/展开失败 → t_err_N = e; goto L_C_N;>
    /// L_C_N:;
    ///   if (t_err_N.code == 1) { t_err_N.code = 0; <catch1> goto L_F_N; }
    ///   else if (t_err_N.code != 0) { t_err_N.code = 0; <catch2> goto L_F_N; }
    /// L_F_N:;
    ///   <finally>
    ///   if (t_err_N.code != 0) { <向外传播> }
    /// }
    /// ```
    fn try_catch_stmt(&mut self, s: &'a ast::N<Stmt>) {
        let n = match &s.k {
            Stmt::TryCatch(n) => n,
            _ => unreachable!(),
        };
        let id = self.try_seq;
        self.try_seq += 1;
        let err_var = format!("t_err_{}", id);
        let catch_label = format!("L_catch_{}", id);
        let fin_label = format!("L_fin_{}", id);
        let depth = self.defers.len();

        self.emit("{".to_string());
        self.ind += 1;
        self.emit(format!("ax_err {} = {{0}};", err_var));

        self.try_stack.push(TryCtx {
            err_var: err_var.clone(),
            target: catch_label.clone(),
            defers_depth: depth,
            finally: n.finally.as_ref(),
        });
        self.block(Some(&n.body), None, None);
        self.try_stack.pop();

        // label 只为 goto 存在；try 体里没有跳转时它会"定义未使用"
        self.emit(format!("if (0) {{ goto {}; }}", catch_label));
        self.emit(format!("{}:;", catch_label));
        for (i, c) in n.catches.iter().enumerate() {
            let cond = match &c.variant {
                Some(v) => format!("{}.code == {}", err_var, err_kind_code(v)),
                None => format!("{}.code != 0", err_var),
            };
            let kw = if i == 0 { "if" } else { "else if" };
            self.emit(format!("{} ({}) {{", kw, cond));
            self.ind += 1;
            self.emit(format!("{}.code = 0;", err_var));
            let vt = self.cg.ct(&t_error());
            let vn = self.cg.lv(&c.name);
            self.emit(format!("{} {} = {};", vt, vn, err_var));
            self.emit(format!("(void){};", vn));
            self.try_stack.push(TryCtx {
                err_var: err_var.clone(),
                target: fin_label.clone(),
                defers_depth: depth,
                finally: n.finally.as_ref(),
            });
            self.block(Some(&c.body), None, None);
            self.try_stack.pop();
            self.emit(format!("goto {};", fin_label));
            self.ind -= 1;
            self.emit("}".to_string());
        }

        self.emit(format!("if (0) {{ goto {}; }}", fin_label));
        self.emit(format!("{}:;", fin_label));
        // finally 里的 throw 走外层上下文（本层已经收尾）
        if let Some(f) = &n.finally {
            self.block(Some(f), None, None);
        }

        // 没有 catch 接住（或 catch 里又抛）→ 交给外层 try 或调用方。
        // 只写 finally 的块也必须生成这段：错误在这里"过一手"但不能被吞掉。
        self.emit(format!("if ({}.code != 0) {{", err_var));
        self.ind += 1;
        self.emit_rethrow(&err_var, depth);
        self.ind -= 1;
        self.emit("}".to_string());
        // 只写 finally 不写 catch 时，错误槽没有读者，显式消掉 -Wunused-variable
        self.emit(format!("(void){};", err_var));
        self.ind -= 1;
        self.emit("}".to_string());
    }

    /// 把 `err_var` 里的错误交给上一层：外层 `try` 存在就转给它，
    /// 否则按原路径离开函数（main 打印并退出 1，其余返回 `fail`）。
    fn emit_rethrow(&mut self, err_var: &str, depth: usize) {
        let ev = err_var.to_string();
        if let Some((outer_err, outer_target)) = self
            .try_stack
            .last()
            .map(|c| (c.err_var.clone(), c.target.clone()))
        {
            // 仍在函数内，外层 defer 不展开（它自己的出口会处理）
            self.emit(format!("{} = {};", outer_err, ev));
            self.emit(format!("goto {};", outer_target));
            return;
        }
        self.emit_defers(Some(depth.min(self.defers.len())));
        if self.is_main {
            self.emit(format!("ax_write_stderr({}.msg);", ev));
            self.emit("ax_write_stderr(AX_S(\"\\n\"));".to_string());
            self.emit("return 1;".to_string());
        } else {
            let rt = self.f.ret.clone();
            let rtc = self.cg.ct(&rt);
            self.emit(format!("{} r_ = {{0}};", rtc));
            self.emit(format!("r_.tag = {}; r_.u.err = {};", AX_FAIL, ev));
            self.emit("return r_;".to_string());
        }
    }

    pub fn emit_defers(&mut self, upto: Option<usize>) {
        let n = self.defers.len();
        let end = upto.unwrap_or(n);
        let frames: Vec<Vec<&'a ast::EPtr>> = self.defers[..end].to_vec();
        for frame in frames.iter().rev() {
            for call in frame.iter().rev() {
                let c = *call;
                self.emit_defer_call(c);
            }
        }
    }

    /// 只展开 `from` 之后（块内）的 defer，块的正常出口会自己展开自己那层。
    pub fn emit_defers_from(&mut self, from: usize) {
        let n = self.defers.len();
        let start = from.min(n);
        let frames: Vec<Vec<&'a ast::EPtr>> = self.defers[start..].to_vec();
        for frame in frames.iter().rev() {
            for call in frame.iter().rev() {
                let c = *call;
                self.emit_defer_call(c);
            }
        }
    }

    fn emit_defer_call(&mut self, call: &'a ast::EPtr) {
        self.gen(call, None, false, true);
    }

    fn break_stmt(&mut self, s: &'a ast::N<Stmt>, is_break: bool) {
        let label = match &s.k {
            Stmt::Break { label } | Stmt::Continue { label } => label.clone(),
            _ => unreachable!(),
        };
        // 找到目标循环帧
        let mut target: Option<usize> = None;
        for i in (0..self.loop_labels.len()).rev() {
            if label.is_none() || self.loop_labels[i].0 == label {
                target = Some(i);
                break;
            }
        }
        let target = match target {
            Some(t) => t,
            None => {
                self.emit("/* break outside loop */".to_string());
                return;
            }
        };
        let frame_depth = self.loop_labels[target].1;
        let lab = self.loop_labels[target].2.clone();
        self.emit_defers(Some(frame_depth + 1));
        self.emit(format!(
            "goto {};",
            if is_break {
                format!("{}_brk", lab)
            } else {
                format!("{}_cont", lab)
            }
        ));
    }

    fn for_stmt(&mut self, s: &'a ast::N<Stmt>) {
        let (label, kind, body) = match &s.k {
            Stmt::For { label, kind, body } => (label.clone(), kind, body),
            _ => unreachable!(),
        };
        match kind {
            ast::ForKind::Cond => {
                self.emit("for (;;) {".to_string());
                let lab = self.tmpn();
                self.loop_labels
                    .push((label.clone(), self.defers.len(), lab.clone()));
                self.ind += 1;
                self.block(Some(body), None, None);
                // continue 跳到这里：等价于 C 的 continue（自增/条件判断在 for 头）
                self.emit(format!("{}_cont: __attribute__((unused));", lab));
                self.ind -= 1;
                self.emit("}".to_string());
                self.loop_labels.pop();
                self.emit(format!("{}_brk: __attribute__((unused));", lab));
            }
            ast::ForKind::Range {
                var,
                lo,
                hi,
                inclusive,
            } => {
                let lo_c = self.gen(lo, Some(&t_int()), false, false);
                let hi_c = self.gen(hi, Some(&t_int()), false, false);
                self.emit("{".to_string());
                self.ind += 1;
                let liv = self.declare(&t_int(), &lo_c);
                let hiv = self.declare(&t_int(), &hi_c);
                let v = self.lv(var);
                self.emit(format!("int64_t {} = {};", v, liv));
                let lab = self.tmpn();
                let op = if *inclusive { "<=" } else { "<" };
                self.emit(format!("for (; {} {} {}; {}++) {{", v, op, hiv, v));
                self.loop_labels
                    .push((label.clone(), self.defers.len(), lab.clone()));
                self.ind += 1;
                self.block(Some(body), None, None);
                self.emit(format!("{}_cont: __attribute__((unused));", lab));
                self.ind -= 1;
                self.emit("}".to_string());
                self.loop_labels.pop();
                self.emit(format!("{}_brk: __attribute__((unused));", lab));
                self.ind -= 1;
                self.emit("}".to_string());
            }
            ast::ForKind::Iter { var, iterable } => {
                let it = iterable;
                let ty = self.ty_or_int(it.id);
                self.emit("{".to_string());
                self.ind += 1;
                if let TyS::Str = &*ty {
                    let itc = self.gen(it, None, false, false);
                    let arr_ty = t_array(t_string(), None);
                    let arr = self.str_chars(&itc);
                    let arr = self.declare(&arr_ty, &arr);
                    let v = self.lv(var);
                    self.emit(format!("int64_t {}_i = 0;", v));
                    self.emit(format!("for (; {0}_i < {1}.len; {0}_i++) {{", v, arr));
                    self.ind += 1;
                    self.emit(format!(
                        "ax_str {} = ((ax_str*){}.data)[{}_i];",
                        v, arr, v
                    ));
                    let lab = self.tmpn();
                    self.loop_labels
                        .push((label.clone(), self.defers.len(), lab.clone()));
                    self.block(Some(body), None, None);
                    self.emit(format!("{}_cont: __attribute__((unused));", lab));
                    self.loop_labels.pop();
                    self.ind -= 1;
                    self.emit("}".to_string());
                    self.emit(format!("{}_brk: __attribute__((unused));", lab));
                    self.ind -= 1;
                    self.emit("}".to_string());
                    return;
                }
                if let TyS::Map(..) = &*ty {
                    let m = self.hoist(it, &ty);
                    let (kt, _vt) = match &*ty {
                        TyS::Map(k, v) => (k.clone(), v.clone()),
                        _ => unreachable!(),
                    };
                    let v = self.lv(var);
                    self.emit(format!("int64_t {}_i = 0;", v));
                    self.emit(format!("for (; {0}_i < {1}.len; {0}_i++) {{", v, m));
                    self.ind += 1;
                    let ktc = self.cg.ct(&kt);
                    self.emit(format!(
                        "{0} {1} = *({0}*){2}.entries[{1}_i].k;",
                        ktc, v, m
                    ));
                    let lab = self.tmpn();
                    self.loop_labels
                        .push((label.clone(), self.defers.len(), lab.clone()));
                    self.block(Some(body), None, None);
                    self.emit(format!("{}_cont: __attribute__((unused));", lab));
                    self.loop_labels.pop();
                    self.ind -= 1;
                    self.emit("}".to_string());
                    self.emit(format!("{}_brk: __attribute__((unused));", lab));
                    self.ind -= 1;
                    self.emit("}".to_string());
                    return;
                }
                // 数组
                let a = self.hoist(it, &ty);
                let (et, fixed) = match &*ty {
                    TyS::Array(e, l) => (e.clone(), *l),
                    _ => (t_int(), None),
                };
                let v = self.lv(var);
                self.emit(format!("int64_t {}_i = 0;", v));
                // 定长数组的长度在类型里，没有 .len 字段；变长数组才取运行时长度
                let hi = match fixed {
                    Some(n) => format!("{}", n),
                    None => format!("{}.len", a),
                };
                self.emit(format!("for (; {0}_i < {1}; {0}_i++) {{", v, hi));
                self.ind += 1;
                let etc = self.cg.ct(&et);
                if fixed.is_some() {
                    self.emit(format!("{} {} = {}.v[{}_i];", etc, v, a, v));
                } else {
                    self.emit(format!(
                        "{0} {1} = (({0}*){2}.data)[{1}_i];",
                        etc, v, a
                    ));
                }
                let lab = self.tmpn();
                self.loop_labels
                    .push((label.clone(), self.defers.len(), lab.clone()));
                self.block(Some(body), None, None);
                self.emit(format!("{}_cont: __attribute__((unused));", lab));
                self.loop_labels.pop();
                self.ind -= 1;
                self.emit("}".to_string());
                self.emit(format!("{}_brk: __attribute__((unused));", lab));
                self.ind -= 1;
                self.emit("}".to_string());
            }
            ast::ForKind::Kv {
                var,
                var2,
                iterable,
            } => {
                let it = iterable;
                let ty = self.ty_or_int(it.id);
                self.emit("{".to_string());
                self.ind += 1;
                let (kt, vt) = match &*ty {
                    TyS::Map(k, v) => (k.clone(), v.clone()),
                    _ => (t_int(), t_int()),
                };
                let m = self.hoist(it, &ty);
                let v = self.lv(var);
                let v2 = self.lv(var2);
                self.emit(format!("int64_t {}_i = 0;", v));
                self.emit(format!("for (; {0}_i < {1}.len; {0}_i++) {{", v, m));
                self.ind += 1;
                let ktc = self.cg.ct(&kt);
                let vtc = self.cg.ct(&vt);
                self.emit(format!("{0} {1} = *({0}*){2}.entries[{1}_i].k;", ktc, v, m));
                self.emit(format!("{0} {1} = *({0}*){2}.entries[{3}_i].v;", vtc, v2, m, v));
                self.emit(format!("(void){};", v2));
                let lab = self.tmpn();
                self.loop_labels
                    .push((label.clone(), self.defers.len(), lab.clone()));
                self.block(Some(body), None, None);
                self.emit(format!("{}_cont: __attribute__((unused));", lab));
                self.loop_labels.pop();
                self.ind -= 1;
                self.emit("}".to_string());
                self.emit(format!("{}_brk: __attribute__((unused));", lab));
                self.ind -= 1;
                self.emit("}".to_string());
            }
        }
    }
}

/// `error` 变体在运行时 `ax_err.kind` 里的编号（与 `ax_err_new` 的调用点一致）。
pub fn err_kind_code(variant: &str) -> u32 {
    match variant {
        "NotFound" => 1,
        "Invalid" => 2,
        "Money" => 3,
        "IO" => 4,
        _ => 2,
    }
}
