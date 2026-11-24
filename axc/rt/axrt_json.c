/* axrt_json.c -- axlang 运行时模块 */
#include "axrt.h"
#include "axrt_internal.h"


/* ================================================================ 标准库 */
ax_row ax_db_query(ax_str sql, ax_str arg) {
    (void)sql;
    ax_row r; r.kv = NULL; r.n = 0;
    const char* fx = getenv("AX_DB_FIXTURE");
    if (!fx) return r;
    ax_str s = ax_env_utf8("AX_DB_FIXTURE");
    int64_t n = 0;
    ax_str* parts = ax_str_split(s, ax_str_from_cstr(","), &n);
    if (n == 0) return r;
    /* id 必须匹配 arg；没有 id 字段时视为匹配 */
    int found_id = 0, matched = 0;
    for (int64_t i = 0; i < n; i++) {
        ax_str p = parts[i];
        int64_t eq = -1;
        for (int64_t k = 0; k < p.len; k++) if (p.p[k] == '=') { eq = k; break; }
        if (eq < 0) continue;
        ax_str key = ax_str_from_bytes(p.p, eq);
        ax_str val = ax_str_from_bytes(p.p + eq + 1, p.len - eq - 1);
        if (ax_str_eq(key, ax_str_from_cstr("id"))) {
            found_id = 1;
            if (ax_str_eq(val, arg)) matched = 1;
        }
    }
    if (found_id && !matched) return r;
    r.kv = (ax_str*)ax_alloc((int64_t)sizeof(ax_str) * 2 * n);
    r.n = n;
    for (int64_t i = 0; i < n; i++) {
        ax_str p = parts[i];
        int64_t eq = -1;
        for (int64_t k = 0; k < p.len; k++) if (p.p[k] == '=') { eq = k; break; }
        if (eq < 0) { r.kv[i*2] = p; r.kv[i*2+1] = ax_str_empty(); continue; }
        r.kv[i*2] = ax_str_from_bytes(p.p, eq);
        r.kv[i*2+1] = ax_str_from_bytes(p.p + eq + 1, p.len - eq - 1);
    }
    return r;
}

/* db.query(...) 的 result<Row> 形态（§11.3 / RUN-1）。
   夹具来自环境变量 AX_DB_FIXTURE，形如 "id=1001,name=柴犬粮,price=19.99,stock=10"；
   命中的行照原样返回，未命中返回 NotFound。 */
ax_res_row ax_db_query_result(ax_str sql, int64_t arg) {
    ax_res_row r;
    memset(&r, 0, sizeof r);
    if (!getenv("AX_DB_FIXTURE")) {
        r.tag = 0;
        r.u.err = ax_err_new(1, AX_S("no db fixture (set AX_DB_FIXTURE)"));
        return r;
    }
    char buf[32];
    snprintf(buf, sizeof buf, "%" PRId64, (int64_t)arg);
    ax_str a = ax_str_from_cstr(buf);
    /* 未命中返回空行（tag=1,n=0）：调用方的 row.empty() 才是"查无此记录"的出口，
       与 §16.10 RUN-3 一致。 */
    r.tag = 1;
    r.u.ok = ax_db_query(sql, a);
    return r;
}


int ax_row_empty(ax_row r) { return r.n == 0; }


ax_str ax_row_get(ax_row r, ax_str key) {
    for (int64_t i = 0; i < r.n; i++) {
        if (ax_str_eq(r.kv[i*2], key)) return r.kv[i*2+1];
    }
    return ax_str_empty();
}

/* ================================================================ json（文稿 11.3）
 * 手写递归下降解析器。值域映射：
 *   object → ax_map（string → ax_box*）      array → ax_arr（元素 ax_box*）
 *   string → tag 14（payload ax_str）        int   → tag 2
 *   float  → tag 6                           true/false → tag 1
 *   null   → NULL（ax_box* 为空指针）
 */
