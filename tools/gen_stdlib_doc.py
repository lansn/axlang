#!/usr/bin/env python3
"""生成 docs/STDLIB.md（内建库与标准库说明，按编译器源码归类）。

设计：
  - 模块归类直接从 rust/src/sema/mod.rs 的 INTERNAL_BUILTINS / STDLIB_MODULES
    常量抽取，**不再硬编码**模块列表，避免与编译器源码漂移。
  - 内建模块（INTERNAL_BUILTINS）：无 ax 源码，裸模块名直接使用，如
    `fmt.printf(...)` 无需 import；其签名在 expr.rs 的 module_sig() 中注册。
  - 标准库模块（STDLIB_MODULES）：ax 语法编写，位于 axc/std/*.ax，
    通过 `import std.xxx` 导入，对应 ax_std_* C 运行时接口。
  - 签名 / 说明：来自下面的 DESC 字典。源码里有而字典没有的条目
    会以「签名待补充」占位——补字典后重跑即可，不会漏。
用法：python tools/gen_stdlib_doc.py
"""
import os, re, sys, collections, datetime

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MEMBER = os.path.join(ROOT, "rust", "src", "sema", "member.rs")
EXPR = os.path.join(ROOT, "rust", "src", "sema", "expr.rs")
SEMAMOD = os.path.join(ROOT, "rust", "src", "sema", "mod.rs")
OUT = os.path.join(ROOT, "docs", "STDLIB.md")

# ---------------------------------------------------------------- 签名与说明
# (签名, 说明)。缺条目时文档自动出现「签名待补充」。

