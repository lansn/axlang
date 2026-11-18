//! axc —— axlang 编译器（Rust 实现，当前唯一在维护的实现）。
//!
//! # 编译管线
//!
//! ```text
//! 源码 .ax ─► frontend::lexer ─► frontend::parser ─► sema ─► backend ─► C11(纯 ASCII) ─► cc ─► 可执行文件
//!             Tok 流              AST                诊断+槽位   生成 C
//! ```
//!
//! # 模块导览
//!
//! | 模块       | 职责 |
//! |------------|------|
//! | `cli`      | 子命令分发、cc 调用、临时目录管理、退出码契约 |
//! | `diag`     | 诊断收集 / 排序 / 输出（text 与 JSONL 两种格式）|
//! | `types`    | 类型表示、canonical 规范名、assignable 判定（全项目唯一入口）|
//! | `frontend` | 词法（lexer）、语法（parser）、AST 定义（ast）|
//! | `sema`     | 语义核心（本模块）+ 语句（stmt）/ 表达式（expr）/ 成员（member）|
//! | `backend`  | C 生成：程序级（本模块）+ 语句级 FnGen（codegen_fn）+ 表达式级（codegen_expr）|
//!
//! 数据单向流动：frontend 产出 AST，sema 把检查结果挂在 `Sema::slots` 上，
//! backend 只读 AST + slots，不做任何重新检查。跨阶段的公共类型
//! （`Tok`、`Type`、`Diags`）分别在 lexer / types / diag 中定义。
//!
//! # 退出码契约（对外承诺，勿改）
//!
//!   0 成功 / 1 有 error 级诊断 / 2 目标程序运行时崩溃。
//!
//! 详见仓库根 `ARCHITECTURE.md`（接手指南）与 `CHANGELOG.md`（版本变更）。

// 有意保留的未使用接口：诊断 warn/Note/Warning 预留、部分 AST 冗余字段（tok/vis）、
// 少量工具方法（canon、def_of 等）。随功能落地逐个摘除；新增死代码不要赖在这条 allow 下。
#![allow(dead_code)]

mod backend;
mod cli;
mod diag;
mod diag_msg;
mod fmt_spec;
mod frontend;
mod sema;
mod types;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(cli::run(&argv));
}
