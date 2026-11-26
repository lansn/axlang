#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""将 tests/run/*.ax 中所有 .snake_case( 方法调用转为 .camelCase(。"""

import os
import re

TEST_DIR = "tests/run"


def snake_to_camel(s: str) -> str:
    parts = s.split("_")
    result = parts[0]
    for p in parts[1:]:
        result += p[:1].upper() + p[1:]
    return result


counter = [0]


def replace_func(m):
    old = m.group(1)
    new = snake_to_camel(old)
    if new != old:
        counter[0] += 1
        return f".{new}("
    return m.group(0)


for f in sorted(os.listdir(TEST_DIR)):
    if not f.endswith(".ax"):
        continue
    path = os.path.join(TEST_DIR, f)
    with open(path, encoding="utf-8") as fh:
        content = fh.read()

    new_content = re.sub(r"\.([a-z]+_[a-z]+)\s*\(", replace_func, content)

    if new_content != content:
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(new_content)

print(f"完成：共转换 {counter[0]} 处 .snake_case( 为 .camelCase(")
