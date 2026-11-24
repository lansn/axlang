/* axrt_str.c -- axlang 运行时模块 */
#include "axrt.h"
#include "axrt_internal.h"


/* ================================================================ UTF-8 */
static int u8_decode(const uint8_t* p, int64_t len, int32_t* cp) {
    if (len <= 0) return 0;
    uint8_t c = p[0];
    if (c < 0x80) { *cp = c; return 1; }
    if ((c & 0xE0) == 0xC0 && len >= 2) {
        *cp = ((c & 0x1F) << 6) | (p[1] & 0x3F);
        return 2;
    }
    if ((c & 0xF0) == 0xE0 && len >= 3) {
        *cp = ((c & 0x0F) << 12) | ((p[1] & 0x3F) << 6) | (p[2] & 0x3F);
        return 3;
    }
    if ((c & 0xF8) == 0xF0 && len >= 4) {
        *cp = ((c & 0x07) << 18) | ((p[1] & 0x3F) << 12) |
              ((p[2] & 0x3F) << 6) | (p[3] & 0x3F);
        return 4;
    }
    *cp = 0xFFFD;
    return 1;
}


static int u8_encode(int32_t cp, uint8_t out[4]) {
    if (cp < 0 || cp > 0x10FFFF || (cp >= 0xD800 && cp <= 0xDFFF)) return 0;
    if (cp < 0x80) { out[0] = (uint8_t)cp; return 1; }
    if (cp < 0x800) {
        out[0] = (uint8_t)(0xC0 | (cp >> 6));
        out[1] = (uint8_t)(0x80 | (cp & 0x3F));
        return 2;
    }
    if (cp < 0x10000) {
        out[0] = (uint8_t)(0xE0 | (cp >> 12));
        out[1] = (uint8_t)(0x80 | ((cp >> 6) & 0x3F));
        out[2] = (uint8_t)(0x80 | (cp & 0x3F));
        return 3;
    }
    out[0] = (uint8_t)(0xF0 | (cp >> 18));
    out[1] = (uint8_t)(0x80 | ((cp >> 12) & 0x3F));
    out[2] = (uint8_t)(0x80 | ((cp >> 6) & 0x3F));
    out[3] = (uint8_t)(0x80 | (cp & 0x3F));
    return 4;
}

/* ================================================================ 字符串 */

/* ================================================================ 字符串 */
ax_str ax_str_from_bytes(const uint8_t* p, int64_t n) {
    if (n <= 0) return ax_str_empty();
    uint8_t* q = (uint8_t*)ax_alloc(n + 1);
    memcpy(q, p, (size_t)n);
    q[n] = 0;
    ax_str s; s.p = q; s.len = n;
    return s;
}


ax_str ax_str_from_cstr(const char* s) {
    if (!s) return ax_str_empty();
    return ax_str_from_bytes((const uint8_t*)s, (int64_t)strlen(s));
}

/* 让控制台按 UTF-8 解释收发字节。
   Windows 控制台默认用系统 ANSI 代码页（中文系统是 936/GBK），而 axlang 的字符串
   内部统一是 UTF-8 字节，直接 fwrite 出去会被控制台按 GBK 解释成乱码。
   这里在程序启动时把控制台的输出/输入代码页切到 UTF-8，并登记 atexit 恢复原值
   （ax_trap 走 exit(2)，同样会触发恢复）。
   只在真正连着控制台时切换：重定向到文件或管道时字节原样写出，不做任何干预，
   保证 `axc run x.ax > out.txt` 与测试里的逐字节比对不受影响。 */

ax_str ax_str_empty(void) {
    static const uint8_t e = 0;
    ax_str s; s.p = &e; s.len = 0;
    return s;
}

int64_t ax_str_len_cp(ax_str s) {
    int64_t i = 0, n = 0;
    while (i < s.len) {
        int32_t cp;
        int k = u8_decode(s.p + i, s.len - i, &cp);
        if (k <= 0) break;
        i += k;
        n++;
    }
    return n;
}

int64_t ax_str_byte_len(ax_str s) { return s.len; }


ax_str ax_str_concat(ax_str a, ax_str b) {
    if (a.len == 0) return b;
    if (b.len == 0) return a;
    uint8_t* q = (uint8_t*)ax_alloc(a.len + b.len + 1);
    memcpy(q, a.p, (size_t)a.len);
    memcpy(q + a.len, b.p, (size_t)b.len);
    q[a.len + b.len] = 0;
    ax_str s; s.p = q; s.len = a.len + b.len;
    return s;
}


int ax_str_eq(ax_str a, ax_str b) {
    if (a.len != b.len) return 0;
    if (a.len == 0) return 1;
    return memcmp(a.p, b.p, (size_t)a.len) == 0;
}


int ax_str_cmp(ax_str a, ax_str b) {
    int64_t n = a.len < b.len ? a.len : b.len;
    int r = n ? memcmp(a.p, b.p, (size_t)n) : 0;
    if (r != 0) return r < 0 ? -1 : 1;
    if (a.len == b.len) return 0;
    return a.len < b.len ? -1 : 1;
}


int ax_str_at(ax_str s, int64_t i, ax_str* out) {
    if (i < 0) return 1;
    int64_t pos = 0;
    while (pos < s.len) {
        int32_t cp;
        int k = u8_decode(s.p + pos, s.len - pos, &cp);
        if (k <= 0) return 1;
        if (i == 0) { *out = ax_str_from_bytes(s.p + pos, k); return 0; }
        pos += k;
        i--;
    }
    return 1;
}


int ax_str_codepoint_at(ax_str s, int64_t i, int32_t* out) {
    if (i < 0) return 1;
    int64_t pos = 0;
    while (pos < s.len) {
        int32_t cp;
        int k = u8_decode(s.p + pos, s.len - pos, &cp);
        if (k <= 0) return 1;
        if (i == 0) { *out = cp; return 0; }
        pos += k;
        i--;
    }
    return 1;
}


int ax_str_from_codepoint(int32_t cp, ax_str* out) {
    uint8_t buf[4];
    int k = u8_encode(cp, buf);
    if (k == 0) return 1;
    *out = ax_str_from_bytes(buf, k);
    return 0;
}


int ax_str_contains(ax_str s, ax_str sub) {
    if (sub.len == 0) return 1;
    if (sub.len > s.len) return 0;
    for (int64_t i = 0; i + sub.len <= s.len; i++) {
        if (memcmp(s.p + i, sub.p, (size_t)sub.len) == 0) return 1;
    }
    return 0;
}

/* string.ContainsRune：判断 s 是否包含码点 cp，对标 Go strings.ContainsRune，
 * 按 UTF-8 码点比较，找到返回 1，否则返回 0。标准 C11 实现。 */

/* string.ContainsRune：判断 s 是否包含码点 cp，对标 Go strings.ContainsRune，
 * 按 UTF-8 码点比较，找到返回 1，否则返回 0。标准 C11 实现。 */
