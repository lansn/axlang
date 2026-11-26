#!/usr/bin/env python3
import json

JSON_PATH = "STDLIB-STATUS.json"

with open(JSON_PATH, "r", encoding="utf-8") as f:
    data = json.load(f)

count = 0
for mod in data["modules"]:
    if mod["name"] == "string":
        # 修改 static_functions
        for f in mod.get("static_functions", []):
            if f.get("builtin") == False:
                f["builtin"] = True
                count += 1
        # 修改 instance_functions
        for f in mod.get("instance_functions", []):
            if f.get("builtin") == False:
                f["builtin"] = True
                count += 1

with open(JSON_PATH, "w", encoding="utf-8") as f:
    json.dump(data, f, ensure_ascii=False, indent=2)
    f.write("\n")

print(f"string 模块已修改 {count} 个函数的 builtin 字段为 True")
