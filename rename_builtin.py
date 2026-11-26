#!/usr/bin/env python3
# 将 ax 内置库模块函数名从 PascalCase 改为小写 C 风格（如 ReadLine → read_line）
# 适用文件：expr.rs、member.rs、codegen_expr.rs

import re
import sys

FILES = [
    r"D:\Project\axlang\rust\src\sema\expr.rs",
    r"D:\Project\axlang\rust\src\sema\member.rs",
    r"D:\Project\axlang\rust\src\backend\codegen_expr.rs",
]

# (module, old_name) -> (module, new_name)  用于 expr.rs module_sig 和 codegen_expr.rs match 臂
MODULE_RENAMES = [
    (("io", "ReadLine"), ("io", "read_line")),
    (("io", "ReadFile"), ("io", "read_file")),
    (("io", "Write"), ("io", "write")),
    (("io", "WriteLine"), ("io", "write_line")),
    (("math", "Equals"), ("math", "equals")),
    (("math", "Compare"), ("math", "compare")),
    (("math", "Abs"), ("math", "abs")),
    (("math", "Max"), ("math", "max")),
    (("math", "Min"), ("math", "min")),
    (("math", "Sqrt"), ("math", "sqrt")),
    (("math", "Pow"), ("math", "pow")),
    (("math", "Exp"), ("math", "exp")),
    (("math", "Log"), ("math", "log")),
    (("math", "Sin"), ("math", "sin")),
    (("math", "Cos"), ("math", "cos")),
    (("math", "Tan"), ("math", "tan")),
    (("math", "Ceil"), ("math", "ceil")),
    (("math", "Floor"), ("math", "floor")),
    (("math", "Round"), ("math", "round")),
    (("math", "Sign"), ("math", "sign")),
    (("math", "Mod"), ("math", "mod")),
    (("math", "Exp2"), ("math", "exp2")),
    (("math", "Log2"), ("math", "log2")),
    (("math", "Log10"), ("math", "log10")),
    (("math", "Log1p"), ("math", "log1p")),
    (("math", "IsNaN"), ("math", "is_nan")),
    (("math", "IsInf"), ("math", "is_inf")),
    (("math", "Sinh"), ("math", "sinh")),
    (("math", "Cosh"), ("math", "cosh")),
    (("math", "Tanh"), ("math", "tanh")),
    (("math", "Asin"), ("math", "asin")),
    (("math", "Acos"), ("math", "acos")),
    (("math", "Atan"), ("math", "atan")),
    (("math", "Atan2"), ("math", "atan2")),
    (("math", "Hypot"), ("math", "hypot")),
    (("math", "Cbrt"), ("math", "cbrt")),
    (("math", "Trunc"), ("math", "trunc")),
    (("json", "Parse"), ("json", "parse")),
    (("json", "Stringify"), ("json", "stringify")),
    (("json", "MarshalIndent"), ("json", "marshal_indent")),
    (("json", "Valid"), ("json", "valid")),
    (("db", "Query"), ("db", "query")),
    (("file", "Open"), ("file", "open")),
    (("file", "Create"), ("file", "create")),
    (("file", "Append"), ("file", "append")),
    (("file", "Remove"), ("file", "remove")),
    (("file", "Rename"), ("file", "rename")),
    (("file", "Exists"), ("file", "exists")),
    (("file", "Mkdir"), ("file", "mkdir")),
    (("file", "IsDir"), ("file", "is_dir")),
    (("file", "IsFile"), ("file", "is_file")),
    (("file", "Size"), ("file", "size")),
    (("file", "ModTime"), ("file", "mod_time")),
    (("file", "ReadAll"), ("file", "read_all")),
    (("file", "WriteAll"), ("file", "write_all")),
    (("file", "Chmod"), ("file", "chmod")),
    (("file", "Chtimes"), ("file", "chtimes")),
    (("file", "TempFile"), ("file", "temp_file")),
    (("string", "FromCodepoint"), ("string", "from_codepoint")),
    (("string", "Compare"), ("string", "compare")),
    (("string", "Fields"), ("string", "fields")),
    (("string", "ToUpperRune"), ("string", "to_upper_rune")),
    (("string", "ToLowerRune"), ("string", "to_lower_rune")),
    (("time", "NowMs"), ("time", "now_ms")),
    (("time", "Now"), ("time", "now")),
    (("time", "FromUnixMs"), ("time", "from_unix_ms")),
    (("time", "Sleep"), ("time", "sleep")),
    (("time", "Unix"), ("time", "unix")),
    (("time", "UnixMilli"), ("time", "unix_milli")),
    (("time", "UnixMicro"), ("time", "unix_micro")),
    (("time", "UnixNano"), ("time", "unix_nano")),
    (("time", "Since"), ("time", "since")),
    (("time", "Date"), ("time", "date")),
    (("time", "Until"), ("time", "until")),
    (("time", "Parse"), ("time", "parse")),
    (("os", "Getenv"), ("os", "getenv")),
    (("os", "Exit"), ("os", "exit")),
    (("os", "Args"), ("os", "args")),
    (("os", "Getwd"), ("os", "getwd")),
    (("os", "Chdir"), ("os", "chdir")),
    (("os", "Getpid"), ("os", "getpid")),
    (("os", "Getppid"), ("os", "getppid")),
    (("os", "Hostname"), ("os", "hostname")),
    (("os", "Environ"), ("os", "environ")),
    (("os", "Setenv"), ("os", "setenv")),
    (("os", "Unsetenv"), ("os", "unsetenv")),
    (("os", "UserHomeDir"), ("os", "user_home_dir")),
    (("atomic", "AddInt"), ("atomic", "add_int")),
    (("atomic", "LoadInt"), ("atomic", "load_int")),
    (("atomic", "StoreInt"), ("atomic", "store_int")),
    (("atomic", "CompareAndSwapInt"), ("atomic", "compare_and_swap_int")),
    (("rand", "Int"), ("rand", "int")),
    (("rand", "Float"), ("rand", "float")),
    (("rand", "Seed"), ("rand", "seed")),
    (("sort", "Ints"), ("sort", "ints")),
    (("sort", "Strings"), ("sort", "strings")),
    (("sort", "Float64s"), ("sort", "float64s")),
    (("sort", "IntsAreSorted"), ("sort", "ints_are_sorted")),
    (("sort", "StringsAreSorted"), ("sort", "strings_are_sorted")),
    (("sort", "SearchInts"), ("sort", "search_ints")),
    (("sort", "SearchStrings"), ("sort", "search_strings")),
    (("csv", "ReadAll"), ("csv", "read_all")),
    (("csv", "Write"), ("csv", "write")),
    (("bytes", "Contains"), ("bytes", "contains")),
    (("bytes", "Equal"), ("bytes", "equal")),
    (("bytes", "Index"), ("bytes", "index")),
    (("bytes", "Split"), ("bytes", "split")),
    (("bytes", "TrimSpace"), ("bytes", "trim_space")),
    (("bytes", "ToUpper"), ("bytes", "to_upper")),
    (("bytes", "ToLower"), ("bytes", "to_lower")),
    (("bytes", "Repeat"), ("bytes", "repeat")),
    (("bufio", "NewReader"), ("bufio", "new_reader")),
    (("bufio", "NewWriter"), ("bufio", "new_writer")),
    (("bufio", "Scanner"), ("bufio", "scanner")),
    (("errors", "New"), ("errors", "new")),
    (("errors", "Is"), ("errors", "is")),
    (("errors", "As"), ("errors", "as")),
    (("log", "Print"), ("log", "print")),
    (("log", "Printf"), ("log", "printf")),
    (("log", "Fatal"), ("log", "fatal")),
    (("strconv", "Atoi"), ("strconv", "atoi")),
    (("strconv", "Itoa"), ("strconv", "itoa")),
    (("strconv", "ParseFloat"), ("strconv", "parse_float")),
    (("strconv", "FormatFloat"), ("strconv", "format_float")),
    (("strconv", "ParseInt"), ("strconv", "parse_int")),
    (("strconv", "FormatInt"), ("strconv", "format_int")),
    (("strconv", "ParseUint"), ("strconv", "parse_uint")),
    (("strconv", "FormatUint"), ("strconv", "format_uint")),
    (("strconv", "CanBackquote"), ("strconv", "can_backquote")),
    (("strconv", "Quote"), ("strconv", "quote")),
    (("strconv", "Unquote"), ("strconv", "unquote")),
    (("filepath", "Join"), ("filepath", "join")),
    (("filepath", "Dir"), ("filepath", "dir")),
    (("filepath", "Base"), ("filepath", "base")),
    (("filepath", "Ext"), ("filepath", "ext")),
    (("filepath", "Split"), ("filepath", "split")),
    (("filepath", "Clean"), ("filepath", "clean")),
    (("filepath", "Abs"), ("filepath", "abs")),
    (("filepath", "IsAbs"), ("filepath", "is_abs")),
]

