//! 语法分析（规格书 §4，G-1…G-16）。
//!
//! 两个换行入口的分工（语句终止规则 G-2/G-4 的关键）：
//!   nl()          —— 纯跳过 NEWLINE（G-4 的插入点）
//!   expect_term() —— 必须有一个语句边界（G-2）
//! 六处歧义按 G-10…G-15 消解；语法层不判定 T(x) 与 {...}（留给 sema）。
//!
//! 实现细节备忘：try/catch 头部解析会临时吃掉换行再回退（save/restore `self.p`），
//! 这样 `try {...}` 后无换行时 expect_term 仍能拿到语句边界。

use crate::frontend::ast::*;
use crate::diag::Diags;
use crate::frontend::lexer::{Lexer, RESERVED_TYPES, StrPart, Tok, TokVal};

pub struct ParseError {
    pub code: &'static str,
    pub msg: String,
    pub tok: Tok,
    pub rule: &'static str,
}

const MAX_PARSE_ERRORS: u32 = 20;

/// G-5：这些 token 不能出现在语句行首（上一行已是完整语句）。
/// `-` `!` `~` 不列入：它们可以作为一元运算符开一个合法表达式。
const LINE_START_ILLEGAL: &[&str] = &[
    "+", "*", "/", "%", "==", "!=", "<", "<=", ">", ">=", "&&", "||",
    "&", "|", "^", "<<", ">>", "=", "+=", "-=", "*=", "/=", "%=", "?", "=>",
];

/// mark 快照：token 位置 + 诊断条数 + 错误计数（reset 时一并回滚）。
#[derive(Clone, Copy)]
struct Mark {
    p: usize,
    ndiags: usize,
    errors: u32,
}

/// Tok 流 → AST。`Mark` 支持回溯试探（歧义消解 G-10…G-15 的手段）；
/// `errors` 达到上限即停止，避免错误雪崩刷屏。
pub struct Parser<'a> {
    toks: Vec<Tok>,
    p: usize,
    d: &'a mut Diags,
    path: String,
    errors: u32,
    allow_incdec: bool,
    next_id: u32,
    /// 顶层语句（入口文件允许，结束时包成隐式 `main`；文稿 9.4）
    top_stmts: Vec<N<Stmt>>,
}

type PResult<T> = Result<T, ParseError>;

impl<'a> Parser<'a> {
    pub fn new(toks: Vec<Tok>, path: String, d: &'a mut Diags, next_id: u32) -> Self {
        Parser { toks, p: 0, d, path, errors: 0, allow_incdec: false, next_id, top_stmts: Vec::new()}
    }

    // ---------------- 基础 ----------------

    fn tok(&self, k: usize) -> &Tok {
        let i = self.p + k;
        if i >= self.toks.len() {
            return self.toks.last().unwrap();
        }
        &self.toks[i]
    }

    fn at(&self, kind: &str) -> bool {
        self.tok(0).is(kind)
    }

    fn advance(&mut self) -> Tok {
        let t = self.tok(0).clone();
        if self.p < self.toks.len() - 1 {
            self.p += 1;
        }
        t
    }

    fn accept(&mut self, kind: &str) -> Option<Tok> {
        if self.at(kind) {
            Some(self.advance())
        } else {
            None
        }
    }

    fn nl(&mut self) {
        while self.at("NL") {
            self.advance();
        }
    }

    fn err(
        &mut self,
        code: &'static str,
        tok: Option<&Tok>,
        rule: &'static str,
        zh: &str,
        args: &[&dyn std::fmt::Display],
    ) {
        let t = tok.cloned().unwrap_or_else(|| self.tok(0).clone());
        let msg = crate::diag_msg::tr(self.d.lang, code, zh, args);
        self.d
            .error(code, msg, &self.path, t.line, t.col, t.eline, t.ecol, rule);
        self.errors += 1;
    }

    fn fail<T>(
        &self,
        code: &'static str,
        tok: &Tok,
        rule: &'static str,
        zh: &str,
        args: &[&dyn std::fmt::Display],
    ) -> PResult<T> {
        // 语言在本次运行内固定：构造 ParseError 时就展开成最终文案
        let msg = crate::diag_msg::tr(self.d.lang, code, zh, args);
        Err(ParseError { code, msg, tok: tok.clone(), rule })
    }

    fn expect(&mut self, kind: &str, what: Option<&str>) -> PResult<Tok> {
        if self.at(kind) {
            return Ok(self.advance());
        }
        let t = self.tok(0).clone();
        let want = what.unwrap_or(&format!("`{}`", kind)).to_string();
        let got = display_tok(&t);
        self.fail("E2001", &t, "G-1", "期望 {0}，得到 {1}", &[&want, &got])
    }

    fn expect_string_lit(&mut self, want_val: &str, what: Option<&str>) -> PResult<Tok> {
        let t = self.tok(0).clone();
        if t.is("STR") {
            if let TokVal::Str(parts) = &t.val {
                // 简单字符串：单个 Text 片段
                if parts.len() == 1 {
                    if let StrPart::Text(s) = &parts[0] {
                        if s == want_val {
                            return Ok(self.advance());
                        }
                    }
                }
            }
        }
        let what = what.unwrap_or(&format!("字符串 `{}`", want_val)).to_string();
        let got = display_tok(&t);
        self.fail("E2001", &t, "G-1", "期望 {0}，得到 {1}", &[&what, &got])
    }

    fn expect_ident(&mut self, what: &str) -> PResult<Tok> {
        let t = self.tok(0).clone();
        if t.is("IDENT") {
            return Ok(self.advance());
        }
        const KEYWORDS: &[&str] = &[
            "var", "const", "class", "type", "if", "for", "while", "match", "return", "try",
            "throw", "defer", "import", "static", "public", "protected", "new", "as",
            "in", "else", "this", "parent", "true", "false", "null", "constructor",
            "interface", "struct", "enum", "extend", "implement", "override",
            "abstract", "final", "export",
            "break", "continue",
        ];
        if KEYWORDS.contains(&t.kind.as_str()) {
            return self.fail("E2001", &t, "G-7", "关键字 `{0}` 不能作{1}", &[&t.kind, &what]);
        }
        self.fail("E2001", &t, "G-1", "期望{0}，得到 `{1}`", &[&what, &display_tok(&t)])
    }

    /// G-2：语句终结符 = NEWLINE | ; | 前瞻到 } | EOF。
    fn expect_term(&mut self) {
        let mut consumed = false;
        if self.at(";") {
            self.advance();
            consumed = true;
        }
        while self.at("NL") {
            self.advance();
            consumed = true;
        }
        if self.at(";") && consumed {
            self.advance();
            while self.at("NL") {
                self.advance();
            }
            return;
        }
        if consumed || self.at("}") || self.at("EOF") {
            return;
        }
        let t = self.tok(0).clone();
        self.err("E2002", Some(&t), "G-2", "语句未以换行或 `;` 结束（同一行不能写两条语句）", &[]);
        // 恢复：吞掉当前 token，避免死循环
        self.advance();
    }

    fn mark(&self) -> Mark {
        Mark { p: self.p, ndiags: self.d.items.len(), errors: self.errors }
    }

    fn reset(&mut self, m: Mark) {
        self.p = m.p;
        self.d.truncate_to(m.ndiags);
        self.d.error_count = self
            .d
            .items
            .iter()
            .filter(|x| matches!(x.severity, crate::diag::Severity::Error))
            .count();
        self.errors = m.errors;
    }

    fn sync(&mut self) {
        let mut depth = 0usize;
        while !self.at("EOF") {
            let k = self.tok(0).kind.clone();
            if k == "(" || k == "[" || k == "{" {
                depth += 1;
            } else if k == ")" || k == "]" || k == "}" {
                if depth == 0 {
                    return;
                }
                depth -= 1;
            } else if (k == "NL" || k == ";") && depth == 0 {
                self.advance();
                return;
            }
            self.advance();
        }
    }

    fn nn<T>(&mut self, tok: Tok, k: T) -> N<T> {
        let id = self.next_id;
        self.next_id += 1;
        N { id, tok, k }
    }

    fn ne<T>(&mut self, tok: Tok, k: T) -> PResult<Box<N<T>>> {
        Ok(Box::new(self.nn(tok, k)))
    }

    // ---------------- 模块 ----------------

    pub fn parse(&mut self) -> Module {
        let mut items = Vec::new();
        loop {
            self.nl();
            if self.at("EOF") {
                break;
            }
            match self.parse_top_item() {
                Ok(mut it) => items.append(&mut it),
                Err(e) => {
                    self.report(&e);
                    self.sync();
                }
            }
            if self.errors >= MAX_PARSE_ERRORS {
                self.err("E2015", None, "G-16", "语法错误过多（已达 {0} 条），停止解析", &[&MAX_PARSE_ERRORS]);
                break;
            }
        }
        // 顶层语句包成隐式 `main`：`Println("hi")` 单独一行就是完整程序（文稿 9.4）。
        // 是否允许由 sema 判：被 import 的模块里出现它要报 E3024。
        if !self.top_stmts.is_empty() {
            let tok = self.top_stmts[0].tok.clone();
            let stmts = std::mem::take(&mut self.top_stmts);
            let ret = Box::new(self.nn(tok.clone(), Ty::Named {
                path: vec!["int".to_string()],
                args: Vec::new(),
            }));
            let f = self.nn(tok.clone(), Item::FuncDecl {
                vis: None,
                ret,
                name: "main".to_string(),
                generics: Vec::new(),
                params: Vec::new(),
                body: Block { tok, stmts: stmts.into_iter().map(Box::new).collect() },
                attrs: Vec::new(),
                implicit: true,
                is_pure: false,
                is_nothrow: false,
                is_ref_return: false,
                is_export: false,
            });
            items.push(f);
        }
        Module { items, parse_errors: self.errors }
    }

    fn report(&mut self, e: &ParseError) {
        self.d.error(
            e.code,
            e.msg.clone(),
            &self.path,
            e.tok.line,
            e.tok.col,
            e.tok.eline,
            e.tok.ecol,
            e.rule,
        );
        self.errors += 1;
    }

