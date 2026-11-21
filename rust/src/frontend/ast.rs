//! AST 节点（规格书 §5）。
//!
//! 节点用枚举承载，每个节点统一带 `id`（sema 阶段挂语义槽的键，全局唯一）
//! 与 `tok`（报诊断用）。`dump` 序列化格式自 v0.9 固定，改动需同步
//! `tests/` 中相关期望输出。

use crate::frontend::lexer::Tok;

pub type SPtr = Box<N<Stmt>>;
pub type EPtr = Box<N<Expr>>;
pub type TPtr = Box<N<Ty>>;

/// 统一节点包装：id 供 sema 侧表使用，tok 供诊断定位。
#[derive(Clone, Debug)]
pub struct N<T> {
    pub id: u32,
    pub tok: Tok,
    pub k: T,
}

#[derive(Clone, Debug, Default)]
pub struct Module {
    pub items: Vec<N<Item>>,
    pub parse_errors: u32,
}

// ---------------------------------------------------------------- 声明

#[derive(Clone, Debug)]
pub enum Item {
    Import {
        path: Vec<String>,
        alias: Option<String>,
        names: Option<Vec<String>>,
    },
    ConstDecl {
        vis: Option<String>,
        ty: Option<TPtr>,
        name: String,
        init: EPtr,
    },
    FuncDecl {
        vis: Option<String>,
        ret: TPtr,
        name: String,
        generics: Vec<TypeParam>,
        params: Vec<N<Param>>,
        body: Block,
        attrs: Vec<Attr>,
        /// 由入口文件的顶层语句自动包出来的隐式 `main`（文稿 9.4）
        implicit: bool,
        /// D 语言风格属性：pure 函数无副作用、不调用非纯函数
        is_pure: bool,
        /// D 语言风格属性：nothrow 函数不抛出错误，不调用可能 throw 的函数
        is_nothrow: bool,
        /// D 语言风格：ref 返回值，按引用返回
        is_ref_return: bool,
    },
    ClassDecl {
        vis: Option<String>,
        is_abstract: bool,
        is_final: bool,
        name: String,
        generics: Vec<TypeParam>,
        parent: Option<TPtr>,
        impls: Vec<TPtr>,
        members: Vec<N<Member>>,
    },
    StructDecl {
        vis: Option<String>,
        name: String,
        generics: Vec<TypeParam>,
        members: Vec<N<Member>>,
    },
    InterfaceDecl {
        vis: Option<String>,
        name: String,
        extends: Vec<TPtr>,
        methods: Vec<N<MethodSig>>,
    },
    EnumDecl {
        vis: Option<String>,
        name: String,
        variants: Vec<String>,
    },
    TypeAlias {
        vis: Option<String>,
        name: String,
        generics: Vec<TypeParam>,
        target: TPtr,
    },
    /// `extern "C" { ... }`：模块级外部 C 函数声明块（FFI）。
    /// 每个元素为 `Item::FuncDecl`（body 为空，implicit=false）。
    ExternBlock {
        funcs: Vec<N<Item>>,
    },
}

#[derive(Clone, Debug)]
pub enum Member {
    Field {
        vis: Option<String>,
        ty: TPtr,
        name: String,
    },
    Ctor {
        name: String,
        params: Vec<N<Param>>,
        body: Block,
    },
    Method {
        vis: Option<String>,
        is_static: bool,
        is_override: bool,
        is_abstract: bool,
        ret: TPtr,
        name: String,
        params: Vec<N<Param>>,
        body: Block,
    },
    /// 类/struct/interface/enum/type 别名 嵌套在类里
    Item(Box<N<Item>>),
}

#[derive(Clone, Debug)]
pub struct MethodSig {
    pub ret: TPtr,
    pub name: String,
    pub params: Vec<N<Param>>,
    pub default_body: Option<Block>,
}