typedef struct {
    const char* p;
    int64_t n, i;
    ax_str  emsg;   /* 解析错误信息（fail 时用） */
    int     epos;   /* 出错处字节下标 */
} jparse;


static void jskip_ws(jparse* j) {
    while (j->i < j->n) {
        char c = j->p[j->i];
        if (c == ' ' || c == '\t' || c == '\n' || c == '\r') j->i++;
        else break;
    }
}


static int jpeek(jparse* j) { return j->i < j->n ? (unsigned char)j->p[j->i] : -1; }


static int jfail(jparse* j, const char* what) {
    if (j->emsg.len == 0) {
        j->emsg = ax_str_from_cstr(what);
        j->epos = (int)j->i;
    }
    return 0;
}

/* 解码 \uXXXX（含 UTF-16 代理对），产出 UTF-8 到动态缓冲 */
typedef struct { uint8_t* p; int64_t n, cap; } jbuf;
static void jb_push(jbuf* b, uint8_t c) {
    if (b->n + 1 > b->cap) {
        b->cap = b->cap ? b->cap * 2 : 64;
        uint8_t* np = (uint8_t*)ax_alloc((int64_t)b->cap);
        memcpy(np, b->p, (size_t)b->n);
        b->p = np;
    }
    b->p[b->n++] = c;
}
static void jb_utf8(jbuf* b, uint32_t cp) {
    if (cp < 0x80) jb_push(b, (uint8_t)cp);
    else if (cp < 0x800) {
        jb_push(b, (uint8_t)(0xC0 | (cp >> 6)));
        jb_push(b, (uint8_t)(0x80 | (cp & 0x3F)));
    } else if (cp < 0x10000) {
        jb_push(b, (uint8_t)(0xE0 | (cp >> 12)));
        jb_push(b, (uint8_t)(0x80 | ((cp >> 6) & 0x3F)));
        jb_push(b, (uint8_t)(0x80 | (cp & 0x3F)));
    } else {
        jb_push(b, (uint8_t)(0xF0 | (cp >> 18)));
        jb_push(b, (uint8_t)(0x80 | ((cp >> 12) & 0x3F)));
        jb_push(b, (uint8_t)(0x80 | ((cp >> 6) & 0x3F)));
        jb_push(b, (uint8_t)(0x80 | (cp & 0x3F)));
    }
}


static ax_str jstring(jparse* j) {
    /* 进入时 j->i 指向开引号 */
    j->i++; /* 跳过 " */
    jbuf b = {0};
    while (j->i < j->n) {
        unsigned char c = (unsigned char)j->p[j->i];
        if (c == '"') { j->i++; return ax_str_from_bytes(b.p, b.n); }
        if (c == '\\') {
            j->i++;
            if (j->i >= j->n) { jfail(j, "unterminated escape"); break; }
            char e = j->p[j->i++];
            switch (e) {
            case '"':  jb_push(&b, '"');  break;
            case '\\': jb_push(&b, '\\'); break;
            case '/':  jb_push(&b, '/');  break;
            case 'b':  jb_push(&b, '\b'); break;
            case 'f':  jb_push(&b, '\f'); break;
            case 'n':  jb_push(&b, '\n'); break;
            case 'r':  jb_push(&b, '\r'); break;
            case 't':  jb_push(&b, '\t'); break;
            case 'u': {
                uint32_t cp = 0;
                for (int k = 0; k < 4; k++) {
                    if (j->i >= j->n) { jfail(j, "bad \\u escape"); return ax_str_empty(); }
                    char h = j->p[j->i++];
                    cp <<= 4;
                    if (h >= '0' && h <= '9') cp |= (uint32_t)(h - '0');
                    else if (h >= 'a' && h <= 'f') cp |= (uint32_t)(h - 'a' + 10);
                    else if (h >= 'A' && h <= 'F') cp |= (uint32_t)(h - 'A' + 10);
                    else { jfail(j, "bad \\u escape"); return ax_str_empty(); }
                }
                if (cp >= 0xD800 && cp <= 0xDBFF && j->i + 1 < j->n
                    && j->p[j->i] == '\\' && j->p[j->i+1] == 'u') {
                    /* 高代理：尝试配对低代理 */
                    uint32_t lo = 0; int64_t save = j->i;
                    j->i += 2;
                    for (int k = 0; k < 4; k++) {
                        if (j->i >= j->n) { j->i = save; break; }
                        char h = j->p[j->i++];
                        lo <<= 4;
                        if (h >= '0' && h <= '9') lo |= (uint32_t)(h - '0');
                        else if (h >= 'a' && h <= 'f') lo |= (uint32_t)(h - 'a' + 10);
                        else if (h >= 'A' && h <= 'F') lo |= (uint32_t)(h - 'A' + 10);
                        else { j->i = save; break; }
                    }
                    if (lo >= 0xDC00 && lo <= 0xDFFF)
                        cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                }
                jb_utf8(&b, cp);
                break;
            }
            default: jfail(j, "unknown escape"); return ax_str_empty();
            }
            continue;
        }
        jb_push(&b, c);
        j->i++;
    }
    jfail(j, "unterminated string");
    return ax_str_empty();
}