    fn parse_top_item(&mut self) -> PResult<Vec<N<Item>>> {
        let mut attrs: Vec<Attr> = Vec::new();
        while self.at("ATTR") {
            let t = self.advance();
            if t.text() != "os" && t.text() != "arch" && t.text() != "pure" && t.text() != "nothrow" && t.text() != "nogc" && t.text() != "deprecated" {
                self.err("E3029", Some(&t), "L-23", "未知属性 `@{0}`（只支持 @os、@arch、@pure、@nothrow、@nogc、@deprecated）", &[&t.text()]);
            }
            let mut args = Vec::new();
            if self.accept("(").is_some() {
                self.nl();
                if !self.at(")") {
                    args.push(self.expect_ident("属性参数")?.text().to_string());
                    while self.accept(",").is_some() {
                        self.nl();
                        if self.at(")") {
                            break;
                        }
                        args.push(self.expect_ident("属性参数")?.text().to_string());
                    }
                }
                self.expect(")", None)?;
            }
            attrs.push(Attr { tok: t.clone(), name: t.text().to_string(), args });
            self.nl();
        }
        if self.at("import") {
            return Ok(vec![self.parse_import()?]);
        }
        let mut vis: Option<String> = None;
        let mut is_abstract = false;
        let mut is_final = false;
        let mut is_export = false;
        if self.at("abstract") {
            self.advance();
            is_abstract = true;
        }
        if self.at("final") {
            self.advance();
            is_final = true;
        }
        if self.at("export") {
            self.advance();
            is_export = true;
        }
        if self.at("public") {
            self.advance();
            vis = Some("public".to_string());
        } else if self.at("protected") {
            let t = self.advance();
            self.err("E3014", Some(&t), "N-12", "`protected` 只能用于类成员，模块级声明不合法", &[]);
            vis = Some("protected".to_string());
        }
        if self.at("const") {
            return Ok(vec![self.parse_const(vis)?]);
        }
        if self.at("class") {
            return Ok(vec![self.parse_class(vis, is_abstract, is_final)?]);
        }
        if self.at("struct") {
            return Ok(vec![self.parse_struct(vis)?]);
        }
        if self.at("interface") {
            return Ok(vec![self.parse_interface(vis)?]);
        }
        if self.at("enum") {
            return Ok(vec![self.parse_enum(vis)?]);
        }
        if self.at("type") {
            return Ok(vec![self.parse_type_alias(vis)?]);
        }
        if self.at("extern") {
            return Ok(vec![self.parse_extern_block(vis)?]);
        }
        // 模块级全局变量声明：var name = init / var ty name = init / ty name = init / ty name
        if self.at("var") {
            return Ok(vec![self.parse_module_var(vis)?]);
        }
        // 显式类型但无 var 关键字：int count = 0 / Mutex mu / public int count = 0
        if self.tok(0).is("IDENT") && self.tok(1).is("IDENT") {
            let third = self.tok(2).kind.clone();
            if third == "=" || third == "NL" || third == ";" || third == "}" || third == "EOF" {
                return Ok(vec![self.parse_module_var(vis)?]);
            }
        }
        // 顶层语句：入口文件里合法（包成隐式 `main`），被 import 的模块里由 sema 报 E3024。
        // 放在 parse_func 之前，避免先把 `Println("x")` 当函数声明去解析再报一条莫名其妙的错。
        if self.top_is_stmt() {
            // `public int x = 0`：先吞了修饰词才发现是语句——用户想声明模块级变量，
            // 而语句又不可能有可见性，直接按"模块级不能声明变量"报，别把它悄悄当成局部变量。
            if vis.is_some() {
                let t = self.tok(0).clone();
                self.err("E3024", Some(&t), "文稿 9.4", "模块级不能声明变量（只能 const）；入口文件的顶层语句会被包成 main，不能带可见性修饰词", &[]);
                self.sync();
                return Ok(vec![]);
            }
            let st = match self.parse_stmt() {
                Ok(Some(st)) => st,
                Ok(None) => return Ok(vec![]),
                Err(e) => return Err(e),
            };
            // 模块级变量声明：小写名→E3024（模块级不能声明变量）；
            // 大写名→E3030（模块内符号默认私有，需显式 public 导出）
            if let Stmt::VarDecl { name, .. } = &st.k {
                let first = name.chars().next();
                if first.map_or(false, |c| c.is_ascii_uppercase()) {
                    self.err("E3030", Some(&st.tok), "文稿 9.1",
                        "模块 `{0}` 没有公开符号 `{1}`（文件内符号默认私有，导出需写 `public`）",
                        &[name, &""]);
                    self.sync();
                    return Ok(vec![]);
                }
                self.err("E3024", Some(&st.tok), "文稿 9.4",
                    "模块级不能声明变量（只能 const）；入口文件的顶层语句会被包成 main，不能带可见性修饰词",
                    &[]);
                self.sync();
                return Ok(vec![]);
            }
            self.top_stmts.push(st);
            self.nl();
            return Ok(vec![]);
        }
        // 函数
        match self.parse_func(attrs, vis, is_export) {
            Ok(it) => Ok(vec![it]),
            Err(e) => {
                if self.at("if") || self.at("for") || self.at("while") || self.at("return") {
                    self.err("E2011", Some(&e.tok), "文稿 9.1", "模块级只能是声明或 import，不能出现语句", &[]);
                }
                Err(e)
            }
        }
    }

    /// 顶层这一行是「语句」而不是「声明」吗？
    /// 入口文件允许顶层语句（文稿 9.4），所以要能把 `int x = 1`、`f(...)`、`a.b = 1`、`x++`
    /// 与 `int f(...)`、`map<string, int> m = {}` 区分开——先跳类型前缀，再看后面是什么。
    fn top_is_stmt(&self) -> bool {
        const KW: &[&str] = &[
            "if", "for", "while", "unsafe", "match", "return", "break", "continue", "defer", "throw", "try", "var", "static",
        ];
        if KW.iter().any(|k| self.at(k)) {
            return true;
        }
        if !self.tok(0).is("IDENT") {
            return false;
        }
        // 跳类型前缀：`int`、`int[]`、`T?`、`map<K, V>`、`a.b.C`
        let mut k = 1usize;
        loop {
            if self.tok(k).is("[") && self.tok(k + 1).is("]") {
                k += 2;
            } else if self.tok(k).is("?") {
                k += 1;
            } else if self.tok(k).is("<") {
                let mut depth = 0usize;
                loop {
                    if self.tok(k).is("EOF") {
                        return false;
                    }
                    if self.tok(k).is("<") {
                        depth += 1;
                    } else if self.tok(k).is(">") {
                        depth -= 1;
                        if depth == 0 {
                            k += 1;
                            break;
                        }
                    }
                    k += 1;
                }
            } else if self.tok(k).is(".") && self.tok(k + 1).is("IDENT") {
                k += 2;
            } else {
                break;
            }
        }
        // `类型 名 =` → 局部变量声明（语句）；`类型 名 (` → 函数声明
        if self.tok(k).is("IDENT") {
            return self.tok(k + 1).is("=");
        }
        true
    }

    fn parse_func(&mut self, attrs: Vec<Attr>, vis: Option<String>, is_export: bool) -> PResult<N<Item>> {
        let is_ref_return = self.at("ref");
        if is_ref_return {
            self.advance();
        }
        let ret = self.parse_type()?;
        let name = self.expect_ident("函数名")?;
        let generics = self.parse_generics()?;
        let params = self.parse_params()?;
        self.nl();
        let body = self.parse_block()?;
        // 从属性中读取 pure/nothrow 标记
        let is_pure = attrs.iter().any(|a| a.name == "pure");
        let is_nothrow = attrs.iter().any(|a| a.name == "nothrow");
        Ok(self.nn(
            name.clone(),
            Item::FuncDecl {
                implicit: false,
                vis,
                ret,
                name: name.text().to_string(),
                generics,
                params,
                body,
                attrs,
                is_pure,
                is_nothrow,
                is_ref_return,
                is_export,
            },
        ))
    }

    fn parse_import(&mut self) -> PResult<N<Item>> {
        let t = self.expect("import", None)?;
        let mut path = vec![self.expect_ident("模块名")?.text().to_string()];
        while self.at(".") && self.tok(1).is("IDENT") {
            self.advance(); // 吃掉 "."
            // 模块路径段可以是任何标识符文本（包括保留字如 string、file 等），
            // 以支持 `import std.string` 这样的多级标准库模块路径。
            let seg = self.tok(0).clone();
            let name = seg.text();
            path.push(name.to_string());
            self.advance();
        }
        let mut alias = None;
        let mut names = None;
        if self.at("as") {
            self.advance();
            alias = Some(self.expect_ident("别名")?.text().to_string());
        } else if self.at(".") && self.tok(1).is("{") {
            self.advance();
            self.advance();
            let mut ns = vec![self.expect_ident("导出名")?.text().to_string()];
            while self.accept(",").is_some() {
                self.nl();
                if self.at("}") {
                    break;
                }
                ns.push(self.expect_ident("导出名")?.text().to_string());
            }
            self.expect("}", None)?;
            names = Some(ns);
        }
        // 消耗可选的语句终止符（`;` 或换行），使 `import std.string;` 能正常结束
        if self.at(";") {
            self.advance();
        } else {
            self.nl();
        }
        Ok(self.nn(t, Item::Import { path, alias, names }))
    }

    fn parse_const(&mut self, vis: Option<String>) -> PResult<N<Item>> {
        let t = self.expect("const", None)?;
        let mut ty: Option<TPtr> = None;
        let m = self.mark();
        match self.parse_type() {
            Ok(cand) => {
                if self.at("IDENT") {
                    ty = Some(cand);
                } else {
                    self.reset(m);
                }
            }
            Err(_) => self.reset(m),
        }
        let name = self.expect_ident("常量名")?;
        self.expect("=", None)?;
        self.nl();
        let init = self.parse_expr()?;
        Ok(self.nn(
            t,
            Item::ConstDecl { vis, ty, name: name.text().to_string(), init },
        ))
    }

    /// 模块级全局变量声明：`[var] [ty] name [= init]`
    /// - `var count = 0`：类型推断，必须有初值
    /// - `var int count = 0`：显式类型
    /// - `int count = 0`：无 var 关键字，显式类型
    /// - `Mutex mu`：无初始化表达式，C 静态存储期零初始化
    fn parse_module_var(&mut self, vis: Option<String>) -> PResult<N<Item>> {
        let t = self.tok(0).clone();
        let mut ty: Option<TPtr> = None;
        if self.at("var") {
            // 可选的 var 关键字，后可跟类型前缀：var int count = 0
            self.advance();
            let m = self.mark();
            match self.parse_type() {
                Ok(cand) => {
                    if self.at("IDENT") {
                        ty = Some(cand);
                    } else {
                        self.reset(m);
                    }
                }
                Err(_) => self.reset(m),
            }
        } else {
            // 无 var 关键字：必须是类型名 ty name [= init]
            ty = Some(self.parse_type()?);
        }
        let name = self.expect_ident("变量名")?;
        let init = if self.accept("=").is_some() {
            self.nl();
            Some(self.parse_expr()?)
        } else {
            None
        };
        Ok(self.nn(
            t,
            Item::VarDecl { vis, ty, name: name.text().to_string(), init },
        ))
    }

    fn parse_generics(&mut self) -> PResult<Vec<TypeParam>> {
        let mut gs = Vec::new();
        if !self.at("<") {
            return Ok(gs);
        }
        self.advance();
        self.nl();
        loop {
            self.nl();
            if self.at(">") {
                break;
            }
            let n = self.expect_ident("类型参数名")?;
            let mut cons = Vec::new();
            if self.accept(":").is_some() {
                self.nl();
                cons.push(self.expect_ident("约束（interface 名）")?.text().to_string());
                while self.accept("+").is_some() {
                    self.nl();
                    cons.push(self.expect_ident("约束（interface 名）")?.text().to_string());
                }
            }
            gs.push(TypeParam { tok: n.clone(), name: n.text().to_string(), constraints: cons });
            self.nl();
            if self.accept(",").is_none() {
                break;
            }
        }
        self.nl();
        self.expect_gt()?;
        Ok(gs)
    }

