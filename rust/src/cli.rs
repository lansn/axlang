//! 命令行入口（规格书 D-03）。
//!
//!     axc check  <file.ax>            只做 词法→语法→语义，报诊断
//!     axc build  <file.ax> [-o out]   再走 C11 后端 + 系统 cc
//!     axc run    <file.ax> [-- args]  build 后立即执行
//!
//! 通用选项
//!     --emit=c            把生成的 C 源码打到 stdout
//!     --dump=ast|types|c  打印中间产物
//!     --diag=text|json    诊断格式（JSON 为 JSONL）
//!     -Werror             警告升级为错误
//!     --cc=<path>         指定 C 编译器（默认取 $AX_CC 或 gcc）
//!     --keep              保留构建目录

use crate::frontend::ast;
use crate::backend;
use crate::driver::compile;
use crate::diag::Diags;
use crate::frontend::lexer;
use crate::frontend::parser;
use crate::backend::san;
use crate::sema::{self, ExtraMod};

// ---------------------------------------------------------------- 用户模块加载
//
// 文稿 9.1–9.2：一个 .ax 文件 = 一个模块；模块路径即目录结构
// （`import a.b` 对应入口文件同目录的 `a/b.ax`）；循环导入是编译错误。
// 解析在 sema 之前完成：拓扑序（被导入者在前）交给 analyze_multi 合并。

/// 基于文件路径推导传统 canon（文件名 sanitized，冲突时加序号）。
fn file_stem_canon(fpath: &str, used_prefixes: &mut std::collections::HashMap<String, u32>) -> String {
    let stem = std::path::Path::new(fpath)
        .file_stem()
        .map(|x| x.to_string_lossy().to_string())
        .unwrap_or_else(|| "mod".to_string());
    let base = san(&stem);
    let n = used_prefixes.entry(base.clone()).or_insert(0);
    *n += 1;
    if *n == 1 {
        base
    } else {
        format!("{}__{}", base, n)
    }
}