static ax_box* jvalue(jparse* j);

static ax_box* jobject(jparse* j) {
    j->i++; /* { */
    ax_map m;
    ax_map_init(&m, 1, (int64_t)sizeof(ax_str), (int64_t)sizeof(ax_box*));
    jskip_ws(j);
    if (jpeek(j) == '}') { j->i++; return ax_box_new(0, &m, (int64_t)sizeof(ax_map)); }
    for (;;) {
        jskip_ws(j);
        if (jpeek(j) != '"') { jfail(j, "expected object key"); return NULL; }
        ax_str key = jstring(j);
        jskip_ws(j);
        if (jpeek(j) != ':') { jfail(j, "expected ':'"); return NULL; }
        j->i++;
        jskip_ws(j);
        ax_box* val = jvalue(j);
        if (val == NULL && j->emsg.len != 0) return NULL;
        /* JSON null 在 map<string, object> 里没有合法载体（object 不能是 null）：
           该键不入表 —— Get 返回 has=0，`== null` / `??` 都能安全生效。
           数组里保留 NULL 占位（位置必须保留）。 */
        if (val != NULL) ax_map_put(&m, &key, &val);
        jskip_ws(j);
        if (jpeek(j) == ',') { j->i++; continue; }
        if (jpeek(j) == '}') { j->i++; return ax_box_new(0, &m, (int64_t)sizeof(ax_map)); }
        jfail(j, "expected ',' or '}'");
        return NULL;
    }
}

static ax_box* jarray(jparse* j) {
    j->i++; /* [ */
    ax_arr a;
    ax_arr_init(&a);
    jskip_ws(j);
    if (jpeek(j) == ']') { j->i++; return ax_box_new(0, &a, (int64_t)sizeof(ax_arr)); }
    for (;;) {
        jskip_ws(j);
        ax_box* val = jvalue(j);
        if (val == NULL && j->emsg.len != 0) return NULL;
        ax_arr_push(&a, &val, (int64_t)sizeof(ax_box*));
        jskip_ws(j);
        if (jpeek(j) == ',') { j->i++; continue; }
        if (jpeek(j) == ']') { j->i++; return ax_box_new(0, &a, (int64_t)sizeof(ax_arr)); }
        jfail(j, "expected ',' or ']'");
        return NULL;
    }
}