    /// 处理 `>>` 作为两个 `>`（嵌套泛型 map<string, Box<int>>）。
    fn expect_gt(&mut self) -> PResult<Tok> {
        if self.at(">") {
            return Ok(self.advance());
        }
        if self.at(">>") {
            let t = self.tok(0).clone();
            // 拆成两个 >
            let half = Tok {
                kind: ">".to_string(),
                val: TokVal::Text(">".to_string()),
                line: t.line,
                col: t.col,
                eline: t.line,
                ecol: t.col + 1,
                i0: t.i0,
                i1: t.i0 + 1,
            };
            let rest = Tok {
                kind: ">".to_string(),
                val: TokVal::Text(">".to_string()),
                line: t.line,
                col: t.col + 1,
                eline: t.eline,
                ecol: t.ecol,
                i0: t.i0 + 1,
                i1: t.i1,
            };
            self.toks[self.p] = half;
            self.toks.insert(self.p + 1, rest);
            return Ok(self.advance());
        }
        let t = self.tok(0).clone();
        self.fail("E2001", &t, "G-1", "期望 `>`，得到 `{0}`", &[&display_tok(&t)])
    }

    fn parse_params(&mut self) -> PResult<Vec<N<Param>>> {
        let mut ps = Vec::new();
        self.expect("(", None)?;
        self.nl();
        // 连续同类型参数语法糖（§15 项 8 方案 1）：`int a, b` 等价于 `int a, int b`
        // 旧写法 `int a, int b` 保持完全兼容。
        let mut prev_ty: Option<TPtr> = None;
        let mut prev_borrow: Option<String> = None;
        while !self.at(")") {
            self.nl();
            // 可变参数标记 `...`：须为最后一个参数，后面紧跟 `)`
            if self.at("...") {
                self.advance(); // 吃掉 "..."
                self.nl();
                if self.at(")") {
                    // 裸 `...`：不命名可变参数，用占位符类型 void + 名称 "args"
                    let tok = self.tok(0).clone();
                    let void_ty = Box::new(self.nn(tok.clone(), Ty::Named { path: vec!["void".to_string()], args: Vec::new() }));
                    ps.push(self.nn(
                        tok,
                        Param { borrow: None, ty: void_ty, name: "args".to_string(), varargs: true },
                    ));
                    break;
                }
                // `... int args`：带名可变参数（ax 扩展，实际调用时仍按 C ... 处理）
                let ty = self.parse_type()?;
                let name = self.expect_ident("可变参数名")?;
                ps.push(self.nn(
                    name.clone(),
                    Param { borrow: None, ty, name: name.text().to_string(), varargs: true },
                ));
                self.nl();
                break;
            }
            let (borrow, ty, name): (Option<String>, TPtr, Tok) = if self.at("ref") {
                // ref 参数：按引用传递，避免拷贝
                self.advance();
                let b = if self.at("const") {
                    self.advance();
                    Some("const".to_string())
                } else {
                    Some("mut".to_string())
                };
                self.nl();
                let t = self.parse_type()?;
                let n = self.expect_ident("参数名")?;
                (b, t, n)
            } else if self.at("&") {
                // 旧写法：带 borrow 修饰符 & 或 &const
                self.advance();
                let b = if self.at("const") {
                    self.advance();
                    Some("const".to_string())
                } else {
                    Some("mut".to_string())
                };
                self.nl();
                let t = self.parse_type()?;
                let n = self.expect_ident("参数名")?;
                (b, t, n)
            } else if self.at("IDENT") && matches!(
                self.tok(1).kind.as_str(),
                "," | ")" | "NL" | "EOF" | ";"
            ) {
                // 新写法：当前标识符是参数名，复用前一个参数的类型与 borrow
                // 判断依据：下一个 token 是参数分隔符（不是 `类型 名字` 的旧写法模式）
                let t = match prev_ty.clone() {
                    Some(t) => t,
                    None => return self.fail("E2001", &self.tok(0), "G-1", "期望参数类型", &[]),
                };
                let b = prev_borrow.clone();
                let n = self.advance();
                (b, t, n)
            } else {
                // 旧写法：类型 名字
                let t = self.parse_type()?;
                let n = self.expect_ident("参数名")?;
                (None, t, n)
            };
            prev_ty = Some(ty.clone());
            prev_borrow = borrow.clone();
            ps.push(self.nn(
                name.clone(),
                Param { borrow, ty, name: name.text().to_string(), varargs: false },
            ));
            self.nl();
            if self.accept(",").is_none() {
                break;
            }
            self.nl();
        }
        self.expect(")", None)?;
        Ok(ps)
    }

    fn parse_class(&mut self, vis: Option<String>, is_abstract: bool, is_final: bool) -> PResult<N<Item>> {
        let t = self.expect("class", None)?;
        let name = self.expect_ident("类名")?;
        let generics = self.parse_generics()?;
        let mut parent: Option<TPtr> = None;
        let mut impls: Vec<TPtr> = Vec::new();
        loop {
            self.nl();
            if self.at("extend") {
                self.advance();
                if parent.is_some() {
                    self.err("E3020", Some(&t), "文稿 6.2", "一个类只能 `extend` 一个父类", &[]);
                }
                parent = Some(self.parse_type()?);
                continue;
            }
            if self.at("implement") {
                self.advance();
                self.nl();
                impls.push(self.parse_type()?);
                while self.accept(",").is_some() {
                    self.nl();
                    impls.push(self.parse_type()?);
                }
                continue;
            }
            break;
        }
        self.nl();
        self.expect("{", None)?;
        let mut members = Vec::new();
        loop {
            self.nl();
            if self.at("}") || self.at("EOF") {
                break;
            }
            match self.parse_class_member() {
                Ok(Some(m)) => members.push(m),
                Ok(None) => {}
                Err(e) => {
                    self.report(&e);
                    let before = self.p;
                    self.sync();
                    if self.p == before {
                        // sync 未前进（如停在 `)`/`]`/`}` 上），强制跳过一个 token，避免死循环
                        self.advance();
                    }
                }
            }
        }
        self.expect("}", None)?;
        Ok(self.nn(
            t,
            Item::ClassDecl { vis, is_abstract, is_final, name: name.text().to_string(), generics, parent, impls, members },
        ))
    }

    fn parse_class_member(&mut self) -> PResult<Option<N<Member>>> {
        let mut vis: Option<String> = None;
        if self.at("public") || self.at("protected") || self.at("private") {
            vis = Some(self.advance().kind.clone());
        }
        let mut stat = false;
        let mut is_override = false;
        let mut is_abstract = false;
        let mut mod_tok: Option<Tok> = None;
        while self.at("static") || self.at("override") || self.at("abstract") {
            let mt = self.advance();
            mod_tok = Some(mt.clone());
            if mt.is("static") {
                stat = true;
            } else if mt.is("override") {
                is_override = true;
            } else {
                is_abstract = true;
            }
            if self.at("public") || self.at("protected") || self.at("private") {
                self.err("E3012", None, "文稿 6.1", "修饰符顺序错误：可见性应写在 `static`/`override`/`abstract` 之前", &[]);
                vis = Some(self.advance().kind.clone());
            }
        }
        if self.at("constructor") {
            let t = self.advance();
            if vis.is_some() {
                self.err("E3011", Some(&t), "N-11", "`constructor` 不能写可见性修饰词（默认 public）", &[]);
            }
            if stat || is_override {
                self.err("E3011", Some(&t), "N-11", "`constructor` 不能是 `static` 或 `override`", &[]);
            }
            // 可选的命名构造：constructor Name(params)。无名时 name = "constructor"。
            let name = if self.at(".") && self.tok(1).is("IDENT") {
                self.advance(); // 吃掉 "."
                self.advance().text().to_string() // 吃掉构造函数名
            } else {
                "constructor".to_string()
            };
            let params = self.parse_params()?;
            self.nl();
            let body = self.parse_block()?;
            return Ok(Some(self.nn(t, Member::Ctor { name, params, body })));
        }
        if self.at("class")
            || self.at("struct")
            || self.at("interface")
            || self.at("enum")
            || self.at("type")
        {
            // 外层已读取的可见性（public/protected/private）归嵌套类型使用，
            // 避免重复消耗导致 sub_vis 为 None。不写可见性时默认 private。
            let sub_vis = match vis.take() {
                Some(v) => Some(v),
                None => {
                    if self.at("public") || self.at("protected") || self.at("private") {
                        Some(self.advance().kind.clone())
                    } else {
                        Some("private".to_string())
                    }
                }
            };
            let it = if self.at("class") {
                self.parse_class(sub_vis, false, false)?
            } else if self.at("struct") {
                self.parse_struct(sub_vis)?
            } else if self.at("interface") {
                self.parse_interface(sub_vis)?
            } else if self.at("enum") {
                self.parse_enum(sub_vis)?
            } else {
                self.parse_type_alias(sub_vis)?
            };
            return Ok(Some(self.nn(it.tok.clone(), Member::Item(Box::new(it)))));
        }
        let ty = self.parse_type()?;
        let name = self.expect_ident("成员名")?;
        // 类成员不写可见性修饰词时 vis 保持 None，由 sema 阶段报 E3037；
        // codegen 中该字段被忽略（字段访问可见性由 check_field_vis 统一处理）。
        if self.at("(") {
            let params = self.parse_params()?;
            self.nl();
            let body = if is_abstract {
                // 抽象方法无方法体，以 `;`、`}` 或换行结束（接口/抽象类方法）
                if self.at(";") {
                    self.advance();
                } else if !self.at("}") {
                    self.nl();
                }
                Block { tok: name.clone(), stmts: Vec::new() }
            } else {
                self.parse_block()?
            };
            if is_abstract && stat {
                let t = mod_tok.unwrap_or_else(|| name.clone());
                self.err("E3049", Some(&t), "文稿 6.1", "抽象方法不能是 `static`", &[]);
            }
            return Ok(Some(self.nn(
                name.clone(),
                Member::Method {
                    vis,
                    is_static: stat,
                    is_override,
                    is_abstract,
                    ret: ty,
                    name: name.text().to_string(),
                    params,
                    body,
                },
            )));
        }
        if stat || is_override {
            let t = mod_tok.unwrap_or_else(|| name.clone());
            self.err("E3036", Some(&t), "文稿 6.1", "`static`/`override` 不能用于字段", &[]);
        }
        if self.at("=") {
            self.err("E3007", None, "文稿 3.3", "字段不能有初值（在 constructor 里赋值）", &[]);
            self.advance();
            self.nl();
            self.parse_expr()?;
        }
        self.expect_term();
        Ok(Some(self.nn(
            name.clone(),
            Member::Field { vis, ty, name: name.text().to_string() },
        )))
    }

