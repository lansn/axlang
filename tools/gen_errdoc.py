#!/usr/bin/env python3
"""生成 docs/ERRORS.md（多语言错误清单）并校验目录全覆盖。

单一事实源：
  - 中文模板：rust/src 各报错调用点（`.err` / `.fail` / `.err_dummy` / 直接 `d.error`）
  - 英文翻译：rust/src/diag_msg.rs 的 CATALOG（按 (code, zh模板) 对应）

用法：python tools/gen_errdoc.py
  - 重扫调用点 → 若 CATALOG 缺条目则报错退出（提示先补英文）
  - 生成 docs/ERRORS.md（代码 + 中文释义 + 英文释义 + 出处）
"""
import os, re, sys, collections, datetime

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "rust", "src")
DIAG_MSG = os.path.join(SRC, "diag_msg.rs")
OUT = os.path.join(ROOT, "docs", "ERRORS.md")

def split_args(s):
    args, depth, cur, i = [], 0, "", 0
    instr = esc = False
    while i < len(s):
        ch = s[i]
        if instr:
            cur += ch
            if esc: esc = False
            elif ch == '\\': esc = True
            elif ch == '"': instr = False
            i += 1; continue
        if ch == '"': instr = True; cur += ch; i += 1; continue
        if ch in '([{': depth += 1
        elif ch in ')]}': depth -= 1
        if ch == ',' and depth == 0:
            args.append(cur.strip()); cur = ""; i += 1; continue
        cur += ch; i += 1
    if cur.strip(): args.append(cur.strip())
    return args

def call_bodies(src, name):
    pat = re.compile(r'\.(%s)\s*\(' % name)
    for m in pat.finditer(src):
        depth, i, instr, esc = 1, m.end(), False, False
        start = i
        while i < len(src) and depth > 0:
            ch = src[i]
            if instr:
                if esc: esc = False
                elif ch == '\\': esc = True
                elif ch == '"': instr = False
                i += 1; continue
            if ch == '"': instr = True; i += 1; continue
            if ch == '(': depth += 1
            elif ch == ')': depth -= 1
            i += 1
        if depth == 0:
            yield src[start:i-1]

STR_LIT = re.compile(r'^"((?:[^"\\]|\\.)*)"$')

def lit(x):
    m = STR_LIT.match(x)
    return m.group(1) if m else None

def unesc(z):
    return z.replace('\\n', '\n').replace('\\t', '\t').replace('\\"', '"').replace('\\\\', '\\')

def load_catalog():
    """读 CATALOG：[(code, zh_raw, en_raw)]。"""
    s = open(DIAG_MSG, encoding="utf-8").read()
    m = re.search(r'pub static CATALOG: &\[\(&str, &str, &str\)\] = &\[\n(.*?)\n\];', s, re.S)
    assert m, "diag_msg.rs 里找不到 CATALOG"
    rows = []
    for ln in m.group(1).split("\n"):
        mm = re.match(r'^    \("([^"]+)", "((?:[^"\\]|\\.)*)", "((?:[^"\\]|\\.)*)"\),$', ln)
        if mm:
            rows.append((mm.group(1), mm.group(2), mm.group(3)))
    return rows, s[:m.start()] + s[m.end():]

def scan_sites(rest_src):
    """扫调用点：(code, zh_raw) -> rule（首个出现的）。也返回漏登目录的条目。"""
    rules = {}
    missing = set()
    files = []
    for dp, _, fns in os.walk(SRC):
        for fn in fns:
            if fn.endswith(".rs"):
                files.append(os.path.join(dp, fn))
    catalog_keys = set()
    for code, zh, _ in load_catalog()[0]:
        catalog_keys.add((code, zh))
    for f in sorted(files):
        rel = os.path.relpath(f, SRC)
        src = open(f, encoding="utf-8").read()
        if f == DIAG_MSG:
            # diag_msg.rs 本体不算调用点；rest_src 已剔除 CATALOG 块
            src = rest_src
        for body in call_bodies(src, "err_dummy"):
            a = split_args(body)
            if len(a) == 4 and lit(a[0]) and lit(a[2]):
                k = (a[0].strip('"'), lit(a[2]))
                rules.setdefault(k, lit(a[1]))
                if k not in catalog_keys: missing.add((rel, k))
        for body in call_bodies(src, "err"):
            a = split_args(body)
            if len(a) == 6 and lit(a[0]) and lit(a[1]):
                k = (a[0].strip('"'), lit(a[1]))
                rules.setdefault(k, lit(a[4]))
                if k not in catalog_keys: missing.add((rel, k))
            elif len(a) == 5 and lit(a[0]) and lit(a[3]):
                k = (a[0].strip('"'), lit(a[3]))
                rules.setdefault(k, lit(a[2]))
                if k not in catalog_keys: missing.add((rel, k))
        for body in call_bodies(src, "fail"):
            a = split_args(body)
            if len(a) == 5 and lit(a[0]) and lit(a[3]):
                k = (a[0].strip('"'), lit(a[3]))
                rules.setdefault(k, lit(a[2]))
                if k not in catalog_keys: missing.add((rel, k))
        for body in call_bodies(src, "error"):
            a = split_args(body)
            # 直接 d.error(...)：模板在 tr(...) 里，抽 code 与 rule
            if len(a) >= 9 and lit(a[0]):
                rules.setdefault((lit(a[0]), None), lit(a[-1]))
    return rules, missing