DESC = {
    # 全局函数
    ("global", "print"):   ("print(s: string) / print(x: 可插值)", "输出不换行；实参非字符串时等价于 `print(\"{x}\")`"),
    ("global", "println"): ("println(s: string) / println(x: 可插值)", "输出并换行；实参非字符串时等价于 `println(\"{x}\")`"),
    ("global", "typeof"): ("typeof(expr: 表达式) -> string", "编译期常量，返回实参静态类型显示名（int / int8 / Dog / int[] / int? / map<string, object>）；生成字符串字面量，零运行时开销"),
    ("global", "sizeof"): ("sizeof(expr: 表达式) -> int", "编译期大小：实参类型在 C 侧的存储字节数（int=8 / uint8=1 / string=16 / int[]=24 / class=指针 8）；对象本身的大小不在此问"),
    ("global", "ok"):      ("ok(v: T) -> result<T>", "构造成功结果，配合 try/throw 使用"),
    ("global", "fail"):    ("fail(e: error)", "构造失败结果，只允许在返回 result 的函数内"),
    ("global", "parent"):  ("parent(实参...)", "在子类 constructor 第一条语句调用父类构造器"),

    # 内建模块（INTERNAL_BUILTINS，来自 module_sig()）
    ("module", "db.query"):      ("db.query(sql: string) -> result<Row[]>", "执行查询，返回行数组"),
    ("module", "fmt.print"):   ("fmt.print(s: string)", "输出不换行；与内建 print 同实现"),
    ("module", "fmt.println"): ("fmt.println(s: string)", "输出并换行；与内建 println 同实现"),
    ("module", "fmt.printf"):  ("fmt.printf(format: string, ...)", "格式化占位符输出（%d/%f/%.Nf/%s/%t/%v/%%），编译期核对格式串"),
    ("module", "log.print"):   ("log.print(...)", "多实参打印，复用 fmt.printf 实现"),
    ("module", "log.printf"):  ("log.printf(format: string, ...)", "格式化打印，复用 fmt.printf 实现"),
    ("module", "log.fatal"):   ("log.fatal(...)", "打印致命错误并退出"),
    ("module", "reflect.typeOf"): ("reflect.typeOf(v: object) -> string", "编译期返回运行时类型名，零运行时开销"),
    ("module", "reflect.numField"): ("reflect.numField(v: object) -> int", "返回结构体/对象字段数量"),
    ("module", "sort.ints"):   ("sort.ints(a: int[])", "对 int 数组升序排序"),
    ("module", "sort.strings"): ("sort.strings(a: string[])", "对 string 数组升序排序"),
    ("module", "sort.float64s"): ("sort.float64s(a: float[])", "对 float 数组升序排序"),
    ("module", "sort.intsAreSorted"): ("sort.intsAreSorted(a: int[]) -> bool", "判断 int 数组是否已升序"),
    ("module", "sort.stringsAreSorted"): ("sort.stringsAreSorted(a: string[]) -> bool", "判断 string 数组是否已升序"),
    ("module", "sort.searchInts"): ("sort.searchInts(a: int[], x: int) -> int", "在有序 int 数组中二分查找 x"),
    ("module", "sort.searchStrings"): ("sort.searchStrings(a: string[], x: string) -> int", "在有序 string 数组中二分查找 x"),
    ("module", "sort.slice"): ("sort.slice(a: object[], cmp: func(int, int) int)", "按自定义比较函数对 object 数组排序"),
    ("module", "sort.sliceStable"): ("sort.sliceStable(a: object[], cmp: func(int, int) int)", "按自定义比较函数稳定排序 object 数组"),

    # string 实例方法
    ("string", "len"):        ("s.len() -> int", "字符数（按码点）"),
    ("string", "byteLen"):    ("s.byteLen() -> int", "UTF-8 字节数"),
    ("string", "at"):         ("s.at(i: int) -> string?", "第 i 个字符；越界返回 null"),
    ("string", "chars"):      ("s.chars() -> string[]", "拆成单字符数组"),
    ("string", "codepointAt"): ("s.codepointAt(i: int) -> int?", "第 i 个字符的码点；越界返回 null"),
    ("string", "contains"):   ("s.contains(sub: string) -> bool", "是否包含子串"),
    ("string", "split"):      ("s.split(sep: string) -> string[]", "按分隔符拆分"),
    ("string", "toString"):   ("s.toString() -> string", "自身"),
    ("string", "trim"):       ("s.trim() -> string", "去除两端空白"),
    ("string", "upper"):      ("s.upper() -> string", "转大写（仅 ASCII，UTF-8 多字节原样保留）"),
    ("string", "lower"):      ("s.lower() -> string", "转小写（仅 ASCII，UTF-8 多字节原样保留）"),
    ("string", "startsWith"): ("s.startsWith(p: string) -> bool", "是否以 p 开头"),
    ("string", "endsWith"):   ("s.endsWith(p: string) -> bool", "是否以 p 结尾"),
    ("string", "join"):       ("s.join(parts: string[]) -> string", "用 s 作为分隔符连接数组"),
    ("string", "replace"):    ("s.replace(old: string, new: string) -> string", "替换所有 old 子串为 new"),
    ("string", "repeat"):     ("s.repeat(n: int) -> string", "重复 s n 次"),
    ("string", "indexOf"):    ("s.indexOf(sub: string) -> int", "子串位置，找不到返回 -1"),
    ("string", "substring"):  ("s.substring(start: int, end: int) -> string", "按码点区间取子串"),
    ("string", "containsRune"):   ("s.containsRune(cp: int) -> bool", "是否包含指定 Unicode 码点"),
    ("string", "containsAny"):    ("s.containsAny(chars: string) -> bool", "是否包含 chars 中任意字符"),
    ("string", "indexAny"):       ("s.indexAny(chars: string) -> int", "chars 中任意字符首次出现的位置，找不到返回 -1"),
    ("string", "lastIndexOf"):    ("s.lastIndexOf(sub: string) -> int", "子串最后出现的位置，找不到返回 -1"),
    ("string", "lastIndexAny"):   ("s.lastIndexAny(chars: string) -> int", "chars 中任意字符最后出现的位置，找不到返回 -1"),
    ("string", "trimLeft"):       ("s.trimLeft(cutset: string) -> string", "去除左侧任意在 cutset 中的字符"),
    ("string", "trimRight"):      ("s.trimRight(cutset: string) -> string", "去除右侧任意在 cutset 中的字符"),
    ("string", "trimPrefix"):     ("s.trimPrefix(prefix: string) -> string", "若以 prefix 开头则去除，否则原样返回"),
    ("string", "trimSuffix"):     ("s.trimSuffix(suffix: string) -> string", "若以 suffix 结尾则去除，否则原样返回"),
    ("string", "equalFold"):      ("s.equalFold(t: string) -> bool", "忽略大小写判断两字符串是否相等（仅 ASCII）"),
    ("string", "splitN"):         ("s.splitN(sep: string, n: int) -> string[]", "按 sep 分割，最多返回 n 段"),
    ("string", "splitAfter"):     ("s.splitAfter(sep: string) -> string[]", "在每个 sep 之后切分"),
    ("string", "count"):          ("s.count(sub: string) -> int", "子串非重叠出现次数"),
    ("string", "left"):           ("s.left(n: int) -> string", "取左侧 n 个字符"),
    ("string", "right"):          ("s.right(n: int) -> string", "取右侧 n 个字符"),
    ("string", "mid"):            ("s.mid(start: int, end: int) -> string", "按码点区间取子串"),
    ("string", "isDigit"):        ("s.isDigit() -> bool", "是否全部为数字字符"),
    ("string", "isAlpha"):        ("s.isAlpha() -> bool", "是否全部为字母字符"),
    ("string", "isSpace"):        ("s.isSpace() -> bool", "是否全部为空白字符"),
    ("string", "isLower"):        ("s.isLower() -> bool", "是否全部为小写字母"),
    ("string", "isUpper"):        ("s.isUpper() -> bool", "是否全部为大写字母"),
    ("string", "isAlnum"):        ("s.isAlnum() -> bool", "是否全部为字母或数字"),
    ("string", "title"):          ("s.title() -> string", "标题式大写（首字母大写，其余小写）"),
    ("string", "map"):            ("s.map(f: func(int) int) -> string", "对每个字符码点应用函数 f 并重建字符串"),

    # 数组实例方法
    ("array", "len"):      ("a.len() -> int", "元素个数"),
    ("array", "push"):     ("a.push(v: T)", "尾部追加（就地修改）"),
    ("array", "pop"):      ("a.pop() -> T", "弹出并返回尾元素"),
    ("array", "remove"):   ("a.remove(i: int)", "按下标删除"),
    ("array", "clear"):    ("a.clear()", "清空"),
    ("array", "at"):       ("a.at(i: int) -> T?", "按下标取；越界返回 null"),
    ("array", "sort"):     ("a.sort(cmp: func(T, T) int)", "按比较函数稳定排序（cmp<0 表示前者在前）"),
    ("array", "toString"): ("a.toString() -> string", "转为展示字符串"),
    ("array", "delete"):   ("a.delete(i: int, j: int)", "删除下标区间 [i, j) 的元素（就地修改）"),
    ("array", "contains"): ("a.contains(v: T) -> bool", "是否包含元素 v"),
    ("array", "index"):    ("a.index(v: T) -> int", "元素 v 首次出现的下标，找不到返回 -1"),
    ("array", "reverse"):  ("a.reverse()", "原地逆序数组元素"),
    ("array", "clone"):    ("a.clone() -> T[]", "浅拷贝数组"),
    ("array", "insert"):   ("a.insert(i: int, v: T)", "在下标 i 处插入元素 v"),
    ("array", "fill"):     ("a.fill(v: T)", "用元素 v 填充整个数组"),
    ("array", "merge"):    ("a.merge(b: T[])", "将数组 b 的所有元素追加到 a"),
    ("array", "min"):      ("a.min() -> T", "数组中的最小元素（需 T 可比较）"),
    ("array", "max"):      ("a.max() -> T", "数组中的最大元素（需 T 可比较）"),
    ("array", "sum"):      ("a.sum() -> T", "数组元素求和（需 T 支持加法）"),
    ("array", "resize"):   ("a.resize(n: int)", "调整数组长度为 n，不足零初始化，超出截断"),

    # map 实例方法
    ("map", "len"):    ("m.len() -> int", "键值对个数"),
    ("map", "get"):    ("m.get(k: K) -> V?", "取值；键不存在返回 null"),
    ("map", "keys"):   ("m.keys() -> K[]", "全部键"),
    ("map", "remove"): ("m.remove(k: K)", "删除键"),
    ("map", "has"):    ("m.has(k: K) -> bool", "键是否存在"),
    ("map", "clear"):  ("m.clear()", "清空所有键值对"),

    # decimal 实例方法
    ("decimal", "round"):   ("d.round(scale: int, mode: Rounding) -> decimal", "按位数与舍入模式取整"),
    ("decimal", "divide"):  ("d.divide(other: decimal, scale: int, mode: Rounding) -> decimal", "显式指定精度与舍入的除法"),
    ("decimal", "toString"): ("d.toString() -> string", "转为字符串"),
    ("decimal", "truncate"): ("d.truncate(scale: int) -> decimal", "按指定小数位截断（向零舍入）"),

    ("time", "toString"): ("t.toString() -> string", "按 \"YYYY-MM-DD HH:MM:SS\" 格式化"),
    ("time", "format"): ("t.format(fmt: string) -> string", "按 strftime 风格格式串格式化"),
    ("time", "unixMs"): ("t.unixMs() -> int", "Unix 毫秒数"),
    ("time", "after"):  ("t.after(other: time) -> bool", "t 是否在 other 之后"),
    ("time", "before"): ("t.before(other: time) -> bool", "t 是否在 other 之前"),
    ("time", "equal"):  ("t.equal(other: time) -> bool", "t 是否等于 other"),
    ("time", "compare"): ("t.compare(other: time) -> int", "比较两个时间，返回 -1/0/1"),
    ("time", "sub"):    ("t.sub(other: time) -> int", "两个时间的时间差（毫秒）"),
    ("time", "add"):    ("t.add(ms: int) -> time", "t 加上指定毫秒数后的时间"),
    ("time", "round"):  ("t.round(scale: int) -> time", "按指定精度取整时间"),
    ("time", "truncate"): ("t.truncate(scale: int) -> time", "按指定精度截断时间"),

    # int / float / bool
    ("int", "toString"):    ("x.toString() -> string", "转为字符串"),
    ("int", "wrappingAdd"): ("x.wrappingAdd(y: int) -> int", "回绕加法（溢出不报错）"),
    ("float", "toString"):  ("x.toString() -> string", "转为字符串"),
    ("bool", "toString"):   ("b.toString() -> string", "\"true\"/\"false\""),
    ("enum", "toString"):   ("x.toString() -> string", "变体名"),

    # 静态方法
    ("static", "parse"):        ("T.parse(s: string) -> result<T>", "字符串解析为数值（int/float/int8/16/32/64 等）"),
    ("static", "truncate"):     ("int8/int16/int32.truncate(x: int) -> T", "把 int 截断到窄类型"),
    ("static", "fromCodepoint"): ("string.fromCodepoint(cp: int) -> result<string>", "码点转字符（与模块 string 同名方法等价）"),
    ("static", "compare"):     ("string.compare(s1: string, s2: string) -> int", "比较两个字符串，返回 -1/0/1"),
    ("static", "fields"):      ("string.fields(s: string) -> string[]", "按空白字符拆分字符串为字段数组"),
    ("static", "toUpperRune"): ("string.toUpperRune(cp: int) -> int", "码点转大写后的码点"),
    ("static", "toLowerRune"): ("string.toLowerRune(cp: int) -> int", "码点转小写后的码点"),

    # db Row
    ("Row", "empty"):     ("row.empty() -> bool", "行是否为空"),
    ("Row", "getString"): ("row.getString(col: string) -> string", "取字符串列"),
    ("Row", "getInt"):    ("row.getInt(col: string) -> int", "取整数列"),
    ("Row", "getDecimal"): ("row.getDecimal(col: string) -> decimal", "取金额列"),

    # file File
    ("File", "close"):    ("f.close()", "关闭文件"),
    ("File", "read"):     ("f.read() -> result<string>", "读全部内容"),
    ("File", "readLine"): ("f.readLine() -> string?", "读一行；EOF 返回 null"),
    ("File", "write"):    ("f.write(s: string) -> result<int>", "写入，返回字节数"),
    ("File", "writeLine"): ("f.writeLine(s: string) -> result<int>", "写入一行，返回字节数"),
}