/// 递归加载 `joined`（如 "utils" / "net.http"）指向的模块文件。
/// `arena` 持有全部模块所有权；`loaded` 去重；`stack` 做循环检测。
#[allow(clippy::too_many_arguments)]
fn load_module_rec(
    joined: &str,
    roots: &[std::path::PathBuf],
    d: &mut Diags,
    arena: &mut Vec<(String, String, ast::Module)>,
    loaded: &mut std::collections::HashMap<String, usize>,
    stack: &mut Vec<String>,
    used_prefixes: &mut std::collections::HashMap<String, u32>,
    path_canon: &mut std::collections::HashMap<String, String>,
    id_base: &mut u32,
) {
    if loaded.contains_key(joined) {
        return;
    }
    if stack.iter().any(|s| s == joined) {
        let mut chain = stack.clone();
        chain.push(joined.to_string());
        {
            let msg = crate::diag_msg::tr(
                d.lang,
                "E3026",
                "循环导入：{0}",
                &[&chain.join(" -> ")],
            );
            d.error("E3026", msg, joined,
            1,
            1,
            1,
            2,
            "文稿 9.2",
            );
        }
        return;
    }
    let rel = if sema::STDLIB_MODULES.contains(&joined) {
        // 标准库模块（std.string / std.file / std.sync / std.atomic）与用户库一样
        // 从文件系统加载，但统一位于 axc/std/ 目录下：import std.string -> axc/std/string.ax
        // 去掉 "std." 前缀，剩余部分作为 axc/std/ 下的相对路径。
        let rest = joined.strip_prefix("std.").unwrap_or(joined);
        format!("std/{}.ax", rest.replace('.', "/"))
    } else {
        format!("{}.ax", joined.replace('.', "/"))
    };
    // 按搜索根顺序找：入口文件目录在最前，其后是 `-I` 给的目录
    let file = match roots.iter().map(|r| r.join(&rel)).find(|f| f.is_file()) {
        Some(f) => f,
        None => {
            let tried: Vec<String> = roots
                .iter()
                .map(|r| r.join(&rel).display().to_string())
                .collect();
            let msg = crate::diag_msg::tr(
                d.lang,
                "E3027",
                "找不到模块 `{0}`（已在 {1} 下查找）",
                &[&joined, &tried.join("、")],
            );
            d.error("E3027", msg, joined, 1, 1, 1, 2, "文稿 9.2");
            return;
        }
    };
    let raw = match fs::read(&file) {
        Ok(b) => b,
        Err(_) => {
            let msg = crate::diag_msg::tr(
                d.lang,
                "E3027",
                "模块文件读取失败：{0}",
                &[&file.display().to_string()],
            );
            d.error("E3027", msg, joined, 1, 1, 1, 2, "文稿 9.2");
            return;
        }
    };
    let src = match String::from_utf8(raw) {
        Ok(s) => s,
        Err(_) => {
            let msg = crate::diag_msg::tr(
                d.lang,
                "E3027",
                "模块文件不是合法 UTF-8：{0}",
                &[&file.display().to_string()],
            );
            d.error("E3027", msg, joined, 1, 1, 1, 2, "文稿 9.2");
            return;
        }
    };
    let fpath = file.to_string_lossy().to_string();
    let normalized = lexer::normalize(&src);
    d.add_source(&fpath, &normalized);
    let toks = lexer::lex(&src, &fpath, d);
    // 每个文件独占一段节点 id 区间（步长 100 万），避免跨模块 slot 冲突
    let m = parser::parse_module(toks, &fpath, d, *id_base);
    *id_base += 1_000_000;

    stack.push(joined.to_string());
    // 先加载它导入的用户模块（拓扑序：被导入者先进 arena）
    for item in &m.items {
        if let ast::Item::Import { path, .. } = &item.k {
            let j = path.join(".");
            if !sema::BUILTIN_MODULES.contains(&path[0].as_str()) {
                load_module_rec(
                    &j,
                    roots,
                    d,
                    arena,
                    loaded,
                    stack,
                    used_prefixes,
                    path_canon,
                    id_base,
                );
            } else {
                check_builtin_shadow(&path[0], roots, d);
            }
        }
    }
    stack.pop();

    // 规范前缀：取文件名 sanitized，冲突时加序号。
    let canon = file_stem_canon(&fpath, used_prefixes);

    let idx = arena.len();
    arena.push((canon.clone(), fpath, m));
    loaded.insert(joined.to_string(), idx);
    path_canon.insert(joined.to_string(), canon);
}

/// 加载入口文件 import 的全部用户模块。返回 (extras, path_canon)。
/// 内建模块名（`io`/`file`/`db`/`json`/`math`/`string`/`time`）是**保留名**：
/// 用户若建了同名文件，编译器一律用内建实现，那份文件被静默忽略——这里补一条明确诊断，
/// 否则报出来的是「模块 `io` 没有符号 `my_val`」，看不出真正原因是文件压根没被加载。
fn check_builtin_shadow(name: &str, roots: &[std::path::PathBuf], d: &mut Diags) {
    if let Some(f) = roots
        .iter()
        .map(|r| r.join(format!("{}.ax", name)))
        .find(|f| f.is_file())
    {
        let msg = crate::diag_msg::tr(
            d.lang,
            "E3039",
            "模块名 `{0}` 与内建模块重名，文件 `{1}` 不会被加载（内建模块优先）；请重命名该文件",
            &[&name, &f.display().to_string()],
        );
        d.error(
            "E3039",
            msg,
            name,
            1,
            1,
            1,
            2,
            "文稿 9.2",
        );
    }
}