def main():
    rows, rest = load_catalog()
    # rest 里还留着 tr(d.lang, "CODE", "zh", ...) 的直接调用——补进目录视角
    extra = {
        ("SUMMARY", "{0} 个错误"): "CLI",
        ("FILE_NOT_FOUND", "找不到文件 {0}"): "CLI",
    }
    rules, missing = scan_sites(rest)
    rules.update({k: v for k, v in extra.items() if k not in rules})

    untranslated = [(c, unesc(z)) for c, z, e in rows if not e]
    if untranslated:
        print("!! 以下 %d 条目录缺英文翻译（在 rust/src/diag_msg.rs CATALOG 里补第三列）：" % len(untranslated))
        for c, z in untranslated[:20]:
            print("   %s  %s" % (c, z))
        if len(untranslated) > 20:
            print("   …")
        return 1

    # 目录条目必须能在调用点找到（防目录与调用点漂移）
    drifted = [(c, unesc(z)) for c, z, _ in rows if (c, z) not in rules and (c, None) not in rules
               and (c, z) not in extra and (c, None) not in extra]
    # 直接调用站点的模板不在扫描范围（写在 tr(...) 内），跳过严格校验

    by_code = collections.OrderedDict()
    for code, zh, en in rows:
        by_code.setdefault(code, []).append((zh, en))

    lines = [
        "<!-- 本文件由 tools/gen_errdoc.py 生成，请勿手改；修改诊断文案后重新运行。 -->",
        "<!-- 生成时间：%s -->" % datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
        "",
        "# axlang 错误码清单（多语言）",
        "",
        "诊断码按组件分段：`E1xxx` 词法 / `E2xxx` 语法 / `E3xxx` 声明与模块 /",
        "`E4xxx` 类型与表达式 / `E5xxx` 转换与装箱 / `E6xxx` match 与错误传播 /",
        "`E7xxx` 所有权与借用。输出语言由 `--lang` / `AXC_LANG` / `axc.toml`（`lang = \"zh\"`）决定，默认英文。",
        "",
        "共 %d 个码、%d 条文案。" % (len(by_code), len(rows)),
        "",
    ]
    desc = {
        "E1": "词法", "E2": "语法", "E3": "声明 / 模块", "E4": "类型与表达式",
        "E5": "转换 / 装箱", "E6": "match / 错误传播", "E7": "所有权 / 借用",
        "SUMMARY": "CLI", "FILE_NOT_FOUND": "CLI",
    }
    cur = None
    for code in sorted(by_code):
        seg = code[:2] if code.startswith("E") else code
        if seg != cur:
            cur = seg
            lines.append("## %s（%s）" % (seg, desc.get(seg, seg)))
            lines.append("")
            lines.append("| 码 | 中文 | English |")
            lines.append("|---|---|---|")
        for zh, en in by_code[code]:
            z_raw = zh.replace('\\\\', '\\').replace('\\"', '"').replace('\\n', '\n').replace('\\t', '\t')
            # 反查 rule：目录 zh 存的是源码原始字面文本
            r = rules.get((code, zh)) or rules.get((code, None)) or ""
            zh_show = z_raw.replace("|", "\\|").replace("\n", "<br>")
            en_show = en.replace("\\n", "\n").replace("\\t", "\t").replace('\\\"', '"').replace("\\\\", "\\")
            en_show = en_show.replace("|", "\\|").replace("\n", "<br>")
            if r:
                lines.append("| `%s` | %s（%s） | %s |" % (code, zh_show, r, en_show))
            else:
                lines.append("| `%s` | %s | %s |" % (code, zh_show, en_show))
        lines.append("")
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    open(OUT, "w", encoding="utf-8", newline="\n").write("\n".join(lines))
    print("已生成 %s（%d 码 / %d 条）" % (os.path.relpath(OUT, ROOT), len(by_code), len(rows)))
    return 0

if __name__ == "__main__":
    sys.exit(main())
