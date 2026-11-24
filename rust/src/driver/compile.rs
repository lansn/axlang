//! driver/compile.rs —— axlang 编译驱动：C 多文件编译链接
//!
//! 职责：将用户生成的 C 源码（unit.c）、标准库预转译的 .c 文件
//!（axc/stdlib/generated/*.c）与 axrt.c 一起传给 gcc 编译链接。
//!
//! 这是“ax 标准库预转译”架构的一部分：ax 编译器专注于词法/语法/语义分析，
//! 标准库以 ax 语法编写后预转译为标准 C，由本驱动模块统一调用 gcc 完成最终编译。

use std::fs;
use std::path::PathBuf;

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

/// 收集运行时目录下的所有 axrt_*.c 文件路径。
pub fn rt_c_files(rt_dir: &std::path::Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(rt_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("axrt_") && name.ends_with(".c") {
                    files.push(path);
                }
            }
        }
    }
    files.sort();
    files
}

/// 标准库预转译输出目录（axc/stdlib/generated）：存放 *.ax 预转译后的 *.c 文件。
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

/// 将标准库 .c 文件预编译为 .o 并缓存至 obj_dir，返回可用于链接的 .o 文件路径列表。
/// 若缓存的 .o 已存在且比源 .c 新，则直接复用；否则调用 gcc -c 编译后缓存。
fn stdlib_obj_files(
    stdlib_dir: &std::path::Path,
    obj_dir: &std::path::Path,
    cc: &str,
) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = fs::read_dir(stdlib_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "c") {
                let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                let obj_path = obj_dir.join(format!("{}.o", stem));

                let need_compile = match (
                    obj_path.exists(),
                    path.metadata(),
                    obj_path.metadata(),
                ) {
                    (true, Ok(src_meta), Ok(obj_meta)) => {
                        // 若 .o 比 .c 新，则复用缓存；否则需要重新编译
                        src_meta.modified().map_or(true, |src_m| {
                            obj_meta.modified().map_or(true, |obj_m| obj_m < src_m)
                        })
                    }
                    _ => true,
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
                        .arg("-c")
                        .arg(&path)
                        .arg("-o")
                        .arg(&obj_path);
                    let out = cmd.output();
                    match out {
                        Ok(o) if o.status.success() => {}
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

/// 把多个 C 源文件与 axrt.c、标准库 .c 一起编译成可执行文件或动态链接库。
pub fn compile_c_files(
    csrc: &str,
    opts: &crate::cli::Opts,
    out_path: &std::path::Path,
    kind: &str,
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
    let mut rt_c_in_workdir: Vec<PathBuf> = Vec::new();
    for cfile in rt_c_files(&rt) {
        let fname = std::path::PathBuf::from(cfile.file_name().unwrap());
        let dst = workdir.join(fname);
        if fs::copy(&cfile, &dst).is_ok() {
            rt_c_in_workdir.push(dst);
        }
    }

    // 复制标准库头文件（如有）
    let mut stdlib_obj_in_workdir: Vec<PathBuf> = Vec::new();
    let mut stdlib_c_in_workdir: Vec<PathBuf> = Vec::new();
    if let Some(ref sdir) = stdlib_dir {
        let _ = fs::copy(sdir.join("axc_stdlib.h"), workdir.join("axc_stdlib.h"));
        // 标准库 .o 缓存策略：优先使用缓存的 .o，避免每次重新编译标准库 .c
        if let Some(odir) = stdlib_obj_dir() {
            let objs = stdlib_obj_files(sdir, &odir, &opts.cc);
            for obj in &objs {
                let fname = std::path::PathBuf::from(obj.file_name().unwrap());
                let dst = workdir.join(fname);
                if fs::copy(obj, &dst).is_ok() {
                    stdlib_obj_in_workdir.push(dst);
                }
            }
        } else {
            // 回退：未找到 obj 缓存目录，直接复制 .c 文件（保持向后兼容）
            for cfile in stdlib_c_files(sdir) {
                let fname = std::path::PathBuf::from(cfile.file_name().unwrap());
                let dst = workdir.join(fname);
                if fs::copy(&cfile, &dst).is_ok() {
                    stdlib_c_in_workdir.push(dst);
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

    // 添加所有 axrt_*.c 运行时源文件
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