int ax_str_contains_rune(ax_str s, int32_t cp) {
    int64_t i = 0;
    while (i < s.len) {
        int32_t ccp;
        int k = u8_decode(s.p + i, s.len - i, &ccp);
        if (k <= 0) k = 1;
        if (ccp == cp) return 1;
        i += k;
    }
    return 0;
}

/* 判断码点 cp 是否在字符集 chars 中（按 UTF-8 码点比较） */

/* 判断码点 cp 是否在字符集 chars 中（按 UTF-8 码点比较） */
static int ax_str_char_in_set(int32_t cp, ax_str chars) {
    int64_t i = 0;
    while (i < chars.len) {
        int32_t ccp;
        int k = u8_decode(chars.p + i, chars.len - i, &ccp);
        if (k <= 0) k = 1;
        if (ccp == cp) return 1;
        i += k;
    }
    return 0;
}

/* string.ContainsAny：s 是否包含 chars 中任意字符，1=是 0=否 */

/* string.ContainsAny：s 是否包含 chars 中任意字符，1=是 0=否 */
int ax_str_contains_any(ax_str s, ax_str chars) {
    if (chars.len == 0) return 0;
    int64_t i = 0;
    while (i < s.len) {
        int32_t cp;
        int k = u8_decode(s.p + i, s.len - i, &cp);
        if (k <= 0) k = 1;
        if (ax_str_char_in_set(cp, chars)) return 1;
        i += k;
    }
    return 0;
}

/* string.IndexAny：首次出现 chars 中任意字符的码点索引，未找到返回 -1 */
int64_t ax_str_index_any(ax_str s, ax_str chars) {
    if (chars.len == 0) return -1;
    int64_t i = 0;
    int64_t cp_idx = 0;
    while (i < s.len) {
        int32_t cp;
        int k = u8_decode(s.p + i, s.len - i, &cp);
        if (k <= 0) k = 1;
        if (ax_str_char_in_set(cp, chars)) return cp_idx;
        i += k;
        cp_idx++;
    }
    return -1;
}

/* string.LastIndexAny：最后出现 chars 中任意字符的码点索引，未找到返回 -1 */
int64_t ax_str_last_index_any(ax_str s, ax_str chars) {
    if (chars.len == 0) return -1;
    int64_t i = 0;
    int64_t cp_idx = 0;
    int64_t last = -1;
    while (i < s.len) {
        int32_t cp;
        int k = u8_decode(s.p + i, s.len - i, &cp);
        if (k <= 0) k = 1;
        if (ax_str_char_in_set(cp, chars)) last = cp_idx;
        i += k;
        cp_idx++;
    }
    return last;
}

/* string.Count：非重叠子串出现次数 */
int64_t ax_str_count(ax_str s, ax_str substr) {
    if (substr.len == 0) return ax_str_len_cp(s) + 1;
    if (substr.len > s.len) return 0;
    int64_t count = 0;
    int64_t i = 0;
    while (i + substr.len <= s.len) {
        if (memcmp(s.p + i, substr.p, (size_t)substr.len) == 0) {
            count++;
            i += substr.len;  /* 非重叠：跳过已匹配的子串 */
        } else {
            i++;
        }
    }
    return count;
}

/* ---------------- 子串查找（按 UTF-8 码点索引，未找到返回 -1） ---------------- */

/* 返回 sub 在 s 中首次出现的码点索引；空子串匹配索引 0。 */
int64_t ax_str_index_of(ax_str s, ax_str sub) {
    if (sub.len == 0) return 0;
    if (sub.len > s.len) return -1;
    int64_t cp_idx = 0;
    int64_t i = 0;
    while (i + sub.len <= s.len) {
        if (memcmp(s.p + i, sub.p, (size_t)sub.len) == 0) return cp_idx;
        int32_t cp;
        int k = u8_decode(s.p + i, s.len - i, &cp);
        if (k <= 0) k = 1;
        i += k;
        cp_idx++;
    }
    return -1;
}

/* 返回 sub 在 s 中最后出现的码点索引；空子串匹配尾部索引。 */
int64_t ax_str_last_index_of(ax_str s, ax_str sub) {
    if (sub.len == 0) return ax_str_len_cp(s);
    if (sub.len > s.len) return -1;
    int64_t cp_idx = 0;
    int64_t i = 0;
    int64_t last = -1;
    while (i + sub.len <= s.len) {
        if (memcmp(s.p + i, sub.p, (size_t)sub.len) == 0) last = cp_idx;
        int32_t cp;
        int k = u8_decode(s.p + i, s.len - i, &cp);
        if (k <= 0) k = 1;
        i += k;
        cp_idx++;
    }
    return last;
}

ax_str* ax_str_split(ax_str s, ax_str sep, int64_t* n) {
    int64_t cap = 8, cnt = 0;
    ax_str* out = (ax_str*)ax_alloc((int64_t)sizeof(ax_str) * cap);
    if (sep.len == 0) {
        out[0] = s; *n = 1; return out;
    }
    int64_t start = 0;
    int64_t i = 0;
    while (i + sep.len <= s.len) {
        if (memcmp(s.p + i, sep.p, (size_t)sep.len) == 0) {
            if (cnt + 1 >= cap) {
                cap *= 2;
                ax_str* nout = (ax_str*)ax_alloc((int64_t)sizeof(ax_str) * cap);
                memcpy(nout, out, (size_t)cnt * sizeof(ax_str));
                out = nout;
            }
            out[cnt++] = ax_str_from_bytes(s.p + start, i - start);
            i += sep.len;
            start = i;
        } else {
            i++;
        }
    }
    if (cnt + 1 > cap) {
        cap = cnt + 1;
        ax_str* nout = (ax_str*)ax_alloc((int64_t)sizeof(ax_str) * cap);
        memcpy(nout, out, (size_t)cnt * sizeof(ax_str));
        out = nout;
    }
    out[cnt++] = ax_str_from_bytes(s.p + start, s.len - start);
    *n = cnt;
    return out;
}

/* string.SplitN：按 sep 分割，最多分 n 份，达到 n 后停止，剩余部分作为最后一份 */
ax_str* ax_str_split_n(ax_str s, ax_str sep, int64_t n, int64_t* out_n) {
    int64_t cap = 8, cnt = 0;
    ax_str* out = (ax_str*)ax_alloc((int64_t)sizeof(ax_str) * cap);
    if (n <= 0) { *out_n = 0; return out; }
    if (sep.len == 0) {
        out[0] = s; *out_n = 1; return out;
    }
    int64_t start = 0;
    int64_t i = 0;
    while (cnt + 1 < n && i + sep.len <= s.len) {
        if (memcmp(s.p + i, sep.p, (size_t)sep.len) == 0) {
            if (cnt + 1 >= cap) {
                cap *= 2;
                ax_str* nout = (ax_str*)ax_alloc((int64_t)sizeof(ax_str) * cap);
                memcpy(nout, out, (size_t)cnt * sizeof(ax_str));
                out = nout;
            }
            out[cnt++] = ax_str_from_bytes(s.p + start, i - start);
            i += sep.len;
            start = i;
        } else {
            i++;
        }
    }
    out[cnt++] = ax_str_from_bytes(s.p + start, s.len - start);
    *out_n = cnt;
    return out;
}

