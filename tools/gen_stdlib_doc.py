#!/usr/bin/env python3
"""生成 docs/STDLIB.md（内置库与标准库说明）。

设计：
  - **方法名 / 归属**：直接从 rust/src/sema 源码抽取（intrinsic 类型方法表、
    check_builtin_instance 的 Row/File 表、check_builtin_static、expr.rs 的
    module_sig、全局函数 print/println/ok/fail）。新增方法只要进了这些表，
    重新运行本脚本就会自动出现在文档里。
  - **签名 / 说明**：来自下面的 DESC 字典。源码里有而字典没有的条目
    会以「签名待补充」占位——补字典后重跑即可，不会漏。
用法：python tools/gen_stdlib_doc.py
"""
import os, re, sys, collections

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MEMBER = os.path.join(ROOT, "rust", "src", "sema", "member.rs")
EXPR = os.path.join(ROOT, "rust", "src", "sema", "expr.rs")
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

    # 内建模块
    ("module", "io.ReadLine"):   ("io.ReadLine() -> string?", "从标准输入读一行；EOF 返回 null"),
    ("module", "io.ReadFile"):   ("io.ReadFile(path: string) -> result<string>", "读整个文本文件"),
    ("module", "math.Equals"):   ("math.Equals(a: float, b: float, eps: float) -> bool", "浮点带容差比较"),
    ("module", "math.Compare"):  ("math.Compare(a: float, b: float) -> int", "浮点比较：-1/0/1"),
    ("module", "json.Parse"):    ("json.Parse(s: string) -> result<map<string, object>>", "解析 JSON 文本"),
    ("module", "json.Stringify"): ("json.Stringify(v: object) -> string", "序列化 object 为 JSON"),
    ("module", "db.Query"):      ("db.Query(sql: string) -> result<Row[]>", "执行查询，返回行数组"),
    ("module", "file.Open"):     ("file.Open(path: string) -> result<File>", "以读写打开文件"),
    ("module", "file.Create"):   ("file.Create(path: string) -> result<File>", "创建/截断文件"),
    ("module", "file.Append"):   ("file.Append(path: string) -> result<File>", "以追加打开文件"),
    ("module", "string.FromCodepoint"): ("string.FromCodepoint(cp: int) -> result<string>", "码点转字符（`string` 既是类型也是模块，与静态方法同名同拼写）"),
    ("module", "time.NowMs"):   ("time.NowMs() -> int", "当前 Unix 时间（毫秒）"),

    # string 实例方法
    ("string", "Len"):        ("s.Len() -> int", "字符数（按码点）"),
    ("string", "ByteLen"):    ("s.ByteLen() -> int", "UTF-8 字节数"),
    ("string", "At"):         ("s.At(i: int) -> string?", "第 i 个字符；越界返回 null"),
    ("string", "Chars"):      ("s.Chars() -> string[]", "拆成单字符数组"),
    ("string", "CodepointAt"): ("s.CodepointAt(i: int) -> int?", "第 i 个字符的码点；越界返回 null"),
    ("string", "Contains"):   ("s.Contains(sub: string) -> bool", "是否包含子串"),
    ("string", "Split"):      ("s.Split(sep: string) -> string[]", "按分隔符拆分"),
    ("string", "ToString"):   ("s.ToString() -> string", "自身"),
    ("string", "Trim"):       ("s.Trim() -> string", "去除两端空白"),
    ("string", "Upper"):      ("s.Upper() -> string", "转大写"),
    ("string", "Lower"):      ("s.Lower() -> string", "转小写"),
    ("string", "StartsWith"): ("s.StartsWith(p: string) -> bool", "前缀判断"),
    ("string", "EndsWith"):   ("s.EndsWith(p: string) -> bool", "后缀判断"),
    ("string", "IndexOf"):    ("s.IndexOf(sub: string) -> int", "子串位置，找不到返回 -1"),
    ("string", "Replace"):    ("s.Replace(old: string, new: string) -> string", "替换全部子串"),
    ("string", "Substring"):  ("s.Substring(start: int, end: int) -> string", "按码点区间取子串"),
    ("string", "Join"):       ("s.Join(parts: string[]) -> string", "用 s 连接数组"),
    ("string", "Repeat"):     ("s.Repeat(n: int) -> string", "重复 n 次"),

    # 数组实例方法
    ("array", "Len"):      ("a.Len() -> int", "元素个数"),
    ("array", "Push"):     ("a.Push(v: T)", "尾部追加（就地修改）"),
    ("array", "Pop"):      ("a.Pop() -> T", "弹出并返回尾元素"),
    ("array", "Remove"):   ("a.Remove(i: int)", "按下标删除"),
    ("array", "Clear"):    ("a.Clear()", "清空"),
    ("array", "At"):       ("a.At(i: int) -> T?", "按下标取；越界返回 null"),
    ("array", "Sort"):     ("a.Sort(cmp: func(T, T) int)", "按比较函数稳定排序（cmp<0 表示前者在前）"),
    ("array", "ToString"): ("a.ToString() -> string", "转为展示字符串"),

    # map 实例方法
    ("map", "Len"):    ("m.Len() -> int", "键值对个数"),
    ("map", "Get"):    ("m.Get(k: K) -> V?", "取值；键不存在返回 null"),
    ("map", "Keys"):   ("m.Keys() -> K[]", "全部键"),
    ("map", "Remove"): ("m.Remove(k: K)", "删除键"),
    ("map", "Has"):    ("m.Has(k: K) -> bool", "键是否存在"),

    # decimal 实例方法
    ("decimal", "Round"):   ("d.Round(scale: int, mode: Rounding) -> decimal", "按位数与舍入模式取整"),
    ("decimal", "Divide"):  ("d.Divide(other: decimal, scale: int, mode: Rounding) -> decimal", "显式指定精度与舍入的除法"),
    ("decimal", "ToString"): ("d.ToString() -> string", "转为字符串"),

    # int / float / bool
    ("int", "ToString"):    ("x.ToString() -> string", "转为字符串"),
    ("int", "WrappingAdd"): ("x.WrappingAdd(y: int) -> int", "回绕加法（溢出不报错）"),
    ("float", "ToString"):  ("x.ToString() -> string", "转为字符串"),
    ("bool", "ToString"):   ("b.ToString() -> string", "\"true\"/\"false\""),
    ("enum", "ToString"):   ("x.ToString() -> string", "变体名"),

    # 静态方法
    ("static", "Parse"):        ("T.Parse(s: string) -> result<T>", "字符串解析为数值（int/float/int8/16/32/64 等）"),
    ("static", "Truncate"):     ("int8/int16/int32.Truncate(x: int) -> T", "把 int 截断到窄类型"),
    ("static", "FromCodepoint"): ("string.FromCodepoint(cp: int) -> result<string>", "码点转字符（与模块 string 同名方法等价）"),

    # db Row
    ("Row", "Empty"):     ("row.Empty() -> bool", "行是否为空"),
    ("Row", "GetString"): ("row.GetString(col: string) -> string", "取字符串列"),
    ("Row", "GetInt"):    ("row.GetInt(col: string) -> int", "取整数列"),
    ("Row", "GetDecimal"): ("row.GetDecimal(col: string) -> decimal", "取金额列"),

    # file File
    ("File", "Close"):    ("f.Close()", "关闭文件"),
    ("File", "Read"):     ("f.Read() -> result<string>", "读全部内容"),
    ("File", "ReadLine"): ("f.ReadLine() -> string?", "读一行；EOF 返回 null"),
    ("File", "Write"):    ("f.Write(s: string) -> result<int>", "写入，返回字节数"),
    ("File", "WriteLine"): ("f.WriteLine(s: string) -> result<int>", "写入一行，返回字节数"),
}

