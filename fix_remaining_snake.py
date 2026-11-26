#!/usr/bin/env python3
# 将 STDLIB-STATUS.json 中残留的 snake_case name 字段转为 camelCase，
# 并同步更新 usage_example / description / signature 中的旧函数名引用。
import json
import re
import sys

JSON_PATH = "STDLIB-STATUS.json"


def to_camel(snake: str) -> str:
    parts = snake.split("_")
    return parts[0] + "".join(p.capitalize() for p in parts[1:])


def main():
    with open(JSON_PATH, "r", encoding="utf-8") as f:
        data = json.load(f)

    # 建立 old_name -> new_name 映射
    name_map = {}
    count = 0
    for mod in data["modules"]:
        for fn in mod.get("functions", []):
            old = fn.get("name", "")
            if "_" in old:
                new = to_camel(old)
                name_map[old] = new
                fn["name"] = new
                count += 1
                print(f"  [{mod['name']}] {old} -> {new}")

    # 同步替换 usage_example / description / signature 中的旧名
    replaced = 0
    for mod in data["modules"]:
        for fn in mod.get("functions", []):
            for field in ("usage_example", "description", "signature"):
                val = fn.get(field, "")
                if not val:
                    continue
                new_val = val
                for old, new in name_map.items():
                    # 只替换完整的函数名引用（带括号或作为标识符）
                    new_val = re.sub(r"\b" + re.escape(old) + r"\b", new, new_val)
                if new_val != val:
                    fn[field] = new_val
                    replaced += 1
                    print(f"  field {field}: {val[:80]} -> {new_val[:80]}")

    with open(JSON_PATH, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=2)
        f.write("\n")

    print(f"\n转换完成：{count} 个 name 字段，{replaced} 处字段引用同步更新。")


if __name__ == "__main__":
    main()