/* string.SplitAfter：按 sep 分割，并将 sep 保留在每一部分的末尾 */
ax_str* ax_str_split_after(ax_str s, ax_str sep, int64_t* n) {
    int64_t cap = 8, cnt = 0;
    ax_str* out = (ax_str*)ax_alloc((int64_t)sizeof(ax_str) * cap);
    if (sep.len == 0) {
        out[0] = s; *n = 1; return out;
    }
    int64_t start = 0;
    int64_t i = 0;
    while (i + sep.len <= s.len) {
        if (memcmp(s.p + i, sep.p, (size_t)sep.len) == 0) {
            if (cnt + 1 >= cap) {
                cap *= 2;
                ax_str* nout = (ax_str*)ax_alloc((int64_t)sizeof(ax_str) * cap);
                memcpy(nout, out, (size_t)cnt * sizeof(ax_str));
                out = nout;
            }
            out[cnt++] = ax_str_from_bytes(s.p + start, i + sep.len - start);
            i += sep.len;
            start = i;
        } else {
            i++;
        }
    }
    if (start < s.len) {
        if (cnt + 1 > cap) {
            cap = cnt + 1;
            ax_str* nout = (ax_str*)ax_alloc((int64_t)sizeof(ax_str) * cap);
            memcpy(nout, out, (size_t)cnt * sizeof(ax_str));
            out = nout;
        }
        out[cnt++] = ax_str_from_bytes(s.p + start, s.len - start);
    }
    *n = cnt;
    return out;
}


ax_str ax_str_trim(ax_str s) {
    int64_t a = 0, b = s.len;
    while (a < b && (s.p[a] == ' ' || s.p[a] == '\t' || s.p[a] == '\n' || s.p[a] == '\r')) a++;
    while (b > a && (s.p[b-1] == ' ' || s.p[b-1] == '\t' || s.p[b-1] == '\n' || s.p[b-1] == '\r')) b--;
    return ax_str_from_bytes(s.p + a, b - a);
}

/* string.TrimLeft：去除左侧 cutset 中任意字符，按 UTF-8 码点返回剩余部分 */

/* string.TrimLeft：去除左侧 cutset 中任意字符，按 UTF-8 码点返回剩余部分 */
ax_str ax_str_trim_left(ax_str s, ax_str cutset) {
    int64_t a = 0;
    while (a < s.len) {
        int32_t cp;
        int k = u8_decode(s.p + a, s.len - a, &cp);
        if (k <= 0) k = 1;
        if (!ax_str_char_in_set(cp, cutset)) break;
        a += k;
    }
    return ax_str_from_bytes(s.p + a, s.len - a);
}

/* string.TrimRight：去除右侧 cutset 中任意字符，按 UTF-8 码点返回剩余部分 */

/* string.TrimRight：去除右侧 cutset 中任意字符，按 UTF-8 码点返回剩余部分 */
ax_str ax_str_trim_right(ax_str s, ax_str cutset) {
    int64_t b = s.len;
    while (b > 0) {
        int64_t i = b - 1;
        while (i > 0 && (s.p[i] & 0xC0) == 0x80) i--;
        int32_t cp;
        int k = u8_decode(s.p + i, b - i, &cp);
        if (k <= 0) k = 1;
        if (!ax_str_char_in_set(cp, cutset)) break;
        b = i;
    }
    return ax_str_from_bytes(s.p, b);
}

/* string.TrimPrefix：去除前缀 prefix（按字节匹配），不匹配返回原串副本 */

/* string.TrimPrefix：去除前缀 prefix（按字节匹配），不匹配返回原串副本 */
ax_str ax_str_trim_prefix(ax_str s, ax_str prefix) {
    if (prefix.len == 0) return ax_str_from_bytes(s.p, s.len);
    if (prefix.len > s.len) return ax_str_from_bytes(s.p, s.len);
    if (memcmp(s.p, prefix.p, (size_t)prefix.len) != 0) return ax_str_from_bytes(s.p, s.len);
    return ax_str_from_bytes(s.p + prefix.len, s.len - prefix.len);
}

/* string.TrimSuffix：去除后缀 suffix（按字节匹配），不匹配返回原串副本 */

/* string.TrimSuffix：去除后缀 suffix（按字节匹配），不匹配返回原串副本 */
ax_str ax_str_trim_suffix(ax_str s, ax_str suffix) {
    if (suffix.len == 0) return ax_str_from_bytes(s.p, s.len);
    if (suffix.len > s.len) return ax_str_from_bytes(s.p, s.len);
    if (memcmp(s.p + s.len - suffix.len, suffix.p, (size_t)suffix.len) != 0)
        return ax_str_from_bytes(s.p, s.len);
    return ax_str_from_bytes(s.p, s.len - suffix.len);
}


static int ax_is_space(uint8_t c) {
    return c == (uint8_t)' ' || c == (uint8_t)'\t' || c == (uint8_t)'\n' ||
           c == (uint8_t)'\r' || c == (uint8_t)'\v' || c == (uint8_t)'\f';
}

/* 字符串分割（对标 Go strings.Fields，按空白符分割，返回 ax_arr，元素为 ax_str） */

/* 字符串分割（对标 Go strings.Fields，按空白符分割，返回 ax_arr，元素为 ax_str） */
ax_arr ax_str_fields(ax_str s) {
    ax_arr arr;
    ax_arr_init(&arr);
    int64_t i = 0;
    while (i < s.len) {
        /* 跳过空白符 */
        while (i < s.len && ax_is_space(s.p[i])) i++;
        if (i >= s.len) break;
        int64_t start = i;
        while (i < s.len && !ax_is_space(s.p[i])) i++;
        ax_str field = ax_str_from_bytes(s.p + start, i - start);
        ax_arr_push(&arr, &field, (int64_t)sizeof(ax_str));
    }
    return arr;
}

/* ---------------- math 扩展（对标 Go math.Sign / math.Mod） ---------------- */

/* 符号函数：x>0→1，x<0→-1，否则（含 0 / NaN）→0 */

/* ---------------- math 扩展（对标 Go math.Sign / math.Mod） ---------------- */

/* 符号函数：x>0→1，x<0→-1，否则（含 0 / NaN）→0 */
int ax_math_sign(double x) {
    if (x > 0.0) return 1;
    if (x < 0.0) return -1;
    return 0;
}

/* 浮点余数（语义同 C fmod）：结果符号同被除数 x，幅值 < |y|；y 为 0 时返回 NaN */
double ax_math_mod(double x, double y) {
    return fmod(x, y);
}