fn load_user_imports<'a>(
    entry: &ast::Module,
    entry_path: &str,
    includes: &[String],
    d: &mut Diags,
    arena: &'a mut Vec<(String, String, ast::Module)>,
) -> (Vec<ExtraMod<'a>>, std::collections::HashMap<String, String>) {
    let mut loaded = std::collections::HashMap::new();
    let mut stack = Vec::new();
    let mut used_prefixes = std::collections::HashMap::new();
    let mut path_canon = std::collections::HashMap::new();
    let mut id_base: u32 = 1_000_000; // 入口文件占 [0, 1_000_000)
    let entry_dir = std::path::Path::new(entry_path)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    // 搜索根：入口文件目录优先，其后依次是 `-I` 给出的目录，然后是基于编译器
    // 可执行文件位置的 release/axc/ 标准库目录，最后是 cwd/axc/ 标准库目录。
    // 这样 release/bin/axc.exe 运行时能自动找到 release/axc/ 下的标准库。
    //（import std.string -> axc/std/string.ax，由 load_module_rec 去掉 "std." 前缀后匹配 std/xxx.ax）
    let mut roots: Vec<std::path::PathBuf> = vec![entry_dir.clone()];
    for inc in includes {
        roots.push(std::path::PathBuf::from(inc));
    }
    // 基于当前 exe 位置的 release/axc/ 目录（如 release/bin/axc.exe -> release/axc/）
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let release_dir = match exe_dir.parent() {
                Some(d) => d,
                None => exe_dir,
            };
            let bundled_stdlib = release_dir.join("axc");
            if std::path::Path::new(&bundled_stdlib).exists() {
                roots.push(bundled_stdlib);
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd.join("axc"));
    }
    for item in &entry.items {
        if let ast::Item::Import { path, .. } = &item.k {
            let j = path.join(".");
            if !sema::BUILTIN_MODULES.contains(&path[0].as_str()) {
                load_module_rec(
                    &j,
                    &roots,
                    d,
                    arena,
                    &mut loaded,
                    &mut stack,
                    &mut used_prefixes,
                    &mut path_canon,
                    &mut id_base,
                );
            } else {
                check_builtin_shadow(&path[0], &roots, d);
            }
        }
    }
    let extras = arena
        .iter()
        .map(|(canon, fpath, m)| ExtraMod {
            canon: canon.clone(),
            file_path: fpath.clone(),
            module: m,
        })
        .collect();
    (extras, path_canon)
}
use std::fs;
use std::io::Write;

/// 命令行选项的解析结果，`cmd` 是子命令名（check/build/run）。
#[derive(Clone)]
pub struct Opts {
    pub cmd: String,
    pub file: Option<String>,
    pub out: Option<String>,
    pub diag: String,
    pub dump: Option<String>,
    pub emit_c: bool,
    pub werror: bool,
    pub cc: String,
    pub keep: bool,
    pub includes: Vec<String>,
    pub prog_args: Vec<String>,
    pub help: bool,
    pub version: bool,
    /// `--lang=en|zh`（None = 走 AXC_LANG / axc.toml / 默认英文）
    pub lang: Option<String>,
    /// 产物类型："exe"（默认）或 "shared"，共享库模式下跳过 main 包装并加 -shared
    pub kind: String,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            cmd: "check".to_string(),
            file: None,
            out: None,
            diag: "text".to_string(),
            dump: None,
            emit_c: false,
            werror: false,
            cc: default_cc(),
            keep: false,
            includes: Vec::new(),
            prog_args: Vec::new(),
            help: false,
            version: false,
            lang: None,
            kind: "exe".to_string(),
        }
    }
}

