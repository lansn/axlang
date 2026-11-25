#!/usr/bin/env python3
"""将 axrt.c 按功能模块拆分为多个 .c 文件。"""

import re
import sys
from pathlib import Path

SRC = Path("axrt.c")

# 读取源码
with open(SRC, "r", encoding="utf-8") as f:
    lines = f.readlines()

# 函数定义正则：匹配行首的函数定义
FUNC_RE = re.compile(r'^(?:static\s+)?(?:ax_\w+|int|void|ax_str|ax_arr|ax_map|ax_dec|ax_box|ax_err|ax_file|ax_thread|ax_queue|ax_mutex|ax_rwmutex|ax_waitgroup|ax_once|ax_res_\w+|ax_sort_\w+|ax_slice_\w+|ax_bytes_\w+|ax_filepath_\w+|ax_csv_\w+|ax_rand_\w+|ax_json_\w+|ax_os_\w+|ax_time_\w+|ax_parse_\w+|str_to_\w+)\s')

# 每个模块的函数前缀映射
MODULE_PREFIXES = {
    "axrt_core.c": [
        "ax_alloc", "ax_calloc", "ax_arena_", "ax_err_", "ax_box_", "ax_unbox_",
        "ax_arr_", "ax_map_", "i128_", "trap_name",
    ],
    "axrt_str.c": [
        "ax_str_", "ax_sb_", "ax_math_", "ax_bytes_",
        "u8_", "ax_is_space", "ax_u8_", "sb_reserve", "ax_str_char_in_set",
    ],
    "axrt_decimal.c": [
        "ax_dec_", "dec_", "div_round_i128", "i128_to_double", "i128_is_zero", "i128_div_u64",
    ],
    "axrt_json.c": [
        "ax_json_", "ax_db_", "ax_row_", "jskip_ws", "jpeek", "jfail", "jb_push",
        "jb_utf8", "jstring", "jvalue", "jobject", "jarray", "jstr_escape",
        "jstringify_box", "jpush_indent", "jstringify_indent_box", "jjson_valid",
        "jparse", "jbuf",
    ],
    "axrt_time.c": [
        "ax_now_ms", "ax_time_", "ax_time_is_leap",
    ],
    "axrt_file.c": [
        "ax_read_line", "ax_read_file", "ax_file_", "ax_dir_", "ax_path_cstr", "ax_os_path_cstr",
    ],
    "axrt_os.c": [
        "ax_os_", "ax_console_utf8", "ax_env_utf8", "ax_parse_", "str_to_i64",
        "ax_cp_", "ax_console_restore", "g_os_args",
    ],
    "axrt_strconv.c": [
        "ax_parse_int_base", "ax_str_of_int_base", "ax_parse_uint_base",
        "ax_str_of_uint_base", "ax_str_can_backquote", "ax_str_quote", "ax_str_unquote",
        "ax_quote_escape", "hexval", "ax_unquote_escape", "ax_u8_is_printable",
    ],
    "axrt_sort.c": [
        "ax_sort_", "ax_arr_reverse", "ax_arr_clone", "ax_arr_insert", "ax_arr_fill",
        "ax_arr_resize", "ax_arr_merge", "ax_arr_min_", "ax_arr_max_", "ax_arr_sum_",
        "ax_slice_", "ax_sort_*_cmp",
    ],
    "axrt_csv.c": [
        "ax_csv_",
    ],
    "axrt_filepath.c": [
        "ax_filepath_", "ax_filepath_has_sep", "ax_filepath_last_sep",
    ],
    "axrt_sync.c": [
        "ax_thread_", "ax_queue_", "ax_mutex_", "ax_rwmutex_", "ax_waitgroup_",
        "ax_once_", "ax_thread_proc",
    ],
    "axrt_atomic.c": [
        "ax_atomic_", "ax_std_",
    ],
    "axrt_rand.c": [
        "ax_rand_",
    ],
}

# 找到每个函数定义的起始行号
func_starts = []
for i, line in enumerate(lines):
    if FUNC_RE.match(line):
        # 排除 struct 定义和宏定义
        if line.strip().startswith("#") or "struct" in line or "typedef" in line:
            continue
        func_starts.append(i)

# 在每个函数开始前添加分隔注释，标记所属模块
# 这里我们需要更精确的逻辑：找到每个函数所属的模块

def find_module(func_line):
    """根据函数行确定所属模块。"""
    # 提取函数名前缀
    for mod, prefixes in MODULE_PREFIXES.items():
        for prefix in prefixes:
            if prefix in func_line:
                return mod
    return None

# 按模块分组函数
module_funcs = {mod: [] for mod in MODULE_PREFIXES}
orphan_funcs = []

for start_idx in func_starts:
    line = lines[start_idx]
    mod = find_module(line)
    if mod:
        module_funcs[mod].append(start_idx)
    else:
        orphan_funcs.append(start_idx)

print("=== 函数分布 ===")
for mod, starts in module_funcs.items():
    print(f"{mod}: {len(starts)} 个函数")
if orphan_funcs:
    print(f"未分类函数: {len(orphan_funcs)} 个")
    for idx in orphan_funcs:
        print(f"  行 {idx+1}: {lines[idx].strip()[:80]}")

# 确定每个模块的代码范围（从函数开始到下一个函数开始之前）
# 需要包含函数前面的注释块

# 找出所有函数开始行（按文件顺序排序）
all_starts = sorted(func_starts)

# 为每个函数确定其结束行（下一个函数开始行 - 1）
func_ranges = []
for i, start in enumerate(all_starts):
    end = all_starts[i + 1] if i + 1 < len(all_starts) else len(lines)
    func_ranges.append((start, end))

# 将每个函数分配到模块
module_code = {mod: [] for mod in MODULE_PREFIXES}

# 还需要包含文件头部（第 1 行到第一个函数之前）
header_end = all_starts[0] if all_starts else len(lines)
header_lines = lines[:header_end]

# 为每个函数分配代码
for start, end in func_ranges:
    line = lines[start]
    mod = find_module(line)
    if mod:
        # 包含函数前面的注释（向上查找直到遇到上一个函数结束）
        comment_start = start
        while comment_start > 0:
            prev = lines[comment_start - 1]
            if prev.strip().startswith("/*") or prev.strip().startswith("*") or prev.strip().startswith("//") or prev.strip() == "":
                comment_start -= 1
            else:
                break
        module_code[mod].extend(lines[comment_start:end])
    else:
        # 未分类函数，分配给 axrt_core.c
        module_code["axrt_core.c"].extend(lines[start:end])

# 写入每个模块文件
for mod, code_lines in module_code.items():
    if not code_lines:
        continue
    # 添加文件头注释
    header = f"/* {mod} -- axlang 运行时模块 */\n#include \"axrt.h\"\n#include \"axrt_internal.h\"\n\n"
    content = header + "".join(code_lines)
    with open(mod, "w", encoding="utf-8") as f:
        f.write(content)
    print(f"写入 {mod}: {len(code_lines)} 行")

print("\n拆分完成！")
