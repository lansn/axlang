//! driver/compile.rs —— axlang 编译驱动：C 多文件编译链接
//!
//! 职责：将用户生成的 C 源码（unit.c）、标准库预转译的 .c 文件
//!（axc/stdlib/generated/*.c）与 axrt.c 一起传给 gcc 编译链接。
//!
//! 这是“ax 标准库预转译”架构的一部分：ax 编译器专注于词法/语法/语义分析，
//! 标准库以 ax 语法编写后预转译为标准 C，由本驱动模块统一调用 gcc 完成最终编译。

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::PathBuf;

use crate::backend::CodeGen;
use crate::diag::Diags;
use crate::diag_msg::Lang;
use crate::frontend::{lexer, parser};
use crate::sema::{self, STDLIB_MODULES};

/// 运行时目录（axrt.h / axrt_*.c）：优先按工作目录找，再沿可执行文件位置上溯。
pub fn rt_dir() -> Option<PathBuf> {
    let mut cands = vec![
        PathBuf::from("axc/rt"),
        PathBuf::from("rt"),
    ];
    if let Ok(exe) = std::env::current_exe() {
        let mut cur = exe.parent().map(|d| d.to_path_buf());
        for _ in 0..5 {
            let q = match cur {
                Some(q) => q,
                None => break,
            };
            cands.push(q.join("axc/rt"));
            cands.push(q.join("rt"));
            cur = q.parent().map(|x| x.to_path_buf());
        }
    }
    for c in cands {
        if c.join("axrt.h").exists() {
            return Some(c);
        }
    }
    None
}

/// 收集运行时目录下的所有 axrt_*.c 文件路径（递归进入子目录）。
pub fn rt_c_files(rt_dir: &std::path::Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    rt_c_files_recursive(rt_dir, &mut files);
    files.sort();
    files
}

/// 递归收集 axrt_*.c 文件。
fn rt_c_files_recursive(dir: &std::path::Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                rt_c_files_recursive(&path, files);
            } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("axrt_") && name.ends_with(".c") {
                    files.push(path);
                }
            }
        }
    }
}

/// 标准库预转译输出目录（axc/stdlib/generated）：存放 *.ax 预转译后的 *.c 文件。
/// 优先返回 generated/ 子目录，不存在则回退到 axc/stdlib/（保留向后兼容）。
pub fn stdlib_gen_dir() -> Option<PathBuf> {
    let mut cands = vec![
        PathBuf::from("axc/stdlib/generated"),
        PathBuf::from("axc/stdlib"),
    ];
    if let Ok(exe) = std::env::current_exe() {
        let mut cur = exe.parent().map(|d| d.to_path_buf());
        for _ in 0..5 {
            let q = match cur {
                Some(q) => q,
                None => break,
            };
            cands.push(q.join("axc/stdlib/generated"));
            cands.push(q.join("axc/stdlib"));
            cur = q.parent().map(|x| x.to_path_buf());
        }
    }
    for c in cands {
        if c.exists() {
            return Some(c);
        }
    }
    None
}

/// 标准库头文件目录（axc/stdlib/）：存放 axc_stdlib.h，与生成的 .c 文件分离。
pub fn stdlib_header_dir() -> Option<PathBuf> {
    let mut cands = vec![
        PathBuf::from("axc/stdlib"),
    ];
    if let Ok(exe) = std::env::current_exe() {
        let mut cur = exe.parent().map(|d| d.to_path_buf());
        for _ in 0..5 {
            let q = match cur {
                Some(q) => q,
                None => break,
            };
            cands.push(q.join("axc/stdlib"));
            cur = q.parent().map(|x| x.to_path_buf());
        }
    }
    for c in cands {
        if c.join("axc_stdlib.h").exists() {
            return Some(c);
        }
    }
    None
}

