#!/usr/bin/env python3
"""把报错调用点从 (code, msg, ...) 改写为 (code, tok/rule, zh模板, &[args])。

处理三种形态（按文件区分）：
  sema/parser  .err("CODE", MSG, TOK, RULE)        -> .err("CODE", TOK, RULE, TPL, &[..])
  err_dummy    .err_dummy("CODE", MSG, RULE)       -> .err_dummy("CODE", RULE, TPL, &[..])
  lexer        .err("CODE", MSG, I0, I1, RULE)     -> .err("CODE", TPL, I0, I1, RULE, &[..])
MSG 为 format!(..) 时抽取模板（内联捕获/位置实参 → {0},{1}…）；
MSG 为字符串字面量时模板即其内容、无实参。其余形态进手工清单。
"""
import re, os, sys, json, collections

ROOT = os.path.join(os.path.dirname(__file__), "..", "rust", "src")
LEXER_FILES = {"lexer.rs"}

def read(p): return open(p, encoding="utf-8").read()

def find_calls(src, names):
    """返回 [(name, start, end, body)]，end 指向闭括号之后。"""
    out = []
    pat = re.compile(r'\.(%s)\s*\(' % "|".join(names))
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
        if depth != 0: continue
        out.append((m.group(1), m.end() - len(m.group(1)) - len(m.group(0)) + len(m.group(0)) - 1, i, src[start:i-1]))
        # 修正 start：调用点从 ".err" 的 '.' 开始
        out[-1] = (m.group(1), m.start(), i, src[start:i-1])
    return out

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

STR_LIT = re.compile(r'^"((?:[^"\\]|\\.)*)"$')
FMT = re.compile(r'^format!\s*\(')

def parse_format(fmt_inner):
    """fmt_inner: format!( 之内、) 之前的文本。
    返回 (tpl_code, args_exprs, zh_plain) 或 None（需手工）。"""
    parts = split_args(fmt_inner)
    m = STR_LIT.match(parts[0])
    if not m: return None
    raw = m.group(1)
    positional = parts[1:]
    # 不支持命名实参
    for p in positional:
        if re.match(r'^[A-Za-z_]\w*\s*=', p) and not p.startswith(("&&", "&")):
            return None
    # 展开格式串：抽槽位
    if re.search(r'\{(\d+)\}', raw) and '{}' in raw:
        return None
    args, slots = [], []
    i, n = 0, len(raw)
    while i < n:
        c = raw[i]
        if c == '{':
            if i + 1 < n and raw[i+1] == '{':
                slots.append((None, "{{")); i += 2; continue
            j = raw.find('}', i)
            if j < 0: return None
            inner = raw[i+1:j]
            if inner == "":
                if not positional: return None
                args.append(positional.pop(0)); slots.append((len(args)-1, None))
            elif inner.isdigit():
                idx = int(inner)
                if idx >= len(positional): return None
                # 位置引用按出现顺序取
                args.append(positional[idx] if idx < len(positional) else None)
                slots.append((len(args)-1, None))
                positional.pop(idx) if idx < len(positional) else None
            else:
                # 内联捕获：ident 或 ident.path(.0 等)，可带 :spec
                spec_i = inner.find(':')
                cap, spec = (inner[:spec_i], inner[spec_i:]) if spec_i >= 0 else (inner, "")
                if spec and spec != ":":
                    return None  # 带格式符的槽位 → 手工
                if spec == ":":
                    return None  # "{x:}" 未知
                if not re.match(r'^[A-Za-z_]\w*(\.[A-Za-z_0-9]+)*$', cap):
                    return None
                args.append(cap); slots.append((len(args)-1, None))
            i = j + 1; continue
        if c == '}' and i + 1 < n and raw[i+1] == '}':
            slots.append((None, "}}")); i += 2; continue
        slots.append((None, c)); i += 1
    if positional:
        return None
    # 重排编号：按出现顺序
    mapping = {}
    order = 0
    out_tpl = []
    for idx, lit in slots:
        if lit is not None:
            out_tpl.append(lit); continue
        if idx not in mapping:
            mapping[idx] = order; order += 1
        out_tpl.append("{%d}" % mapping[idx])
    tpl_code = "".join(out_tpl)
    zh_plain = tpl_code.replace("{{", "{").replace("}}", "}").replace('\\"', '"').replace("\\'", "'").replace("\\n", "\n").replace("\\t", "\t")
    return (tpl_code, args, zh_plain)

def unescape_lit(raw):
    return raw.replace('\\"', '"').replace("\\'", "'").replace("\\n", "\n").replace("\\t", "\t").replace("{{", "{").replace("}}", "}")