def parse_builtin_arrays(sema_mod_src):
    """从 mod.rs 解析 INTERNAL_BUILTINS / STDLIB_MODULES 常量数组。

    返回 (internal_builtins: list[str], stdlib_modules: list[str])。
    不依赖数组长度字段，只提取引号内的字符串。
    """
    def extract_array(name):
        # 找到 `pub const NAME: [&str; N] = [` ... `]` 的块
        start_pat = re.compile(r'pub\s+const\s+' + re.escape(name) + r'\s*:\s*\[&str;\s*\d+\]\s*=\s*\[')
        m = start_pat.search(sema_mod_src)
        if not m:
            return []
        start = m.end()
        depth = 1
        i = start
        while i < len(sema_mod_src) and depth > 0:
            ch = sema_mod_src[i]
            if ch == '[':
                depth += 1
            elif ch == ']':
                depth -= 1
            i += 1
        block = sema_mod_src[start:i - 1]
        # 提取所有双引号字符串
        return re.findall(r'"([^"]+)"', block)

    internal = extract_array("INTERNAL_BUILTINS")
    stdlib = extract_array("STDLIB_MODULES")
    return internal, stdlib


def lit_tables(member_src):
    """抽 intrinsic 里的类型方法表：TyS::X 块内的 ("Name", vec![...], t_y()) 元组。"""
    start = member_src.find("let table: Option<Vec<(&str, Vec<Ty>, Ty)>> = match &**rt {")
    end = member_src.find("let table = match table", start)
    assert start > 0 and end > start
    seg = member_src[start:end]
    heads = []
    for m in re.finditer(r'TyS::(Str|Array|Map|Decimal|Named)\b[^=]*=>', seg):
        nm = m.group(1)
        if nm == "Named":
            mm = re.search(r'DefId::Name\("([^"]+)"\)', seg[m.start():m.start() + 60])
            key = mm.group(1) if mm else None
            if key is None:
                continue  # DefId::Id(id) 情况，跳过（不产生固定方法表）
            owner = key  # 使用类型名本身作为 owner，如 "time", "thread", "Mutex", "RWMutex", 等
        else:
            owner = {"Str": "string", "Array": "array", "Map": "map", "Decimal": "decimal"}[nm]
        heads.append((m.start(), owner))
    for m in re.finditer(r'TyS::(?:Int\(\.\.\)|Float)\s*\|?\s*(?:TyS::Float)?\s*=>\s*\{', seg):
        heads.append((m.start(), None))
    for m in re.finditer(r'TyS::Bool\s*\|\s*TyS::Enum\([^)]*\)\s*=>\s*\{', seg):
        heads.append((m.start(), None))
    heads.sort()
    out = collections.defaultdict(list)
    for m in re.finditer(r'\(\s*"([A-Za-z_]\w*)"\s*,\s*vec!\[', seg):
        owner = None
        for pos, name in heads:
            if pos < m.start():
                owner = name
        if owner:
            if m.group(1) not in out[owner]:
                out[owner].append(m.group(1))
    return out