# codegen_expr.rs / member.rs 中的 name == "X" 比较，仅替换为 name == "x"
NAME_EQUALS_RENAMES = [
    ('name == "Print"', 'name == "print"'),
    ('name == "Println"', 'name == "println"'),
    ('name == "Printf"', 'name == "printf"'),
    ('name == "Sprint"', 'name == "sprint"'),
    ('name == "Sprintf"', 'name == "sprintf"'),
    ('name == "Fprintf"', 'name == "fprintf"'),
    ('name == "Errorf"', 'name == "errorf"'),
    ('name == "Scanf"', 'name == "scanf"'),
    ('name == "Fatal"', 'name == "fatal"'),
]

# MEMBERS 数组
MEMBERS_OLD = '["Print", "Println", "Printf", "Sprint", "Sprintf", "Fprintf", "Errorf", "Scanf"]'
MEMBERS_NEW = '["print", "println", "printf", "sprint", "sprintf", "fprintf", "errorf", "scanf"]'

# 错误提示字符串中的 PascalCase 引用
ERROR_STR_RENAMES = [
    ("可选 Print / Println / Printf / Sprint / Sprintf / Fprintf / Errorf / Scanf",
     "可选 print / println / printf / sprint / sprintf / fprintf / errorf / scanf"),
    ("fmt.Print/Println 与 print/println 走同一套多实参实现",
     "fmt.print/println 与 print/println 走同一套多实参实现"),
    ("`fmt.Print(args...)` / `fmt.Println(args...)`：与 `print` / `println` 完全同实现",
     "`fmt.print(args...)` / `fmt.println(args...)`：与 `print` / `println` 完全同实现"),
]