    fn parse_struct(&mut self, vis: Option<String>) -> PResult<N<Item>> {
        let t = self.expect("struct", None)?;
        let name = self.expect_ident("struct 名")?;
        let generics = self.parse_generics()?;
        self.nl();
        if self.at("extend") || self.at("implement") {
            self.err("E3021", None, "文稿 6.5", "struct 不能 `extend` 或 `implement`", &[]);
            self.advance();
            self.parse_type()?;
        }
        self.expect("{", None)?;
        let mut members = Vec::new();
        loop {
            self.nl();
            if self.at("}") || self.at("EOF") {
                break;
            }
            match self.parse_struct_member() {
                Ok(Some(m)) => members.push(m),
                Ok(None) => {}
                Err(e) => {
                    self.report(&e);
                    self.sync();
                }
            }
        }
        self.expect("}", None)?;
        Ok(self.nn(
            t,
            Item::StructDecl { vis, name: name.text().to_string(), generics, members },
        ))
    }

    fn parse_struct_member(&mut self) -> PResult<Option<N<Member>>> {
        if self.at("public") || self.at("protected") {
            let bad = self.advance();
            self.err("E3011", Some(&bad), "N-10", "struct 成员不能写可见性修饰词（一律 public）", &[]);
        }
        if self.at("constructor") {
            let ct = self.advance();
            // 可选的命名构造：constructor Name(params)。无名时 name = "constructor"。
            let name = if self.at(".") && self.tok(1).is("IDENT") {
                self.advance(); // 吃掉 "."
                self.advance().text().to_string() // 吃掉构造函数名
            } else {
                "constructor".to_string()
            };
            let params = self.parse_params()?;
            self.nl();
            let body = self.parse_block()?;
            return Ok(Some(self.nn(ct, Member::Ctor { name, params, body })));
        }
        if self.at("class")
            || self.at("struct")
            || self.at("interface")
            || self.at("enum")
            || self.at("type")
        {
            let it = if self.at("class") {
                self.parse_class(None, false, false)?
            } else if self.at("struct") {
                self.parse_struct(None)?
            } else if self.at("interface") {
                self.parse_interface(None)?
            } else if self.at("enum") {
                self.parse_enum(None)?
            } else {
                self.parse_type_alias(None)?
            };
            return Ok(Some(self.nn(it.tok.clone(), Member::Item(Box::new(it)))));
        }
        let ty = self.parse_type()?;
        let nm = self.expect_ident("成员名")?;
        if self.at("(") {
            let params = self.parse_params()?;
            self.nl();
            let body = self.parse_block()?;
            return Ok(Some(self.nn(
                nm.clone(),
                Member::Method {
                    vis: None,
                    is_static: false,
                    is_override: false,
                    is_abstract: false,
                    ret: ty,
                    name: nm.text().to_string(),
                    params,
                    body,
                },
            )));
        }
        if self.at("=") {
            self.err("E3007", None, "文稿 3.3", "字段不能有初值（在 constructor 里赋值）", &[]);
            self.advance();
            self.nl();
            self.parse_expr()?;
        }
        self.expect_term();
        Ok(Some(self.nn(
            nm.clone(),
            Member::Field { vis: None, ty, name: nm.text().to_string() },
        )))
    }

    fn parse_interface(&mut self, vis: Option<String>) -> PResult<N<Item>> {
        let t = self.expect("interface", None)?;
        let name = self.expect_ident("interface 名")?;
        let mut extends = Vec::new();
        self.nl();
        if self.at("extend") {
            self.advance();
            self.nl();
            extends.push(self.parse_type()?);
            while self.accept(",").is_some() {
                self.nl();
                extends.push(self.parse_type()?);
            }
        }
        self.nl();
        self.expect("{", None)?;
        let mut methods = Vec::new();
        loop {
            self.nl();
            if self.at("}") || self.at("EOF") {
                break;
            }
            match self.parse_interface_member() {
                Ok(Some(m)) => methods.push(m),
                Ok(None) => {}
                Err(e) => {
                    self.report(&e);
                    self.sync();
                }
            }
        }
        self.expect("}", None)?;
        Ok(self.nn(
            t,
            Item::InterfaceDecl { vis, name: name.text().to_string(), extends, methods },
        ))
    }

    fn parse_interface_member(&mut self) -> PResult<Option<N<MethodSig>>> {
        if self.at("public") || self.at("protected") {
            let bad = self.advance();
            self.err("E3011", Some(&bad), "N-10", "interface 成员不能写可见性修饰词", &[]);
        }
        if self.at("constructor") {
            let bad = self.advance();
            self.err("E3032", Some(&bad), "文稿 6.3", "interface 不能有 constructor", &[]);
            self.parse_params()?;
            return Ok(None);
        }
        let ty = self.parse_type()?;
        let nm = self.expect_ident("方法名")?;
        let params = self.parse_params()?;
        let mut body = None;
        self.nl();
        if self.at("{") {
            body = Some(self.parse_block()?);
        }
        self.expect_term();
        Ok(Some(self.nn(
            nm.clone(),
            MethodSig { ret: ty, name: nm.text().to_string(), params, default_body: body },
        )))
    }

    fn parse_enum(&mut self, vis: Option<String>) -> PResult<N<Item>> {
        let t = self.expect("enum", None)?;
        let name = self.expect_ident("enum 名")?;
        self.nl();
        self.expect("{", None)?;
        let mut variants = Vec::new();
        loop {
            self.nl();
            if self.at("}") {
                break;
            }
            let v = self.expect_ident("枚举成员")?;
            if self.at("=") || self.at("(") {
                self.err("E3031", None, "文稿 6.4", "enum 成员不能带初值或载荷", &[]);
                if self.accept("=").is_some() {
                    self.parse_expr()?;
                } else if self.accept("(").is_some() {
                    while !self.at(")") && !self.at("EOF") {
                        self.advance();
                    }
                    self.expect(")", None)?;
                }
            }
            variants.push(v.text().to_string());
            self.nl();
            if self.accept(",").is_none() {
                break;
            }
        }
        self.nl();
        self.expect("}", None)?;
        Ok(self.nn(
            t,
            Item::EnumDecl { vis, name: name.text().to_string(), variants },
        ))
    }

    fn parse_type_alias(&mut self, vis: Option<String>) -> PResult<N<Item>> {
        let t = self.expect("type", None)?;
        let name = self.expect_ident("类型名")?;
        let generics = self.parse_generics()?;
        self.expect("=", None)?;
        self.nl();
        let target = self.parse_type()?;
        Ok(self.nn(
            t,
            Item::TypeAlias {
                vis,
                name: name.text().to_string(),
                generics,
                target,
            },
        ))
    }

    /// `extern "C" { int puts(string s); ... }`：外部 C 函数声明块。
    fn parse_extern_block(&mut self, _vis: Option<String>) -> PResult<N<Item>> {
        let t = self.expect("extern", None)?;
        self.expect_string_lit("C", None)?;
        self.nl();
        self.expect("{", None)?;
        self.nl();
        let mut funcs: Vec<N<Item>> = Vec::new();
        while !self.at("}") && !self.at("EOF") {
            let ret = self.parse_type()?;
            let name = self.expect_ident("函数名")?;
            let generics = self.parse_generics()?;
            if !generics.is_empty() {
                self.err("E3033", Some(&name), "文稿 15.1", "`extern` 函数不支持泛型", &[]);
            }
            let params = self.parse_params()?;
            self.nl();
            if self.at("{") {
                let tk = self.tok(0).clone();
                self.err("E3046", Some(&tk), "文稿 15.1", "`extern` 函数不能有函数体，只能声明", &[]);
                // 跳过函数体以便同步
                let _ = self.parse_block();
            }
            funcs.push(self.nn(
                name.clone(),
                Item::FuncDecl {
                    implicit: false,
                    vis: None,
                    ret,
                    name: name.text().to_string(),
                    generics,
                    params,
                    body: Block { tok: name.clone(), stmts: Vec::new() },
                    attrs: Vec::new(),
                    is_pure: false,
                    is_nothrow: false,
                    is_ref_return: false,
                    is_export: false,
                },
            ));
            self.nl();
            if self.accept(";").is_none() {
                let tk = self.tok(0).clone();
                self.err("E3047", Some(&tk), "文稿 15.1", "`extern` 函数声明后缺少 `;`", &[]);
            }
            self.nl();
        }
        self.expect("}", None)?;
        Ok(self.nn(
            t,
            Item::ExternBlock { funcs },
        ))
    }

    // ---------------- 类型 ----------------

    fn parse_type(&mut self) -> PResult<TPtr> {
        let mut t = self.parse_prim_type()?;
        loop {
            if self.at("?") {
                let tok = t.tok.clone();
                self.advance();
                t = self.ne(tok, Ty::Nullable { inner: t })?;
            } else if self.at("[") {
                let tok = t.tok.clone();
                self.advance();
                self.nl();
                if self.at("]") {
                    self.advance();
                    t = self.ne(tok, Ty::Array { elem: t, len: None })?;
                } else {
                    let e = self.parse_expr()?;
                    self.nl();
                    self.expect("]", None)?;
                    t = self.ne(tok, Ty::Array { elem: t, len: Some(e) })?;
                }
            } else {
                break;
            }
        }
        Ok(t)
    }

    fn parse_prim_type(&mut self) -> PResult<TPtr> {
        // `func` 在词法上是 IDENT（保留类型名表），这里按文本识别——
        // 语法：`func(参数类型...) 返回类型`（文稿 4.1）
        let at_func = self.at("func") || (self.at("IDENT") && self.tok(0).text() == "func");
        if at_func {
            let t = self.advance();
            self.expect("(", None)?;
            self.nl();
            let mut ps = Vec::new();
            while !self.at(")") {
                self.nl();
                ps.push(self.parse_type()?);
                self.nl();
                if self.accept(",").is_none() {
                    break;
                }
                self.nl();
            }
            self.expect(")", None)?;
            self.nl();
            let ret = self.parse_type()?;
            return self.ne(t, Ty::Func { params: ps, ret });
        }
        if self.at("(") {
            let t = self.advance();
            self.nl();
            let first = self.parse_type()?;
            self.nl();
            if self.at(",") {
                let mut elems = vec![first];
                while self.accept(",").is_some() {
                    self.nl();
                    if self.at(")") {
                        break;
                    }
                    elems.push(self.parse_type()?);
                    self.nl();
                }
                self.expect(")", None)?;
                return self.ne(t, Ty::Tuple { elems });
            }
            self.expect(")", None)?;
            return Ok(first);
        }
        let t = self.tok(0).clone();
        let mut path = vec![self.expect_ident("类型名")?.text().to_string()];
        while self.at(".") && self.tok(1).is("IDENT") {
            self.advance();
            path.push(self.advance().text().to_string());
        }
        let mut args = Vec::new();
        if self.at("<") {
            args = self.parse_type_args()?;
        }
        self.ne(t.clone(), Ty::Named { path, args })
    }

    fn parse_type_args(&mut self) -> PResult<Vec<TPtr>> {
        self.expect("<", None)?;
        self.nl();
        let mut args = Vec::new();
        while !self.at(">") {
            self.nl();
            args.push(self.parse_type()?);
            self.nl();
            if self.accept(",").is_none() {
                break;
            }
        }
        self.nl();
        self.expect_gt()?;
        Ok(args)
    }

    // ---------------- 语句 ----------------