/* math.IsNaN：C11 isnan 宏，x 为 NaN 返回 1，否则 0 */

/* math.IsNaN：C11 isnan 宏，x 为 NaN 返回 1，否则 0 */
int ax_math_isnan(double x) {
    return isnan(x) ? 1 : 0;
}

/* math.IsInf：C11 isinf 宏。sign>0 只判 +Inf，sign<0 只判 -Inf，sign==0 判任意无穷 */

/* math.IsInf：C11 isinf 宏。sign>0 只判 +Inf，sign<0 只判 -Inf，sign==0 判任意无穷 */
int ax_math_isinf(double x, int sign) {
    if (!isinf(x)) return 0;
    if (sign > 0) return x > 0.0 ? 1 : 0;
    if (sign < 0) return x < 0.0 ? 1 : 0;
    return 1;
}

/* math.Signbit：C11 signbit 宏，x 为负数（含 -0 / -NaN）返回 1，否则 0 */
int ax_math_signbit(float x) {
    return signbit(x) ? 1 : 0;
}

/* math.Copysign：C11 copysign，返回 x 的幅值、y 的符号 */
float ax_math_copysign(float x, float y) {
    return copysignf(x, y);
}

/* math.Dim：C11 fdim，返回 max(x - y, 0) */
float ax_math_dim(float x, float y) {
    return fdimf(x, y);
}

/* math.Ldexp：C11 ldexp，返回 x * 2^exp */
float ax_math_ldexp(float x, int exp) {
    return ldexpf(x, exp);
}

/* ---------------- 大小写转换（仅 ASCII，UTF-8 多字节原样保留） ---------------- */


/* ---------------- 大小写转换（仅 ASCII，UTF-8 多字节原样保留） ---------------- */

static int u8_is_ascii_lower(uint8_t c) { return c >= 'a' && c <= 'z'; }
static int u8_is_ascii_upper(uint8_t c) { return c >= 'A' && c <= 'Z'; }
static uint8_t u8_to_ascii_upper(uint8_t c) { return u8_is_ascii_lower(c) ? (uint8_t)(c - 'a' + 'A') : c; }
static uint8_t u8_to_ascii_lower(uint8_t c) { return u8_is_ascii_upper(c) ? (uint8_t)(c - 'A' + 'a') : c; }


ax_str ax_str_to_upper(ax_str s) {
    uint8_t* buf = (uint8_t*)ax_alloc(s.len <= 0 ? 1 : s.len);
    for (int64_t i = 0; i < s.len; i++) buf[i] = u8_to_ascii_upper(s.p[i]);
    return ax_str_from_bytes(buf, s.len);
}


ax_str ax_str_to_lower(ax_str s) {
    uint8_t* buf = (uint8_t*)ax_alloc(s.len <= 0 ? 1 : s.len);
    for (int64_t i = 0; i < s.len; i++) buf[i] = u8_to_ascii_lower(s.p[i]);
    return ax_str_from_bytes(buf, s.len);
}

/* string.ToUpperRune：对单个码点做大小写转换（对标 Go strings.ToUpperRune / C toupper，
 * 仅处理 ASCII 范围：a-z -> A-Z，非 ASCII 码点原样返回）。标准 C11 实现。 */
int64_t ax_str_to_upper_rune(int64_t c) {
    if (c >= (int64_t)'a' && c <= (int64_t)'z') return c - (int64_t)'a' + (int64_t)'A';
    return c;
}

/* string.ToLowerRune：对单个码点做小写转换（对标 Go strings.ToLowerRune / C tolower，
 * 仅处理 ASCII 范围：A-Z -> a-z，非 ASCII 码点原样返回）。标准 C11 实现。 */
int64_t ax_str_to_lower_rune(int64_t c) {
    if (c >= (int64_t)'A' && c <= (int64_t)'Z') return c - (int64_t)'A' + (int64_t)'a';
    return c;
}

/* ---------------- 字节数组（Rt-7，对标 Go bytes，元素为 uint8_t，C 层为 ax_arr） ---------------- */

/* bytes.Contains：判断 s 是否包含子片 sub（按字节比较）。空子片匹配任意 s。 */

/* ---------------- 字节数组（Rt-7，对标 Go bytes，元素为 uint8_t，C 层为 ax_arr） ---------------- */

/* bytes.Contains：判断 s 是否包含子片 sub（按字节比较）。空子片匹配任意 s。 */
int ax_bytes_contains(ax_arr s, ax_arr sub) {
    if (sub.len == 0) return 1;
    if (s.len < sub.len) return 0;
    const uint8_t* sp = (const uint8_t*)s.data;
    const uint8_t* subp = (const uint8_t*)sub.data;
    for (int64_t i = 0; i + sub.len <= s.len; i++) {
        if (memcmp(sp + i, subp, (size_t)sub.len) == 0) return 1;
    }
    return 0;
}

/* bytes.Equal：判断两个字节数组是否相等（长度相同且所有字节相同）。 */

/* bytes.Equal：判断两个字节数组是否相等（长度相同且所有字节相同）。 */
int ax_bytes_equal(ax_arr a, ax_arr b) {
    if (a.len != b.len) return 0;
    if (a.len == 0) return 1;
    return memcmp(a.data, b.data, (size_t)a.len) == 0;
}

/* bytes.Index：查找子片 sub 在 s 中首次出现的索引，未找到返回 -1。空子片匹配索引 0。 */
int64_t ax_bytes_index(ax_arr s, ax_arr sub) {
    if (sub.len == 0) return 0;
    if (s.len < sub.len) return -1;
    const uint8_t* sp = (const uint8_t*)s.data;
    const uint8_t* subp = (const uint8_t*)sub.data;
    for (int64_t i = 0; i + sub.len <= s.len; i++) {
        if (memcmp(sp + i, subp, (size_t)sub.len) == 0) return i;
    }
    return -1;
}

/* bytes.Split：按分隔符 sep 分割 s，返回 ax_arr（元素为 ax_arr，每个元素是一个子片）。 */

/* bytes.Split：按分隔符 sep 分割 s，返回 ax_arr（元素为 ax_arr，每个元素是一个子片）。 */
ax_arr ax_bytes_split(ax_arr s, ax_arr sep) {
    ax_arr out; ax_arr_init(&out);
    if (sep.len == 0) {
        ax_arr_push(&out, &s, (int64_t)sizeof(ax_arr));
        return out;
    }
    const uint8_t* sp = (const uint8_t*)s.data;
    const uint8_t* sepp = (const uint8_t*)sep.data;
    int64_t start = 0;
    int64_t i = 0;
    while (i + sep.len <= s.len) {
        if (memcmp(sp + i, sepp, (size_t)sep.len) == 0) {
            ax_arr part; ax_arr_init(&part);
            int64_t partlen = i - start;
            uint8_t* buf = (uint8_t*)ax_alloc(partlen <= 0 ? 1 : partlen);
            if (partlen > 0) memcpy(buf, sp + start, (size_t)partlen);
            part.data = buf; part.len = partlen; part.cap = partlen;
            ax_arr_push(&out, &part, (int64_t)sizeof(ax_arr));
            i += sep.len;
            start = i;
        } else {
            i++;
        }
    }
    ax_arr part; ax_arr_init(&part);
    int64_t lastlen = s.len - start;
    uint8_t* buf = (uint8_t*)ax_alloc(lastlen <= 0 ? 1 : lastlen);
    if (lastlen > 0) memcpy(buf, sp + start, (size_t)lastlen);
    part.data = buf; part.len = lastlen; part.cap = lastlen;
    ax_arr_push(&out, &part, (int64_t)sizeof(ax_arr));
    return out;
}