static ax_box* jvalue(jparse* j) {
    jskip_ws(j);
    int c = jpeek(j);
    if (c == '{') return jobject(j);
    if (c == '[') return jarray(j);
    if (c == '"') {
        ax_str s = jstring(j);
        if (j->emsg.len != 0) return NULL;
        return ax_box_new(AX_TAG_STRING, &s, (int64_t)sizeof(ax_str));
    }
    if (c == 't') {
        if (j->i + 4 <= j->n && memcmp(j->p + j->i, "true", 4) == 0) {
            j->i += 4;
            int one = 1;
            return ax_box_new(AX_TAG_BOOL, &one, (int64_t)sizeof(int));
        }
        jfail(j, "bad literal"); return NULL;
    }
    if (c == 'f') {
        if (j->i + 5 <= j->n && memcmp(j->p + j->i, "false", 5) == 0) {
            j->i += 5;
            int zero = 0;
            return ax_box_new(AX_TAG_BOOL, &zero, (int64_t)sizeof(int));
        }
        jfail(j, "bad literal"); return NULL;
    }
    if (c == 'n') {
        if (j->i + 4 <= j->n && memcmp(j->p + j->i, "null", 4) == 0) {
            j->i += 4;
            return NULL;
        }
        jfail(j, "bad literal"); return NULL;
    }
    if (c == '-' || (c >= '0' && c <= '9')) {
        int64_t start = j->i;
        if (c == '-') j->i++;
        int isf = 0;
        while (j->i < j->n) {
            char d = j->p[j->i];
            if (d >= '0' && d <= '9') { j->i++; continue; }
            if (d == '.' || d == 'e' || d == 'E' || d == '+' || d == '-') {
                isf = (d == '.' || d == 'e' || d == 'E');
                j->i++;
                continue;
            }
            break;
        }
        char nb[64];
        int64_t len = j->i - start;
        if (len <= 0 || len >= 64) { jfail(j, "bad number"); return NULL; }
        memcpy(nb, j->p + start, (size_t)len);
        nb[len] = 0;
        if (isf) {
            double d = strtod(nb, NULL);
            return ax_box_new(AX_TAG_FLOAT, &d, (int64_t)sizeof(double));
        }
        int64_t v = strtoll(nb, NULL, 10);
        return ax_box_new(AX_TAG_INT, &v, (int64_t)sizeof(int64_t));
    }
    jfail(j, "unexpected character");
    return NULL;
}


ax_res_map ax_json_parse_result(ax_str s) {
    ax_res_map r;
    memset(&r, 0, sizeof r);
    jparse j;
    j.p = (const char*)s.p;
    j.n = s.len;
    j.i = 0;
    j.emsg = ax_str_empty();
    j.epos = 0;
    ax_box* v = jvalue(&j);
    if (j.emsg.len != 0) {
        r.tag = 0;
        r.u.err = ax_err_new(1, j.emsg);
        return r;
    }
    jskip_ws(&j);
    if (j.i != j.n) {
        r.tag = 0;
        r.u.err = ax_err_new(1, ax_str_from_cstr("trailing characters after value"));
        return r;
    }
    /* 解出来的顶层必须是 object（文稿 11.3：Parse 返回 map<string, object>） */
    if (v == NULL || v->size != (int64_t)sizeof(ax_map)) {
        r.tag = 0;
        r.u.err = ax_err_new(1, ax_str_from_cstr("top-level value must be an object"));
        return r;
    }
    r.tag = 1;
    memcpy(&r.u.ok, v->payload, sizeof(ax_map));
    return r;
}

/* stringify：标量 + map（size==sizeof(ax_map)）+ 数组（size==sizeof(ax_arr)）。
   遇到无法识别的复合值输出 null（不崩溃——stringify 是尽力而为的调试出口）。 */