    fn parse_block(&mut self) -> PResult<Block> {
        let t = self.expect("{", None)?;
        let mut stmts: Vec<SPtr> = Vec::new();
        loop {
            self.nl();
            if self.at("}") || self.at("EOF") {
                break;
            }
            // G-5：上一行已是完整语句，本行却以运算符开头 → E2003
            if LINE_START_ILLEGAL.contains(&self.tok(0).kind.as_str()) {
                let tk = self.tok(0).clone();
                let msg = crate::diag_msg::tr(
                    self.d.lang,
                    "E2003",
                    "行首不能是运算符 `{0}`（上一行已是完整语句；续行请把运算符留在行末）",
                    &[&tk.kind],
                );
                self.d.error(
                    "E2003",
                    msg,
                    &self.path,
                    tk.line,
                    tk.col,
                    tk.eline,
                    tk.ecol,
                    "G-5",
                );
                self.errors += 1;
                while !(self.at("NL") || self.at("EOF")) {
                    self.advance();
                }
                continue;
            }
            match self.parse_stmt() {
                // 历史行为（测试钉死）：expect_term 在 try 块内，出错恢复后不执行
                Ok(Some(s)) => {
                    stmts.push(Box::new(s));
                    self.expect_term();
                }
                Ok(None) => {
                    // 语句被同步跳过（如不支持的 switch 已报 E2010 并跳过整个块），
                    // 不再 expect_term，避免把下一条语句的起始 token 误报为 E2002。
                    self.nl();
                }
                Err(e) => {
                    self.report(&e);
                    self.sync();
                }
            }
            if self.errors >= MAX_PARSE_ERRORS {
                break;
            }
        }
        self.expect("}", None)?;
        Ok(Block { tok: t, stmts })
    }

    fn parse_stmt(&mut self) -> PResult<Option<N<Stmt>>> {
        let t = self.tok(0).clone();
        let k = t.kind.clone();
        if k == "{" {
            let b = self.parse_block()?;
            return Ok(Some(self.nn(t, Stmt::Block(b))));
        }
        if k == "if" {
            let n = self.parse_if()?;
            return Ok(Some(self.nn(t, Stmt::If(Box::new(n)))));
        }
        if k == "match" {
            let n = self.parse_match()?;
            return Ok(Some(self.nn(t, Stmt::Match(Box::new(n)))));
        }
        if k == "switch" {
            let n = self.parse_switch()?;
            return Ok(Some(self.nn(t, Stmt::Switch(Box::new(n)))));
        }
        if k == "for" {
            let f = self.parse_for(None)?;
            return Ok(Some(self.nn(t, f.k)));
        }
        if k == "while" {
            let w = self.parse_while()?;
            return Ok(Some(self.nn(t, w.k)));
        }
        if k == "unsafe" {
            self.advance();
            self.nl();
            let body = self.parse_block()?;
            return Ok(Some(self.nn(t, Stmt::Unsafe { body })));
        }
        if k == "try" {
            let s = self.parse_try_catch(&t)?;
            return Ok(Some(self.nn(t, s)));
        }
        if k == "return" {
            self.advance();
            self.nl();
            if self.at("NL") || self.at(";") || self.at("}") || self.at("EOF") {
                return Ok(Some(self.nn(t, Stmt::Return { value: None })));
            }
            if self.at("throw") {
                let tk = self.tok(0).clone();
                return self.fail("E2001", &tk, "G-15", "`throw` 本身是语句，不能作为 return 的表达式", &[])
                .map(Some);
            }
            let v = self.parse_expr()?;
            return Ok(Some(self.nn(t, Stmt::Return { value: Some(v) })));
        }
        if k == "throw" {
            self.advance();
            self.nl();
            let (variant, msg) = self.parse_error_ctor_parts()?;
            return Ok(Some(self.nn(t, Stmt::Throw { variant, msg })));
        }
        if k == "defer" {
            self.advance();
            self.nl();
            let call = self.parse_expr()?;
            return Ok(Some(self.nn(t, Stmt::Defer { call })));
        }
        if k == "break" {
            self.advance();
            let mut label = None;
            if self.at("IDENT") {
                label = Some(self.advance().text().to_string());
            }
            return Ok(Some(self.nn(t, Stmt::Break { label })));
        }
        if k == "continue" {
            self.advance();
            let mut label = None;
            if self.at("IDENT") {
                label = Some(self.advance().text().to_string());
            }
            return Ok(Some(self.nn(t, Stmt::Continue { label })));
        }
        if k == "var" {
            self.advance();
            if self.at("(") {
                self.advance();
                let mut names = vec![self.expect_ident("变量名")?.text().to_string()];
                while self.accept(",").is_some() {
                    names.push(self.expect_ident("变量名")?.text().to_string());
                }
                self.expect(")", None)?;
                self.expect("=", None)?;
                self.nl();
                let init = self.parse_expr()?;
                return Ok(Some(self.nn(t, Stmt::VarTuple { names, init })));
            }
            let name = self.expect_ident("变量名")?;
            if !self.at("=") {
                self.err("E3002", Some(&name), "文稿 3.3", "变量声明必须有初值", &[]);
            }
            self.expect("=", None)?;
            self.nl();
            let init = self.parse_expr()?;
            return Ok(Some(self.nn(
                name.clone(),
                Stmt::VarDecl {
                    explicit_ty: None,
                    is_var: true,
                    name: name.text().to_string(),
                    init: Some(init),
                },
            )));
        }
        if k == "const" {
            return Ok(Some(self.parse_const_stmt()?));
        }
        // 标签：IDENT ':' for
        if k == "IDENT" && self.tok(1).is(":") && self.tok(2).is("for") {
            let label = self.advance().text().to_string();
            self.advance();
            let f = self.parse_for(Some(label))?;
            return Ok(Some(self.nn(t, f.k)));
        }
        // 显式类型声明（投机）。一旦吃到 `=`，就已是声明而非"以类型名开头的表达式"，
        // 此后的语法错误必须保留（否则 `int n = g() try.len()` 会丢掉 E2009）
        let m = self.mark();
        match self.parse_type() {
            Ok(ty) => {
                if self.at("IDENT") {
                    let name = self.advance();
                    if self.at("=") {
                        // 已提交为声明：此后任何解析错误都必须向上传播
                        self.advance();
                        self.nl();
                        let init = self.parse_expr()?;
                        return Ok(Some(self.nn(
                            name.clone(),
                            Stmt::VarDecl {
                                explicit_ty: Some(ty),
                                is_var: false,
                                name: name.text().to_string(),
                                init: Some(init),
                            },
                        )));
                    }
                    if self.at("NL") || self.at(";") || self.at("}") || self.at("EOF") {
                        self.err("E3002", Some(&name), "文稿 3.3", "变量声明必须有初值", &[]);
                        return Ok(Some(self.nn(
                            name.clone(),
                            Stmt::VarDecl {
                                explicit_ty: Some(ty),
                                is_var: false,
                                name: name.text().to_string(),
                                init: None,
                            },
                        )));
                    }
                }
                self.reset(m);
            }
            Err(_) => self.reset(m),
        }
        // 表达式语句 / 赋值
        self.allow_incdec = true;
        let lhs = self.parse_expr();
        self.allow_incdec = false;
        let lhs = lhs?;
        if self.at("=")
            || self.at("+=")
            || self.at("-=")
            || self.at("*=")
            || self.at("/=")
            || self.at("%=")
        {
            let op = self.advance().kind.clone();
            self.nl();
            let rhs = self.parse_expr()?;
            // G-19：链式赋值 a = b = c，a = (b = c) 中 b = c 用作表达式，应报 E4001
            if self.at("=") || self.at("+=") || self.at("-=") || self.at("*=") || self.at("/=") || self.at("%=") {
                let tk = self.tok(0).clone();
                self.err("E4001", Some(&tk), "文稿 3.3", "赋值不能作为表达式（不支持链式赋值 `a = b = c`）", &[]);
                self.advance();
                self.nl();
                let _ = self.parse_expr();
            }
            return Ok(Some(self.nn(
                t,
                Stmt::Simple { lhs, op: Some(op), rhs: Some(rhs), incdec: None },
            )));
        }
        if self.at("++") || self.at("--") {
            let op = self.advance().kind.clone();
            return Ok(Some(self.nn(
                t,
                Stmt::Simple { lhs, op: None, rhs: None, incdec: Some(op) },
            )));
        }
        Ok(Some(self.nn(t, Stmt::Simple { lhs, op: None, rhs: None, incdec: None })))
    }

    /// 语句级 `const`：与模块级 ConstDecl 同构（sema 走 check_local_const）。
    fn parse_const_stmt(&mut self) -> PResult<N<Stmt>> {
        let t = self.expect("const", None)?;
        let mut ty: Option<TPtr> = None;
        let m = self.mark();
        match self.parse_type() {
            Ok(cand) => {
                if self.at("IDENT") {
                    ty = Some(cand);
                } else {
                    self.reset(m);
                }
            }
            Err(_) => self.reset(m),
        }
        let name = self.expect_ident("常量名")?;
        self.expect("=", None)?;
        self.nl();
        let init = self.parse_expr()?;
        Ok(self.nn(
            t,
            Stmt::Const { vis: None, ty, name: name.text().to_string(), init },
        ))
    }

    fn parse_branch(&mut self, what: &str) -> PResult<Block> {
        if self.at("{") {
            return self.parse_block();
        }
        // 支持 C 系习惯：单条语句作为分支体，不需要 {}
        let stmt = self.parse_stmt()?;
        match stmt {
            Some(s) => {
                let tok = s.tok.clone();
                Ok(Block { tok, stmts: vec![Box::new(s)] })
            }
            None => {
                let t = self.tok(0).clone();
                self.fail("E2004", &t, "G-1", "{0} 必须用 `{{}}` 括起来（即使只有一条语句）", &[&what])
            }
        }
    }

    fn parse_if(&mut self) -> PResult<IfNode> {
        let t = self.expect("if", None)?;
        self.nl();
        let cond = self.parse_expr()?;
        self.nl();
        // G-18：if 条件中不能出现赋值表达式（if a = b），应报 E4001 而非 E2004
        if self.at("=") || self.at("+=") || self.at("-=") || self.at("*=") || self.at("/=") || self.at("%=") {
            let tk = self.tok(0).clone();
            self.err("E4001", Some(&tk), "文稿 3.3", "赋值不能作为表达式（if 条件中不能写 `a = b`，请改为 `a == b`）", &[]);
            // 跳过赋值右侧表达式，直到 { 或 行尾，避免后续 parse_branch 误报 E2004
            while !self.at("{") && !self.at("NL") && !self.at("EOF") {
                self.advance();
            }
        }
        self.nl();
        let then_ = self.parse_branch("if 的分支体")?;
        let mut els = None;
        let save = self.p;
        self.nl();
        if self.at("else") {
            self.advance();
            self.nl();
            if self.at("if") {
                let inner = self.parse_if()?;
                let itok = self.tok(0).clone();
                els = Some(IfTail::If(Box::new(self.nn(itok, inner))));
            } else {
                els = Some(IfTail::Block(self.parse_branch("else 的分支体")?));
            }
        } else {
            self.p = save;
        }
        let _ = t;
        Ok(IfNode { cond, then_, els })
    }