/// 默认 C 编译器：优先使用发行包自带的 `release/gcc/bin/gcc.exe`（与 axc.exe 同发布目录下）；
/// 其次回退到用户安装的 GCC 16.2.0（`D:/Portable/mingw64/bin/gcc.exe`）；
/// 最后回退到系统 PATH 中的 `gcc`。
/// 用户可通过 `AX_CC` 环境变量或 `--cc=` 覆盖。
fn default_cc() -> String {
    if let Ok(v) = std::env::var("AX_CC") {
        return v;
    }
    // 优先查找发行包自带的 release/gcc/bin/ 目录（与 axc.exe 同发布目录；Windows 布局）。
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let release_dir = match exe_dir.parent() {
                Some(d) => d,
                None => exe_dir,
            };
            // Windows: release/gcc/bin/gcc.exe
            let bundled = release_dir.join("gcc").join("bin").join("gcc.exe");
            if std::path::Path::new(&bundled).exists() {
                return bundled.to_string_lossy().to_string();
            }
            // Linux/macOS: release/gcc/bin/gcc
            let bundled_no_ext = release_dir.join("gcc").join("bin").join("gcc");
            if std::path::Path::new(&bundled_no_ext).exists() {
                return bundled_no_ext.to_string_lossy().to_string();
            }
        }
    }
    // 在 Windows 上，Rust 的 Path 需要使用 "D:/..." 或 "D:\\..." 格式的绝对路径，
    // Git Bash 风格的 "/d/..." 相对路径不会被识别为存在的文件。
    let new_gcc = "D:/Portable/mingw64/bin/gcc.exe";
    if std::path::Path::new(new_gcc).exists() {
        return new_gcc.to_string();
    }
    // 也尝试反斜杠形式（兼容手动输入）
    let new_gcc_bs = "D:\\Portable\\mingw64\\bin\\gcc.exe";
    if std::path::Path::new(new_gcc_bs).exists() {
        return new_gcc_bs.to_string();
    }
    "gcc".to_string()
}

pub fn parse_args(argv: &[String]) -> Opts {
    let mut o = Opts::default();
    let mut i = 0;
    let mut after_dashdash = false;
    while i < argv.len() {
        let a = &argv[i];
        if after_dashdash {
            o.prog_args.push(a.clone());
            i += 1;
            continue;
        }
        match a.as_str() {
            "check" | "build" | "run" => o.cmd = a.clone(),
            "-h" | "--help" => o.help = true,
            "-V" | "--version" => o.version = true,
            "-I" | "--include" => {
                i += 1;
                if i < argv.len() {
                    o.includes.push(argv[i].clone());
                }
            }
            "-o" => {
                i += 1;
                if i < argv.len() {
                    o.out = Some(argv[i].clone());
                }
            }
            "--emit=c" => o.emit_c = true,
            "-Werror" => o.werror = true,
            "--keep" => o.keep = true,
            "--" => after_dashdash = true,
            _ => {
                if let Some(v) = a.strip_prefix("--diag=") {
                    o.diag = v.to_string();
                } else if let Some(v) = a.strip_prefix("--dump=") {
                    o.dump = Some(v.to_string());
                } else if let Some(v) = a.strip_prefix("--cc=") {
                    o.cc = v.to_string();
                } else if let Some(v) = a.strip_prefix("--lang=") {
                    o.lang = Some(v.to_string());
                } else if let Some(v) = a.strip_prefix("--kind=") {
                    o.kind = v.to_string();
                } else if a.starts_with('-') {
                    // 未知选项：忽略（历史宽松解析行为，测试钉死）
                } else if o.file.is_none() {
                    o.file = Some(a.clone());
                } else {
                    o.prog_args.push(a.clone());
                }
            }
        }
        i += 1;
    }
    o
}