def named_tables(member_src):
    """check_builtin_instance 里的 ("Row"/"File", "Method") 表。"""
    start = member_src.find("fn check_builtin_instance")
    end = member_src.find("pub fn method_owner", start)
    seg = member_src[start:end]
    out = collections.defaultdict(list)
    for m in re.finditer(r'\("(\w+)",\s*"(\w+)"\)\s*=>', seg):
        if m.group(2) not in out[m.group(1)]:
            out[m.group(1)].append(m.group(2))
    return out


def module_tables(expr_src):
    """module_sig() 里的 ("mod", "name") 表。"""
    start = expr_src.find("pub fn module_sig")
    end = expr_src.find("impl<'a> Sema<'a>", start)
    seg = expr_src[start:end]
    out = collections.defaultdict(list)
    for m in re.finditer(r'\("(\w+)",\s*"(\w+)"\)\s*=>', seg):
        if m.group(2) not in out[m.group(1)]:
            out[m.group(1)].append(m.group(2))
    return out


def static_names(member_src):
    start = member_src.find("fn check_builtin_static")
    end = member_src.find("fn check_module_call", start)
    seg = member_src[start:end]
    out = []
    for m in re.finditer(r'(?<!type_)name == "(\w+)"', seg):
        if m.group(1) not in out:
            out.append(m.group(1))
    return out


