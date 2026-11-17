/* axrt.c -- axlang 运行时实现 */
#include "axrt.h"
#include <stdarg.h>
#include <math.h>
#include <inttypes.h>
#ifdef _WIN32
#include <windows.h>
#include <io.h>
#else
#include <sys/time.h>
#endif

/* ================================================================ 崩溃 */
static const char* trap_name(int code) {
    switch (code) {
    case R_INDEX:    return "index-out-of-range";
    case R_DIVZERO:  return "divide-by-zero";
    case R_INEXACT:  return "inexact-division";
    case R_OVERFLOW: return "overflow";
    case R_SHIFT:    return "shift-out-of-range";
    case R_FLOAT:    return "float-not-finite";
    case R_CONVERT:  return "bad-conversion";
    case R_OOM:      return "out-of-memory";
    default:         return "trap";
    }
}

_Noreturn void ax_trap(int code, const char* file, int line, int col,
                       const char* fmt, ...) {
    char detail[1024];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(detail, sizeof detail, fmt, ap);
    va_end(ap);
    fprintf(stderr, "axlang: runtime trap [R%d] %s\n", code, trap_name(code));
    fprintf(stderr, "  at %s:%d:%d\n", file ? file : "<unknown>", line, col);
    fprintf(stderr, "  detail: %s\n", detail);
    fflush(stderr);
    exit(2);
}

_Noreturn void ax_oom(void) {
    fprintf(stderr, "axlang: runtime trap [R100] out-of-memory\n");
    fprintf(stderr, "  at <runtime>:0:0\n");
    fprintf(stderr, "  detail: allocation failed\n");
    fflush(stderr);
    exit(2);
}

void* ax_alloc(int64_t size) {
    void* p = malloc(size <= 0 ? 1 : (size_t)size);
    if (!p) ax_oom();
    return p;
}

void* ax_calloc(int64_t n, int64_t size) {
    void* p = calloc(n <= 0 ? 1 : (size_t)n, size <= 0 ? 1 : (size_t)size);
    if (!p) ax_oom();
    return p;
}

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
#ifdef _WIN32
static UINT ax_cp_out_saved = 0;
static UINT ax_cp_in_saved = 0;

static void ax_console_restore(void) {
    if (ax_cp_out_saved) SetConsoleOutputCP(ax_cp_out_saved);
    if (ax_cp_in_saved) SetConsoleCP(ax_cp_in_saved);
}
#endif

void ax_console_utf8(void) {
#ifdef _WIN32
    if (_isatty(_fileno(stdout)) || _isatty(_fileno(stderr))) {
        UINT cp = GetConsoleOutputCP();
        if (cp != 0 && cp != CP_UTF8 && SetConsoleOutputCP(CP_UTF8)) {
            ax_cp_out_saved = cp;
            atexit(ax_console_restore);
        }
    }
    if (_isatty(_fileno(stdin))) {
        UINT cp = GetConsoleCP();
        if (cp != 0 && cp != CP_UTF8 && SetConsoleCP(CP_UTF8)) {
            ax_cp_in_saved = cp;
        }
    }
#endif
}

/* 读环境变量并统一成 UTF-8。
   Windows 的窄字符 getenv 返回 ANSI 代码页字节，中文夹具会变成 GBK，
   与源码里的 UTF-8 字面量混在一起就是乱码 —— 这里走宽字符接口再转 UTF-8。 */
ax_str ax_env_utf8(const char* name) {
#ifdef _WIN32
    wchar_t wname[64];
    int i = 0;
    for (; name[i] && i < 63; i++) wname[i] = (wchar_t)(unsigned char)name[i];
    wname[i] = 0;
    wchar_t wval[4096];
    DWORD n = GetEnvironmentVariableW(wname, wval, 4096);
    if (n == 0 || n >= 4096) return ax_str_empty();
    char buf[8192];
    int m = WideCharToMultiByte(CP_UTF8, 0, wval, (int)n, buf, (int)sizeof buf, NULL, NULL);
    if (m <= 0) return ax_str_empty();
    return ax_str_from_bytes((const uint8_t*)buf, (int64_t)m);
#else
    const char* v = getenv(name);
    return v ? ax_str_from_cstr(v) : ax_str_empty();
#endif
}

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

