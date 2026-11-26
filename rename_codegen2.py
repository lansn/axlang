#!/usr/bin/env python3
# 处理 codegen_expr.rs 中剩余的 PascalCase → 小写变更（mkind / bname 字符串比较）

PATH = r"D:\Project\axlang\rust\src\backend\codegen_expr.rs"

with open(PATH, "r", encoding="utf-8") as f:
    content = f.read()

original = content

# ---------- method_call: mkind 字符串比较 ----------
content = content.replace('        if mk == "Parse" {', '        if mk == "parse" {')
content = content.replace('        if mk == "Truncate" {', '        if mk == "truncate" {')
content = content.replace('        if mk == "FromCodepoint" {', '        if mk == "from_codepoint" {')
content = content.replace('        if mk == "StrCompare" {', '        if mk == "str_compare" {')
content = content.replace('        if mk == "StrFields" {', '        if mk == "str_fields" {')
content = content.replace('        if mk == "ToUpperRune" {', '        if mk == "to_upper_rune" {')
content = content.replace('        if mk == "ToLowerRune" {', '        if mk == "to_lower_rune" {')

# 相关注释
content = content.replace('        // `string.ToUpperRune(c)`：接收者 `string` 仅是命名空间，取单个 int 实参。',
                          '        // `string.to_upper_rune(c)`：接收者 `string` 仅是命名空间，取单个 int 实参。')
content = content.replace('        // `string.ToLowerRune(c)`：接收者 `string` 仅是命名空间，取单个 int 实参。',
                          '        // `string.to_lower_rune(c)`：接收者 `string` 仅是命名空间，取单个 int 实参。')

# ---------- builtin_instance: Row/File/sync 方法名 小写 ----------
content = content.replace('            "Row.Empty" => format!("ax_row_empty({})", tv),',
                          '            "row.empty" => format!("ax_row_empty({})", tv),')
content = content.replace('            "Row.GetString" => {',
                          '            "row.get_string" => {')
content = content.replace('            "Row.GetInt" => {',
                          '            "row.get_int" => {')
content = content.replace('            "Row.GetDecimal" => {',
                          '            "row.get_decimal" => {')
content = content.replace('            "File.Close" => {',
                          '            "file.close" => {')
content = content.replace('            "File.Read" => {',
                          '            "file.read" => {')
content = content.replace('            "File.ReadLine" => {',
                          '            "file.read_line" => {')
content = content.replace('            "File.Write" | "File.WriteLine" => {',
                          '            "file.write" | "file.write_line" => {')
content = content.replace('                let call = if b == "File.Write" {',
                          '                let call = if b == "file.write" {')
content = content.replace('                } else {',
                          '                } else {')
content = content.replace('                    format!("ax_file_write_line({}, {})", tv, a)',
                          '                    format!("ax_file_write_line({}, {})", tv, a)')
# Note: b == "File.WriteLine" -> b == "file.write_line"
content = content.replace('                let call = if b == "file.write" {',
                          '                let call = if b == "file.write" {')
# Need to handle the "File.WriteLine" branch condition too
content = content.replace('                } else {\n                    format!("ax_file_write_line({}, {})", tv, a)',
                          '                } else {\n                    format!("ax_file_write_line({}, {})", tv, a)')
# Fix: the condition checks b == "File.Write" → b == "file.write", and else branch handles write_line
# But we also need to check: b == "file.write_line" in the else branch? Actually the else handles both
# Let's look at the original: if b == "File.Write" { ax_file_write } else { ax_file_write_line }
# After rename: if b == "file.write" { ax_file_write } else { ax_file_write_line }
# That's fine since only write and write_line reach here

# ---------- sync 方法名 小写 ----------
content = content.replace('            "Mutex.Lock" => {', '            "mutex.lock" => {')
content = content.replace('            "Mutex.Unlock" => {', '            "mutex.unlock" => {')
content = content.replace('            "RWMutex.Lock" => {', '            "rwmutex.lock" => {')
content = content.replace('            "RWMutex.Unlock" => {', '            "rwmutex.unlock" => {')
content = content.replace('            "RWMutex.RLock" => {', '            "rwmutex.rlock" => {')
content = content.replace('            "RWMutex.Runlock" => {', '            "rwmutex.runlock" => {')
content = content.replace('            "WaitGroup.Add" =>', '            "waitgroup.add" =>')
content = content.replace('            "WaitGroup.Done" => {', '            "waitgroup.done" => {')
content = content.replace('            "WaitGroup.Wait" => {', '            "waitgroup.wait" => {')
content = content.replace('            "Once.Do" => {', '            "once.do" => {')

# ---------- module_call 中的 fmt/log 名称（第一个脚本已处理 name == "X"，但这里有 module == "fmt" && name == "X"）----------
# 第一个脚本已经处理了 name == "X" 替换，但 module_call 中的 match 臂如 ("io", "ReadLine") 已由脚本处理
# 这里检查是否有遗漏的 module == "fmt" && name == "Print" 等
# 脚本已处理 name == "Print" → name == "print" 等，应该已经覆盖

# ---------- 方法调用分支中的注释 ----------
content = content.replace('            // `string.Compare(a, b)` 是模块级静态方法',
                          '            // `string.compare(a, b)` 是模块级静态方法')
content = content.replace('            // `string.Fields(s)` 同理，接收者 `string` 仅是命名空间。',
                          '            // `string.fields(s)` 同理，接收者 `string` 仅是命名空间。')

with open(PATH, "w", encoding="utf-8") as f:
    f.write(content)

if content != original:
    print(f"[已更新] {PATH}")
else:
    print(f"[无变化] {PATH}")