pub fn print_help() {
    println!("axc {} —— axlang 编译器", env!("CARGO_PKG_VERSION"));
    println!();
    println!("用法: axc <check|build|run> <file.ax> [选项]");
    println!();
    println!("  check   词法 / 语法 / 语义检查，只报诊断");
    println!("  build   生成 C11 源码并调用 cc 产出可执行文件");
    println!("  run     build 后立即执行");
    println!();
    println!("选项:");
    println!("  -I <dir>            追加模块搜索根（可多次；入口文件目录优先）");
    println!("  -o <path>           输出文件名");
    println!("  --emit=c            只输出 C 源码到 stdout");
    println!("  --dump=ast|hir|types|c  打印中间产物");
    println!("  --diag=text|json    诊断格式，json 为 JSONL");
    println!("  -Werror             警告视为错误");
    println!("  --cc=<path>         指定 C 编译器");
    println!("  --lang=en|zh        诊断文案语言（默认 en；也可用 AXC_LANG 环境变量或 axc.toml）");
    println!("  --keep              保留构建目录（排查生成代码用）");
    println!("  --kind=exe|shared   产物类型：exe（默认可执行文件）或 shared（动态链接库 .dll/.so/.dylib）");
    println!("  -V, --version       打印版本号后退出");
}

fn report(d: &Diags, mode: &str) {
    if d.items.is_empty() {
        return;
    }
    let text = if mode == "json" {
        d.format_json()
    } else {
        d.format_text()
    };
    if !text.is_empty() {
        let stderr = std::io::stderr();
        let mut h = stderr.lock();
        let _ = h.write_all(text.as_bytes());
        if !text.ends_with('\n') {
            let _ = h.write_all(b"\n");
        }
    }
}

pub fn run(argv: &[String]) -> i32 {
    let opts = parse_args(argv);
    if opts.version {
        // 版本号单一来源是 rust/Cargo.toml 的 version（雏形期 0.0.x 每里程碑 +1；
        // 1.0 及以上永远保留给语法冻结，进位规则见 ARCHITECTURE.md §8）
        println!("axc {} (axlang compiler)", env!("CARGO_PKG_VERSION"));
        return 0;
    }
    if opts.help || opts.file.is_none() {
        print_help();
        return if opts.help { 0 } else { 1 };
    }
    let path = opts.file.clone().unwrap();

    let raw = match fs::read(&path) {
        Ok(b) => b,
        Err(_) => {
            eprintln!(
            "axc: {}",
            crate::diag_msg::tr(
                crate::diag_msg::Lang::resolve(None),
                "FILE_NOT_FOUND",
                "axc: 找不到文件 {0}",
                &[&path]
            )
        );
            return 1;
        }
    };
    let src = match String::from_utf8(raw) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("axc: 源文件不是合法 UTF-8：{}", path);
            return 1;
        }
    };

    let mut d = Diags::new();
    d.lang = crate::diag_msg::Lang::resolve(opts.lang.as_deref());
    let normalized = lexer::normalize(&src);
    d.add_source(&path, &normalized);

    let toks = lexer::lex(&src, &path, &mut d);

    // 词法一错，token 流已不可信；继续解析只会追加源于同一处的假语法错误
    // （§16 要求负例"恰好一个诊断码"）
    if d.has_error() {
        report(&d, &opts.diag);
        eprintln!(
            "axc: {}",
            crate::diag_msg::tr(d.lang, "SUMMARY", "{0} 个错误", &[&d.error_count])
        );
        return 1;
    }

    let mod_ = parser::parse_module(toks, &path, &mut d, 0);
    if opts.dump.as_deref() == Some("ast") {
        println!("{}", ast::dump_module(&mod_));
    }
    if d.has_error() {
        report(&d, &opts.diag);
        eprintln!(
            "axc: {}",
            crate::diag_msg::tr(d.lang, "SUMMARY", "{0} 个错误", &[&d.error_count])
        );
        return 1;
    }

    // check 阶段不要求 main；shared 模式生成动态库，也不需要 main 入口
    let need_main = matches!(opts.cmd.as_str(), "build" | "run") && opts.kind != "shared";

    // 用户模块加载（文稿 9）：解析 import，递归加载非内建模块，循环导入报错
    let mut arena: Vec<(String, String, ast::Module)> = Vec::new();
    let (extras, path_canon) = load_user_imports(&mod_, &path, &opts.includes, &mut d, &mut arena);
    if d.has_error() {
        report(&d, &opts.diag);
        eprintln!(
            "axc: {}",
            crate::diag_msg::tr(d.lang, "SUMMARY", "{0} 个错误", &[&d.error_count])
        );
        return 1;
    }

    let s = sema::analyze_multi(&mod_, &mut d, &path, need_main, extras, path_canon);

    // --dump=hir：语义检查完成后输出高级中间表示
    if opts.dump.as_deref() == Some("hir") {
        println!("{}", s.dump_hir());
        return 0;
    }

    // 借用说明：Sema 持有 &mut Diags，此后诊断统一从 s.d 读
    if (*s.d).has_error() {
        report(&*s.d, &opts.diag);
        eprintln!(
            "axc: {}",
            crate::diag_msg::tr(s.d.lang, "SUMMARY", "{0} 个错误", &[&(*s.d).error_count])
        );
        return 1;
    }
    // -Werror：警告升级为错误
    if opts.werror && (*s.d).warning_count > 0 {
        report(&*s.d, &opts.diag);
        eprintln!(
            "axc: {}",
            crate::diag_msg::tr(s.d.lang, "SUMMARY", "{0} 个错误", &[&(*s.d).warning_count])
        );
        return 1;
    }
    report(&*s.d, &opts.diag); // 只剩 warning

    finish(&s, &opts, &path, &opts.kind)
}