/* bytes.TrimSpace：去除 s 首尾的空白字节，返回新数组。 */

/* bytes.TrimSpace：去除 s 首尾的空白字节，返回新数组。 */
ax_arr ax_bytes_trim_space(ax_arr s) {
    const uint8_t* sp = (const uint8_t*)s.data;
    int64_t start = 0, end = s.len;
    while (start < end &&
           (sp[start] == ' ' || sp[start] == '\t' || sp[start] == '\n' ||
            sp[start] == '\r' || sp[start] == '\v' || sp[start] == '\f')) start++;
    while (end > start &&
           (sp[end - 1] == ' ' || sp[end - 1] == '\t' || sp[end - 1] == '\n' ||
            sp[end - 1] == '\r' || sp[end - 1] == '\v' || sp[end - 1] == '\f')) end--;
    ax_arr out; ax_arr_init(&out);
    int64_t len = end - start;
    uint8_t* buf = (uint8_t*)ax_alloc(len <= 0 ? 1 : len);
    if (len > 0) memcpy(buf, sp + start, (size_t)len);
    out.data = buf; out.len = len; out.cap = len;
    return out;
}

/* bytes.ToUpper：将 s 中的 ASCII 小写字母转为大写，返回新数组。 */

/* bytes.ToUpper：将 s 中的 ASCII 小写字母转为大写，返回新数组。 */
ax_arr ax_bytes_to_upper(ax_arr s) {
    const uint8_t* sp = (const uint8_t*)s.data;
    uint8_t* buf = (uint8_t*)ax_alloc(s.len <= 0 ? 1 : s.len);
    for (int64_t i = 0; i < s.len; i++) buf[i] = u8_to_ascii_upper(sp[i]);
    ax_arr out; ax_arr_init(&out);
    out.data = buf; out.len = s.len; out.cap = s.len;
    return out;
}

/* bytes.ToLower：将 s 中的 ASCII 大写字母转为小写，返回新数组。 */

/* bytes.ToLower：将 s 中的 ASCII 大写字母转为小写，返回新数组。 */
ax_arr ax_bytes_to_lower(ax_arr s) {
    const uint8_t* sp = (const uint8_t*)s.data;
    uint8_t* buf = (uint8_t*)ax_alloc(s.len <= 0 ? 1 : s.len);
    for (int64_t i = 0; i < s.len; i++) buf[i] = u8_to_ascii_lower(sp[i]);
    ax_arr out; ax_arr_init(&out);
    out.data = buf; out.len = s.len; out.cap = s.len;
    return out;
}

/* bytes.Repeat：将 s 重复 count 次，返回新数组。count <= 0 返回空数组。 */

/* bytes.Repeat：将 s 重复 count 次，返回新数组。count <= 0 返回空数组。 */
ax_arr ax_bytes_repeat(ax_arr s, int64_t count) {
    if (count <= 0) {
        ax_arr out; ax_arr_init(&out);
        return out;
    }
    const uint8_t* sp = (const uint8_t*)s.data;
    int64_t total = s.len * count;
    uint8_t* buf = (uint8_t*)ax_alloc(total <= 0 ? 1 : total);
    for (int64_t i = 0; i < count; i++) {
        if (s.len > 0) memcpy(buf + i * s.len, sp, (size_t)s.len);
    }
    ax_arr out; ax_arr_init(&out);
    out.data = buf; out.len = total; out.cap = total;
    return out;
}

/* ---------------- bufio 模块（对标 Go bufio，P2） ----------------
 * axlang 的 Reader/Writer 即 File 句柄（ax_file*）。bufio 函数接收并返回 File。
 * 本版本建立 API：在 C 层返回同一句柄，缓冲由底层 FILE* 的 stdio 缓冲提供。
 * 后续版本可在此基础上增加显式 ax_buf_reader/ax_buf_writer 结构体与可配置缓冲大小。
 */

/* bufio.NewReader(f: File) -> File：创建缓冲读取器，对标 Go bufio.NewReader。 */
ax_file* ax_file_buf_reader(ax_file* f) {
    return f;
}

/* bufio.NewWriter(f: File) -> File：创建缓冲写入器，对标 Go bufio.NewWriter。 */
ax_file* ax_file_buf_writer(ax_file* f) {
    return f;
}

/* bufio.Scanner(f: File) -> File：创建扫描器，对标 Go bufio.NewScanner。 */
ax_file* ax_file_buf_scanner(ax_file* f) {
    return f;
}

/* string.Title：每个单词首字母大写（对标 Go strings.Title，仅处理 ASCII。
 * 单词由空白符分隔；非 ASCII 字符不作大小写变换。 */

/* string.Title：每个单词首字母大写（对标 Go strings.Title，仅处理 ASCII。
 * 单词由空白符分隔；非 ASCII 字符不作大小写变换。 */
ax_str ax_str_title(ax_str s) {
    uint8_t* buf = (uint8_t*)ax_alloc(s.len <= 0 ? 1 : s.len);
    int64_t pos = 0;
    int new_word = 1;
    for (int64_t i = 0; i < s.len; i++) {
        uint8_t c = s.p[i];
        if (ax_is_space(c)) {
            buf[pos++] = c;
            new_word = 1;
        } else if (new_word && u8_is_ascii_lower(c)) {
            buf[pos++] = (uint8_t)(c - 'a' + 'A');
            new_word = 0;
        } else {
            buf[pos++] = c;
            new_word = 0;
        }
    }
    return ax_str_from_bytes(buf, pos);
}

/* string.Map：对每个字符调用 mapping(int) -> int，结果拼接成新字符串。
 * mapping 返回 0 表示删除该字符；按 UTF-8 码点处理。（axlang int 映射为 int64_t） */

/* string.Map：对每个字符调用 mapping(int) -> int，结果拼接成新字符串。
 * mapping 返回 0 表示删除该字符；按 UTF-8 码点处理。（axlang int 映射为 int64_t） */