def transform(path):
    src = read(path)
    base = os.path.basename(path)
    calls = find_calls(src, ["err", "err_dummy", "fail"])
    calls.sort(key=lambda t: t[1])
    manual, rewritten = [], []
    catalog = []  # (code, zh_plain)
    pieces, last = [], 0
    for name, s, e, body in calls:
        args = split_args(body)
        is_dummy = name == "err_dummy"
        is_fail = name == "fail"
        is_lexer = base in LEXER_FILES
        # 形态识别
        if is_dummy:
            ok_shape = len(args) == 3
            code_a, msg_a, tail_a = args[0], args[1], args[2]
        elif is_lexer:
            ok_shape = len(args) == 5 or len(args) == 6
            if len(args) == 6:  # 已改写的新形态，跳过由外层 already 判定
                code_a, msg_a, tail_a = args[0], "", args[1:]
            else:
                code_a, msg_a, tail_a = args[0], args[1], args[2:]  # i0,i1,rule
        else:
            ok_shape = len(args) == 4
            code_a, msg_a, tail_a = args[0], args[1], args[2:]  # tok,rule
        already = args[-1].startswith("&[") or (len(args) >= 1 and args[-1].strip() == "&[]")
        if already:
            continue
        if not ok_shape or not code_a.startswith('"'):
            manual.append((path, s, body[:90])); continue
        code = code_a.strip('"')
        # MSG
        ma = STR_LIT.match(msg_a)
        if ma is None and msg_a.endswith(".to_string()"):
            ma = STR_LIT.match(msg_a[:-len(".to_string()")].strip())
            if ma is not None: msg_a = msg_a[:-len(".to_string()")].strip()
        if ma:
            tpl_code = ma.group(1)
            zh = unescape_lit(tpl_code)
            arg_exprs = []
        elif FMT.match(msg_a):
            # 找 format!( 的配对括号
            p0 = FMT.match(msg_a).end() - 1
            depth, i, instr, esc = 1, p0 + 1, False, False
            start = p0 + 1
            while i < len(msg_a) and depth > 0:
                ch = msg_a[i]
                if instr:
                    if esc: esc = False
                    elif ch == '\\': esc = True
                    elif ch == '"': instr = False
                    i += 1; continue
                if ch == '"': instr = True; i += 1; continue
                if ch == '(': depth += 1
                elif ch == ')': depth -= 1
                i += 1
            parsed = parse_format(msg_a[start:i-1]) if depth == 0 else None
            if parsed is None:
                manual.append((path, s, body[:90])); continue
            tpl_code, arg_exprs, zh = parsed
            arg_exprs = [a for a in arg_exprs if a is not None]
        else:
            manual.append((path, s, body[:90])); continue
        new_args = "&[" + ", ".join("&" + a for a in arg_exprs) + "]"
        if is_fail:
            new_call = '.fail("%s", %s, "%s", %s)' % (code, ", ".join(tail_a), tpl_code, new_args)
        elif is_dummy:
            new_call = '.err_dummy("%s", %s, "%s", %s)' % (code, tail_a, tpl_code, new_args)
        elif is_lexer:
            new_call = '.err("%s", "%s", %s, %s)' % (code, tpl_code, ", ".join(tail_a), new_args)
        else:
            new_call = '.err("%s", %s, "%s", %s)' % (code, ", ".join(tail_a), tpl_code, new_args)
        pieces.append(src[last:s]); pieces.append(new_call)
        last = e
        rewritten.append((code, zh))
        catalog.append((code, zh))
    pieces.append(src[last:])
    return "".join(pieces), manual, catalog, len(calls)

def main():
    files = []
    for dp, _, fns in os.walk(ROOT):
        for fn in fns:
            if fn.endswith(".rs"): files.append(os.path.join(dp, fn))
    all_catalog = []
    all_manual = []
    total = 0
    for f in sorted(files):
        src = read(f)
        if ".err(" not in src and ".err_dummy(" not in src: continue
        new, manual, catalog, n = transform(f)
        total += n
        if manual: all_manual += manual
        if catalog:
            all_catalog += catalog
            open(f, "w", encoding="utf-8", newline="").write(new)
            print("rewrite %-38s calls=%3d rewritten=%3d manual=%d" % (os.path.relpath(f, ROOT), n, len(catalog), len(manual)))
    # 目录去重
    seen = {}
    for c, z in all_catalog: seen.setdefault((c, z), True)
    json.dump([{"code": c, "zh": z} for (c, z) in seen], open("tools/_catalog.json", "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    print("\n总调用点 %d，改写 %d 条目录项（去重后 %d），手工 %d" % (total, len(all_catalog), len(seen), len(all_manual)))
    for p, s, b in all_manual:
        print("MANUAL %s @%d: %s" % (os.path.relpath(p, ROOT), s, b.replace("\n", " ")[:110]))

if __name__ == "__main__":
    main()
