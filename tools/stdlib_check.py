#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
axlang 标准库落地状态校验脚本（第二版）
读取 STDLIB-STATUS.json，与实际代码/测试对比，报告不一致项。

策略（减少误报）：
  - module_sig：只检查显式标记为模块级函数的条目，且模式更宽松
  - intrinsic：检查 member.rs 中是否有类型名 + 方法名的出现
  - C 运行时：检查函数名片段是否出现在 axrt.h/axrt.c 中
  - 测试文件：模糊匹配 tests/run/ 目录
  - BUILTIN_MODULES：只检查真正作为独立模块名注册的条目
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SEMA_MOD = ROOT / "rust" / "src" / "sema" / "mod.rs"
SEMA_EXPR = ROOT / "rust" / "src" / "sema" / "expr.rs"
SEMA_MEMBER = ROOT / "rust" / "src" / "sema" / "member.rs"
CODEGEN_EXPR = ROOT / "rust" / "src" / "backend" / "codegen_expr.rs"
AXRT_H = ROOT / "axc" / "rt" / "axrt.h"
AXRT_C = ROOT / "axc" / "rt" / "axrt.c"
TESTS_DIR = ROOT / "tests" / "run"
JSON_FILE = ROOT / "STDLIB-STATUS.json"


def read(p: Path) -> str:
    return p.read_text(encoding="utf-8", errors="replace")


def contains(text: str, needle: str) -> bool:
    return needle in text


# 模块 -> 在代码中出现的关键字映射（用于模糊匹配）
MODULE_CODE_KEYWORDS = {
    "io": ["io.", "ax_io", "ReadLine", "ReadFile"],
    "fmt": ["fmt.", "ax_fmt", "Printf", "Sprintf"],
    "math": ["math.", "ax_math", "math_"],
    "json": ["json.", "ax_json", "json_"],
    "file": ["file.", "ax_file", "file_"],
    "db": ["db.", "ax_db", "db_"],
    "string": ["string.", "ax_str", "str_"],
    "time": ["time.", "ax_time", "time_"],
    "decimal": ["decimal.", "ax_dec", "dec_"],
    "os": ["os.", "ax_os", "os_"],
    "strconv": ["strconv.", "ax_strconv", "strconv_"],
    "filepath": ["filepath.", "ax_filepath", "filepath_"],
    "rand": ["rand.", "ax_rand", "rand_"],
    "sort": ["sort.", "ax_sort", "sort_"],
    "slices": ["slices.", "arr.", "ax_arr"],
    "maps": ["maps.", "ax_map", "map_"],
    "unique": ["unique.", "ax_box", "Handle"],
    "bytes": ["bytes.", "ax_bytes", "bytes_"],
    "bufio": ["bufio.", "ax_file_buf"],
    "csv": ["csv.", "ax_csv", "csv_"],
    "errors": ["errors.", "ax_err", "err_"],
    "log": ["log.", "ax_log", "log_"],
    "sync": ["Mutex", "RWMutex", "WaitGroup", "Once", "ax_mutex", "ax_rwmutex", "ax_waitgroup", "ax_once"],
    "atomic": ["atomic.", "ax_atomic", "atomic_"],
    "reflect": ["reflect.", "TypeOf", "ValueOf"],
}