#[derive(Clone, Debug)]
pub struct TypeParam {
    pub tok: Tok,
    pub name: String,
    pub constraints: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Param {
    pub borrow: Option<String>, // None | "mut" | "const"
    pub ty: TPtr,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct Arg {
    pub expr: EPtr,
    pub borrow: bool,
}

/// `@os(...)` / `@arch(...)`
#[derive(Clone, Debug)]
pub struct Attr {
    pub tok: Tok,
    pub name: String,
    pub args: Vec<String>,
}

// ---------------------------------------------------------------- 语句

#[derive(Clone, Debug)]
pub struct Block {
    pub tok: Tok,
    pub stmts: Vec<SPtr>,
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Block(Block),
    VarDecl {
        explicit_ty: Option<TPtr>,
        is_var: bool,
        name: String,
        init: Option<EPtr>,
    },
    VarTuple {
        names: Vec<String>,
        init: EPtr,
    },
    /// 语句级 const（与模块级 ConstDecl 同构；sema 走 check_local_const）
    Const {
        vis: Option<String>,
        ty: Option<TPtr>,
        name: String,
        init: EPtr,
    },
    /// 表达式语句 / 赋值 / 后缀 ++ --
    Simple {
        lhs: EPtr,
        op: Option<String>,
        rhs: Option<EPtr>,
        incdec: Option<String>,
    },
    If(Box<IfNode>),
    Match(Box<MatchNode>),
    For {
        label: Option<String>,
        kind: ForKind,
        body: Block,
    },
    While {
        cond: EPtr,
        body: Block,
    },
    Return {
        value: Option<EPtr>,
    },
    Throw {
        variant: String,
        msg: EPtr,
    },
    /// `try { } catch (error e) { } catch (error.Variant e) { } finally { }`
    ///
    /// 块内 `throw` 与失败展开都跳到本块的 catch 链，而不是返回调用方。
    TryCatch(Box<TryCatchNode>),
    Defer {
        call: EPtr,
    },
    Break {
        label: Option<String>,
    },
    Continue {
        label: Option<String>,
    },
    /// `unsafe { ... }`：不安全块，包裹调用 extern 函数等操作。
    Unsafe {
        body: Block,
    },
}

/// IfNode 语句与表达式两用（if 作表达式是语言特性）。
#[derive(Clone, Debug)]
pub struct IfNode {
    pub cond: EPtr,
    pub then_: Block,
    pub els: Option<IfTail>,
}

#[derive(Clone, Debug)]
pub enum IfTail {
    Block(Block),
    If(Box<N<IfNode>>),
}

#[derive(Clone, Debug)]
pub struct MatchNode {
    pub scrutinee: EPtr,
    pub arms: Vec<Arm>,
}

/// `try` 块的语法节点。`catches` 为空表示只写了 `finally`。
#[derive(Clone, Debug)]
pub struct TryCatchNode {
    pub body: Block,
    pub catches: Vec<CatchArm>,
    pub finally: Option<Block>,
}

#[derive(Clone, Debug)]
pub struct CatchArm {
    /// `None` 表示 `catch (error e)`，捕获任意变体；
    /// `Some("NotFound")` 表示 `catch (error.NotFound e)`。
    pub variant: Option<String>,
    /// 变体名（或 `error` 本身）的位置，报诊断时指这里。
    pub variant_tok: Tok,
    pub name: String,
    pub name_tok: Tok,
    pub body: Block,
}

#[derive(Clone, Debug)]
pub struct Arm {
    pub tok: Tok,
    pub pat: N<Pat>,
    pub body: EPtr,
}

#[derive(Clone, Debug)]
pub enum ForKind {
    Cond,
    Range {
        var: String,
        lo: EPtr,
        hi: EPtr,
        inclusive: bool,
    },
    Iter {
        var: String,
        iterable: EPtr,
    },
    Kv {
        var: String,
        var2: String,
        iterable: EPtr,
    },
    /// `for i in (0..100).Step(2)`：区间带步长（文稿 §5.2）
    Step {
        var: String,
        lo: EPtr,
        hi: EPtr,
        inclusive: bool,
        step: EPtr,
    },
}

// ---------------------------------------------------------------- 表达式

#[derive(Clone, Debug)]
pub enum SPart {
    Text(String),
    Interp { expr: EPtr, fmt: Option<u32> },
}

#[derive(Clone, Debug)]
pub enum Expr {
    IntLit { value: i64, raw: String },
    FloatLit { value: f64, raw: String, is_f32: bool },
    DecLit { coeff: i128, scale: i32, raw: String },
    BoolLit { value: bool },
    NullLit,
    StrLit { parts: Vec<SPart> },
    Path { segs: Vec<String>, args: Vec<TPtr> },
    This,
    Parent,
    TupleLit { elems: Vec<EPtr> },
    ArrayLit { elems: Vec<EPtr> },
    BraceLit { pairs: Vec<(String, EPtr)> },
    Call { callee: EPtr, args: Vec<N<Arg>> },
    MethodCall { recv: EPtr, name: String, args: Vec<N<Arg>> },
    FieldAccess { recv: EPtr, name: String },
    SafeAccess { recv: EPtr, name: String },
    SafeCall { recv: EPtr, name: String, args: Vec<N<Arg>> },
    Index { recv: EPtr, index: EPtr },
    Unary { op: String, operand: EPtr },
    Binary { op: String, lhs: EPtr, rhs: EPtr },
    Ternary { cond: EPtr, then_: EPtr, els: EPtr },
    Coalesce { lhs: EPtr, rhs: EPtr },
    Range { lo: EPtr, hi: EPtr, inclusive: bool },
    Try { inner: EPtr },
    New { path: Vec<String>, args: Vec<N<Arg>> },
    If(Box<IfNode>),
    Match(Box<MatchNode>),
    AnonFunc { params: Vec<N<Param>>, body: Block },
    /// `is(T == Type)` / `is(T : Interface)` / `is(typeof(Expr) == Type)`：编译期类型查询，返回 bool
    IsType {
        /// 检查方式："eq" (==), "impl" (:), "typeof_eq" (typeof(Expr) == Type)
        kind: String,
        /// 类型（或左侧类型）
        ty: Option<TPtr>,
        /// 右侧类型（用于 eq/impl）
        target: Option<TPtr>,
        /// 表达式（用于 typeof_eq）
        expr: Option<EPtr>,
        /// 接口名（用于 impl）
        interface: Option<Vec<String>>,
    },
}

// ---------------------------------------------------------------- 类型

#[derive(Clone, Debug)]
pub enum Ty {
    Named { path: Vec<String>, args: Vec<TPtr> },
    Array { elem: TPtr, len: Option<EPtr> },
    Nullable { inner: TPtr },
    Func { params: Vec<TPtr>, ret: TPtr },
    Tuple { elems: Vec<TPtr> },
}

// ---------------------------------------------------------------- 模式

#[derive(Clone, Debug)]
pub enum Pat {
    Lit { expr: EPtr },
    Range { lo: EPtr, hi: EPtr, inclusive: bool },
    Enum { ty: Vec<String>, variant: String },
    Result { is_ok: bool, bind: String },
    Type { ty: TPtr, bind: String },
    Wild,
    Bind { name: String },
    /// 多值替代模式：`1,3,6 =>` 表示任意一个匹配即可
    /// 子模式仅限非绑定模式（Lit / Range / Enum / Wild），不含 Bind/Type/Result
    Multi { pats: Vec<N<Pat>> },
}

// ---------------------------------------------------------------- dump

fn vis_s(v: &Option<String>) -> &str {
    v.as_deref().unwrap_or("private")
}

pub fn dump_module(mod_: &Module) -> String {
    let mut out: Vec<String> = Vec::new();
    for it in &mod_.items {
        dump_item(&it.k, &it.tok, 1, &mut out);
    }
    out.join("\n")
}

fn w(out: &mut Vec<String>, ind: usize, s: String) {
    out.push(format!("{}{}", "  ".repeat(ind), s));
}

fn dump_item(it: &Item, tok: &Tok, ind: usize, out: &mut Vec<String>) {
    match it {
        Item::Import { path, .. } => w(out, ind, format!("IMPORT {}", path.join("."))),
        Item::ConstDecl { name, ty, init, .. } => w(
            out,
            ind,
            format!(
                "CONST {} ty={} init={}",
                name,
                ty.as_ref().map_or("<infer>".to_string(), |t| ex_ty(t)),
                ex(init)
            ),
        ),
        Item::FuncDecl {
            vis,
            ret,
            name,
            generics,
            params,
            body,
            ..
        } => {
            w(
                out,
                ind,
                format!(
                    "FUNC {} vis={} ret={} generics={}",
                    name,
                    vis_s(vis),
                    ex_ty(ret),
                    dump_generics(generics)
                ),
            );
            w(out, ind + 1, format!("PARAMS {}", dump_params(params)));
            dump_block(body, ind + 1, out);
        }
        Item::ClassDecl {
            vis,
            name,
            parent,
            impls,
            members,
            ..
        } => {
            let mut s = format!("CLASS {} vis={}", name, vis_s(vis));
            if let Some(p) = parent {
                s += &format!(" extend {}", ex_ty(p));
            }
            if !impls.is_empty() {
                s += &format!(
                    " implement {}",
                    impls.iter().map(ex_ty).collect::<Vec<_>>().join(",")
                );
            }
            w(out, ind, s);
            for m in members {
                dump_member(&m.k, ind + 1, out);
            }
        }
        Item::StructDecl { name, members, .. } => {
            w(out, ind, format!("STRUCT {}", name));
            for m in members {
                dump_member(&m.k, ind + 1, out);
            }
        }
        Item::InterfaceDecl {
            name,
            extends,
            methods,
            ..
        } => {
            w(
                out,
                ind,
                format!(
                    "INTERFACE {} extends={}",
                    name,
                    extends.iter().map(ex_ty).collect::<Vec<_>>().join(",")
                ),
            );
            for m in methods {
                w(
                    out,
                    ind + 1,
                    format!(
                        "SIG {}{} ret={}",
                        m.k.name,
                        dump_params(&m.k.params),
                        ex_ty(&m.k.ret)
                    ),
                );
            }
        }
        Item::EnumDecl { name, variants, .. } => {
            w(out, ind, format!("ENUM {} variants={}", name, variants.join(",")));
        }
        Item::TypeAlias { name, target, .. } => {
            w(out, ind, format!("TYPE {} = {}", name, ex_ty(target)));
        }
        Item::ExternBlock { funcs } => {
            w(out, ind, format!("EXTERN_BLOCK n_funcs={}", funcs.len()));
            for f in funcs {
                if let Item::FuncDecl { name, ret, params, .. } = &f.k {
                    w(out, ind + 1, format!("FUNC {} ret={} params={}", name, ex_ty(ret), dump_params(params)));
                }
            }
        }
    }
    let _ = tok;
}

fn dump_member(m: &Member, ind: usize, out: &mut Vec<String>) {
    match m {
        Member::Field { vis, ty, name } => w(
            out,
            ind,
            format!("FIELD {} vis={} ty={}", name, vis_s(vis), ex_ty(ty)),
        ),
        Member::Ctor { name, params, body } => {
            w(out, ind, format!("CTOR {} params={}", name, dump_params(params)));
            dump_block(body, ind + 1, out);
        }
        Member::Method {
            vis,
            is_static,
            is_override,
            is_abstract,
            ret,
            name,
            params,
            body,
        } => {
            w(
                out,
                ind,
                format!(
                    "METHOD {}{}{}{} vis={} ret={}",
                    name,
                    if *is_static { " is_static" } else { "" },
                    if *is_override { " override" } else { "" },
                    if *is_abstract { " abstract" } else { "" },
                    vis_s(vis),
                    ex_ty(ret)
                ),
            );
            w(out, ind + 1, format!("PARAMS {}", dump_params(params)));
            dump_block(body, ind + 1, out);
        }
        Member::Item(it) => dump_item(&it.k, &it.tok, ind, out),
    }
}

fn dump_block(b: &Block, ind: usize, out: &mut Vec<String>) {
    for s in &b.stmts {
        dump_stmt(&s.k, &s.tok, ind, out);
    }
}

fn dump_stmt(s: &Stmt, tok: &Tok, ind: usize, out: &mut Vec<String>) {
    match s {
        Stmt::Block(b) => {
            w(out, ind, "BLOCK".to_string());
            dump_block(b, ind + 1, out);
        }
        Stmt::VarDecl {
            explicit_ty,
            name,
            init,
            ..
        } => w(
            out,
            ind,
            format!(
                "VAR {} ty={} init={}",
                name,
                explicit_ty
                    .as_ref()
                    .map_or("<infer>".to_string(), |t| ex_ty(t)),
                init.as_ref().map_or(String::new(), ex)
            ),
        ),
        Stmt::VarTuple { names, init } => w(
            out,
            ind,
            format!("VAR_TUPLE ({}) = {}", names.join(","), ex(init)),
        ),
        Stmt::Const { name, ty, init, .. } => w(
            out,
            ind,
            format!(
                "CONST {} ty={} init={}",
                name,
                ty.as_ref().map_or("<infer>".to_string(), |t| ex_ty(t)),
                ex(init)
            ),
        ),
        Stmt::Simple {
            lhs,
            op,
            rhs,
            incdec,
        } => {
            if let Some(d) = incdec {
                w(out, ind, format!("EXPR {}{}", ex(lhs), d));
            } else if let Some(op) = op {
                w(
                    out,
                    ind,
                    format!("{} {} {}", ex(lhs), op, rhs.as_ref().map(ex).unwrap_or_default()),
                );
            } else {
                w(out, ind, format!("EXPR {}", ex(lhs)));
            }
        }
        Stmt::If(n) => dump_if(n, ind, out),
        Stmt::Match(n) => dump_match(n, ind, out),
        Stmt::For { label, kind, body } => {
            let _ = label;
            w(out, ind, format!("FOR {}", dump_for_kind(kind)));
            dump_block(body, ind + 1, out);
        }
        Stmt::While { cond, body } => {
            w(out, ind, format!("WHILE cond={}", ex(cond)));
            dump_block(body, ind + 1, out);
        }
        Stmt::Return { value } => w(
            out,
            ind,
            format!("RETURN {}", value.as_ref().map_or(String::new(), ex)),
        ),
        Stmt::Throw { variant, msg } => {
            w(out, ind, format!("THROW error.{}({})", variant, ex(msg)));
        }
        Stmt::TryCatch(n) => {
            w(out, ind, "TRY".to_string());
            dump_block(&n.body, ind + 1, out);
            for c in &n.catches {
                let pat = match &c.variant {
                    Some(v) => format!("error.{} {}", v, c.name),
                    None => format!("error {}", c.name),
                };
                w(out, ind, format!("CATCH ({})", pat));
                dump_block(&c.body, ind + 1, out);
            }
            if let Some(f) = &n.finally {
                w(out, ind, "FINALLY".to_string());
                dump_block(f, ind + 1, out);
            }
        }
        Stmt::Defer { call } => w(out, ind, format!("DEFER {}", ex(call))),
        Stmt::Break { label } => w(
            out,
            ind,
            format!("BREAK {}", label.as_deref().unwrap_or("")),
        ),
        Stmt::Continue { label } => w(
            out,
            ind,
            format!("CONTINUE {}", label.as_deref().unwrap_or("")),
        ),
        Stmt::Unsafe { body } => {
            w(out, ind, "UNSAFE".to_string());
            dump_block(body, ind + 1, out);
        }
    }
    let _ = tok;
}

fn dump_if(n: &IfNode, ind: usize, out: &mut Vec<String>) {
    w(out, ind, format!("IF cond={}", ex(&n.cond)));
    dump_block(&n.then_, ind + 1, out);
    if let Some(els) = &n.els {
        w(out, ind, "ELSE".to_string());
        match els {
            IfTail::Block(b) => dump_block(b, ind + 1, out),
            IfTail::If(inner) => dump_if(&inner.k, ind + 1, out),
        }
    }
}

fn dump_match(n: &MatchNode, ind: usize, out: &mut Vec<String>) {
    w(out, ind, format!("MATCH {}", ex(&n.scrutinee)));
    for a in &n.arms {
        w(out, ind + 1, format!("ARM {} => {}", dump_pat(&a.pat), ex(&a.body)));
    }
}

fn dump_for_kind(s: &ForKind) -> String {
    match s {
        ForKind::Range {
            var,
            lo,
            hi,
            inclusive,
        } => format!(
            "{} in {}{}{}",
            var,
            ex(lo),
            if *inclusive { "..=" } else { ".." },
            ex(hi)
        ),
        ForKind::Iter { var, iterable } => format!("{} in {}", var, ex(iterable)),
        ForKind::Kv { var, var2, iterable } => {
            format!("({}, {}) in {}", var, var2, ex(iterable))
        }
        ForKind::Cond => "{ }".to_string(),
        ForKind::Step { var, lo, hi, inclusive, step } => format!(
            "{} in {}{}{}.Step({})",
            var,
            ex(lo),
            if *inclusive { "..=" } else { ".." },
            ex(hi),
            ex(step)
        ),
    }
}

fn dump_pat(p: &N<Pat>) -> String {
    match &p.k {
        Pat::Lit { expr } => ex(expr),
        Pat::Range { lo, hi, inclusive } => format!(
            "{}{}{}",
            ex(lo),
            if *inclusive { "..=" } else { ".." },
            ex(hi)
        ),
        Pat::Enum { ty, variant } => format!("{}.{}", ty.join("."), variant),
        Pat::Result { is_ok, bind } => {
            format!("{}({})", if *is_ok { "ok" } else { "fail" }, bind)
        }
        Pat::Type { ty, bind } => format!("{} {}", ex_ty(ty), bind),
        Pat::Wild => "_".to_string(),
        Pat::Bind { name } => name.clone(),
        Pat::Multi { pats } => {
            pats.iter().map(dump_pat).collect::<Vec<_>>().join(",")
        }
    }
}

fn dump_generics(gs: &[TypeParam]) -> String {
    if gs.is_empty() {
        return "[]".to_string();
    }
    let parts: Vec<String> = gs
        .iter()
        .map(|g| {
            if g.constraints.is_empty() {
                g.name.clone()
            } else {
                format!("{}:{}", g.name, g.constraints.join("+"))
            }
        })
        .collect();
    format!("[{}]", parts.join(","))
}

fn dump_params(ps: &[N<Param>]) -> String {
    let out: Vec<String> = ps
        .iter()
        .map(|p| {
            let pre = match p.k.borrow.as_deref() {
                Some("const") => "&const ",
                Some("mut") => "&",
                _ => "",
            };
            format!("({}{},{})", pre, ex_ty(&p.k.ty), p.k.name)
        })
        .collect();
    format!("[{}]", out.join(","))
}

pub fn ex_ty(t: &TPtr) -> String {
    match &t.k {
        Ty::Named { path, args } => {
            let mut s = path.join(".");
            if !args.is_empty() {
                s += &format!("<{}>", args.iter().map(ex_ty).collect::<Vec<_>>().join(","));
            }
            s
        }
        Ty::Array { elem, len } => match len {
            None => format!("{}[]", ex_ty(elem)),
            Some(e) => format!("{}[{}]", ex_ty(elem), ex(e)),
        },
        Ty::Nullable { inner } => format!("{}?", ex_ty(inner)),
        Ty::Func { params, ret } => format!(
            "func({}) {}",
            params.iter().map(ex_ty).collect::<Vec<_>>().join(","),
            ex_ty(ret)
        ),
        Ty::Tuple { elems } => format!(
            "({})",
            elems.iter().map(ex_ty).collect::<Vec<_>>().join(",")
        ),
    }
}

pub fn ex(e: &EPtr) -> String {
    match &e.k {
        Expr::IntLit { raw, .. } => raw.clone(),
        Expr::FloatLit { raw, .. } => raw.clone(),
        Expr::DecLit { raw, .. } => raw.clone(),
        Expr::BoolLit { value } => if *value { "true" } else { "false" }.to_string(),
        Expr::NullLit => "null".to_string(),
        Expr::StrLit { parts } => {
            let mut s = String::from("\"");
            for p in parts {
                match p {
                    SPart::Text(t) => s += &t.replace('\n', "\\n"),
                    SPart::Interp { expr, fmt } => {
                        s += "{";
                        s += &ex(expr);
                        if let Some(f) = fmt {
                            s += &format!(":.{}", f);
                        }
                        s += "}";
                    }
                }
            }
            s + "\""
        }
        Expr::Path { segs, args } => {
            let mut s = segs.join(".");
            if !args.is_empty() {
                s += &format!("<{}>", args.iter().map(ex_ty).collect::<Vec<_>>().join(","));
            }
            s
        }
        Expr::This => "this".to_string(),
        Expr::Parent => "parent".to_string(),
        Expr::TupleLit { elems } => format!(
            "({})",
            elems.iter().map(ex).collect::<Vec<_>>().join(",")
        ),
        Expr::ArrayLit { elems } => format!(
            "[{}]",
            elems.iter().map(ex).collect::<Vec<_>>().join(",")
        ),
        Expr::BraceLit { pairs } => format!(
            "{{{}}}",
            pairs
                .iter()
                .map(|(k, v)| format!("\"{}\":{}", k, ex(v)))
                .collect::<Vec<_>>()
                .join(",")
        ),
        Expr::Call { callee, args } => format!(
            "CALL({}) args=[{}]",
            ex(callee),
            args.iter().map(|a| ex(&a.k.expr)).collect::<Vec<_>>().join(",")
        ),
        Expr::MethodCall { recv, name, args } => format!(
            "MCALL({}.{}) args=[{}]",
            ex(recv),
            name,
            args.iter().map(|a| ex(&a.k.expr)).collect::<Vec<_>>().join(",")
        ),
        Expr::FieldAccess { recv, name } => format!("{}.{}", ex(recv), name),
        Expr::SafeAccess { recv, name } => format!("{}?.{}", ex(recv), name),
        Expr::SafeCall { recv, name, args } => format!(
            "{}?.{} args=[{}]",
            ex(recv),
            name,
            args.iter().map(|a| ex(&a.k.expr)).collect::<Vec<_>>().join(",")
        ),
        Expr::Index { recv, index } => format!("{}[{}]", ex(recv), ex(index)),
        Expr::Unary { op, operand } => format!("({}{})", op, ex(operand)),
        Expr::Binary { op, lhs, rhs } => format!("({} {} {})", ex(lhs), op, ex(rhs)),
        Expr::Ternary { cond, then_, els } => {
            format!("({} ? {} : {})", ex(cond), ex(then_), ex(els))
        }
        Expr::Coalesce { lhs, rhs } => format!("({} ?? {})", ex(lhs), ex(rhs)),
        Expr::Range { lo, hi, inclusive } => format!(
            "({}{}{})",
            ex(lo),
            if *inclusive { "..=" } else { ".." },
            ex(hi)
        ),
        Expr::Try { inner } => format!("TRY({})", ex(inner)),
        Expr::New { path, args } => format!(
            "NEW {} args=[{}]",
            path.join("."),
            args.iter().map(|a| ex(&a.k.expr)).collect::<Vec<_>>().join(",")
        ),
        Expr::If(n) => format!("IF({}){{...}}", ex(&n.cond)),
        Expr::Match(n) => format!("MATCH({}){{...}}", ex(&n.scrutinee)),
        Expr::AnonFunc { params, .. } => format!("ANON{}", dump_params(params)),
        Expr::IsType { kind, .. } => format!("is({})", kind),
    }
}