ax_str ax_str_map(ax_str s, int64_t (*mapping)(int64_t)) {
    uint8_t* buf = (uint8_t*)ax_alloc(s.len <= 0 ? 1 : s.len * 4);
    int64_t pos = 0;
    int64_t i = 0;
    while (i < s.len) {
        int32_t cp;
        int k = u8_decode(s.p + i, s.len - i, &cp);
        if (k <= 0) { buf[pos++] = s.p[i]; i++; continue; }
        int r = mapping(cp);
        if (r > 0) {
            int kk = u8_encode((int32_t)r, buf + pos);
            if (kk > 0) pos += kk;
        }
        /* r <= 0：删除该字符 */
        i += k;
    }
    return ax_str_from_bytes(buf, pos);
}

/* string.EqualFold：忽略大小写的字符串相等比较，对标 Go strings.EqualFold。
 * 仅处理 ASCII 大小写（A-Z<->a-z），非 ASCII 字符按字节比较；相等返回 1，否则返回 0。
 * 空字符串相等返回 1。标准 C11 实现。 */

/* string.EqualFold：忽略大小写的字符串相等比较，对标 Go strings.EqualFold。
 * 仅处理 ASCII 大小写（A-Z<->a-z），非 ASCII 字符按字节比较；相等返回 1，否则返回 0。
 * 空字符串相等返回 1。标准 C11 实现。 */
int ax_str_equal_fold(ax_str a, ax_str b) {
    if (a.len != b.len) return 0;
    for (int64_t i = 0; i < a.len; i++) {
        uint8_t ca = u8_to_ascii_lower(a.p[i]);
        uint8_t cb = u8_to_ascii_lower(b.p[i]);
        if (ca != cb) return 0;
    }
    return 1;
}

/* ---------------- 前缀/后缀判断 ---------------- */


/* ---------------- 前缀/后缀判断 ---------------- */

int ax_str_starts_with(ax_str s, ax_str prefix) {
    if (prefix.len == 0) return 1;
    if (prefix.len > s.len) return 0;
    return memcmp(s.p, prefix.p, (size_t)prefix.len) == 0 ? 1 : 0;
}


int ax_str_ends_with(ax_str s, ax_str suffix) {
    if (suffix.len == 0) return 1;
    if (suffix.len > s.len) return 0;
    return memcmp(s.p + s.len - suffix.len, suffix.p, (size_t)suffix.len) == 0 ? 1 : 0;
}

/* ---------------- 字符属性判断（对标 C <ctype.h>） ----------------
 * 作用于整个字符串：所有字符均满足返回 1，否则返回 0；空字符串返回 0。
 * 仅处理 ASCII 字符，UTF-8 多字节字符均视为不匹配。 */


/* ---------------- 字符属性判断（对标 C <ctype.h>） ----------------
 * 作用于整个字符串：所有字符均满足返回 1，否则返回 0；空字符串返回 0。
 * 仅处理 ASCII 字符，UTF-8 多字节字符均视为不匹配。 */

static int ax_u8_is_digit(uint8_t c) { return c >= (uint8_t)'0' && c <= (uint8_t)'9'; }
static int ax_u8_is_alpha(uint8_t c) {
    return (c >= (uint8_t)'a' && c <= (uint8_t)'z') || (c >= (uint8_t)'A' && c <= (uint8_t)'Z');
}
static int ax_u8_is_space(uint8_t c) {
    return c == (uint8_t)' ' || c == (uint8_t)'\t' || c == (uint8_t)'\n' ||
           c == (uint8_t)'\r' || c == (uint8_t)'\v' || c == (uint8_t)'\f';
}
static int ax_u8_is_lower(uint8_t c) { return c >= (uint8_t)'a' && c <= (uint8_t)'z'; }
static int ax_u8_is_upper(uint8_t c) { return c >= (uint8_t)'A' && c <= (uint8_t)'Z'; }
static int ax_u8_is_alnum(uint8_t c) { return ax_u8_is_digit(c) || ax_u8_is_alpha(c); }


int ax_str_is_digit(ax_str s) {
    if (s.len == 0) return 0;
    for (int64_t i = 0; i < s.len; i++) {
        if (!ax_u8_is_digit(s.p[i])) return 0;
    }
    return 1;
}


int ax_str_is_alpha(ax_str s) {
    if (s.len == 0) return 0;
    for (int64_t i = 0; i < s.len; i++) {
        if (!ax_u8_is_alpha(s.p[i])) return 0;
    }
    return 1;
}


int ax_str_is_space(ax_str s) {
    if (s.len == 0) return 0;
    for (int64_t i = 0; i < s.len; i++) {
        if (!ax_u8_is_space(s.p[i])) return 0;
    }
    return 1;
}


int ax_str_is_lower(ax_str s) {
    if (s.len == 0) return 0;
    for (int64_t i = 0; i < s.len; i++) {
        if (!ax_u8_is_lower(s.p[i])) return 0;
    }
    return 1;
}


int ax_str_is_upper(ax_str s) {
    if (s.len == 0) return 0;
    for (int64_t i = 0; i < s.len; i++) {
        if (!ax_u8_is_upper(s.p[i])) return 0;
    }
    return 1;
}


int ax_str_is_alnum(ax_str s) {
    if (s.len == 0) return 0;
    for (int64_t i = 0; i < s.len; i++) {
        if (!ax_u8_is_alnum(s.p[i])) return 0;
    }
    return 1;
}

/* ---------------- 连接数组（sep 作为分隔符） ---------------- */


/* ---------------- 连接数组（sep 作为分隔符） ---------------- */

ax_str ax_str_join(ax_str sep, ax_str* parts, int64_t n) {
    if (n <= 0) return ax_str_empty();
    int64_t total = 0;
    for (int64_t i = 0; i < n; i++) {
        total += parts[i].len;
        if (i + 1 < n) total += sep.len;
    }
    uint8_t* buf = (uint8_t*)ax_alloc(total <= 0 ? 1 : total);
    int64_t pos = 0;
    for (int64_t i = 0; i < n; i++) {
        if (parts[i].len > 0) {
            memcpy(buf + pos, parts[i].p, (size_t)parts[i].len);
            pos += parts[i].len;
        }
        if (i + 1 < n && sep.len > 0) {
            memcpy(buf + pos, sep.p, (size_t)sep.len);
            pos += sep.len;
        }
    }
    return ax_str_from_bytes(buf, pos);
}

/* ---------------- 替换所有出现 ---------------- */


/* ---------------- 替换所有出现 ---------------- */

