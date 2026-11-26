#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""将 STDLIB-STATUS.json 中所有 snake_case 的 name 字段转为 camelCase，
并同步更新 usage_example 和 description 字段中的旧函数名引用。"""

import json
import re
import sys

JSON_PATH = "STDLIB-STATUS.json"

# 特殊映射：snake_case -> camelCase（NaN 等保持大写）
SPECIAL = {
    "is_na_n": "isNaN",
}

def snake_to_camel(s: str) -> str:
    """snake_case -> camelCase。特殊情况用 SPECIAL 表。"""
    if s in SPECIAL:
        return SPECIAL[s]
    parts = s.split("_")
    result = parts[0]
    for p in parts[1:]:
        result += p[:1].upper() + p[1:]
    return result

def build_name_map(data):
    """遍历 JSON，收集所有 snake_case name 及其对应的 camelCase。"""
    name_map = {}

    def walk(obj):
        if isinstance(obj, dict):
            name = obj.get("name")
            if name and "_" in name:
                camel = snake_to_camel(name)
                if camel != name:
                    name_map[name] = camel
            for v in obj.values():
                walk(v)
        elif isinstance(obj, list):
            for v in obj:
                walk(v)

    walk(data)
    return name_map

def replace_in_text(text, name_map):
    """在文本中替换旧函数名为新函数名（只替换完整单词，避免误伤）。"""
    for old, new in sorted(name_map.items(), key=lambda x: -len(x[0])):
        # 使用词边界匹配，避免将 read_line 替换成 readLine 后又匹配到 readLine_line 等情况
        text = re.sub(r'\b' + re.escape(old) + r'\b', new, text)
    return text

def main():
    with open(JSON_PATH, encoding="utf-8") as f:
        data = json.load(f)

    name_map = build_name_map(data)
    print(f"发现 {len(name_map)} 个 snake_case name 需转换:")
    for old, new in sorted(name_map.items()):
        print(f"  {old} -> {new}")

    # 转换 name 字段
    def walk_convert(obj):
        if isinstance(obj, dict):
            name = obj.get("name")
            if name in name_map:
                obj["name"] = name_map[name]
                print(f"  转换 name: {name} -> {name_map[name]}")
            # 同时转换 usage_example 和 description
            for key in ("usage_example", "description"):
                if key in obj and isinstance(obj[key], str):
                    obj[key] = replace_in_text(obj[key], name_map)
            for v in obj.values():
                walk_convert(v)
        elif isinstance(obj, list):
            for v in obj:
                walk_convert(v)

    walk_convert(data)

    with open(JSON_PATH, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=2)
        f.write("\n")

    print(f"\n完成：{JSON_PATH} 已更新，共转换 {len(name_map)} 个函数名。")

if __name__ == "__main__":
    main()
