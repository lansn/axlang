//! 前端：源码文本 → AST。
//!
//! 三个阶段（与规格书 §3–§5 一一对应）：
//!
//! - [`lexer`]：字符流 → `Tok` 流。负责 36 个关键字、三类字符串
//!   （普通/裸/插值，插值在词法层片段化）、注释、码点列号。
//! - [`parser`]：`Tok` 流 → AST。负责文法 G-1…G-16、错误恢复
//!   （`nl`/`expect_term` 的换行分工）、运算符优先级与结合性。
//! - [`ast`]：AST 节点定义 + `--dump=ast` 序列化。节点与规格书 §5 对应。
//!
//! 依赖方向（单向）：parser → ast + lexer；ast → lexer（仅 `Tok`）。
//! 前端不依赖 sema 与 backend。

pub(crate) mod ast;
pub(crate) mod lexer;
pub(crate) mod parser;