ax_str ax_str_replace(ax_str s, ax_str old, ax_str new) {
    if (old.len == 0) return ax_str_from_bytes(s.p, s.len); /* 空模式不替换 */
    /* 第一遍：计算结果长度 */
    int64_t count = 0, pos = 0;
    while (pos + old.len <= s.len) {
        if (memcmp(s.p + pos, old.p, (size_t)old.len) == 0) {
            count++;
            pos += old.len;
        } else {
            pos++;
        }
    }
    int64_t total = s.len + count * (new.len - old.len);
    if (total < 0) total = 0;
    uint8_t* buf = (uint8_t*)ax_alloc(total <= 0 ? 1 : total);
    int64_t out = 0;
    pos = 0;
    while (pos + old.len <= s.len) {
        if (memcmp(s.p + pos, old.p, (size_t)old.len) == 0) {
            if (new.len > 0) {
                memcpy(buf + out, new.p, (size_t)new.len);
                out += new.len;
            }
            pos += old.len;
        } else {
            buf[out++] = s.p[pos++];
        }
    }
    /* 拷贝剩余尾部 */
    while (pos < s.len) {
        buf[out++] = s.p[pos++];
    }
    return ax_str_from_bytes(buf, out);
}

/* ---------------- 重复 n 次 ---------------- */


/* ---------------- 重复 n 次 ---------------- */

ax_str ax_str_repeat(ax_str s, int64_t n) {
    if (n <= 0 || s.len == 0) return ax_str_empty();
    int64_t total = s.len * n;
    uint8_t* buf = (uint8_t*)ax_alloc(total <= 0 ? 1 : total);
    for (int64_t i = 0; i < n; i++) {
        memcpy(buf + i * s.len, s.p, (size_t)s.len);
    }
    return ax_str_from_bytes(buf, total);
}


ax_str ax_str_slice_cp(ax_str s, int64_t from, int64_t to) {
    if (from < 0) from = 0;
    int64_t pos = 0, c0 = -1, c1 = -1, idx = 0;
    while (pos <= s.len) {
        if (idx == from) c0 = pos;
        if (idx == to) { c1 = pos; break; }
        if (pos == s.len) break;
        int32_t cp;
        int k = u8_decode(s.p + pos, s.len - pos, &cp);
        if (k <= 0) break;
        pos += k;
        idx++;
    }
    if (c0 < 0) c0 = s.len;
    if (c1 < 0) c1 = s.len;
    if (c1 < c0) c1 = c0;
    return ax_str_from_bytes(s.p + c0, c1 - c0);
}


ax_str ax_str_of_int(int64_t v) {
    char buf[32];
    int n = snprintf(buf, sizeof buf, "%" PRId64, v);
    return ax_str_from_bytes((const uint8_t*)buf, n);
}


ax_str ax_str_of_uint(uint64_t v) {
    char buf[32];
    int n = snprintf(buf, sizeof buf, "%" PRIu64, v);
    return ax_str_from_bytes((const uint8_t*)buf, n);
}


ax_str ax_str_of_bool(int v) {
    return v ? ax_str_from_cstr("true") : ax_str_from_cstr("false");
}


ax_str ax_str_of_f64(double v) {
    char buf[64];
    int n = snprintf(buf, sizeof buf, "%.15g", v);
    /* 保证是浮点形态：没有 '.' 也没有 'e' 时补 ".0" */
    int has = 0;
    for (int i = 0; i < n; i++) if (buf[i] == '.' || buf[i] == 'e' || buf[i] == 'E') has = 1;
    if (!has) { buf[n++] = '.'; buf[n++] = '0'; buf[n] = 0; }
    return ax_str_from_bytes((const uint8_t*)buf, n);
}


ax_str ax_str_of_dec(double v) { return ax_str_of_f64(v); }


/* ================================================================ builder */
void ax_sb_init(ax_sb* b) { b->buf = NULL; b->len = 0; b->cap = 0; }


static void sb_reserve(ax_sb* b, int64_t need) {
    if (b->len + need <= b->cap) return;
    int64_t cap = b->cap ? b->cap : 32;
    while (cap < b->len + need) cap *= 2;
    uint8_t* nb = (uint8_t*)ax_alloc(cap);
    if (b->buf) memcpy(nb, b->buf, (size_t)b->len);
    b->buf = nb;
    b->cap = cap;
}


void ax_sb_push(ax_sb* b, ax_str s) {
    if (s.len <= 0) return;
    sb_reserve(b, s.len);
    memcpy(b->buf + b->len, s.p, (size_t)s.len);
    b->len += s.len;
}


void ax_sb_push_cstr(ax_sb* b, const char* s) {
    ax_sb_push(b, ax_str_from_cstr(s));
}


void ax_sb_push_int(ax_sb* b, int64_t v) { ax_sb_push(b, ax_str_of_int(v)); }
void ax_sb_push_uint(ax_sb* b, uint64_t v) { ax_sb_push(b, ax_str_of_uint(v)); }
void ax_sb_push_f64(ax_sb* b, double v) { ax_sb_push(b, ax_str_of_f64(v)); }


void ax_sb_push_f64_n(ax_sb* b, double v, int32_t digits) {
    char fmt[16];
    snprintf(fmt, sizeof fmt, "%%.%df", (int)digits);
    char buf[256];
    int n = snprintf(buf, sizeof buf, fmt, v);
    ax_sb_push(b, ax_str_from_bytes((const uint8_t*)buf, n));
}


void ax_sb_push_dec(ax_sb* b, ax_dec v) { ax_sb_push(b, ax_dec_to_str(v)); }


void ax_sb_push_dec_n(ax_sb* b, ax_dec v, int32_t digits) {
    /* `:.N` 的语义是「固定 N 位小数」——先按 N 位四舍五入，再把尾零补回去。
       ax_dec_to_str 会去掉无意义的尾零（10.50 → "10.5"），不能直接用它收尾。 */
    ax_dec d = ax_dec_round(v, digits, ROUND_HALF_UP);
    ax_str s = ax_dec_to_str(d);
    int64_t dot = -1;
    for (int64_t i = 0; i < s.len; i++) {
        if (s.p[i] == '.') { dot = i; break; }
    }
    int64_t frac = dot < 0 ? 0 : (s.len - dot - 1);
    ax_sb_push(b, s);
    if (digits > 0 && frac < digits) {
        if (dot < 0) ax_sb_push_cstr(b, ".");
        for (int32_t i = (int32_t)frac; i < digits; i++) ax_sb_push_cstr(b, "0");
    }
}


ax_str ax_sb_finish(ax_sb* b) {
    ax_str s = ax_str_from_bytes(b->buf ? b->buf : (const uint8_t*)"", b->len);
    b->buf = NULL;
    b->len = b->cap = 0;
    return s;
}

/* ================================================================ decimal */


ax_str ax_str_of_int_base(int64_t v, int base) {
    char buf[64];
    int n;
    if (base <= 0 || base == 10) {
        n = snprintf(buf, sizeof buf, "%lld", (long long)v);
    } else if (base == 16) {
        n = snprintf(buf, sizeof buf, "%llx", (unsigned long long)v);
    } else if (base == 8) {
        n = snprintf(buf, sizeof buf, "%llo", (unsigned long long)v);
    } else if (base == 2) {
        /* 标准 C 无 %b，手动二进制 */
        char tmp[65];
        int i = 64;
        tmp[i--] = 0;
        unsigned long long u = (unsigned long long)v;
        if (u == 0) {
            tmp[i--] = '0';
        } else {
            while (u > 0) {
                tmp[i--] = (char)('0' + (int)(u & 1ULL));
                u >>= 1;
            }
        }
        n = (int)(64 - i - 1);
        memcpy(buf, tmp + i + 1, (size_t)n);
        buf[n] = 0;
    } else {
        n = snprintf(buf, sizeof buf, "%lld", (long long)v);
    }
    return ax_str_from_bytes((const uint8_t*)buf, n);
}

