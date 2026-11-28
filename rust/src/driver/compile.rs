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
use std::process::Command;

use crate::backend::CodeGen;
use crate::diag::Diags;
use crate::diag_msg::Lang;
use crate::frontend::{lexer, parser};
use crate::sema::{self, STDLIB_MODULES};

/// 检查指定的 C 编译器是否可用（通过运行 `--version` 验证）。
pub fn check_compiler_available(cc: &str) -> bool {
    Command::new(cc)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// 生成 C 编译器缺失时的完整错误提示和解决方案。
/// 返回给用户的错误信息字符串。
pub fn compiler_missing_error(cc: &str) -> String {
    let platform_tips = if cfg!(windows) {
        r#"
Windows 平台解决方案：
  1. 安装 MinGW-w64（推荐）：
     - 下载：https://sourceforge.net/projects/mingw-w64/files/latest/download
     - 安装后将 <安装目录>/bin 添加到系统 PATH 环境变量
     - 或将 gcc.exe 所在目录复制到 axc 同目录下，axc 会自动发现
  2. 安装 MSVC（Visual Studio C++）：
     - 安装 Visual Studio 2019/2022，勾选「使用 C++ 的桌面开发」工作负载
     - axc 会自动查找常见 VC++ 安装目录下的 cl.exe
  3. 手动指定编译器路径：
     - axc build <file.ax> --cc="C:/mingw64/bin/gcc.exe"
     - axc env --set=AX_CC="C:/mingw64/bin/gcc.exe"（持久化保存）
  4. 验证编译器是否可用：
     - 在命令行运行：gcc --version
     - 若提示 "gcc 不是内部或外部命令"，说明未正确安装或未加入 PATH"#
    } else {
        r#"
Linux/macOS 平台解决方案：
  1. 安装 GCC（推荐）：
     - Ubuntu/Debian：sudo apt-get install build-essential
     - CentOS/RHEL：sudo yum install gcc
     - macOS：xcode-select --install  或  brew install gcc
  2. 安装 Clang：
     - Ubuntu/Debian：sudo apt-get install clang
     - macOS：brew install llvm
  3. 手动指定编译器路径：
     - axc build <file.ax> --cc=/usr/bin/gcc
     - axc env --set=AX_CC=/usr/bin/gcc（持久化保存）
  4. 验证编译器是否可用：
     - 在终端运行：gcc --version
     - 若提示 "command not found"，说明未正确安装或未加入 PATH"#
    };
    format!(
        r#"
╔════════════════════════════════════════════════════════════════════════════╗
║  axc 编译错误：找不到 C 编译器 "{}"                                 ║
╚════════════════════════════════════════════════════════════════════════════╝

axc 语言需要系统 C 编译器来生成最终可执行文件，但未能找到可用的 {}。

自动查找顺序：
  1. AX_CC 环境变量
  2. CC 环境变量
  3. axc 同目录的 .axc_cc_cache 缓存文件
  4. axc 可执行文件所在目录及其 bin/、gcc/bin/、clang/bin/ 子目录
  5. 系统 PATH 环境变量
  6. Windows：常见 VC++ 安装目录
  7. 回退到裸 "gcc"

建议操作：{}
{} 缓存文件路径：<axc 同目录>/.axc_cc_cache
示例：axc env --set=AX_CC=<你的编译器路径>
     axc build <file.ax> --cc=<你的编译器路径>"#,
        cc,
        cc,
        platform_tips,
        cc
    )
}

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

/// 标准库 .o 目标文件缓存目录（dist/axc/stdlib/obj）：存放预编译后的 *.o 文件，
/// 用户编译时复用缓存，避免每次重新编译标准库 .c 源码。
/// 查找顺序：1. AXC_OBJ_DIR 环境变量指定的目录下的 stdlib/obj；
///           2. axc.exe 所在位置上溯的已存在目录，不自动创建，不回退到 CWD/项目目录。
pub fn stdlib_obj_dir() -> Option<PathBuf> {
    // 优先使用环境变量显式指定的缓存目录
    if let Ok(obj_root) = std::env::var("AXC_OBJ_DIR") {
        let p = PathBuf::from(&obj_root).join("stdlib/obj");
        if p.exists() {
            return Some(p);
        }
    }
    let mut cands: Vec<PathBuf> = Vec::new();
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
    for c in &cands {
        if c.exists() {
            return Some(c.clone());
        }
    }
    None
}

/// axrt 运行时 .o 目标文件缓存目录（dist/axc/rt/obj）：存放预编译后的 *.o 文件，
/// 用户编译时复用缓存，避免每次重新编译 axrt 运行时 C 源码。
/// 查找顺序：1. AXC_OBJ_DIR 环境变量指定的目录下的 rt/obj；
///           2. axc.exe 所在位置上溯的已存在目录，不自动创建，不回退到 CWD/项目目录。
pub fn rt_obj_dir() -> Option<PathBuf> {
    // 优先使用环境变量显式指定的缓存目录
    if let Ok(obj_root) = std::env::var("AXC_OBJ_DIR") {
        let p = PathBuf::from(&obj_root).join("rt/obj");
        if p.exists() {
            return Some(p);
        }
    }
    let mut cands: Vec<PathBuf> = Vec::new();
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
    for c in &cands {
        if c.exists() {
            return Some(c.clone());
        }
    }
    None
}

/// 用户项目文件 .o 目标文件缓存目录：存放用户源文件编译后的 *.o 文件及其 .hash 缓存。
/// 缓存位于用户项目目录下的 `cache/` 子目录，不再写入编译器安装目录（dist/axc/），
/// 避免扰乱发行包结构。
pub fn user_obj_dir(project_dir: &std::path::Path) -> PathBuf {
    project_dir.join("cache")
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
pub fn rt_obj_files(
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


/// 多文件编译：将多个 C 源码分别编译为独立的 {stem}.o（带 hash 缓存），
/// 然后全部与 axrt / 标准库 .o 一起链接。
pub fn compile_multi_c_files(
    srcs: &[(String, String)], // (c_source, file_path)
    opts: &crate::cli::Opts,
    out_path: &std::path::Path,
    kind: &str,
    _skip_stdlib: bool,
) -> Option<()> {
    let rt = match rt_dir() {
        Some(r) => r,
        None => {
            eprintln!("axc: 找不到运行时目录 axc/rt（缺 axrt.h / axrt.c）");
            return None;
        }
    };

    let workdir = std::env::temp_dir().join(format!(
        "axc-{}-{}",
        std::process::id(),
        now_nanos()
    ));
    if fs::create_dir_all(&workdir).is_err() {
        eprintln!("axc: 无法创建构建目录 {}", workdir.display());
        return None;
    }

    // 复制 axrt.h 和 axrt_internal.h 到工作目录
    let _ = fs::copy(rt.join("axrt.h"), workdir.join("axrt.h"));
    let _ = fs::copy(rt.join("axrt_internal.h"), workdir.join("axrt_internal.h"));

    // axrt 运行时 .o：直接复用预编译缓存，不 hash 检查、不重新编译
    let mut rt_objs: Vec<PathBuf> = Vec::new();
    let rt_odir = match rt_obj_dir() {
        Some(d) => d,
        None => {
            eprintln!("axc: 找不到 axrt 运行时 .o 缓存目录（dist/axc/rt/obj），请确保发行包完整或设置 AXC_OBJ_DIR");
            return None;
        }
    };
    if let Ok(entries) = fs::read_dir(&rt_odir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "o") {
                let fname = path.file_name().unwrap().to_string_lossy().to_string();
                let dst = workdir.join(&fname);
                if fs::copy(&path, &dst).is_ok() {
                    rt_objs.push(dst);
                }
            }
        }
    }
    rt_objs.sort();

    // 复制标准库头文件（从 axc/stdlib/，与生成的 .c 目录分离）
    let hdir = match stdlib_header_dir() {
        Some(d) => d,
        None => {
            eprintln!("axc: 找不到标准库头文件目录 axc/stdlib（缺 axc_stdlib.h）");
            return None;
        }
    };
    let _ = fs::copy(hdir.join("axc_stdlib.h"), workdir.join("axc_stdlib.h"));

    // 标准库 .o：直接复用预编译缓存，不 hash 检查、不重新编译
    let mut stdlib_objs: Vec<PathBuf> = Vec::new();
    let stdlib_odir = match stdlib_obj_dir() {
        Some(d) => d,
        None => {
            eprintln!("axc: 找不到标准库 .o 缓存目录（dist/axc/stdlib/obj），请确保发行包完整或设置 AXC_OBJ_DIR");
            return None;
        }
    };
    if let Ok(entries) = fs::read_dir(&stdlib_odir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "o") {
                let fname = path.file_name().unwrap().to_string_lossy().to_string();
                let dst = workdir.join(&fname);
                if fs::copy(&path, &dst).is_ok() {
                    stdlib_objs.push(dst);
                }
            }
        }
    }
    stdlib_objs.sort();

    // 确定用户项目目录：取第一个源文件的父目录作为项目根，缓存写入其下的 cache/ 子目录
    // 规范为绝对路径，避免 gcc 在 workdir 中运行时相对路径解析错误
    let project_dir = std::path::Path::new(&srcs[0].1)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
    let project_dir = if project_dir.is_relative() {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join(&project_dir)
    } else {
        project_dir
    };

    // 为每个用户源文件独立编译为 {stem}.o，带 hash 缓存
    let mut user_objs: Vec<PathBuf> = Vec::new();
    for (csrc, path) in srcs {
        let stem = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unit")
            .to_string();

        let cpath = workdir.join(format!("{}.c", stem));
        if fs::write(&cpath, csrc).is_err() {
            eprintln!("axc: 无法写入 {}", cpath.display());
            return None;
        }

        let obj_path = compile_user_file_to_obj(&cpath, &stem, &workdir, &project_dir, opts)?;
        let obj_display = obj_path.display().to_string();
        user_objs.push(obj_path);
        println!("axc: 用户文件 {} -> {} (缓存)", stem, obj_display);
    }

    let out_abs = if out_path.is_absolute() {
        out_path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|c| c.join(out_path))
            .unwrap_or_else(|_| out_path.to_path_buf())
    };

    let is_shared = kind == "shared";
    let is_static = kind == "static";

    // 共享库模式：收集导出符号（从所有 C 源码中扫描 ax_export_）
    let mut export_list_path: Option<PathBuf> = None;
    if is_shared {
        let mut all_exports: Vec<String> = Vec::new();
        for (csrc, _) in srcs {
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
            all_exports.extend(exports);
        }
        all_exports.sort();
        all_exports.dedup();

        if !all_exports.is_empty() {
            if cfg!(windows) {
                let def_path = workdir.join("exports.def");
                let mut def_content = String::from("EXPORTS\n");
                for name in &all_exports {
                    def_content.push_str(&format!("    {}\n", name));
                }
                if fs::write(&def_path, &def_content).is_ok() {
                    export_list_path = Some(def_path);
                }
            } else {
                let map_path = workdir.join("exports.map");
                let mut map_content = String::from("{\n  global:\n");
                for (i, name) in all_exports.iter().enumerate() {
                    let comma = if i + 1 < all_exports.len() { ";" } else { "" };
                    map_content.push_str(&format!("    {};{}\n", name, comma));
                }
                map_content.push_str("  local: *;\n};\n");
                if fs::write(&map_path, &map_content).is_ok() {
                    export_list_path = Some(map_path);
                }
            }
        }
    }

    // 静态链接库：将所有 .o 打包为 .a/.lib
    if is_static {
        let mut all_objs: Vec<PathBuf> = Vec::new();
        all_objs.extend(user_objs);
        all_objs.extend(rt_objs);
        all_objs.extend(stdlib_objs);

        let ar_path = std::path::Path::new(&opts.cc)
            .parent()
            .and_then(|dir| {
                ["ar.exe", "ar", "llvm-ar.exe", "llvm-ar", "gcc-ar.exe", "gcc-ar"]
                    .iter()
                    .find_map(|name| {
                        let p = dir.join(name);
                        p.is_file().then(|| p.to_string_lossy().to_string())
                    })
            })
            .unwrap_or_else(|| "ar".to_string());

        let mut ar_cmd = std::process::Command::new(&ar_path);
        ar_cmd.arg("rcs").arg(&out_abs);
        for obj in &all_objs {
            ar_cmd.arg(obj);
        }
        let ar_out = ar_cmd.current_dir(&workdir).output();
        match ar_out {
            Ok(o) if o.status.success() => {}
            Ok(o) => {
                eprintln!("axc: 静态库打包失败（ar rcs）");
                eprint!("{}", String::from_utf8_lossy(&o.stderr));
                return None;
            }
            Err(e) => {
                eprintln!("axc: 无法调用 ar 工具 {}：{}", ar_path, e);
                return None;
            }
        }

        if opts.keep {
            eprintln!("axc: 构建目录 {}", workdir.display());
        } else {
            let _ = fs::remove_dir_all(&workdir);
        }
        return Some(());
    }

    // 构建 gcc 链接命令：所有用户 .o + axrt .o + 标准库 .o
    let mut cmd = std::process::Command::new(&opts.cc);
    cmd.arg("-std=c11")
        .arg("-O2")
        .arg("-Wall")
        .arg("-Wno-unused-function")
        .arg("-Wno-return-type")
        .arg("-I")
        .arg(&workdir)
        .arg("-o")
        .arg(&out_abs);

    // 添加所有用户 .o（独立编译的翻译单元）
    for obj in &user_objs {
        cmd.arg(obj);
    }

    // 添加所有 axrt 运行时 .o（预编译缓存，直接链接）
    for obj in &rt_objs {
        cmd.arg(obj);
    }

    cmd.arg("-lm")
        .current_dir(&workdir);

    // 添加所有标准库 .o（预编译缓存，直接链接）
    for obj in &stdlib_objs {
        cmd.arg(obj);
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

    // 在调用 gcc 之前，先检查编译器是否可用，避免用户等待漫长过程后才报错
    if !check_compiler_available(&opts.cc) {
        eprintln!("{}", compiler_missing_error(&opts.cc));
        return None;
    }

    let out = cmd.output();
    let out = match out {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{}", compiler_missing_error(&opts.cc));
            eprintln!("底层错误：{}", e);
            return None;
        }
    };
    if !out.status.success() {
        eprintln!("axc: C 编译失败");
        eprint!("{}", String::from_utf8_lossy(&out.stderr));
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

/// 将单个用户 C 源文件编译为 {stem}.o，带 hash 缓存。
/// 缓存目录：`<project_dir>/cache/`，不再写入编译器安装目录。
fn compile_user_file_to_obj(
    cpath: &std::path::Path,
    stem: &str,
    workdir: &std::path::Path,
    project_dir: &std::path::Path,
    opts: &crate::cli::Opts,
) -> Option<PathBuf> {
    let cache_dir = user_obj_dir(project_dir);
    let obj_path = cache_dir.join(format!("{}.o", stem));
    let hash_path = cache_dir.join(format!("{}.hash", stem));
    let need_compile = match (
        obj_path.exists(),
        hash_path.exists(),
        file_content_hash(cpath),
    ) {
        (true, true, Some(current_hash)) => {
            match read_cached_hash(&hash_path) {
                Some(cached_hash) => cached_hash != current_hash,
                None => true,
            }
        }
        _ => true,
    };
    if need_compile {
        if let Err(e) = fs::create_dir_all(&cache_dir) {
            eprintln!("axc: 无法创建用户 obj 缓存目录 {}：{}", cache_dir.display(), e);
            return None;
        }
        let mut cc_cmd = std::process::Command::new(&opts.cc);
        cc_cmd.arg("-std=c11")
            .arg("-O2")
            .arg("-Wall")
            .arg("-Wno-unused-function")
            .arg("-Wno-return-type")
            .arg("-I")
            .arg(workdir)
            .arg("-c")
            .arg(cpath)
            .arg("-o")
            .arg(&obj_path)
            .current_dir(workdir);
        let cc_out = cc_cmd.output();
        match cc_out {
            Ok(o) if o.status.success() => {
                if let Some(h) = file_content_hash(cpath) {
                    write_cached_hash(&hash_path, h);
                }
            }
            Ok(o) => {
                eprintln!("axc: 用户文件 {} 编译失败", cpath.display());
                eprint!("{}", String::from_utf8_lossy(&o.stderr));
                return None;
            }
            Err(e) => {
                eprintln!("axc: 无法调用 C 编译器 {}：{}", opts.cc, e);
                return None;
            }
        }
    }
    if obj_path.exists() {
        Some(obj_path)
    } else {
        None
    }
}

fn now_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}