    fn parse_while(&mut self) -> PResult<N<Stmt>> {
        let t = self.expect("while", None)?;
        self.nl();
        let cond = self.parse_expr()?;
        self.nl();
        let body = self.parse_branch("while 的分支体")?;
        Ok(self.nn(
            t,
            Stmt::While { cond, body },
        ))
    }

    fn parse_for(&mut self, label: Option<String>) -> PResult<N<Stmt>> {
        let t = self.expect("for", None)?;
        self.nl();
        if self.at("{") {
            let body = self.parse_block()?;
            return Ok(self.nn(
                t,
                Stmt::For { label, kind: ForKind::Cond, body },
            ));
        }
        if self.at("(") {
            self.advance();
            let v1 = self.expect_ident("循环变量")?.text().to_string();
            self.expect(",", None)?;
            let v2 = self.expect_ident("循环变量")?.text().to_string();
            self.expect(")", None)?;
            self.expect("in", None)?;
            self.nl();
            let it = self.parse_expr()?;
            self.nl();
            let body = self.parse_block()?;
            return Ok(self.nn(
                t,
                Stmt::For { label, kind: ForKind::Kv { var: v1, var2: v2, iterable: it }, body },
            ));
        }
        let v = self.expect_ident("循环变量")?.text().to_string();
        self.expect("in", None)?;
        self.nl();
        let it = self.parse_expr()?;
        self.nl();
        let body = self.parse_block()?;
        let kind = match &it.k {
            Expr::Range { lo, hi, inclusive } => ForKind::Range {
                var: v,
                lo: lo.clone(),
                hi: hi.clone(),
                inclusive: *inclusive,
            },
            Expr::MethodCall { recv, name, args } if name == "Step" => {
                if let Expr::Range { lo, hi, inclusive } = &recv.k {
                    if args.len() != 1 {
                        let tk = args.first().map_or_else(
                            || recv.tok.clone(),
                            |a| a.k.expr.tok.clone(),
                        );
                        return self.fail(
                            "E4031",
                            &tk,
                            "文稿 5.2",
                            "`Step` 需要 1 个实参，得到 {0}",
                            &[&args.len().to_string()],
                        );
                    }
                    let step = args[0].k.expr.clone();
                    ForKind::Step {
                        var: v,
                        lo: lo.clone(),
                        hi: hi.clone(),
                        inclusive: *inclusive,
                        step,
                    }
                } else {
                    ForKind::Iter { var: v, iterable: it }
                }
            }
            _ => ForKind::Iter { var: v, iterable: it },
        };
        Ok(self.nn(t, Stmt::For { label, kind, body }))
    }

    fn parse_match(&mut self) -> PResult<MatchNode> {
        let t = self.expect("match", None)?;
        self.nl();
        let scrut = self.parse_expr()?;
        self.nl();
        self.expect("{", None)?;
        let mut arms = Vec::new();
        loop {
            self.nl();
            if self.at("}") {
                break;
            }
            let pat = self.parse_pattern()?;
            let ptok = pat.tok.clone();
            self.nl();
            self.expect("=>", None)?;
            self.nl();
            let body = self.parse_expr()?;
            arms.push(Arm { tok: ptok.clone(), pat, body });
            self.nl();
            if self.accept(",").is_none() {
                break;
            }
        }
        self.nl();
        self.expect("}", None)?;
        let _ = t;
        Ok(MatchNode { scrutinee: scrut, arms })
    }

    fn parse_switch(&mut self) -> PResult<SwitchNode> {
        let t = self.expect("switch", None)?;
        self.nl();

        // 判断是否有表达式：
        //   `switch (` 或 `switch {` —— 原有行为（有表达式带括号 / expressionless）
        //   `switch expr {` —— Go 风格：有表达式但省略括号
        let scrutinee = if self.at("(") {
            self.advance();
            self.nl();
            let s = self.parse_expr()?;
            self.nl();
            self.expect(")", None)?;
            Some(s)
        } else if self.at("{") {
            None
        } else {
            let s = self.parse_expr()?;
            self.nl();
            Some(s)
        };

        self.nl();
        self.expect("{", None)?;
        let mut arms = Vec::new();
        loop {
            self.nl();
            if self.at("}") {
                break;
            }
            // default 分支
            if self.at("default") {
                let dtk = self.advance();
                self.nl();
                self.expect(":", None)?;
                self.nl();
                let mut body = self.parse_block()?;
                let mut fallthrough = false;
                if let Some(last) = body.stmts.last() {
                    if let Stmt::Simple { lhs, op: None, rhs: None, incdec: None } = &last.k {
                        if let Expr::Path { segs, .. } = &lhs.k {
                            if segs.len() == 1 && segs[0] == "fallthrough" {
                                fallthrough = true;
                                body.stmts.pop();
                            }
                        }
                    }
                }
                arms.push(SwitchArm { tok: dtk, pats: Vec::new(), body, fallthrough });
                self.nl();
                continue;
            }
            // case 分支：case v1, v2, ...: （值 switch，每个值可为任意表达式）
            //       或 case T [x]: （类型 switch，T 为类型名，可选绑定名 x）
            let ctk = self.expect("case", None)?;
            self.nl();

            let mut pats: Vec<N<Pat>> = Vec::new();
            loop {
                // 类型 switch：case 后跟 RESERVED_TYPES 中的类型名
                if self.at("IDENT") && RESERVED_TYPES.contains(&self.tok(0).text()) {
                    let ty = self.parse_type()?;
                    // 可选绑定名：case string x: 或 case string x,
                    let bind = if self.at("IDENT")
                        && (self.tok(1).is(":") || self.tok(1).is(","))
                    {
                        Some(self.advance().text().to_string())
                    } else {
                        None
                    };
                    pats.push(self.nn(ctk.clone(), Pat::Type { ty, bind: bind.unwrap_or_default() }));
                } else {
                    // 值 switch：case 后接受任意表达式
                    let expr = self.parse_expr()?;
                    pats.push(self.nn(ctk.clone(), Pat::Lit { expr }));
                }
                self.nl();
                if self.accept(",").is_none() {
                    break;
                }
                self.nl();
            }

            self.nl();
            self.expect(":", None)?;
            self.nl();
            let mut body = self.parse_block()?;

            // 检查 fallthrough：分支体最后一条语句若为 `fallthrough` 标识符，则移除并标记
            let mut fallthrough = false;
            if let Some(last) = body.stmts.last() {
                if let Stmt::Simple { lhs, op: None, rhs: None, incdec: None } = &last.k {
                    if let Expr::Path { segs, .. } = &lhs.k {
                        if segs.len() == 1 && segs[0] == "fallthrough" {
                            fallthrough = true;
                            body.stmts.pop();
                        }
                    }
                }
            }

            arms.push(SwitchArm { tok: ctk, pats, body, fallthrough });
            self.nl();
        }
        self.nl();
        self.expect("}", None)?;
        let _ = t;
        Ok(SwitchNode { scrutinee, arms })
    }

    fn parse_pattern(&mut self) -> PResult<N<Pat>> {
        let t = self.tok(0).clone();
        let first = self.parse_single_pattern()?;
        // 多值替代：`1,3,6 =>` 表示任意一个匹配即可
        if self.accept(",").is_some() {
            let mut pats = vec![first];
            loop {
                self.nl();
                if self.at("=>") || self.at("}") || self.at("EOF") {
                    break;
                }
                pats.push(self.parse_single_pattern()?);
                if self.accept(",").is_none() {
                    break;
                }
            }
            return Ok(self.nn(t, Pat::Multi { pats }));
        }
        Ok(first)
    }

    /// 解析单个模式（不含逗号分隔的多值）
    fn parse_single_pattern(&mut self) -> PResult<N<Pat>> {
        let t = self.tok(0).clone();
        if self.at("_") {
            self.advance();
            return Ok(self.nn(t, Pat::Wild));
        }
        if t.is("INT") || t.is("FLOAT") || t.is("DEC") || t.is("STR")
            || t.is("true") || t.is("false") || t.is("null")
        {
            let lo = self.parse_primary()?;
            if self.at("..") || self.at("..=") {
                let incl = self.advance().is("..=");
                self.nl();
                let hi = self.parse_primary()?;
                return Ok(self.nn(t, Pat::Range { lo, hi, inclusive: incl }));
            }
            return Ok(self.nn(t, Pat::Lit { expr: lo }));
        }
        if t.is("IDENT") {
            // ok(x) / fail(x)
            if (t.text() == "ok" || t.text() == "fail") && self.tok(1).is("(") {
                self.advance();
                self.advance();
                let bind = self.expect_ident("绑定名")?.text().to_string();
                self.expect(")", None)?;
                let is_ok = t.text() == "ok";
                return Ok(self.nn(t, Pat::Result { is_ok, bind }));
            }
            // 枚举成员：Path '.' ident
            if self.tok(1).is(".") {
                let save = self.p;
                let mut path = vec![self.advance().text().to_string()];
                while self.at(".") && self.tok(1).is("IDENT") {
                    self.advance();
                    path.push(self.advance().text().to_string());
                }
                if path.len() >= 2 {
                    let variant = path.pop().unwrap();
                    return Ok(self.nn(t, Pat::Enum { ty: path, variant }));
                }
                self.p = save;
            }
            // 类型模式 vs 绑定（G-11）
            let m = self.mark();
            match self.parse_type() {
                Ok(ty) => {
                    if self.at("IDENT") {
                        let bind = self.advance().text().to_string();
                        return Ok(self.nn(t, Pat::Type { ty, bind }));
                    }
                    self.reset(m);
                }
                Err(_) => self.reset(m),
            }
            let name = self.expect_ident("模式")?;
            return Ok(self.nn(t, Pat::Bind { name: name.text().to_string() }));
        }
        self.fail("E2001", &t, "G-1", "非法的 match 模式", &[])
    }

    /// `try { } catch (error[.Variant] e) { } ... finally { }`
    fn parse_try_catch(&mut self, try_tok: &Tok) -> PResult<Stmt> {
        self.advance(); // try
        self.nl();
        let body = self.parse_block()?;
        let mut catches: Vec<crate::frontend::ast::CatchArm> = Vec::new();
        let mut finally: Option<Block> = None;
        loop {
            // 先记住位置：没有 catch/finally 时要把行尾换行留给 expect_term，
            // 否则上一条语句会被判成"未以换行或 `;` 结束"。
            let save = self.p;
            self.nl();
            if self.at("catch") {
                self.advance();
                self.nl();
                self.expect("(", None)?;
                self.nl();
                let e = self.tok(0).clone();
                if !(self.at("IDENT") && e.text() == "error") {
                    return self.fail("E2001", &e, "文稿 10.2", "`catch` 的捕获类型必须是 `error` 或 `error.变体`", &[]);
                }
                let mut variant_tok = e.clone();
                self.advance();
                self.nl();
                let mut variant = None;
                if self.at(".") {
                    self.advance();
                    self.nl();
                    let vt = self.expect_ident("错误变体")?;
                    variant = Some(vt.text().to_string());
                    variant_tok = vt;
                    self.nl();
                }
                let name_tok = self.expect_ident("catch 变量名")?;
                let name = name_tok.text().to_string();
                self.nl();
                self.expect(")", None)?;
                self.nl();
                let b = self.parse_block()?;
                catches.push(crate::frontend::ast::CatchArm {
                    variant,
                    variant_tok,
                    name,
                    name_tok,
                    body: b,
                });
                continue;
            }
            if self.at("finally") {
                self.advance();
                self.nl();
                finally = Some(self.parse_block()?);
            } else {
                self.p = save;
            }
            break;
        }
        if catches.is_empty() && finally.is_none() {
            return self.fail("E2001", try_tok, "文稿 10.2", "`try` 之后必须跟 `catch` 或 `finally`", &[]);
        }
        Ok(Stmt::TryCatch(Box::new(crate::frontend::ast::TryCatchNode {
            body,
            catches,
            finally,
        })))
    }