/// 收集标准库预转译生成的所有 .c 文件路径。
pub fn stdlib_c_files(stdlib_dir: &std::path::Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(stdlib_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "c") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// 标准库 .o 目标文件缓存目录（release/axc/stdlib/obj）：存放预编译后的 *.o 文件，
/// 用户编译时复用缓存，避免每次重新编译标准库 .c 源码。
pub fn stdlib_obj_dir() -> Option<PathBuf> {
    let mut cands = vec![
        PathBuf::from("axc/stdlib/obj"),
        PathBuf::from("stdlib/obj"),
    ];
    if let Ok(exe) = std::env::current_exe() {
        let mut cur = exe.parent().map(|d| d.to_path_buf());
        for _ in 0..5 {
            let q = match cur {
                Some(q) => q,
                None => break,
            };
            cands.push(q.join("axc/stdlib/obj"));
            cands.push(q.join("stdlib/obj"));
            cur = q.parent().map(|x| x.to_path_buf());
        }
    }
    for c in cands {
        if c.exists() {
            return Some(c);
        }
    }
    None
}

/// axrt 运行时 .o 目标文件缓存目录（release/axc/rt/obj）：存放预编译后的 *.o 文件，
/// 用户编译时复用缓存，避免每次重新编译 axrt 运行时 C 源码。
pub fn rt_obj_dir() -> Option<PathBuf> {
    let mut cands = vec![
        PathBuf::from("axc/rt/obj"),
        PathBuf::from("rt/obj"),
    ];
    if let Ok(exe) = std::env::current_exe() {
        let mut cur = exe.parent().map(|d| d.to_path_buf());
        for _ in 0..5 {
            let q = match cur {
                Some(q) => q,
                None => break,
            };
            cands.push(q.join("axc/rt/obj"));
            cands.push(q.join("rt/obj"));
            cur = q.parent().map(|x| x.to_path_buf());
        }
    }
    for c in cands {
        if c.exists() {
            return Some(c);
        }
    }
    None
}

/// 计算文件内容的 64 位哈希值，用于缓存失效判断。
/// 基于文件内容而非修改时间，避免文件拷贝、touch 等操作导致的误判。
fn file_content_hash(path: &std::path::Path) -> Option<u64> {
    let mut f = fs::File::open(path).ok()?;
    let mut hasher = DefaultHasher::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.write(&buf[..n]);
    }
    Some(hasher.finish())
}

/// 读取缓存的哈希文件（与 .o 同名，扩展名为 .hash）。
fn read_cached_hash(hash_path: &std::path::Path) -> Option<u64> {
    let content = fs::read_to_string(hash_path).ok()?;
    content.trim().parse::<u64>().ok()
}

/// 写入缓存哈希文件，记录源文件编译时的内容哈希。
fn write_cached_hash(hash_path: &std::path::Path, hash: u64) {
    let _ = fs::write(hash_path, hash.to_string());
}

/// 将标准库 .c 文件预编译为 .o 并缓存至 obj_dir，返回可用于链接的 .o 文件路径列表。
/// 若缓存的 .o 对应的源文件内容未变，则直接复用；否则调用 gcc -c 编译后缓存。
pub fn stdlib_obj_files(
    stdlib_dir: &std::path::Path,
    obj_dir: &std::path::Path,
    cc: &str,
    rt_include: &std::path::Path,
    header_include: &std::path::Path,
) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = fs::read_dir(stdlib_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "c") {
                let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                let obj_path = obj_dir.join(format!("{}.o", stem));
                let hash_path = obj_dir.join(format!("{}.hash", stem));

                let need_compile = match (
                    obj_path.exists(),
                    hash_path.exists(),
                    file_content_hash(&path),
                ) {
                    (true, true, Some(current_hash)) => {
                        // 比较当前源文件内容哈希与缓存的哈希，内容未变则复用缓存
                        match read_cached_hash(&hash_path) {
                            Some(cached_hash) => cached_hash != current_hash,
                            None => true, // 无法读取缓存哈希，重新编译
                        }
                    }
                    _ => true, // .o 不存在、.hash 不存在、或无法读取源文件，都需要重新编译
                };

                if need_compile {
                    if let Err(e) = fs::create_dir_all(obj_dir) {
                        eprintln!(
                            "axc: 无法创建标准库 obj 缓存目录 {}：{}",
                            obj_dir.display(),
                            e
                        );
                        continue;
                    }
                    let mut cmd = std::process::Command::new(cc);
                    cmd.arg("-std=c11")
                        .arg("-O2")
                        .arg("-Wall")
                        .arg("-Wno-unused-function")
                        .arg("-Wno-return-type")
                        .arg("-I")
                        .arg(rt_include)
                        .arg("-I")
                        .arg(header_include)
                        .arg("-c")
                        .arg(&path)
                        .arg("-o")
                        .arg(&obj_path);
                    let out = cmd.output();
                    match out {
                        Ok(o) if o.status.success() => {
                            // 编译成功，写入内容哈希缓存
                            if let Some(h) = file_content_hash(&path) {
                                write_cached_hash(&hash_path, h);
                            }
                        }
                        Ok(o) => {
                            eprintln!("axc: 标准库 {} 预编译失败", path.display());
                            eprint!("{}", String::from_utf8_lossy(&o.stderr));
                            continue;
                        }
                        Err(e) => {
                            eprintln!("axc: 无法调用 C 编译器 {}：{}", cc, e);
                            continue;
                        }
                    }
                }

                if obj_path.exists() {
                    result.push(obj_path);
                }
            }
        }
    }
    result.sort();
    result
}