/* strconv 无符号进制解析（对标 Go strconv.ParseUint，标准 C11 实现）：
 * axlang 无 uint 类型，故返回 int（int64）；失败或超出 int64 范围返回 0。
 * base 支持 2、8、10、16；base<=0 时按字符串前缀自动判断。 */
int64_t ax_parse_uint_base(ax_str s, int base) {
    char buf[64];
    if (s.len <= 0 || s.len >= 63) return 0;
    memcpy(buf, s.p, (size_t)s.len);
    buf[s.len] = 0;
    char* end = NULL;
    int b = base;
    if (b <= 0) {
        if (s.len >= 2 && buf[0] == '0' && (buf[1] == 'x' || buf[1] == 'X')) b = 16;
        else if (buf[0] == '0') b = 8;
        else b = 10;
    }
    errno = 0;
    unsigned long long u = strtoull(buf, &end, b);
    if (errno != 0 || !end || end == buf) return 0;
    while (*end == ' ' || *end == '\t') end++;
    if (*end != 0) return 0;
    if (u > (unsigned long long)INT64_MAX) return 0;  /* 超出 int64 范围 */
    return (int64_t)u;
}

/* strconv 无符号进制格式化（对标 Go strconv.FormatUint，标准 C11 实现）：
 * v 按无符号解释并格式化；base 支持 2、8、10、16；base<=0 时按 10 进制输出。 */

/* strconv 无符号进制格式化（对标 Go strconv.FormatUint，标准 C11 实现）：
 * v 按无符号解释并格式化；base 支持 2、8、10、16；base<=0 时按 10 进制输出。 */
ax_str ax_str_of_uint_base(int64_t v, int base) {
    char buf[64];
    int n;
    unsigned long long u = (unsigned long long)v;  /* 按无符号解释 */
    if (base <= 0 || base == 10) {
        n = snprintf(buf, sizeof buf, "%llu", u);
    } else if (base == 16) {
        n = snprintf(buf, sizeof buf, "%llx", u);
    } else if (base == 8) {
        n = snprintf(buf, sizeof buf, "%llo", u);
    } else if (base == 2) {
        /* 标准 C 无 %b，手动二进制 */
        char tmp[65];
        int i = 64;
        tmp[i--] = 0;
        if (u == 0) {
            tmp[i--] = '0';
        } else {
            while (u > 0) {
                tmp[i--] = (char)('0' + (int)(u & 1ULL));
                u >>= 1;
            }
        }
        n = (int)(64 - i - 1);
        memcpy(buf, tmp + i + 1, (size_t)n);
        buf[n] = 0;
    } else {
        n = snprintf(buf, sizeof buf, "%llu", u);
    }
    return ax_str_from_bytes((const uint8_t*)buf, n);
}

/* ---------------- strconv 字符串引用处理（对标 Go strconv.Quote/Unquote/CanBackquote） ----------------
 * 标准 C11 实现，无编译器扩展。 */

/* 检查字符是否为可打印字符（Go 的 IsPrint 语义：0x20~0x7E，以及 tab 0x09 在反引号中允许） */

/* ---------------- strconv 字符串引用处理（对标 Go strconv.Quote/Unquote/CanBackquote） ----------------
 * 标准 C11 实现，无编译器扩展。 */

/* 检查字符是否为可打印字符（Go 的 IsPrint 语义：0x20~0x7E，以及 tab 0x09 在反引号中允许） */
int ax_u8_is_printable(uint8_t c) {
    return c >= (uint8_t)0x20 && c <= (uint8_t)0x7e;
}


int ax_str_can_backquote(ax_str s) {
    /* 单行反引号字符串：不含换行符，不含除 tab 外的控制字符 */
    for (int64_t i = 0; i < s.len; i++) {
        uint8_t c = s.p[i];
        if (c == (uint8_t)'\n' || c == (uint8_t)'\r') return 0;
        if (c != (uint8_t)'\t' && (c < (uint8_t)0x20 || c == (uint8_t)0x7f)) return 0;
    }
    return 1;
}

/* 将一个字节写入 Go 转义序列，返回写入的字符数（不含 NUL）。
 * buf 必须至少有 6 字节空间（\UXXXXXXXX 最长 10 字节，这里保守按 6 足够）。 */

ax_str ax_str_quote(ax_str s) {
    /* 最坏情况：每个字节都转义为 \xHH（4 字节），加首尾引号和 NUL */
    int64_t cap = s.len * 4 + 3;
    uint8_t* buf = (uint8_t*)ax_alloc(cap);
    int64_t pos = 0;
    buf[pos++] = '"';
    for (int64_t i = 0; i < s.len; i++) {
        char esc[8];
        int n = ax_quote_escape(s.p[i], esc);
        if (pos + n > cap - 2) break;  /* 保留空间给结尾引号和 NUL */
        memcpy(buf + pos, esc, (size_t)n);
        pos += n;
    }
    buf[pos++] = '"';
    buf[pos] = 0;
    return ax_str_from_bytes(buf, pos);
}

/* 十六进制字符转数值，失败返回 -1（前向声明，供 ax_unquote_escape 调用） */

ax_str ax_str_unquote(ax_str s) {
    /* 必须以双引号开始和结束，且长度至少 2 */
    if (s.len < 2 || s.p[0] != (uint8_t)'"' || s.p[s.len - 1] != (uint8_t)'"')
        return ax_str_empty();

    const uint8_t* p = s.p + 1;       /* 跳过首引号 */
    const uint8_t* end = s.p + s.len - 1;  /* 不含尾引号 */
    int64_t cap = s.len;              /* 最多不超过原长度（去掉引号后） */
    uint8_t* buf = (uint8_t*)ax_alloc(cap <= 0 ? 1 : cap);
    int64_t pos = 0;

    while (p < end) {
        if (*p == (uint8_t)'\\') {
            p++;  /* 跳过反斜杠 */
            uint8_t ch;
            if (ax_unquote_escape(&p, end, &ch) < 0) {
                /* 转义序列非法，返回空串 */
                return ax_str_empty();
            }
            buf[pos++] = ch;
        } else {
            buf[pos++] = *p;
            p++;
        }
    }
    return ax_str_from_bytes(buf, pos);
}

/* ---------------- 文件（Rt-13） ---------------- */

/* ax_str → 以 NUL 结尾的栈缓冲；路径过长返回 0（与 ax_read_file 同一上限）。 */