def lit_tables(member_src):
    """抽 intrinsic 里的类型方法表：TyS::X 块内的 ("Name", vec![...], t_y()) 元组。"""
    start = member_src.find("let table: Option<Vec<(&str, Vec<Ty>, Ty)>> = match &**rt {")
    end = member_src.find("let table = match table", start)
    assert start > 0 and end > start
    seg = member_src[start:end]
    # 块头位置
    heads = []
    for m in re.finditer(r'TyS::(Str|Array|Map|Decimal)\b[^=]*=>', seg):
        heads.append((m.start(), {"Str": "string", "Array": "array", "Map": "map", "Decimal": "decimal"}[m.group(1)]))
    # Int/Float、Bool/Enum 走 if 分支而不是表，占位防止归属前一块
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
        "",
        "# axlang 内置库与标准库",
        "",
        "本文档由 `tools/gen_stdlib_doc.py` 从编译器源码（`rust/src/sema/member.rs` 与",
        "`expr.rs` 的内建签名表）自动抽取生成。给内建表新增方法后重新运行该脚本，",
        "新方法会自动列出；描述在脚本的 `DESC` 字典里维护，缺描述的条目会标「签名待补充」。",
        "",
    ]

    L += ["## 全局函数", "", "| 函数 | 签名 / 说明 |", "|---|---|"]
    for n in globals_:
        L.append(row("global", "global", n))
    L.append("")

    L += ["## 内建模块（`BUILTIN_MODULES`）", "",
          "`db` / `file` / `io` / `json` / `math` / `string` / `time` 是编译器内建模块，",
          "与用户模块同名会被拒绝（`E3039`）。", ""]
    for mod in ["db", "file", "io", "json", "math", "string", "time"]:
        names = mods.get(mod, [])
        if not names:
            L.append("### %s" % mod)
            L.append("")
            L.append("*此模块当前没有通过 `module_sig` 注册的符号。*")
            L.append("")
            continue
        L.append("### %s" % mod)
        L.append("")
        for n in names:
            L.append(row("module", mod, mod + "." + n))
        L.append("")

    L += ["## 内建类型实例方法", ""]
    order = [("string", intr.get("string", [])),
             ("array", intr.get("array", [])),
             ("map", intr.get("map", [])),
             ("decimal", intr.get("decimal", []))]
    for owner, names in order:
        L.append("### %s" % owner)
        L.append("")
        for n in names:
            L.append(row(owner, owner, n))
        L.append("")
    # int/float/bool/enum 走 if 分支而非表，靠 DESC 维护；源码核对存在性
    for owner, names in [("int", ["ToString", "WrappingAdd"]), ("float", ["ToString"]),
                         ("bool", ["ToString"]), ("enum", ["ToString"])]:
        L.append("### %s" % owner)
        L.append("")
        for n in names:
            L.append(row(owner, owner, n))
        L.append("")

    L += ["## 静态方法", "", "| 方法 | 签名 / 说明 |", "|---|---|"]
    for n in statics:
        L.append(row("static", "static", n))
    L.append("")

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
    print("已生成 %s（全局 %d，模块 %d 组，待补充 %d）" %
          (os.path.relpath(OUT, ROOT), len(globals_), len(mods), len(pending)))
    return 0

if __name__ == "__main__":
    sys.exit(main())