    fn parse_error_ctor_parts(&mut self) -> PResult<(String, EPtr)> {
        let t = self.tok(0).clone();
        if !(self.at("IDENT") && t.text() == "error") {
            return self.fail("E2001", &t, "文稿 10.1", "`throw` 后必须是 `error.变体(消息)`", &[]);
        }
        self.advance();
        self.expect(".", None)?;
        let variant = self.expect_ident("错误变体")?.text().to_string();
        self.expect("(", None)?;
        self.nl();
        let msg = self.parse_expr()?;
        self.nl();
        self.expect(")", None)?;
        Ok((variant, msg))
    }

    // ---------------- 表达式 ----------------

    fn parse_expr(&mut self) -> PResult<EPtr> {
        self.parse_range()
    }

    fn parse_range(&mut self) -> PResult<EPtr> {
        let lo = self.parse_ternary()?;
        if self.at("..") || self.at("..=") {
            let incl = self.advance().is("..=");
            self.nl();
            let hi = self.parse_ternary()?;
            let tok = lo.tok.clone();
            return self.ne(tok, Expr::Range { lo, hi, inclusive: incl });
        }
        Ok(lo)
    }

    fn parse_ternary(&mut self) -> PResult<EPtr> {
        let c = self.parse_coalesce()?;
        if self.at("?") {
            self.advance();
            self.nl();
            let th = self.parse_expr()?;
            self.nl();
            self.expect(":", None)?;
            self.nl();
            let el = self.parse_ternary()?;
            let tok = c.tok.clone();
            return self.ne(tok, Expr::Ternary { cond: c, then_: th, els: el });
        }
        Ok(c)
    }

    fn parse_coalesce(&mut self) -> PResult<EPtr> {
        let lhs = self.parse_or()?;
        if self.at("??") {
            self.advance();
            self.nl();
            let rhs = self.parse_coalesce()?;
            let tok = lhs.tok.clone();
            return self.ne(tok, Expr::Coalesce { lhs, rhs });
        }
        Ok(lhs)
    }