def main():
    member_src = open(MEMBER, encoding="utf-8").read()
    expr_src = open(EXPR, encoding="utf-8").read()
    sema_mod_src = open(SEMAMOD, encoding="utf-8").read()

    # 从编译器源码读取模块归类（单一事实源）
    internal_builtins, stdlib_modules = parse_builtin_arrays(sema_mod_src)

    intr = lit_tables(member_src)
    named = named_tables(member_src)
    mods = module_tables(expr_src)
    statics = static_names(member_src)
    globals_ = ["print", "println", "ok", "fail", "parent", "typeof", "sizeof"]

    pending = []
    def row(key, owner, name):
        d = DESC.get((key, name))
        if d is None:
            pending.append((key, name))
            return "| `%s` | *签名待补充*（在 tools/gen_stdlib_doc.py 的 DESC 里补） |" % name
        return "| `%s` | %s——%s |" % (name, d[0], d[1])

    L = [
        "<!-- 本文件由 tools/gen_stdlib_doc.py 生成，请勿手改；内建方法变更后重新运行。 -->",
        "<!-- 生成时间：%s -->" % datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
        "",
        "# axlang 内置库与标准库",
        "",
        "本文档由 `tools/gen_stdlib_doc.py` 从编译器源码（`rust/src/sema/mod.rs` 的",
        "`INTERNAL_BUILTINS` / `STDLIB_MODULES` 及 `expr.rs` 的 `module_sig()`）",
        "自动抽取生成。内建与标准库模块互斥，不得并存；模块归类以编译器源码为准。",
        "",
    ]

    # ---- 全局函数 ----
    L += ["## 全局函数", "", "| 函数 | 签名 / 说明 |", "|---|---|"]
    for n in globals_:
        L.append(row("global", "global", n))
    L.append("")

    # ---- 内建模块（INTERNAL_BUILTINS） ----
    L += ["## 内建模块（`INTERNAL_BUILTINS`）", ""]
    L += [
        "以下模块为编译器 Rust 端内建，无对应 ax 源码，裸模块名直接使用，",
        "无需 `import`（如 `fmt.printf(\"pi = %.2f\\n\", 3.14159)`）。",
        "与标准库模块互斥，不得并存。",
        "",
    ]
    if not internal_builtins:
        L.append("*未在 `mod.rs` 中发现 `INTERNAL_BUILTINS` 条目。*")
        L.append("")
    for mod_name in internal_builtins:
        names = mods.get(mod_name, [])
        L.append("### %s" % mod_name)
        L.append("")
        if not names:
            L.append("*此内建模块当前没有通过 `module_sig()` 注册的符号。*")
        else:
            for n in names:
                L.append(row("module", mod_name, mod_name + "." + n))
        L.append("")

    # ---- 标准库模块（STDLIB_MODULES） ----
    L += ["## 标准库模块（`STDLIB_MODULES`）", ""]
    L += [
        "以下模块用 ax 语法编写，位于 `axc/std/*.ax`，通过 `import std.xxx` 导入，",
        "对应 ax 层与 C 运行时 `ax_std_*` 接口的桥梁。模块名统一以 `std.` 前缀开头，",
        "如 `std.string`、`std.file`、`std.io` 等。",
        "",
    ]
    if not stdlib_modules:
        L.append("*未在 `mod.rs` 中发现 `STDLIB_MODULES` 条目。*")
        L.append("")
    for full in stdlib_modules:
        mod_name = full[len("std."):] if full.startswith("std.") else full
        # 标准库模块的符号不在 module_sig() 中（它们是 ax 源码），此处仅列出模块名
        L.append("### %s" % full)
        L.append("")
        L.append("*对应 `axc/std/%s.ax`（标准库 ax 源码），符号定义见各模块源文件。*\n" % mod_name)
        L.append("")

    # ---- 内建类型实例方法 ----
    L += ["## 内建类型实例方法", ""]
    order = [("string", intr.get("string", [])),
             ("array", intr.get("array", [])),
             ("map", intr.get("map", [])),
             ("decimal", intr.get("decimal", [])),
             ("time", intr.get("time", []))]
    for owner, names in order:
        L.append("### %s" % owner)
        L.append("")
        for n in names:
            L.append(row(owner, owner, n))
        L.append("")
    for owner, names in [("int", ["toString", "wrappingAdd"]), ("float", ["toString"]),
                         ("bool", ["toString"]), ("enum", ["toString"])]:
        L.append("### %s" % owner)
        L.append("")
        for n in names:
            L.append(row(owner, owner, n))
        L.append("")

    # ---- 静态方法 ----
    L += ["## 静态方法", "", "| 方法 | 签名 / 说明 |", "|---|---|"]
    for n in statics:
        L.append(row("static", "static", n))
    L.append("")

    # ---- Row / File ----
    L += ["## 数据库行：`Row`（`db.Query` 的元素）", "", "| 方法 | 签名 / 说明 |", "|---|---|"]
    for n in named.get("Row", []):
        L.append(row("Row", "Row", n))
    L.append("")

    L += ["## 文件句柄：`File`（`file.Open / file.Create / file.Append` 的返回值）", "", "| 方法 | 签名 / 说明 |", "|---|---|"]
    for n in named.get("File", []):
        L.append(row("File", "File", n))
    L.append("")

    if pending:
        L += ["## 待补充", "",
              "以下条目已存在于编译器内建表，但 `DESC` 字典还没有描述：", ""]
        for key, name in pending:
            L.append("- %s.%s" % (key, name))
        L.append("")

    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    open(OUT, "w", encoding="utf-8", newline="\n").write("\n".join(L))
    print("已生成 %s（全局 %d，内建模块 %d，标准库模块 %d，待补充 %d）" %
          (os.path.relpath(OUT, ROOT), len(globals_), len(internal_builtins),
           len(stdlib_modules), len(pending)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