static void jstr_escape(const ax_str s, ax_sb* out) {
    /* 追加转义后的带引号字符串 */
    static const char hex[] = "0123456789abcdef";
    ax_sb_push_cstr(out, "\"");
    for (int64_t i = 0; i < s.len; i++) {
        unsigned char c = s.p[i];
        switch (c) {
        case '"':  ax_sb_push_cstr(out, "\\\""); break;
        case '\\': ax_sb_push_cstr(out, "\\\\"); break;
        case '\n': ax_sb_push_cstr(out, "\\n");  break;
        case '\r': ax_sb_push_cstr(out, "\\r");  break;
        case '\t': ax_sb_push_cstr(out, "\\t");  break;
        default:
            if (c < 0x20) {
                char b[8];
                b[0] = '\\'; b[1] = 'u'; b[2] = '0'; b[3] = '0';
                b[4] = hex[(c >> 4) & 0xF]; b[5] = hex[c & 0xF]; b[6] = 0;
                ax_sb_push_cstr(out, b);
            } else {
                ax_sb_push(out, ax_str_from_bytes(s.p + i, 1));
            }
        }
    }
    ax_sb_push_cstr(out, "\"");
}


static void jstringify_box(const ax_box* b, ax_sb* out) {
    if (b == NULL) { ax_sb_push_cstr(out, "null"); return; }
    if (b->tag == AX_TAG_BOOL) {
        int v = *(const int*)b->payload;
        ax_sb_push_cstr(out, v ? "true" : "false");
        return;
    }
    if (b->tag == AX_TAG_INT) {
        ax_sb_push_int(out, *(const int64_t*)b->payload);
        return;
    }
    if (b->tag == AX_TAG_FLOAT) {
        ax_sb_push_f64(out, *(const double*)b->payload);
        return;
    }
    if (b->tag == AX_TAG_STRING) {
        jstr_escape(*(const ax_str*)b->payload, out);
        return;
    }
    if (b->size == (int64_t)sizeof(ax_map)) {
        const ax_map* m = (const ax_map*)b->payload;
        ax_sb_push_cstr(out, "{");
        for (int64_t i = 0; i < m->len; i++) {
            if (i > 0) ax_sb_push_cstr(out, ",");
            jstr_escape(*(const ax_str*)m->entries[i].k, out);
            ax_sb_push_cstr(out, ":");
            jstringify_box(*(ax_box* const*)m->entries[i].v, out);
        }
        ax_sb_push_cstr(out, "}");
        return;
    }
    if (b->size == (int64_t)sizeof(ax_arr)) {
        const ax_arr* a = (const ax_arr*)b->payload;
        ax_sb_push_cstr(out, "[");
        for (int64_t i = 0; i < a->len; i++) {
            if (i > 0) ax_sb_push_cstr(out, ",");
            jstringify_box(*(ax_box* const*)((const uint8_t*)a->data
                + (size_t)i * sizeof(ax_box*)), out);
        }
        ax_sb_push_cstr(out, "]");
        return;
    }
    ax_sb_push_cstr(out, "null");
}


ax_str ax_json_stringify(ax_box* v) {
    ax_sb sb;
    ax_sb_init(&sb);
    jstringify_box(v, &sb);
    return ax_sb_finish(&sb);
}

/* ---------- json.MarshalIndent：带缩进序列化 ---------- */
/* 写出当前层级的前缀+缩进 */

/* ---------- json.MarshalIndent：带缩进序列化 ---------- */
/* 写出当前层级的前缀+缩进 */
static void jpush_indent(ax_sb* out, ax_str prefix, ax_str indent,
                         int64_t level) {
    ax_sb_push(out, ax_str_from_bytes(prefix.p, prefix.len));
    for (int64_t i = 0; i < level; i++) ax_sb_push(out, ax_str_from_bytes(indent.p, indent.len));
}


