#!/usr/bin/env python3
# 处理 member.rs 中剩余的 PascalCase → 小写变更

import re

PATH = r"D:\Project\axlang\rust\src\sema\member.rs"

with open(PATH, "r", encoding="utf-8") as f:
    content = f.read()

original = content

# ---------- 1. check_module_call: 修正 pascal_member 错误逻辑 ----------
# 原注释：模块成员一律 PascalCase → 改为 小写
content = content.replace(
    "// 模块成员一律 PascalCase（文稿 2.3）：小写写法给一条精确的迁移提示，",
    "// 模块成员一律小写（文稿 2.3）：PascalCase 写法给一条精确的迁移提示，",
)
content = content.replace(
    '                        "模块成员一律 PascalCase：`{0}.{1}` 请写 `{0}.{2}`",',
    '                        "模块成员一律小写：`{0}.{1}` 请写 `{0}.{2}`",',
)
# 将 pascal_member 逻辑反转：现在 module_sig 里是小写名，用户写了 PascalCase 应提示改小写
# 原代码：let pascal = pascal_member(&name); if pascal != name && module_sig(m, &pascal).is_some()
# 新代码：如果 name 本身是 PascalCase（与 pascal_member 结果不同），且小写版在 module_sig 中存在，则提示
old_block = '''                let pascal = crate::sema::expr::pascal_member(&name);
                if pascal != name && module_sig(m, &pascal).is_some() {
                    self.err(
                        "E3003",
                        &e.tok,
                        "文稿 2.3",
                        "模块成员一律小写：`{0}.{1}` 请写 `{0}.{2}`",
                        &[&m, &name, &pascal],
                    );
                } else {
                    self.err("E3003", &e.tok, "文稿 9.2", "模块 `{0}` 没有符号 `{1}`", &[&m, &name]);
                }'''
new_block = '''                // 用户可能误写成 PascalCase，尝试转小写后在 module_sig 中查找
                let lower = name.to_lowercase();
                if lower != name && module_sig(m, &lower).is_some() {
                    self.err(
                        "E3003",
                        &e.tok,
                        "文稿 2.3",
                        "模块成员一律小写：`{0}.{1}` 请写 `{0}.{2}`",
                        &[&m, &name, &lower],
                    );
                } else {
                    self.err("E3003", &e.tok, "文稿 9.2", "模块 `{0}` 没有符号 `{1}`", &[&m, &name]);
                }'''
content = content.replace(old_block, new_block)

# ---------- 2. check_fmt_call: 修正 pascal_member 错误逻辑 ----------
content = content.replace(
    '                    "模块成员一律 PascalCase：`fmt.{0}` 请写 `fmt.{1}`",',
    '                    "模块成员一律小写：`fmt.{0}` 请写 `fmt.{1}`",',
)
old_fmt_block = '''            let pascal = crate::sema::expr::pascal_member(name);
            if pascal != name && MEMBERS.contains(&pascal.as_str()) {'''
new_fmt_block = '''            let lower = name.to_lowercase();
            if lower != name && MEMBERS.contains(&lower) {'''
content = content.replace(old_fmt_block, new_fmt_block)

# ---------- 3. check_maps_module_call: PascalCase → 小写 ----------
content = content.replace('            "Keys" => {', '            "keys" => {')
content = content.replace('            "Values" => {', '            "values" => {')
content = content.replace('            "Copy" => {', '            "copy" => {')
content = content.replace('            "Delete" => {', '            "delete" => {')
content = content.replace('            "Equal" => {', '            "equal" => {')

# maps 错误消息
content = content.replace(
    '"模块 `maps` 没有符号 `{0}`（可用：Keys/Values/Clone/Copy/Delete/Equal）"',
    '"模块 `maps` 没有符号 `{0}`（可用：keys/values/clone/copy/delete/equal）"',
)

# ---------- 4. check_unique_module_call: PascalCase → 小写 ----------
content = content.replace('            "Make" => {', '            "make" => {')
content = content.replace('            "Value" => {', '            "value" => {')
content = content.replace(
    '"模块 `unique` 没有符号 `{0}`（可用：Make/Value）"',
    '"模块 `unique` 没有符号 `{0}`（可用：make/value）"',
)
content = content.replace('"`unique.Make` 需要 1 个实参，得到 0"', '"`unique.make` 需要 1 个实参，得到 0"')
content = content.replace('"`unique.Value` 需要 1 个实参，得到 0"', '"`unique.value` 需要 1 个实参，得到 0"')

