#!/usr/bin/env python3
# add_builtin_plan.py —— 为 builtin=true 的函数添加 builtin_plan 字段
#
# builtin_plan 取值：
#   "keep"   → 长期保留在编译器 Rust 端内建，不迁移到 ax_stdlib
#   "migrate" → 计划从编译器内建迁移到 axc/std/ 标准库（标记"待迁移"）
#
# 用法：python add_builtin_plan.py

import json
from pathlib import Path

BASE = Path(__file__).resolve().parent
JSON_PATH = BASE / "STDLIB-STATUS.json"

# 模块级：keep（保留在编译器） vs migrate（计划迁移到 stdlib）
MODULE_LEVEL_KEEP = {
    "fmt": {"print", "println", "printf", "sprint", "sprintf", "fprintf", "errorf", "scanf"},
    "maps": {"keys", "values", "clone", "copy", "delete", "delete_func", "equal"},
    "unique": {"make", "value"},
    "sort": {"ints", "strings", "float64s", "ints_are_sorted", "strings_are_sorted",
             "search_ints", "search_strings", "slice", "slice_stable"},
    "json": {"parse"},
    "db": {"query"},
    "log": {"print", "printf", "fatal"},
}

# 模块级：migrate（计划迁移到 stdlib）
MODULE_LEVEL_MIGRATE = {
    "rand": {"int", "float", "seed"},
    "bytes": {"contains", "equal", "index", "split", "trim_space", "to_upper", "to_lower", "repeat"},
    "bufio": {"new_reader", "new_writer", "scanner"},
    "csv": {"read_all", "write"},
}

# 实例方法 / 静态方法：keep
INSTANCE_KEEP = {
    # 没有实例方法保留在编译器
}

# 实例方法 / 静态方法：migrate
INSTANCE_MIGRATE = {
    "decimal": {"parse", "truncate"},
    "int/float/bool/enum": {"to_string", "wrapping_add", "round", "divide"},
    "array": {"push", "pop", "remove", "clear", "to_string", "==", "!=", "切片"},
    "map": {"clear", "remove", "==", "!="},
    "range": {"step"},
    "thread": {"push", "pop", "close", "join"},
    "queue": {"try_pop"},
    "reflect": {"type_of", "value_of", "num_field"},
}

def mark_module_level_funcs(funcs, mod_name):
    """为模块级函数添加 builtin_plan。"""
    keep_names = MODULE_LEVEL_KEEP.get(mod_name, set())
    migrate_names = MODULE_LEVEL_MIGRATE.get(mod_name, set())
    for f in funcs:
        if not isinstance(f, dict):
            continue
        name = f.get("name", "")
        if name in keep_names:
            f["builtin_plan"] = "keep"
        elif name in migrate_names:
            f["builtin_plan"] = "migrate"
        else:
            # 未明确分类的，默认 keep
            f["builtin_plan"] = "keep"

def mark_instance_methods(methods, mod_name):
    """为实例方法/静态方法添加 builtin_plan。"""
    keep_names = INSTANCE_KEEP.get(mod_name, set())
    migrate_names = INSTANCE_MIGRATE.get(mod_name, set())
    for m in methods:
        if not isinstance(m, dict):
            continue
        name = m.get("name", "")
        if name in keep_names:
            m["builtin_plan"] = "keep"
        elif name in migrate_names:
            m["builtin_plan"] = "migrate"
        else:
            m["builtin_plan"] = "keep"

def main():
    with open(JSON_PATH, "r", encoding="utf-8") as f:
        data = json.load(f)

    for mod in data["modules"]:
        mod_name = mod["name"]
        builtin = None
        # 先确定该模块是否为 builtin=true
        for key in ("functions", "static_functions", "instance_functions", "module_functions"):
            if key in mod and isinstance(mod[key], list):
                for f in mod[key]:
                    if isinstance(f, dict):
                        builtin = f.get("builtin")
                        break
                if builtin is not None:
                    break
        if builtin is None and "types" in mod:
            for t in mod["types"]:
                if "methods" in t and isinstance(t["methods"], list):
                    for m in t["methods"]:
                        if isinstance(m, dict):
                            builtin = m.get("builtin")
                            break
                    if builtin is not None:
                        break

        if not builtin:
            continue  # ax_stdlib 模块，跳过

        # 处理模块级函数
        for key in ("functions", "static_functions", "instance_functions", "module_functions"):
            if key in mod and isinstance(mod[key], list):
                mark_module_level_funcs(mod[key], mod_name)

        # 处理 types[].methods（结构体方法）
        if "types" in mod and isinstance(mod["types"], list):
            for t in mod["types"]:
                if "methods" in t and isinstance(t["methods"], list):
                    mark_instance_methods(t["methods"], mod_name)

    with open(JSON_PATH, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=2)
        f.write("\n")

    print(f"✅ 已更新 {JSON_PATH}")
    print("builtin_plan='keep' (保留编译器内建): fmt, maps, unique, sort, json.parse, db.query, log")
    print("builtin_plan='migrate' (计划迁移到 stdlib): decimal, int/float/bool/enum, array, map, range, thread, queue, rand, bytes, bufio, csv, reflect")

if __name__ == "__main__":
    main()