static void jstringify_indent_box(const ax_box* b, ax_sb* out,
                                  ax_str prefix, ax_str indent,
                                  int64_t level) {
    if (b == NULL) { jpush_indent(out, prefix, indent, level); ax_sb_push_cstr(out, "null"); return; }
    if (b->tag == AX_TAG_BOOL) {
        int v = *(const int*)b->payload;
        jpush_indent(out, prefix, indent, level);
        ax_sb_push_cstr(out, v ? "true" : "false");
        return;
    }
    if (b->tag == AX_TAG_INT) {
        jpush_indent(out, prefix, indent, level);
        ax_sb_push_int(out, *(const int64_t*)b->payload);
        return;
    }
    if (b->tag == AX_TAG_FLOAT) {
        jpush_indent(out, prefix, indent, level);
        ax_sb_push_f64(out, *(const double*)b->payload);
        return;
    }
    if (b->tag == AX_TAG_STRING) {
        jpush_indent(out, prefix, indent, level);
        jstr_escape(*(const ax_str*)b->payload, out);
        return;
    }
    if (b->size == (int64_t)sizeof(ax_map)) {
        const ax_map* m = (const ax_map*)b->payload;
        jpush_indent(out, prefix, indent, level);
        ax_sb_push_cstr(out, "{");
        if (m->len > 0) {
            ax_sb_push_cstr(out, "\n");
            for (int64_t i = 0; i < m->len; i++) {
                jpush_indent(out, prefix, indent, level + 1);
                jstr_escape(*(const ax_str*)m->entries[i].k, out);
                ax_sb_push_cstr(out, ": ");
                jstringify_indent_box(*(ax_box* const*)m->entries[i].v, out, prefix, indent, level + 1);
                if (i < m->len - 1) ax_sb_push_cstr(out, ",");
                ax_sb_push_cstr(out, "\n");
            }
            jpush_indent(out, prefix, indent, level);
        }
        ax_sb_push_cstr(out, "}");
        return;
    }
    if (b->size == (int64_t)sizeof(ax_arr)) {
        const ax_arr* a = (const ax_arr*)b->payload;
        jpush_indent(out, prefix, indent, level);
        ax_sb_push_cstr(out, "[");
        if (a->len > 0) {
            ax_sb_push_cstr(out, "\n");
            for (int64_t i = 0; i < a->len; i++) {
                jpush_indent(out, prefix, indent, level + 1);
                jstringify_indent_box(*(ax_box* const*)((const uint8_t*)a->data
                    + (size_t)i * sizeof(ax_box*)), out, prefix, indent, level + 1);
                if (i < a->len - 1) ax_sb_push_cstr(out, ",");
                ax_sb_push_cstr(out, "\n");
            }
            jpush_indent(out, prefix, indent, level);
        }
        ax_sb_push_cstr(out, "]");
        return;
    }
    jpush_indent(out, prefix, indent, level);
    ax_sb_push_cstr(out, "null");
}


ax_str ax_json_marshal_indent(ax_box* v, ax_str prefix, ax_str indent) {
    ax_sb sb;
    ax_sb_init(&sb);
    jstringify_indent_box(v, &sb, prefix, indent, 0);
    return ax_sb_finish(&sb);
}

/* ---------- json.Valid：判断 JSON 是否合法 ---------- */

/* ---------- json.Valid：判断 JSON 是否合法 ---------- */
static int jjson_valid(const char* p, int64_t n) {
    jparse j;
    j.p = p;
    j.n = n;
    j.i = 0;
    j.emsg = ax_str_empty();
    j.epos = 0;
    (void)jvalue(&j);
    if (j.emsg.len != 0) return 0;
    jskip_ws(&j);
    if (j.i != j.n) return 0;
    return 1;
}


int ax_json_valid(ax_str s) {
    return jjson_valid((const char*)s.p, s.len);
}

int64_t ax_now_ms(void) {
#ifdef _WIN32
    FILETIME ft;
    GetSystemTimeAsFileTime(&ft);
    /* 1601-01-01 → 1970-01-01：11644473600 秒 */
    int64_t t = ((int64_t)ft.dwHighDateTime << 32) | (int64_t)ft.dwLowDateTime;
    return t / 10000 - 11644473600000LL;
#else
    struct timeval tv;
    gettimeofday(&tv, NULL);
    return (int64_t)tv.tv_sec * 1000 + tv.tv_usec / 1000;
#endif
}