/// 将 axrt_*.c 运行时源文件预编译为 .o 并缓存至 obj_dir，返回可用于链接的 .o 文件路径列表。
/// 若缓存的 .o 已存在且比源 .c 新，则直接复用；否则调用 gcc -c 编译后缓存。
/// 会递归进入子目录查找所有 axrt_*.c 文件。
fn rt_obj_files(
    rt_dir: &std::path::Path,
    obj_dir: &std::path::Path,
    cc: &str,
) -> Vec<PathBuf> {
    let mut result = Vec::new();
    rt_obj_files_recursive(rt_dir, obj_dir, cc, rt_dir, &mut result);
    result.sort();
    result
}

/// 递归收集并预编译 axrt_*.c 文件。
fn rt_obj_files_recursive(
    dir: &std::path::Path,
    obj_dir: &std::path::Path,
    cc: &str,
    rt_include: &std::path::Path,
    result: &mut Vec<PathBuf>,
) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                rt_obj_files_recursive(&path, obj_dir, cc, rt_include, result);
            } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("axrt_") && name.ends_with(".c") {
                    let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                    let obj_path = obj_dir.join(format!("{}.o", stem));
                    let hash_path = obj_dir.join(format!("{}.hash", stem));

                    let need_compile = match (
                        obj_path.exists(),
                        hash_path.exists(),
                        file_content_hash(&path),
                    ) {
                        (true, true, Some(current_hash)) => {
                            // 比较当前源文件内容哈希与缓存的哈希，内容未变则复用缓存
                            match read_cached_hash(&hash_path) {
                                Some(cached_hash) => cached_hash != current_hash,
                                None => true, // 无法读取缓存哈希，重新编译
                            }
                        }
                        _ => true, // .o 不存在、.hash 不存在、或无法读取源文件，都需要重新编译
                    };

                    if need_compile {
                        if let Err(e) = fs::create_dir_all(obj_dir) {
                            eprintln!(
                                "axc: 无法创建运行时 obj 缓存目录 {}：{}",
                                obj_dir.display(),
                                e
                            );
                            continue;
                        }
                        let mut cmd = std::process::Command::new(cc);
                        cmd.arg("-std=c11")
                            .arg("-O2")
                            .arg("-Wall")
                            .arg("-Wno-unused-function")
                            .arg("-Wno-return-type")
                            .arg("-I")
                            .arg(rt_include)
                            .arg("-c")
                            .arg(&path)
                            .arg("-o")
                            .arg(&obj_path);
                        let out = cmd.output();
                        match out {
                            Ok(o) if o.status.success() => {
                                // 编译成功，写入内容哈希缓存
                                if let Some(h) = file_content_hash(&path) {
                                    write_cached_hash(&hash_path, h);
                                }
                            }
                            Ok(o) => {
                                eprintln!("axc: 运行时 {} 预编译失败", path.display());
                                eprint!("{}", String::from_utf8_lossy(&o.stderr));
                                continue;
                            }
                            Err(e) => {
                                eprintln!("axc: 无法调用 C 编译器 {}：{}", cc, e);
                                continue;
                            }
                        }
                    }

                    if obj_path.exists() {
                        result.push(obj_path);
                    }
                }
            }
        }
    }
}

/// 检查标准库 .o 缓存是否完整：generated/ 下的所有 .c 文件都有对应的 .o 缓存。
/// 若缓存不完整（如首次运行尚未生成），返回 false，backend 将正常生成标准库代码。
pub fn stdlib_obj_cache_complete() -> bool {
    let Some(sdir) = stdlib_gen_dir() else { return false; };
    let Some(odir) = stdlib_obj_dir() else { return false; };
    let c_files: Vec<_> = stdlib_c_files(&sdir);
    if c_files.is_empty() {
        return false;
    }
    c_files.iter().all(|cfile| {
        let stem = cfile.file_stem().unwrap().to_string_lossy().to_string();
        let obj_path = odir.join(format!("{}.o", stem));
        obj_path.exists()
    })
}