# ---------- 5. check_builtin_instance: File 方法名 小写 ----------
content = content.replace('            ("File", "ReadLine") => Some((vec![], t_nullable(t_string()))),',
                          '            ("File", "read_line") => Some((vec![], t_nullable(t_string()))),')
content = content.replace('            ("File", "Read") => Some((vec![], t_result(t_string()))),',
                          '            ("File", "read") => Some((vec![], t_result(t_string()))),')
content = content.replace('            ("File", "Write") => Some((vec![t_string()], t_result(t_int()))),',
                          '            ("File", "write") => Some((vec![t_string()], t_result(t_int()))),')
content = content.replace('            ("File", "WriteLine") => Some((vec![t_string()], t_result(t_int()))),',
                          '            ("File", "write_line") => Some((vec![t_string()], t_result(t_int()))),')
content = content.replace('            ("File", "Close") => Some((vec![], t_void())),',
                          '            ("File", "close") => Some((vec![], t_void())),')

# ---------- 6. check_static_call: Row/File 方法名 小写 ----------
content = content.replace('                "ReadLine" => (vec![], t_nullable(t_string())),',
                          '                "read_line" => (vec![], t_nullable(t_string())),')
content = content.replace('                "Read" => (vec![], t_result(t_string())),',
                          '                "read" => (vec![], t_result(t_string())),')
content = content.replace('                "Write" => (vec![t_string()], t_result(t_int())),',
                          '                "write" => (vec![t_string()], t_result(t_int())),')
content = content.replace('                "WriteLine" => (vec![t_string()], t_result(t_int())),',
                          '                "write_line" => (vec![t_string()], t_result(t_int())),')
content = content.replace('                "Close" => (vec![], t_void()),',
                          '                "close" => (vec![], t_void()),')

# ---------- 7. check_builtin_static: 类型级静态方法名 小写 ----------
content = content.replace('        if name == "Parse" {',
                          '        if name == "parse" {')
content = content.replace('                self.err("E3003", &e.tok, "N-17", "string 没有 parse", &[]);',
                          '                self.err("E3003", &e.tok, "N-17", "string 没有 parse", &[]);')
content = content.replace('        if name == "Truncate" && matches!(type_name, "int8" | "int16" | "int32") {',
                          '        if name == "truncate" && matches!(type_name, "int8" | "int16" | "int32") {')
content = content.replace('        if name == "FromCodepoint" && type_name == "string" {',
                          '        if name == "from_codepoint" && type_name == "string" {')
content = content.replace('        if name == "Compare" && type_name == "string" {',
                          '        if name == "compare" && type_name == "string" {')
content = content.replace('        if name == "Fields" && type_name == "string" {',
                          '        if name == "fields" && type_name == "string" {')
content = content.replace('        if name == "ToUpperRune" && type_name == "string" {',
                          '        if name == "to_upper_rune" && type_name == "string" {')
content = content.replace('        if name == "ToLowerRune" && type_name == "string" {',
                          '        if name == "to_lower_rune" && type_name == "string" {')
content = content.replace('            s.mkind = Some("Parse");', '            s.mkind = Some("parse");')
content = content.replace('            s.mkind = Some("Truncate");', '            s.mkind = Some("truncate");')
content = content.replace('            s.mkind = Some("FromCodepoint");', '            s.mkind = Some("from_codepoint");')
content = content.replace('            s.mkind = Some("StrCompare");', '            s.mkind = Some("str_compare");')
content = content.replace('            s.mkind = Some("StrFields");', '            s.mkind = Some("str_fields");')
content = content.replace('            s.mkind = Some("ToUpperRune");', '            s.mkind = Some("to_upper_rune");')
content = content.replace('            s.mkind = Some("ToLowerRune");', '            s.mkind = Some("to_lower_rune");')

# ---------- 8. codegen_expr.rs 中的 mkind 匹配（在方法调用分支）----------
# codegen_expr.rs 中的 method_match 分支使用 "Parse", "Truncate", "FromCodepoint", "StrCompare", "StrFields", "ToUpperRune", "ToLowerRune"
# 这些需要同步改小写（但 codegen_expr.rs 已被第一个脚本处理过 ("string", "Compare") 等 match 臂，
# 这里的 mkind 字符串比较需要单独处理）

with open(PATH, "w", encoding="utf-8") as f:
    f.write(content)

if content != original:
    print(f"[已更新] {PATH}")
else:
    print(f"[无变化] {PATH}")