def apply_module_renames(content):
    """替换 ("module", "OldName") -> ("module", "new_name")"""
    for (mod, old), (_, new) in MODULE_RENAMES:
        old_pattern = f'("{mod}", "{old}")'
        new_pattern = f'("{mod}", "{new}")'
        content = content.replace(old_pattern, new_pattern)
    return content

def apply_name_equals_renames(content):
    # 替换 name == "OldName" -> name == "new_name"
    for old, new in NAME_EQUALS_RENAMES:
        content = content.replace(old, new)
    return content

def apply_members_rename(content):
    """替换 MEMBERS 数组"""
    return content.replace(MEMBERS_OLD, MEMBERS_NEW)

def apply_error_str_renames(content):
    """替换错误提示字符串"""
    for old, new in ERROR_STR_RENAMES:
        content = content.replace(old, new)
    return content

def process_file(path):
    with open(path, "r", encoding="utf-8") as f:
        content = f.read()

    original = content

    # 所有文件都替换 ("module", "OldName")
    content = apply_module_renames(content)

    # 所有文件都替换 name == "OldName"
    content = apply_name_equals_renames(content)

    # MEMBERS 数组
    content = apply_members_rename(content)

    # 错误提示字符串
    content = apply_error_str_renames(content)

    if content != original:
        with open(path, "w", encoding="utf-8") as f:
            f.write(content)
        print(f"[已更新] {path}")
    else:
        print(f"[无变化] {path}")

if __name__ == "__main__":
    for path in FILES:
        process_file(path)
    print("\n完成。请检查 diff 并重新编译 axc。")