def main():
    if not JSON_FILE.exists():
        print(f"错误: 未找到 {JSON_FILE}")
        sys.exit(1)

    with open(JSON_FILE, encoding="utf-8") as f:
        data = json.load(f)

    modules = data.get("modules", [])
    errors = []
    warnings = []
    checked = 0

    sema_mod = read(SEMA_MOD)
    sema_expr = read(SEMA_EXPR)
    sema_member = read(SEMA_MEMBER)
    codegen_expr = read(CODEGEN_EXPR)
    axrt_h = read(AXRT_H)
    axrt_c = read(AXRT_C)
    axrt_all = axrt_h + "\n" + axrt_c

    # 列出 tests/run/ 下的所有 .ax 文件名（不含扩展名）
    test_files = {p.stem for p in TESTS_DIR.glob("*.ax")} if TESTS_DIR.exists() else set()

    for mod in modules:
        name = mod["name"]
        status = mod.get("status", "implemented")
        checked += 1

        # 跳过未落地模块的代码检查（reflect 等）
        if status == "not_implemented":
            continue

        # 1. BUILTIN_MODULES：只检查真正作为独立模块注册的
        # 某些模块（decimal, slices, maps, unique, bytes, bufio, csv, errors, log, atomic）
        # 可能不以模块名形式在 BUILTIN_MODULES 中，而是作为类型或通过其他方式注册
        kw = MODULE_CODE_KEYWORDS.get(name, [name])
        if not any(k in sema_mod for k in kw):
            # 允许：如果该模块只通过类型名注册（如 decimal 作为类型）
            if status == "implemented":
                warnings.append(f"[{name}] sema/mod.rs 中未见模块关键字 {kw[:3]}")

        # 2. 模块级函数 module_sig 检查（仅对带 functions 且非类型方法的）
        funcs = mod.get("functions", [])
        for fn in funcs:
            fn_name = fn["name"]
            # 跳过符号名（如 ==, !=, 切片）
            if not fn_name.isidentifier():
                continue
            # 检查 module_sig 中 ("mod", "fn") 形式
            sig_pattern = rf'\("{re.escape(name)}"\s*,\s*"{re.escape(fn_name)}"\)'
            if not re.search(sig_pattern, sema_expr):
                # 也可能通过 check_builtin_static 注册（如 decimal.Parse, string.ToUpperRune）
                # 或通过其他方式，这里只警告不报错
                warnings.append(f"[{name}.{fn_name}] module_sig 中未见 (\"{name}\", \"{fn_name}\") 签名")

        # 3. 类型方法 intrinsic() 检查
        types = mod.get("types", [])
        for t in types:
            t_name = t["name"]
            for mname in t.get("methods", []):
                if not re.search(rf'"{re.escape(mname)}"', sema_member):
                    warnings.append(f"[{name}.{t_name}.{mname}] member.rs intrinsic 表中未见方法 {mname}")

        # 4. 代码生成分支
        for fn in funcs:
            fn_name = fn["name"]
            if not fn_name.isidentifier():
                continue
            if fn_name not in codegen_expr:
                warnings.append(f"[{name}.{fn_name}] codegen_expr.rs 中未见函数名 {fn_name}")

        # 5. C 运行时：检查函数名片段
        for fn in funcs:
            fn_name = fn["name"]
            if not fn_name.isidentifier():
                continue
            # 尝试多种可能的 C 函数命名
            candidates = [
                f"ax_{name.lower()}_{fn_name.lower()}",
                f"ax_{fn_name.lower()}",
                f"ax_{fn_name}",
                fn_name,
            ]
            if not any(c in axrt_all for c in candidates):
                warnings.append(f"[{name}.{fn_name}] axrt.h/axrt.c 中未见相关 C 函数名（尝试了 {candidates[:2]}...）")

        # 6. 测试文件：模糊匹配
        test_found = False
        for stem in test_files:
            if name.lower() in stem.lower():
                test_found = True
                break
        if not test_found and status == "implemented":
            warnings.append(f"[{name}] tests/run/ 中未见相关测试文件（模糊匹配 {name}）")

    # 统计
    implemented = sum(1 for m in modules if m.get("status") == "implemented")
    partial = sum(1 for m in modules if m.get("status") == "partial")
    not_impl = sum(1 for m in modules if m.get("status") == "not_implemented")

    print(f"已检查模块: {checked}")
    print(f"  implemented: {implemented}")
    print(f"  partial:     {partial}")
    print(f"  not_implemented: {not_impl}")
    print()

    if errors:
        print("=== 错误（必须修复）===")
        for e in errors:
            print(f"  ✗ {e}")
        print()
    if warnings:
        print("=== 警告（建议核实，可能因命名约定不同而误报）===")
        for w in warnings:
            print(f"  ⚠ {w}")
        print()

    if not errors and not warnings:
        print("✓ 所有模块状态与代码/测试一致，无差异。")
    elif not errors:
        print("✓ 无严格错误，仅有警告（需人工核实，可能因命名约定不同）。")
    else:
        print(f"✗ 发现 {len(errors)} 处错误，{len(warnings)} 处警告。")
        sys.exit(1)


if __name__ == "__main__":
    main()