/// 语义检查通过后的产出阶段：emit-c / build / run。
fn finish(s: &sema::Sema, opts: &Opts, path: &str, kind: &str) -> i32 {
    if opts.emit_c || opts.dump.as_deref() == Some("c") {
        let csrc = backend::CodeGen::build_c(s, path, kind);
        println!("{}", csrc);
        return 0;
    }
    if opts.cmd == "check" {
        return 0;
    }

    let csrc = backend::CodeGen::build_c(s, path, kind);

    // run 的产物是临时的：落到临时目录，跑完即删，避免在工作目录里堆 exe；
    // 显式给了 -o 就尊重用户的选择（也方便"编译一次跑多次"）。
    let mut tmpdir: Option<std::path::PathBuf> = None;
    let out: std::path::PathBuf = if opts.cmd == "run" && opts.out.is_none() {
        let d = std::env::temp_dir().join(format!(
            "axc-bin-{}-{}",
            std::process::id(),
            now_nanos()
        ));
        let _ = fs::create_dir_all(&d);
        tmpdir = Some(d.clone());
        d.join(default_out(path, kind))
    } else {
        std::path::PathBuf::from(opts.out.clone().unwrap_or_else(|| default_out(path, kind)))
    };

    let res = compile::compile_c_files(&csrc, opts, &out, kind);
    let rc = match res {
        None => 1,
        Some(()) => {
            if opts.cmd == "build" {
                0
            } else {
                let abs = fs::canonicalize(&out).unwrap_or_else(|_| out.clone());
                match std::process::Command::new(abs).args(&opts.prog_args).status() {
                    Ok(st) => st.code().unwrap_or(2),
                    Err(e) => {
                        eprintln!("axc: 无法执行 {}：{}", out.display(), e);
                        1
                    }
                }
            }
        }
    };
    if let Some(d) = tmpdir {
        let _ = fs::remove_dir_all(d);
    }
    rc
}

fn now_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

fn default_out(path: &str, kind: &str) -> String {
    let base = std::path::Path::new(path)
        .file_stem()
        .map(|x| x.to_string_lossy().to_string())
        .unwrap_or_else(|| "a".to_string());
    match kind {
        "shared" => {
            if cfg!(windows) {
                format!("{}.dll", base)
            } else if cfg!(target_os = "macos") {
                format!("lib{}.dylib", base)
            } else {
                format!("lib{}.so", base)
            }
        }
        _ => {
            if cfg!(windows) {
                format!("{}.exe", base)
            } else {
                base
            }
        }
    }
}