/// 检查标准库源码是否完整：generated/ 下是否已为每个 STDLIB_MODULES 生成 .c 文件。
pub fn stdlib_src_complete() -> bool {
    let Some(sdir) = stdlib_gen_dir() else { return false; };
    let c_files: Vec<_> = stdlib_c_files(&sdir);
    if c_files.is_empty() {
        return false;
    }
    STDLIB_MODULES.iter().all(|mod_name| {
        let short = mod_name.strip_prefix("std.").unwrap_or(mod_name);
        c_files.iter().any(|cf| {
            cf.file_stem()
                .and_then(|s| s.to_str())
                .map_or(false, |stem| stem == short)
        })
    })
}

/// 查找 axc/std/ 标准库源码目录
fn find_std_src_dir() -> Option<PathBuf> {
    let mut cands = vec![
        PathBuf::from("axc/std"),
        PathBuf::from("std"),
    ];
    if let Ok(exe) = std::env::current_exe() {
        let mut cur = exe.parent().map(|d| d.to_path_buf());
        for _ in 0..5 {
            let q = match cur {
                Some(q) => q,
                None => break,
            };
            cands.push(q.join("axc/std"));
            cands.push(q.join("std"));
            cur = q.parent().map(|x| x.to_path_buf());
        }
    }
    for c in cands {
        if c.join("string.ax").exists() {
            return Some(c);
        }
    }
    None
}

/// 标准库模块名：从 axc/std/ 下的 .ax 文件名推导，如 string.ax -> std.string
fn stdlib_mod_name(ax_file: &std::path::Path) -> Option<String> {
    let stem = ax_file.file_stem()?.to_string_lossy().to_string();
    Some(format!("std.{}", stem))
}

/// 预编译标准库：为 axc/std/ 下的每个 .ax 模块生成独立的 .c 文件，
/// 写入 release/axc/stdlib/generated/，供后续编译为 .o 缓存。
/// 返回生成的 .c 文件路径列表。
pub fn precompile_stdlib() -> Vec<PathBuf> {
    let std_src_dir = match find_std_src_dir() {
        Some(d) => d,
        None => {
            eprintln!("axc: 找不到标准库源码目录 axc/std/");
            return Vec::new();
        }
    };

    // 生成目录：stdlib_gen_dir() 已解析为 axc/stdlib/generated/（存放 .c 文件的目录），
    // 与头文件 axc_stdlib.h（在 axc/stdlib/）分离。
    // 以 stdlib_gen_dir() 解析的目录为基准，确保预编译写入的位置与
    // compile_c_files() 读取 .c / .o 的位置一致。
    let stdlib_dir = match stdlib_gen_dir() {
        Some(d) => d,
        None => {
            eprintln!("axc: 找不到标准库目录 axc/stdlib/");
            return Vec::new();
        }
    };
    let gen_dir = stdlib_dir;
    if fs::create_dir_all(&gen_dir).is_err() {
        eprintln!("axc: 无法创建标准库生成目录 {}", gen_dir.display());
        return Vec::new();
    }

    let mut ax_files: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(&std_src_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "ax") {
                ax_files.push(path);
            }
        }
    }
    ax_files.sort();

    let mut generated = Vec::new();

    for ax_path in &ax_files {
        let short_name = match ax_path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let mod_name = format!("std.{}", short_name);

        // 读取源码
        let src = match fs::read_to_string(ax_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("axc: 无法读取标准库 {}：{}", mod_name, e);
                continue;
            }
        };

        // 词法+语法分析
        let mut d = Diags::new();
        d.lang = Lang::resolve(None);
        let normalized = lexer::normalize(&src);
        d.add_source(ax_path.to_string_lossy().as_ref(), &normalized);
        let toks = lexer::lex(&src, ax_path.to_string_lossy().as_ref(), &mut d);

        if d.has_error() {
            eprintln!("axc: 标准库 {} 词法/语法错误，跳过预编译", mod_name);
            eprintln!("{}", d.format_text());
            continue;
        }

        let mod_ = parser::parse_module(toks, ax_path.to_string_lossy().as_ref(), &mut d, 0);

        if d.has_error() {
            eprintln!("axc: 标准库 {} 解析错误，跳过预编译", mod_name);
            eprintln!("{}", d.format_text());
            continue;
        }

        // 语义分析
        let mut s = sema::analyze_multi(
            &mod_,
            &mut d,
            ax_path.to_string_lossy().as_ref(),
            false, // require_main
            Vec::new(), // extras（stdlib 模块不互相导入）
            HashMap::new(), // path_canon
        );

        if (*s.d).has_error() {
            eprintln!("axc: 标准库 {} 语义错误，跳过预编译", mod_name);
            eprintln!("{}", (*s.d).format_text());
            continue;
        }

        // 生成 C 代码（skip_stdlib=true，仅生成目标模块）
        let csrc = CodeGen::build_c(
            &s,
            ax_path.to_string_lossy().as_ref(),
            "exe",
            true,
            Some(&mod_name),
        );

        // 写入生成目录：<模块名>.c
        let c_file_name = format!("{}.c", short_name);
        let c_path = gen_dir.join(&c_file_name);
        match fs::write(&c_path, &csrc) {
            Ok(()) => {
                generated.push(c_path);
            }
            Err(e) => {
                eprintln!("axc: 无法写入标准库 C 文件 {}：{}", c_path.display(), e);
            }
        }
    }

    generated
}