    fn parse_or(&mut self) -> PResult<EPtr> {
        let mut lhs = self.parse_and()?;
        while self.at("||") {
            self.advance();
            self.nl();
            let rhs = self.parse_and()?;
            let tok = lhs.tok.clone();
            lhs = self.ne(tok, Expr::Binary { op: "||".to_string(), lhs, rhs })?;
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> PResult<EPtr> {
        let mut lhs = self.parse_equality()?;
        while self.at("&&") {
            self.advance();
            self.nl();
            let rhs = self.parse_equality()?;
            let tok = lhs.tok.clone();
            lhs = self.ne(tok, Expr::Binary { op: "&&".to_string(), lhs, rhs })?;
        }
        Ok(lhs)
    }

    fn parse_equality(&mut self) -> PResult<EPtr> {
        let mut lhs = self.parse_relational()?;
        while self.at("==") || self.at("!=") {
            let op = self.advance().kind.clone();
            self.nl();
            let rhs = self.parse_relational()?;
            let tok = lhs.tok.clone();
            lhs = self.ne(tok, Expr::Binary { op, lhs, rhs })?;
        }
        Ok(lhs)
    }

    fn parse_relational(&mut self) -> PResult<EPtr> {
        let lhs = self.parse_bitor()?;
        if self.at("<") || self.at("<=") || self.at(">") || self.at(">=") {
            let op = self.advance().kind.clone();
            self.nl();
            let rhs = self.parse_bitor()?;
            let tok = lhs.tok.clone();
            let node = self.ne(tok, Expr::Binary { op, lhs, rhs })?;
            if self.at("<") || self.at("<=") || self.at(">") || self.at(">=") {
                self.err("E4016", None, "文稿 11.1", "比较不能连写（`a < b < c` 请写成 `a < b && b < c`）", &[]);
                self.advance();
                self.nl();
                self.parse_bitor()?;
            }
            return Ok(node);
        }
        Ok(lhs)
    }

    fn parse_bitor(&mut self) -> PResult<EPtr> {
        let mut lhs = self.parse_bitxor()?;
        while self.at("|") {
            self.advance();
            self.nl();
            let rhs = self.parse_bitxor()?;
            let tok = lhs.tok.clone();
            lhs = self.ne(tok, Expr::Binary { op: "|".to_string(), lhs, rhs })?;
        }
        Ok(lhs)
    }

    fn parse_bitxor(&mut self) -> PResult<EPtr> {
        let mut lhs = self.parse_bitand()?;
        while self.at("^") {
            self.advance();
            self.nl();
            let rhs = self.parse_bitand()?;
            let tok = lhs.tok.clone();
            lhs = self.ne(tok, Expr::Binary { op: "^".to_string(), lhs, rhs })?;
        }
        Ok(lhs)
    }

    fn parse_bitand(&mut self) -> PResult<EPtr> {
        let mut lhs = self.parse_shift()?;
        while self.at("&") {
            self.advance();
            self.nl();
            let rhs = self.parse_shift()?;
            let tok = lhs.tok.clone();
            lhs = self.ne(tok, Expr::Binary { op: "&".to_string(), lhs, rhs })?;
        }
        Ok(lhs)
    }

    fn parse_shift(&mut self) -> PResult<EPtr> {
        let mut lhs = self.parse_additive()?;
        while self.at("<<") || self.at(">>") {
            let op = self.advance().kind.clone();
            self.nl();
            let rhs = self.parse_additive()?;
            let tok = lhs.tok.clone();
            lhs = self.ne(tok, Expr::Binary { op, lhs, rhs })?;
        }
        Ok(lhs)
    }

    fn parse_additive(&mut self) -> PResult<EPtr> {
        let mut lhs = self.parse_mul()?;
        while self.at("+") || self.at("-") {
            let op = self.advance().kind.clone();
            self.nl();
            let rhs = self.parse_mul()?;
            let tok = lhs.tok.clone();
            lhs = self.ne(tok, Expr::Binary { op, lhs, rhs })?;
        }
        Ok(lhs)
    }

    fn parse_mul(&mut self) -> PResult<EPtr> {
        let mut lhs = self.parse_unary()?;
        while self.at("*") || self.at("/") || self.at("%") {
            let op = self.advance().kind.clone();
            self.nl();
            let rhs = self.parse_unary()?;
            let tok = lhs.tok.clone();
            lhs = self.ne(tok, Expr::Binary { op, lhs, rhs })?;
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> PResult<EPtr> {
        let t = self.tok(0).clone();
        if t.is("!") || t.is("-") || t.is("~") || t.is("&") {
            self.advance();
            let operand = self.parse_unary()?;
            return self.ne(t.clone(), Expr::Unary { op: t.kind.clone(), operand });
        }
        if t.is("++") || t.is("--") {
            self.err("E2010", Some(&t), "文稿 11.1", "不支持前缀 `{0}`，请写成语句 `x {1}`", &[&t.kind, &t.kind]);
            self.advance();
            let operand = self.parse_unary()?;
            return self.ne(t.clone(), Expr::Unary { op: t.kind.clone(), operand });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> PResult<EPtr> {
        // `++`/`--` 只认**语句最外层**的 postfix 链：进入本函数时把标志一次性
        // 消费掉，之后任何嵌套表达式（实参、下标、括号、数组元素）里再遇到
        // `++` 都走 E2010。否则 `print(a++)` 里的 `++` 会被语句层当成尾巴吞掉，
        // 只剩一串同一位置的恢复错误。
        let top = std::mem::replace(&mut self.allow_incdec, false);
        let mut e = self.parse_primary()?;
        loop {
            if self.at(".") {
                let next = &self.tok(1);
                // 成员名可为标识符，也可为关键字（如 errors.new() 中的 new）；
                // 排除标点/运算符（如 .( .) .} 等），避免把非成员访问误解析。
                let is_word = next.is("IDENT")
                    || next.kind.chars().next().map_or(false, |c| c.is_alphabetic());
                if !is_word {
                    break;
                }
                self.advance();
                let name = self.advance().text().to_string();
                if self.at("(") {
                    let args = self.parse_call_args()?;
                    let tok = e.tok.clone();
                    e = self.ne(tok, Expr::MethodCall { recv: e, name, args })?;
                } else {
                    let tok = e.tok.clone();
                    e = self.ne(tok, Expr::FieldAccess { recv: e, name })?;
                }
            } else if self.at("?.") {
                self.advance();
                let name = self.expect_ident("成员名")?.text().to_string();
                if self.at("(") {
                    let args = self.parse_call_args()?;
                    let tok = e.tok.clone();
                    e = self.ne(tok, Expr::SafeCall { recv: e, name, args })?;
                } else {
                    let tok = e.tok.clone();
                    e = self.ne(tok, Expr::SafeAccess { recv: e, name })?;
                }
            } else if self.at("[") {
                self.advance();
                self.nl();
                let idx = self.parse_expr()?;
                self.nl();
                self.expect("]", None)?;
                let tok = e.tok.clone();
                e = self.ne(tok, Expr::Index { recv: e, index: idx })?;
            } else if self.at("(") {
                let args = self.parse_call_args()?;
                let tok = e.tok.clone();
                e = self.ne(tok, Expr::Call { callee: e, args })?;
            } else if self.at("try") {
                let t = self.advance();
                let tok = t.clone();
                e = self.ne(tok, Expr::Try { inner: e })?;
                if self.at(".") || self.at("[") {
                    let tk = self.tok(0).clone();
                    return self.fail("E2009", &tk, "文稿 10.1", "`try` 之后不能再接 `.` 或 `[`（`try` 绑定最紧）", &[]);
                }
            } else if self.at("++") || self.at("--") {
                if top {
                    break;
                }
                let op = self.tok(0).kind.clone();
                self.err("E2010", None, "文稿 11.1", "`{0}` 只能单独成语句，不能出现在表达式内部", &[&op]);
                self.advance();
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn parse_call_args(&mut self) -> PResult<Vec<N<Arg>>> {
        self.expect("(", None)?;
        self.nl();
        let mut args = Vec::new();
        while !self.at(")") {
            self.nl();
            let mut borrow = false;
            if self.at("&") {
                self.advance();
                borrow = true;
                self.nl();
            }
            let e = self.parse_expr()?;
            let tok = e.tok.clone();
            args.push(self.nn(tok, Arg { expr: e, borrow }));
            self.nl();
            if self.accept(",").is_none() {
                break;
            }
            self.nl();
        }
        self.nl();
        self.expect(")", None)?;
        Ok(args)
    }

    fn parse_primary(&mut self) -> PResult<EPtr> {
        let t = self.tok(0).clone();
        let k = t.kind.clone();
        if k == "INT" {
            self.advance();
            let (value, raw) = match &t.val {
                TokVal::Int { value, raw } => (*value, raw.clone()),
                _ => unreachable!(),
            };
            return self.ne(t, Expr::IntLit { value, raw });
        }
        if k == "FLOAT" {
            self.advance();
            let (value, raw, is_f32) = match &t.val {
                TokVal::Float { value, raw, is_f32 } => (*value, raw.clone(), *is_f32),
                _ => unreachable!(),
            };
            return self.ne(t, Expr::FloatLit { value, raw, is_f32 });
        }
        if k == "DEC" {
            self.advance();
            let (coeff, scale, raw) = match &t.val {
                TokVal::Dec { coeff, scale, raw } => (*coeff, *scale, raw.clone()),
                _ => unreachable!(),
            };
            return self.ne(t, Expr::DecLit { coeff, scale, raw });
        }
        if k == "true" || k == "false" {
            self.advance();
            return self.ne(t.clone(), Expr::BoolLit { value: k == "true" });
        }
        if k == "null" {
            self.advance();
            return self.ne(t, Expr::NullLit);
        }
        if k == "STR" {
            self.advance();
            return self.build_str(t);
        }
        if k == "new" {
            self.advance();
            let mut path = vec![self.expect_ident("类型名")?.text().to_string()];
            while self.at(".") && self.tok(1).is("IDENT") {
                self.advance();
                path.push(self.advance().text().to_string());
            }
            let args = self.parse_call_args()?;
            return self.ne(t, Expr::New { path, args });
        }
        if k == "(" {
            return self.parse_paren_or_anon();
        }
        if k == "[" {
            self.advance();
            self.nl();
            let mut elems = Vec::new();
            while !self.at("]") {
                self.nl();
                elems.push(self.parse_expr()?);
                self.nl();
                if self.accept(",").is_none() {
                    break;
                }
                self.nl();
            }
            self.expect("]", None)?;
            return self.ne(t, Expr::ArrayLit { elems });
        }
        if k == "{" {
            self.advance();
            self.nl();
            let mut pairs: Vec<(String, EPtr)> = Vec::new();
            while !self.at("}") {
                self.nl();
                let kt = self.tok(0).clone();
                let key: String;
                if !kt.is("STR") {
                    self.err("E4033", Some(&kt), "GAP-1", "`{...}` 的键必须是字符串字面量", &[]);
                    key = String::new();
                    self.advance();
                } else {
                    self.advance();
                    let segs = match &kt.val {
                        TokVal::Str(s) => s.clone(),
                        _ => Vec::new(),
                    };
                    if segs.len() == 1 {
                        if let StrPart::Text(txt) = &segs[0] {
                            key = txt.clone();
                        } else {
                            self.err("E4033", Some(&kt), "GAP-1", "`{...}` 的键必须是字符串字面量（不能含插值）", &[]);
                            key = segs
                                .iter()
                                .map(|p| match p {
                                    StrPart::Text(x) => x.clone(),
                                    _ => String::new(),
                                })
                                .collect();
                        }
                    } else {
                        self.err("E4033", Some(&kt), "GAP-1", "`{...}` 的键必须是字符串字面量（不能含插值）", &[]);
                        key = segs
                            .iter()
                            .map(|p| match p {
                                StrPart::Text(x) => x.clone(),
                                _ => String::new(),
                            })
                            .collect();
                    }
                }
                self.nl();
                self.expect(":", None)?;
                self.nl();
                let v = self.parse_expr()?;
                pairs.push((key, v));
                self.nl();
                if self.accept(",").is_none() {
                    break;
                }
                self.nl();
            }
            self.expect("}", None)?;
            return self.ne(t, Expr::BraceLit { pairs });
        }
        if k == "if" {
            let n = self.parse_if()?;
            return self.ne(t, Expr::If(Box::new(n)));
        }
        if k == "match" {
            let n = self.parse_match()?;
            return self.ne(t, Expr::Match(Box::new(n)));
        }
        if k == "this" {
            self.advance();
            return self.ne(t, Expr::This);
        }
        if k == "parent" {
            self.advance();
            return self.ne(t, Expr::Parent);
        }
        if k == "is" {
            return self.parse_is_expr(t);
        }
        if k == "IDENT" {
            self.advance();
            let segs = vec![t.text().to_string()];
            // 显式泛型实参：f<int>(...)
            if self.at("<") {
                let m = self.mark();
                match self.parse_type_args() {
                    Ok(args) => {
                        if self.at("(") {
                            return self.ne(t, Expr::Path { segs, args });
                        }
                        self.reset(m);
                    }
                    Err(_) => self.reset(m),
                }
            }
            return self.ne(t, Expr::Path { segs, args: Vec::new() });
        }
        self.fail("E2001", &t, "G-1", "非法的表达式起点 `{0}`", &[&display_tok(&t)])
    }

    fn parse_is_expr(&mut self, t: Tok) -> PResult<EPtr> {
        self.advance(); // consume `is`, current is now `(`
        self.expect("(", None)?; // consume `(`
        self.nl();
        // 检查 typeof(Expr) == Type 形式：typeof 关键字在左侧
        if self.at("IDENT") && self.tok(0).text() == "typeof" {
            self.advance();
            self.nl();
            self.expect("(", None)?;
            self.nl();
            let expr = self.parse_expr()?;
            self.nl();
            self.expect(")", None)?;
            self.nl();
            self.expect("==", None)?;
            self.nl();
            let right_ty = self.parse_type()?;
            self.nl();
            self.expect(")", None)?;
            return self.ne(t, Expr::IsType {
                kind: "typeof_eq".to_string(),
                ty: None,
                target: Some(right_ty),
                expr: Some(expr),
                interface: None,
            });
        }
        let left_ty = self.parse_type()?;
        self.nl();
        if self.at("==") {
            self.advance();
            self.nl();
            let right_ty = self.parse_type()?;
            self.nl();
            self.expect(")", None)?;
            return self.ne(t, Expr::IsType {
                kind: "eq".to_string(),
                ty: Some(left_ty),
                target: Some(right_ty),
                expr: None,
                interface: None,
            });
        }
        if self.at(":") {
            self.advance();
            self.nl();
            let mut path = vec![self.expect_ident("接口名")?.text().to_string()];
            while self.at(".") && self.tok(1).is("IDENT") {
                self.advance();
                path.push(self.advance().text().to_string());
            }
            self.nl();
            self.expect(")", None)?;
            return self.ne(t, Expr::IsType {
                kind: "impl".to_string(),
                ty: Some(left_ty),
                target: None,
                expr: None,
                interface: Some(path),
            });
        }
        let tk = self.tok(0).clone();
        return self.fail("E2001", &tk, "G-1", "期望 `==` 或 `:` 或 `typeof(` 在 `is(` 之后", &[]);
    }

    fn build_str(&mut self, t: Tok) -> PResult<EPtr> {
        let mut parts: Vec<SPart> = Vec::new();
        let segs = match &t.val {
            TokVal::Str(s) => s.clone(),
            _ => Vec::new(),
        };
        for p in &segs {
            match p {
                StrPart::Text(txt) => parts.push(SPart::Text(txt.clone())),
                StrPart::Interp { src, line, col, fmt } => {
                    let (expr, nid) = parse_interp(src, &self.path, self.d, *line, *col, self.next_id)?;
                    self.next_id = nid;
                    parts.push(SPart::Interp { expr, fmt: *fmt });
                }
            }
        }
        self.ne(t, Expr::StrLit { parts })
    }

    fn parse_paren_or_anon(&mut self) -> PResult<EPtr> {
        let t = self.expect("(", None)?;
        let m = self.mark();
        // G-10：投机解析参数列表 + '{'
        match self.try_anon() {
            Ok(Some(node)) => return Ok(node),
            _ => self.reset(m),
        }
        self.nl();
        let first = self.parse_expr()?;
        self.nl();
        if self.at(",") {
            let mut elems = vec![first];
            while self.accept(",").is_some() {
                self.nl();
                if self.at(")") {
                    break;
                }
                elems.push(self.parse_expr()?);
                self.nl();
            }
            self.expect(")", None)?;
            return self.ne(t, Expr::TupleLit { elems });
        }
        self.expect(")", None)?;
        Ok(first)
    }

    /// G-10 的投机分支：像不像匿名函数。返回 None 表示不像（调用方 reset）。
    fn try_anon(&mut self) -> PResult<Option<EPtr>> {
        let t = self.tok(0).clone(); // "("
        let mut params: Vec<N<Param>> = Vec::new();
        self.nl();
        if !self.at(")") {
            loop {
                self.nl();
                let mut borrow = None;
                if self.at("&") {
                    self.advance();
                    borrow = Some("mut".to_string());
                    if self.at("const") {
                        self.advance();
                        borrow = Some("const".to_string());
                    }
                    self.nl();
                }
                let ty = self.parse_type()?;
                if !self.at("IDENT") {
                    return Ok(None);
                }
                let nm = self.advance();
                params.push(self.nn(
                    nm.clone(),
                    Param { borrow, ty, name: nm.text().to_string(), varargs: false },
                ));
                self.nl();
                if self.accept(",").is_none() {
                    break;
                }
                self.nl();
            }
        }
        if self.at(")") {
            self.advance();
            self.nl();
            if self.at("{") {
                let body = self.parse_block()?;
                return Ok(Some(
                    self.ne(t, Expr::AnonFunc { params, body })?,
                ));
            }
        }
        Ok(None)
    }
}

/// 解析字符串插值片段里的表达式（与主解析器共享诊断与 id 计数）。
fn parse_interp(
    src: &str,
    path: &str,
    d: &mut Diags,
    line: u32,
    col: u32,
    next_id: u32,
) -> PResult<(EPtr, u32)> {
    let chars: Vec<char> = src.chars().collect();
    let mut lx = Lexer::new(chars, path, d, line, col);
    let toks = lx.run();
    let mut sp = Parser::new(toks, path.to_string(), d, next_id);
    let expr = sp.parse_expr()?;
    if !sp.at("EOF") {
        let t = sp.tok(0).clone();
        sp.err("E2001", Some(&t), "L-18", "插值表达式后有多余内容", &[]);
    }
    Ok((expr, sp.next_id))
}

fn display_tok(t: &Tok) -> String {
    if t.is("EOF") {
        return "文件结束".to_string();
    }
    format!("`{}`", t.text())
}

/// 模块入口：解析全文 + 回填错误数给调用方。
pub fn parse_module(toks: Vec<Tok>, path: &str, d: &mut Diags, id_base: u32) -> Module {
    let mut p = Parser::new(toks, path.to_string(), d, id_base);
    let mut m = p.parse();
    m.parse_errors = p.errors;
    m
}
