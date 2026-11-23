#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
修复 STDLIB-CHECKLIST.md 中因脚本漏处理而导致的问题：
1. 某些数据行被误判为表头（因包含"对标"等关键字），导致两列未插入
2. 某些行反引号未闭合
3. 某些行说明列为空
4. P3 表某些行 ax库函数名 缺失
"""

import re

FILE = r"D:\Project\axlang\STDLIB-CHECKLIST.md"

# 需要修复的具体行：(行号, 新内容)
FIXES = {
    57: "| `fmt.Print` | `(args: any...) -> void` | Go `fmt.Print` | fmt.Print | 已通过 `print/println` 部分实现，fmt 模块函数待补 | 🔄 | 已通过 `print/println` 部分实现，fmt 模块函数待补 |",
    87: "| `math.Exp2` | `(x: float) -> float` | Go `math.Exp2` / C `exp2` | math.Exp2 | 已落地（C11 exp2，对标 Go math.Exp2） | ✅ | 已落地（C11 exp2，对标 Go math.Exp2） |",
    88: "| `math.Log10` | `(x: float) -> float` | Go `math.Log10` / C `log10` | math.Log10 | 已落地（C11 log10，对标 Go math.Log10） | ✅ | 已落地（C11 log10，对标 Go math.Log10） |",
    103: "| `math.IsNaN` | `(f: float) -> bool` | Go `math.IsNaN` / C `isnan` | math.IsNaN | 已落地（C11 isnan，对标 Go math.IsNaN） | ✅ | 已落地（C11 isnan，对标 Go math.IsNaN） |",
    104: "| `math.IsInf` | `(f: float, sign: int) -> bool` | Go `math.IsInf` / C `isinf` | math.IsInf | 已落地（C11 isinf，对标 Go math.IsInf） | ✅ | 已落地（C11 isinf，对标 Go math.IsInf） |",
    203: "| `time.ToString` | 实例 | `() -> string` | — | time.ToString | 格式化 `YYYY-MM-DD HH:MM:SS` | ✅ | 格式化 `YYYY-MM-DD HH:MM:SS` |",
    227: "| `int16.Truncate` | 实例 | `(v: int) -> int16` | int16.Truncate | 截断为 int16，对标 C 类型截断 | ✅ | 截断为 int16 |",
    228: "| `int32.Truncate` | 实例 | `(v: int) -> int32` | int32.Truncate | 截断为 int32，对标 C 类型截断 | ✅ | 截断为 int32 |",
    281: "| `os.Environ` | `() -> string[]` | Go `os.Environ` | os.Environ | 环境变量列表 `"'"'KEY=VALUE'"'"` | ✅ | 环境变量列表 `"'"'KEY=VALUE'"'"` |",
    493: "| `arr.Sort(cmp)` | `(a: T[], less: func(T,T) int) -> void` | Go `sort.Slice` | arr.Sort(cmp) | 数组带比较函数排序，对标 Go sort.Slice，GAP #10 | ✅ | 已落地（带比较函数，GAP #10） |",
    578: "| `go` | `ast`, `build`, `constant`, `doc`, `format`, `parser`, `printer`, `scanner`, `token`, `types`, `version` | `go` | go | Go 源码工具链（axlang 自用，低优先级） | ⬜ | Go 源码工具链（axlang 自用，低优先级） |",
    585: "| `math/cmplx` | — | `math` | math/cmplx | 复数数学函数，对标 Go math/cmplx | ⬜ | 复数数学函数 |",
    593: "| `regexp` / `syntax` | — | `regexp` | regexp/syntax | 正则表达式，对标 Go regexp / syntax | ⬜ | 正则表达式 |",
    599: "| `unicode` / `utf8` / `utf16` | — | `unicode` | unicode/utf8/utf16 | Unicode 码点属性与 UTF 编解码，对标 Go unicode/utf8/utf16 | ⬜ | Unicode 码点属性与 UTF 编解码 |",
}


def main():
    with open(FILE, "r", encoding="utf-8") as f:
        lines = f.readlines()

    fixed_count = 0
    for line_num, new_content in FIXES.items():
        idx = line_num - 1  # 转为 0-based
        if idx < len(lines):
            old = lines[idx]
            # 确保新内容以换行符结尾
            if not new_content.endswith("\n"):
                new_content += "\n"
            if old != new_content:
                lines[idx] = new_content
                fixed_count += 1
                print(f"修复第 {line_num} 行")
            else:
                print(f"第 {line_num} 行已正确，跳过")
        else:
            print(f"警告：第 {line_num} 行超出文件范围")

    with open(FILE, "w", encoding="utf-8") as f:
        f.writelines(lines)

    print(f"\n共修复 {fixed_count} 行。")


if __name__ == "__main__":
    main()