ax_str ax_str_trim(ax_str s) {
    int64_t a = 0, b = s.len;
    while (a < b && (s.p[a] == ' ' || s.p[a] == '\t' || s.p[a] == '\n' || s.p[a] == '\r')) a++;
    while (b > a && (s.p[b-1] == ' ' || s.p[b-1] == '\t' || s.p[b-1] == '\n' || s.p[b-1] == '\r')) b--;
    return ax_str_from_bytes(s.p + a, b - a);
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

void ax_write_stdout(ax_str s) {
    if (s.len > 0) fwrite(s.p, 1, (size_t)s.len, stdout);
}

void ax_write_stderr(ax_str s) {
    if (s.len > 0) fwrite(s.p, 1, (size_t)s.len, stderr);
}

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
static __int128 pow10_128(int n) {
    __int128 r = 1;
    for (int i = 0; i < n; i++) r *= 10;
    return r;
}

static int mul_ovf(__int128 a, __int128 b, __int128* out) {
    return __builtin_mul_overflow(a, b, out);
}

static int add_ovf(__int128 a, __int128 b, __int128* out) {
    return __builtin_add_overflow(a, b, out);
}

ax_dec ax_dec_from_i64(int64_t v) {
    ax_dec d; d.coeff = (__int128)v; d.scale = 0; d._pad = 0;
    return d;
}

ax_dec ax_dec_lit(const char* digits, int32_t scale) {
    int neg = 0;
    const char* p = digits;
    if (*p == '-') { neg = 1; p++; }
    __int128 c = 0;
    while (*p) {
        if (*p >= '0' && *p <= '9') c = c * 10 + (*p - '0');
        p++;
    }
    ax_dec d; d.coeff = neg ? -c : c; d.scale = scale; d._pad = 0;
    return d;
}

ax_dec ax_dec_from_parts(__int128 coeff, int32_t scale) {
    ax_dec d; d.coeff = coeff; d.scale = scale; d._pad = 0;
    return d;
}

static ax_dec dec_norm(ax_dec v) {
    while (v.scale > 0 && v.coeff != 0 && v.coeff % 10 == 0) {
        v.coeff /= 10;
        v.scale--;
    }
    if (v.coeff == 0) v.scale = 0;
    return v;
}

static void dec_align(ax_dec a, ax_dec b, __int128* ac, __int128* bc, int32_t* sc,
                      const char* f, int l, int c) {
    int32_t s = a.scale > b.scale ? a.scale : b.scale;
    __int128 x, y;
    if (mul_ovf(a.coeff, pow10_128(s - a.scale), &x)) ax_trap(R_OVERFLOW, f, l, c, "decimal add overflow");
    if (mul_ovf(b.coeff, pow10_128(s - b.scale), &y)) ax_trap(R_OVERFLOW, f, l, c, "decimal add overflow");
    *ac = x; *bc = y; *sc = s;
}

ax_dec ax_dec_add(ax_dec a, ax_dec b) {
    __int128 x, y, r;
    int32_t s;
    dec_align(a, b, &x, &y, &s, "<runtime>", 0, 0);
    if (add_ovf(x, y, &r)) ax_trap(R_OVERFLOW, "<runtime>", 0, 0, "decimal overflow");
    return dec_norm(ax_dec_from_parts(r, s));
}

ax_dec ax_dec_sub(ax_dec a, ax_dec b) {
    __int128 x, y, r;
    int32_t s;
    dec_align(a, b, &x, &y, &s, "<runtime>", 0, 0);
    if (__builtin_sub_overflow(x, y, &r)) ax_trap(R_OVERFLOW, "<runtime>", 0, 0, "decimal overflow");
    return dec_norm(ax_dec_from_parts(r, s));
}

ax_dec ax_dec_mul(ax_dec a, ax_dec b) {
    __int128 r;
    if (mul_ovf(a.coeff, b.coeff, &r)) ax_trap(R_OVERFLOW, "<runtime>", 0, 0, "decimal overflow");
    return dec_norm(ax_dec_from_parts(r, a.scale + b.scale));
}

static __int128 i128_abs(__int128 v) { return v < 0 ? -v : v; }

static __int128 i128_gcd(__int128 a, __int128 b) {
    a = i128_abs(a); b = i128_abs(b);
    while (b != 0) { __int128 t = a % b; a = b; b = t; }
    return a;
}

ax_dec ax_dec_div_exact(ax_dec a, ax_dec b, const char* f, int l, int c) {
    if (b.coeff == 0) ax_trap(R_DIVZERO, f, l, c, "decimal division by zero");
    __int128 g = i128_gcd(a.coeff, b.coeff);
    if (g == 0) g = 1;
    __int128 num = a.coeff / g, den0 = b.coeff / g;
    if (den0 < 0) { den0 = -den0; num = -num; }
    int p = 0, q = 0;
    __int128 d = den0;
    while (d % 2 == 0) { d /= 2; p++; }
    while (d % 5 == 0) { d /= 5; q++; }
    if (d != 1) {
        ax_trap(R_INEXACT, f, l, c,
                "除不尽（建议用 divide(a, b, digits, Rounding.HalfUp)）");
    }
    int m = p > q ? p : q;
    __int128 coeff = num * pow10_128(m) / den0;
    int32_t scale = (int32_t)(m - (b.scale - a.scale));
    if (scale < 0) {
        __int128 t;
        if (mul_ovf(coeff, pow10_128(-scale), &t)) ax_trap(R_OVERFLOW, f, l, c, "decimal overflow");
        coeff = t;
        scale = 0;
    }
    return dec_norm(ax_dec_from_parts(coeff, scale));
}

static __int128 div_round_i128(__int128 num, __int128 den, int mode) {
    if (den == 0) return 0;
    int neg = (num < 0) != (den < 0);
    __int128 n = i128_abs(num), d = i128_abs(den);
    __int128 q = n / d, r = n % d;
    if (r != 0) {
        if (mode == ROUND_DOWN) {
            /* 向零截断 */
        } else if (mode == ROUND_HALF_EVEN) {
            __int128 twice = r * 2;
            if (twice > d || (twice == d && (q % 2 != 0))) q += 1;
        } else {
            __int128 twice = r * 2;
            if (twice >= d) q += 1;
        }
    }
    return neg ? -q : q;
}

ax_dec ax_dec_div_round(ax_dec a, ax_dec b, int32_t digits, int mode) {
    if (b.coeff == 0) ax_trap(R_DIVZERO, "<runtime>", 0, 0, "decimal division by zero");
    int k = digits + b.scale - a.scale;
    __int128 num = a.coeff, den = b.coeff;
    __int128 t;
    if (k > 0) {
        if (mul_ovf(num, pow10_128(k), &t)) ax_trap(R_OVERFLOW, "<runtime>", 0, 0, "decimal overflow");
        num = t;
    } else if (k < 0) {
        if (mul_ovf(den, pow10_128(-k), &t)) ax_trap(R_OVERFLOW, "<runtime>", 0, 0, "decimal overflow");
        den = t;
    }
    __int128 q = div_round_i128(num, den, mode);
    return dec_norm(ax_dec_from_parts(q, digits));
}

ax_dec ax_dec_rem(ax_dec a, ax_dec b) {
    if (b.coeff == 0) ax_trap(R_DIVZERO, "<runtime>", 0, 0, "decimal remainder by zero");
    __int128 x, y, r;
    int32_t s;
    dec_align(a, b, &x, &y, &s, "<runtime>", 0, 0);
    r = x % y;
    return dec_norm(ax_dec_from_parts(r, s));
}

ax_dec ax_dec_round(ax_dec a, int32_t digits, int mode) {
    if (a.scale <= digits) return a;
    int drop = a.scale - digits;
    __int128 den = pow10_128(drop);
    __int128 q = div_round_i128(a.coeff, den, mode);
    return dec_norm(ax_dec_from_parts(q, digits));
}

int ax_dec_cmp(ax_dec a, ax_dec b) {
    __int128 x, y;
    int32_t s;
    dec_align(a, b, &x, &y, &s, "<runtime>", 0, 0);
    if (x < y) return -1;
    if (x > y) return 1;
    return 0;
}

int64_t ax_dec_to_i64(ax_dec a, const char* f, int l, int c) {
    __int128 v = a.coeff / pow10_128(a.scale);
    if (v > (__int128)INT64_MAX || v < (__int128)INT64_MIN)
        ax_trap(R_CONVERT, f, l, c, "decimal %s 超出 int 范围", "value");
    return (int64_t)v;
}

double ax_dec_to_f64(ax_dec v) {
    return (double)v.coeff / pow((double)10, (double)v.scale);
}

double ax_dec_as_double(ax_dec v) { return ax_dec_to_f64(v); }

ax_str ax_dec_to_str(ax_dec v) {
    ax_dec d = dec_norm(v);
    int neg = d.coeff < 0;
    __int128 a = neg ? -d.coeff : d.coeff;
    char digits[64];
    int n = 0;
    if (a == 0) digits[n++] = '0';
    while (a > 0 && n < 60) { digits[n++] = (char)('0' + (int)(a % 10)); a /= 10; }
    char out[80];
    int k = 0;
    if (neg) out[k++] = '-';
    if (d.scale == 0) {
        for (int i = n - 1; i >= 0; i--) out[k++] = digits[i];
    } else {
        int intlen = n - d.scale;
        if (intlen <= 0) {
            out[k++] = '0';
            out[k++] = '.';
            for (int i = 0; i < -intlen; i++) out[k++] = '0';
            for (int i = n - 1; i >= 0; i--) out[k++] = digits[i];
        } else {
            for (int i = n - 1; i >= d.scale; i--) out[k++] = digits[i];
            out[k++] = '.';
            for (int i = d.scale - 1; i >= 0; i--) out[k++] = digits[i];
        }
    }
    return ax_str_from_bytes((const uint8_t*)out, k);
}

ax_dec ax_dec_parse(ax_str s, int* ok) {
    *ok = 0;
    int64_t i = 0;
    int neg = 0;
    if (i < s.len && (s.p[i] == '+' || s.p[i] == '-')) { neg = s.p[i] == '-'; i++; }
    __int128 coeff = 0;
    int digits = 0, scale = 0, seen = 0;
    while (i < s.len && s.p[i] >= '0' && s.p[i] <= '9') {
        coeff = coeff * 10 + (s.p[i] - '0');
        digits++; i++; seen = 1;
        if (digits > 30) return ax_dec_from_i64(0);
    }
    if (i < s.len && s.p[i] == '.') {
        i++;
        while (i < s.len && s.p[i] >= '0' && s.p[i] <= '9') {
            coeff = coeff * 10 + (s.p[i] - '0');
            scale++; i++; seen = 1;
            if (digits + scale > 30) return ax_dec_from_i64(0);
        }
    }
    if (!seen) return ax_dec_from_i64(0);
    int exp = 0;
    if (i < s.len && (s.p[i] == 'e' || s.p[i] == 'E')) {
        i++;
        int eneg = 0;
        if (i < s.len && (s.p[i] == '+' || s.p[i] == '-')) { eneg = s.p[i] == '-'; i++; }
        while (i < s.len && s.p[i] >= '0' && s.p[i] <= '9') { exp = exp * 10 + (s.p[i] - '0'); i++; }
        if (eneg) exp = -exp;
    }
    if (i != s.len) return ax_dec_from_i64(0);
    scale -= exp;
    if (scale < 0) {
        coeff *= pow10_128(-scale);
        scale = 0;
    }
    *ok = 1;
    return dec_norm(ax_dec_from_parts(neg ? -coeff : coeff, scale));
}

ax_dec ax_dec_parse_or(ax_str s, ax_dec fallback) {
    int ok = 0;
    ax_dec d = ax_dec_parse(s, &ok);
    return ok ? d : fallback;
}

/* ================================================================ 数组 */
void ax_arr_init(ax_arr* a) { a->len = 0; a->cap = 0; a->data = NULL; }

void ax_arr_from(void* out, const void* src, int64_t n, int64_t esz) {
    ax_arr* a = (ax_arr*)out;
    a->len = n;
    a->cap = n;
    if (n <= 0) { a->data = NULL; return; }
    a->data = ax_alloc(n * esz);
    memcpy(a->data, src, (size_t)(n * esz));
}

ax_arr ax_arr_of(const void* src, int64_t n, int64_t esz) {
    ax_arr a;
    ax_arr_from(&a, src, n, esz);
    return a;
}

void ax_arr_push(ax_arr* a, const void* v, int64_t esz) {
    if (a->len >= a->cap) {
        int64_t cap = a->cap ? a->cap * 2 : 4;
        void* nd = ax_alloc(cap * esz);
        if (a->len > 0) memcpy(nd, a->data, (size_t)(a->len * esz));
        a->data = nd;
        a->cap = cap;
    }
    memcpy((uint8_t*)a->data + a->len * esz, v, (size_t)esz);
    a->len++;
}

void* ax_arr_at(ax_arr* a, int64_t i, int64_t esz, const char* f, int l, int c) {
    if (i < 0 || i >= a->len)
        ax_trap(R_INDEX, f, l, c, "index %" PRId64 ", len %" PRId64, i, a->len);
    return (uint8_t*)a->data + i * esz;
}

int ax_arr_try_at(ax_arr* a, int64_t i, int64_t esz, void* out) {
    if (i < 0 || i >= a->len) return 1;
    memcpy(out, (uint8_t*)a->data + i * esz, (size_t)esz);
    return 0;
}

void ax_arr_pop(ax_arr* a, void* out, int64_t esz, const char* f, int l, int c) {
    if (a->len == 0) ax_trap(R_INDEX, f, l, c, "pop from empty array (len 0)");
    memcpy(out, (uint8_t*)a->data + (a->len - 1) * esz, (size_t)esz);
    a->len--;
}

void ax_arr_remove(ax_arr* a, int64_t i, int64_t esz, const char* f, int l, int c) {
    if (i < 0 || i >= a->len)
        ax_trap(R_INDEX, f, l, c, "remove index %" PRId64 ", len %" PRId64, i, a->len);
    memmove((uint8_t*)a->data + i * esz,
            (uint8_t*)a->data + (i + 1) * esz,
            (size_t)((a->len - i - 1) * esz));
    a->len--;
}

void ax_arr_clear(ax_arr* a) { a->len = 0; }

/* ================================================================ map */
static uint64_t hash_bytes(const uint8_t* p, int64_t n) {
    uint64_t h = 0x811C9DC5u;
    for (int64_t i = 0; i < n; i++) {
        h ^= p[i];
        h *= 0x01000193u;
    }
    return h ? h : 1;
}

static uint64_t key_hash(const void* k, int kind) {
    switch (kind) {
    case 0: { int64_t v; memcpy(&v, k, 8); uint64_t x = (uint64_t)v; x ^= x >> 33; x *= 0xff51afd7ed558ccdULL; x ^= x >> 33; return x ? x : 1; }
    case 1: { ax_str s; memcpy(&s, k, sizeof s); return hash_bytes(s.p, s.len); }
    case 2: { uint8_t v; memcpy(&v, k, 1); return (uint64_t)v + 1; }
    case 3: { ax_dec d; memcpy(&d, k, sizeof d); uint64_t x = (uint64_t)d.coeff ^ ((uint64_t)d.scale << 32); x ^= x >> 33; return x ? x : 1; }
    case 4: { uint32_t v; memcpy(&v, k, 4); return (uint64_t)v + 1; }
    default: {
        uint64_t v; memcpy(&v, k, 8);
        uint64_t x = v; x ^= x >> 33; x *= 0xff51afd7ed558ccdULL; x ^= x >> 33;
        return x ? x : 1;
    }
    }
}

static int key_eq(const void* a, const void* b, int kind, int64_t ksz) {
    if (kind == 1) {
        ax_str x, y;
        memcpy(&x, a, sizeof x);
        memcpy(&y, b, sizeof y);
        return ax_str_eq(x, y);
    }
    if (kind == 3) {
        ax_dec x, y;
        memcpy(&x, a, sizeof x);
        memcpy(&y, b, sizeof y);
        return ax_dec_cmp(x, y) == 0;
    }
    return memcmp(a, b, (size_t)ksz) == 0;
}

void ax_map_init(ax_map* m, int khash, int64_t ksz, int64_t vsz) {
    m->len = 0;
    m->cap = 0;
    m->entries = NULL;
    m->idx = NULL;
    m->idxcap = 0;
    m->seed = 0;
    m->ksz = ksz;
    m->vsz = vsz;
    m->khash = khash;
}

static void map_rehash(ax_map* m, int64_t newcap) {
    m->idx = (int32_t*)ax_calloc(newcap, (int64_t)sizeof(int32_t));
    m->idxcap = newcap;
    for (int64_t i = 0; i < m->len; i++) {
        uint64_t h = m->entries[i].hash;
        int64_t pos = (int64_t)(h & (uint64_t)(newcap - 1));
        while (m->idx[pos] != 0) pos = (pos + 1) & (newcap - 1);
        m->idx[pos] = (int32_t)(i + 1);
    }
}

static int64_t map_find(ax_map* m, const void* k, uint64_t h, int64_t* slot) {
    if (m->idxcap == 0) { if (slot) *slot = -1; return -1; }
    int64_t mask = m->idxcap - 1;
    int64_t pos = (int64_t)(h & (uint64_t)mask);
    while (m->idx[pos] != 0) {
        int64_t ei = m->idx[pos] - 1;
        if (m->entries[ei].hash == h &&
            key_eq(m->entries[ei].k, k, m->khash, m->ksz)) {
            if (slot) *slot = pos;
            return ei;
        }
        pos = (pos + 1) & mask;
    }
    if (slot) *slot = pos;
    return -1;
}

static void map_grow(ax_map* m) {
    if (m->len < m->cap) return;
    int64_t cap = m->cap ? m->cap * 2 : 8;
    ax_mentry* ne = (ax_mentry*)ax_alloc(cap * (int64_t)sizeof(ax_mentry));
    if (m->len) memcpy(ne, m->entries, (size_t)m->len * sizeof(ax_mentry));
    m->entries = ne;
    m->cap = cap;
}

void* ax_map_get(ax_map* m, const void* k) {
    uint64_t h = key_hash(k, m->khash);
    int64_t ei = map_find(m, k, h, NULL);
    if (ei < 0) return NULL;
    return m->entries[ei].v;
}

int ax_map_has(ax_map* m, const void* k) {
    uint64_t h = key_hash(k, m->khash);
    return map_find(m, k, h, NULL) >= 0;
}

void* ax_map_get_chk(ax_map* m, const void* k, const char* f, int l, int c,
                     const char* keydesc) {
    void* v = ax_map_get(m, k);
    if (!v) ax_trap(R_INDEX, f, l, c, "key not found: %s", keydesc ? keydesc : "?");
    return v;
}

void ax_map_put(ax_map* m, const void* k, const void* v) {
    uint64_t h = key_hash(k, m->khash);
    int64_t slot = -1;
    int64_t ei = map_find(m, k, h, &slot);
    if (ei >= 0) {
        memcpy(m->entries[ei].v, v, (size_t)m->vsz);
        return;
    }
    if (m->idxcap == 0 || (m->len + 1) * 10 >= m->idxcap * 7) {
        int64_t nc = m->idxcap ? m->idxcap * 2 : 16;
        map_rehash(m, nc);
    }
    map_grow(m);
    void* kc = ax_alloc(m->ksz);
    void* vc = ax_alloc(m->vsz);
    memcpy(kc, k, (size_t)m->ksz);
    memcpy(vc, v, (size_t)m->vsz);
    m->entries[m->len].hash = h;
    m->entries[m->len].k = kc;
    m->entries[m->len].v = vc;
    m->len++;
    map_rehash(m, m->idxcap);
}

void ax_map_remove(ax_map* m, const void* k) {
    uint64_t h = key_hash(k, m->khash);
    int64_t ei = map_find(m, k, h, NULL);
    if (ei < 0) return;
    memmove(&m->entries[ei], &m->entries[ei + 1],
            (size_t)(m->len - ei - 1) * sizeof(ax_mentry));
    m->len--;
    map_rehash(m, m->idxcap);
}

void ax_map_clear(ax_map* m) {
    m->len = 0;
    if (m->idxcap) memset(m->idx, 0, (size_t)m->idxcap * sizeof(int32_t));
}

/* ================================================================ 装箱 */
ax_box* ax_box_new(uint32_t tag, const void* payload, int64_t size) {
    ax_box* b = (ax_box*)ax_alloc((int64_t)sizeof(ax_box));
    b->tag = tag;
    b->size = size;
    b->payload = ax_alloc(size);
    if (size > 0 && payload) memcpy(b->payload, payload, (size_t)size);
    return b;
}

static void box_check(ax_box* b, uint32_t want, const char* f, int l, int c) {
    if (!b || b->tag != want) {
        ax_trap(R_CONVERT, f, l, c, "unbox tag mismatch: box=%u want=%u",
                b ? b->tag : 0u, want);
    }
}

int64_t ax_unbox_int(ax_box* b, const char* f, int l, int c) {
    box_check(b, AX_TAG_INT, f, l, c);
    int64_t v; memcpy(&v, b->payload, 8); return v;
}

double ax_unbox_f64(ax_box* b, const char* f, int l, int c) {
    box_check(b, AX_TAG_FLOAT, f, l, c);
    double v; memcpy(&v, b->payload, 8); return v;
}

ax_dec ax_unbox_dec(ax_box* b, const char* f, int l, int c) {
    box_check(b, AX_TAG_DEC, f, l, c);
    ax_dec v; memcpy(&v, b->payload, sizeof v); return v;
}

ax_str ax_unbox_str(ax_box* b, const char* f, int l, int c) {
    box_check(b, AX_TAG_STRING, f, l, c);
    ax_str v; memcpy(&v, b->payload, sizeof v); return v;
}

void* ax_unbox_ref(ax_box* b, uint32_t want, const char* f, int l, int c) {
    box_check(b, want, f, l, c);
    void* p; memcpy(&p, b->payload, sizeof p); return p;
}

int ax_unbox_bool(ax_box* b, const char* f, int l, int c) {
    box_check(b, AX_TAG_BOOL, f, l, c);
    uint8_t v; memcpy(&v, b->payload, 1); return v;
}

/* ================================================================ 整数 */
int64_t ax_add_i64(int64_t a, int64_t b, const char* f, int l, int c) {
    int64_t r;
    if (__builtin_add_overflow(a, b, &r))
        ax_trap(R_OVERFLOW, f, l, c, "%" PRId64 " + %" PRId64, a, b);
    return r;
}

int64_t ax_sub_i64(int64_t a, int64_t b, const char* f, int l, int c) {
    int64_t r;
    if (__builtin_sub_overflow(a, b, &r))
        ax_trap(R_OVERFLOW, f, l, c, "%" PRId64 " - %" PRId64, a, b);
    return r;
}

int64_t ax_mul_i64(int64_t a, int64_t b, const char* f, int l, int c) {
    int64_t r;
    if (__builtin_mul_overflow(a, b, &r))
        ax_trap(R_OVERFLOW, f, l, c, "%" PRId64 " * %" PRId64, a, b);
    return r;
}

int64_t ax_div_i64(int64_t a, int64_t b, const char* f, int l, int c) {
    if (b == 0) ax_trap(R_DIVZERO, f, l, c, "%" PRId64 " / 0", a);
    if (a == INT64_MIN && b == -1)
        ax_trap(R_OVERFLOW, f, l, c, "INT64_MIN / -1");
    return a / b;
}

int64_t ax_mod_i64(int64_t a, int64_t b, const char* f, int l, int c) {
    if (b == 0) ax_trap(R_DIVZERO, f, l, c, "%" PRId64 " %% 0", a);
    if (a == INT64_MIN && b == -1) return 0;
    return a % b;
}

int64_t ax_shl_i64(int64_t a, int64_t b, const char* f, int l, int c) {
    if (b < 0 || b >= 64) ax_trap(R_SHIFT, f, l, c, "shift %" PRId64, b);
    return (int64_t)((uint64_t)a << (unsigned)b);
}

int64_t ax_shr_i64(int64_t a, int64_t b, const char* f, int l, int c) {
    if (b < 0 || b >= 64) ax_trap(R_SHIFT, f, l, c, "shift %" PRId64, b);
    return a >> (unsigned)b;
}

int64_t ax_neg_i64(int64_t a, const char* f, int l, int c) {
    if (a == INT64_MIN) ax_trap(R_OVERFLOW, f, l, c, "-INT64_MIN");
    return -a;
}

int64_t ax_narrow(int64_t v, int bits, int is_signed, const char* f, int l, int c) {
    if (is_signed) {
        int64_t lo = -((int64_t)1 << (bits - 1));
        int64_t hi = ((int64_t)1 << (bits - 1)) - 1;
        if (v < lo || v > hi)
            ax_trap(R_CONVERT, f, l, c, "%" PRId64 " 超出 int%d 范围", v, bits);
    } else {
        /* bits==64 时 `1 << 64` 是 UB（x86 移位量取模 → 1），必须特判 */
        uint64_t hi = (bits >= 64) ? UINT64_MAX : (((uint64_t)1 << bits) - 1);
        if (v < 0 || (uint64_t)v > hi)
            ax_trap(R_CONVERT, f, l, c, "%" PRId64 " 超出 uint%d 范围", v, bits);
    }
    return v;
}

static void f_check(double r, const char* f, int l, int c) {
    if (!isfinite(r)) ax_trap(R_FLOAT, f, l, c, "float 结果不是有限值");
}

double ax_fadd(double a, double b) { double r = a + b; f_check(r, "<runtime>", 0, 0); return r; }
double ax_fsub(double a, double b) { double r = a - b; f_check(r, "<runtime>", 0, 0); return r; }
double ax_fmul(double a, double b) { double r = a * b; f_check(r, "<runtime>", 0, 0); return r; }

double ax_fdiv(double a, double b, const char* f, int l, int c) {
    double r = a / b;
    f_check(r, f, l, c);
    return r;
}

/* ================================================================ error */
ax_err ax_err_new(int32_t code, ax_str msg) {
    ax_err e; e.code = code; e.msg = msg; return e;
}

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
int ax_read_line(ax_str* out) {
    size_t cap = 128, len = 0;
    char* buf = (char*)ax_alloc((int64_t)cap);
    int ch;
    while ((ch = fgetc(stdin)) != EOF && ch != '\n') {
        if (len + 2 >= cap) {
            cap *= 2;
            char* nb = (char*)ax_alloc((int64_t)cap);
            memcpy(nb, buf, len);
            buf = nb;
        }
        buf[len++] = (char)ch;
    }
    if (ch == EOF && len == 0) return 1;
    if (len > 0 && buf[len-1] == '\r') len--;
    *out = ax_str_from_bytes((const uint8_t*)buf, (int64_t)len);
    return 0;
}

int ax_read_file(ax_str path, ax_str* out) {
    char name[1024];
    int64_t n = path.len < 1023 ? path.len : 1023;
    memcpy(name, path.p, (size_t)n);
    name[n] = 0;
    FILE* fp = fopen(name, "rb");
    if (!fp) return 1;
    fseek(fp, 0, SEEK_END);
    long sz = ftell(fp);
    fseek(fp, 0, SEEK_SET);
    if (sz < 0) { fclose(fp); return 1; }
    uint8_t* buf = (uint8_t*)ax_alloc((int64_t)sz + 1);
    size_t rd = fread(buf, 1, (size_t)sz, fp);
    fclose(fp);
    *out = ax_str_from_bytes(buf, (int64_t)rd);
    return 0;
}

int ax_parse_i64(ax_str s, int64_t* out) {
    char buf[64];
    if (s.len <= 0 || s.len >= 63) return 0;
    memcpy(buf, s.p, (size_t)s.len);
    buf[s.len] = 0;
    char* end = NULL;
    long long v = strtoll(buf, &end, 10);
    if (!end || end == buf) return 0;
    while (*end == ' ' || *end == '\t') end++;
    if (*end != 0) return 0;
    *out = (int64_t)v;
    return 1;
}

int ax_parse_f64(ax_str s, double* out) {
    char buf[64];
    if (s.len <= 0 || s.len >= 63) return 0;
    memcpy(buf, s.p, (size_t)s.len);
    buf[s.len] = 0;
    char* end = NULL;
    double v = strtod(buf, &end);
    if (!end || end == buf) return 0;
    while (*end == ' ' || *end == '\t') end++;
    if (*end != 0) return 0;
    *out = v;
    return 1;
}

int64_t str_to_i64(ax_str s) {
    int64_t v = 0;
    if (!ax_parse_i64(s, &v)) return 0;
    return v;
}

/* ---------------- 文件（Rt-13） ---------------- */

/* ax_str → 以 NUL 结尾的栈缓冲；路径过长返回 0（与 ax_read_file 同一上限）。 */
static int ax_path_cstr(ax_str path, char* out, int cap) {
    if (path.len >= (int64_t)cap) return 0;
    memcpy(out, path.p, (size_t)path.len);
    out[path.len] = 0;
    return 1;
}

ax_res_file ax_file_open_result(ax_str path, int mode) {
    ax_res_file r;
    char name[1024];
    /* 一律二进制模式：Windows 文本模式会把 '\n' 改写成 CRLF，导致同一份代码
     * 在 Linux 与 Windows 上写出的字节不同、读回的字符串长度也不同。 */
    const char* m = mode == 1 ? "wb" : (mode == 2 ? "ab" : "rb");
    FILE* fp = NULL;
    if (ax_path_cstr(path, name, (int)sizeof name)) fp = fopen(name, m);
    if (!fp) {
        r.tag = 0;
        r.u.err = ax_err_new(4, ax_str_from_cstr("cannot open file"));
        return r;
    }
    ax_file* f = (ax_file*)ax_alloc((int64_t)sizeof(ax_file));
    f->fp = fp;
    r.tag = 1;
    r.u.ok = f;
    return r;
}

int ax_file_read_all(ax_file* f, ax_str* out) {
    if (!f || !f->fp) return 1;
    ax_sb b;
    ax_sb_init(&b);
    char buf[4096];
    size_t n;
    while ((n = fread(buf, 1, sizeof buf, f->fp)) > 0) {
        ax_sb_push(&b, ax_str_from_bytes((const uint8_t*)buf, (int64_t)n));
    }
    if (ferror(f->fp)) return 1;
    *out = ax_sb_finish(&b);
    return 0;
}

int ax_file_read_line(ax_file* f, ax_str* out) {
    if (!f || !f->fp) return 1;
    ax_sb b;
    ax_sb_init(&b);
    int ch;
    int any = 0;
    while ((ch = fgetc(f->fp)) != EOF && ch != '\n') {
        any = 1;
        uint8_t c = (uint8_t)ch;
        ax_sb_push(&b, ax_str_from_bytes(&c, 1));
    }
    if (ch == EOF && !any) return 1;
    *out = ax_sb_finish(&b);
    /* 去掉 CRLF 的 '\r'：与 ax_read_line 同语义（文稿 11.3） */
    if (out->len > 0 && out->p[out->len - 1] == '\r') {
        *out = ax_str_from_bytes(out->p, out->len - 1);
    }
    return 0;
}

int64_t ax_file_write(ax_file* f, ax_str s) {
    if (!f || !f->fp) return -1;
    if (s.len == 0) return 0;
    size_t n = fwrite(s.p, 1, (size_t)s.len, f->fp);
    if (n != (size_t)s.len) return -1;
    return (int64_t)n;
}

int64_t ax_file_write_line(ax_file* f, ax_str s) {
    int64_t n = ax_file_write(f, s);
    if (n < 0) return -1;
    if (fputc('\n', f->fp) == EOF) return -1;
    return n + 1;
}

void ax_file_close(ax_file* f) {
    if (f && f->fp) {
        fclose(f->fp);
        f->fp = NULL;
    }
}