pub fn compile_c_files(
    csrc: &str,
    opts: &crate::cli::Opts,
    out_path: &std::path::Path,
    kind: &str,
    skip_stdlib: bool,
) -> Option<()> {
    let rt = match rt_dir() {
        Some(r) => r,
        None => {
            eprintln!("axc: 找不到运行时目录 axc/rt（缺 axrt.h / axrt.c）");
            return None;
        }
    };
    let stdlib_dir = stdlib_gen_dir();

    let workdir = std::env::temp_dir().join(format!(
        "axc-{}-{}",
        std::process::id(),
        now_nanos()
    ));
    if fs::create_dir_all(&workdir).is_err() {
        eprintln!("axc: 无法创建构建目录 {}", workdir.display());
        return None;
    }

    // 将用户生成的 C 源码写入 unit.c
    let cpath = workdir.join("unit.c");
    if fs::write(&cpath, csrc).is_err() {
        eprintln!("axc: 无法写入 {}", cpath.display());
        return None;
    }

    // 复制 axrt.h 和 axrt_internal.h 到工作目录
    let _ = fs::copy(rt.join("axrt.h"), workdir.join("axrt.h"));
    let _ = fs::copy(rt.join("axrt_internal.h"), workdir.join("axrt_internal.h"));
    // axrt 运行时：优先使用预编译的 .o 缓存，避免每次重新编译 5000+ 行运行时代码
    let mut rt_obj_in_workdir: Vec<PathBuf> = Vec::new();
    let mut rt_c_in_workdir: Vec<PathBuf> = Vec::new();
    if let Some(odir) = rt_obj_dir() {
        let objs = rt_obj_files(&rt, &odir, &opts.cc);
        for obj in &objs {
            let fname = std::path::PathBuf::from(obj.file_name().unwrap());
            let dst = workdir.join(fname);
            if fs::copy(obj, &dst).is_ok() {
                rt_obj_in_workdir.push(dst);
            }
        }
    } else {
        // 回退：未找到 obj 缓存目录，直接复制 .c 文件（保持向后兼容）
        for cfile in rt_c_files(&rt) {
            let fname = std::path::PathBuf::from(cfile.file_name().unwrap());
            let dst = workdir.join(fname);
            if fs::copy(&cfile, &dst).is_ok() {
                rt_c_in_workdir.push(dst);
            }
        }
    }

    // 复制标准库头文件（从 axc/stdlib/，与生成的 .c 目录分离）
    let mut stdlib_obj_in_workdir: Vec<PathBuf> = Vec::new();
    let mut stdlib_c_in_workdir: Vec<PathBuf> = Vec::new();
    if let Some(ref hdir) = stdlib_header_dir() {
        let _ = fs::copy(hdir.join("axc_stdlib.h"), workdir.join("axc_stdlib.h"));
        // skip_stdlib=true 时，标准库代码不在 unit.c 中，需链接预编译的 .o 缓存；
        // skip_stdlib=false 时，标准库已合并进 unit.c，此处不再单独链接 .c/.o，
        // 避免与 unit.c 中的符号重复定义。
        if skip_stdlib {
            if let Some(odir) = stdlib_obj_dir() {
                if let Some(ref sdir) = stdlib_dir {
                    let objs = stdlib_obj_files(sdir, &odir, &opts.cc, &rt, hdir);
                    for obj in &objs {
                        let fname = std::path::PathBuf::from(obj.file_name().unwrap());
                        let dst = workdir.join(fname);
                        if fs::copy(obj, &dst).is_ok() {
                            stdlib_obj_in_workdir.push(dst);
                        }
                    }
                }
            }
        }
    }

    let out_abs = if out_path.is_absolute() {
        out_path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|c| c.join(out_path))
            .unwrap_or_else(|_| out_path.to_path_buf())
    };

    let is_shared = kind == "shared";

    // 共享库模式：收集导出符号
    let mut export_list_path: Option<PathBuf> = None;
    if is_shared {
        let exports: Vec<String> = csrc
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                let pos = match trimmed.find("ax_export_") {
                    Some(p) => p,
                    None => return None,
                };
                let rest = &trimmed[pos..];
                let end = rest
                    .find(|c: char| c == '(' || c.is_whitespace())
                    .unwrap_or(rest.len());
                let name = &rest[..end];
                if !name.is_empty() {
                    Some(name.to_string())
                } else {
                    None
                }
            })
            .collect();
        if !exports.is_empty() {
            if cfg!(windows) {
                let def_path = workdir.join("exports.def");
                let mut def_content = String::from("EXPORTS\n");
                for name in &exports {
                    def_content.push_str(&format!("    {}\n", name));
                }
                if fs::write(&def_path, &def_content).is_ok() {
                    export_list_path = Some(def_path);
                }
            } else {
                let map_path = workdir.join("exports.map");
                let mut map_content = String::from("{\n  global:\n");
                for (i, name) in exports.iter().enumerate() {
                    let comma = if i + 1 < exports.len() { ";" } else { "" };
                    map_content.push_str(&format!("    {};{}\n", name, comma));
                }
                map_content.push_str("  local: *;\n};\n");
                if fs::write(&map_path, &map_content).is_ok() {
                    export_list_path = Some(map_path);
                }
            }
        }
    }

    // 构建 gcc 命令行：unit.c + axrt_*.c + 标准库 .c 文件
    let mut cmd = std::process::Command::new(&opts.cc);
    cmd.arg("-std=c11")
        .arg("-O2")
        .arg("-Wall")
        .arg("-Wno-unused-function")
        .arg("-Wno-return-type")
        .arg("-I")
        .arg(&workdir)
        .arg("-o")
        .arg(&out_abs)
        .arg(&cpath);

    // 添加所有 axrt 运行时文件：优先 .o 缓存，回退到 .c 源文件
    for obj in &rt_obj_in_workdir {
        cmd.arg(obj);
    }
    for cfile in &rt_c_in_workdir {
        cmd.arg(cfile);
    }

    cmd.arg("-lm")
        .current_dir(&workdir);

    // 添加标准库 .o 文件（缓存策略）或回退到 .c 源文件
    for obj in &stdlib_obj_in_workdir {
        cmd.arg(obj);
    }
    for cfile in &stdlib_c_in_workdir {
        cmd.arg(cfile);
    }

    if is_shared {
        cmd.arg("-shared");
        if cfg!(windows) {
            if let Some(ref def_path) = export_list_path {
                cmd.arg(def_path);
            }
        } else {
            if let Some(ref map_path) = export_list_path {
                cmd.arg(format!("-Wl,--version-script={}", map_path.display()));
            }
        }
    }

    let out = cmd.output();
    let out = match out {
        Ok(o) => o,
        Err(e) => {
            eprintln!("axc: 无法调用 C 编译器 {}：{}", opts.cc, e);
            return None;
        }
    };
    if !out.status.success() {
        eprintln!("axc: C 编译失败");
        eprint!("{}", String::from_utf8_lossy(&out.stderr));
        eprintln!("\n--- 生成的 C 源码保留于 {} ---", cpath.display());
        return None;
    }
    let err_txt = String::from_utf8_lossy(&out.stderr).to_string();
    if !err_txt.trim().is_empty() {
        eprint!("{}", err_txt);
    }
    if opts.keep {
        eprintln!("axc: 构建目录 {}", workdir.display());
    } else {
        let _ = fs::remove_dir_all(&workdir);
    }
    Some(())
}

fn now_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}
