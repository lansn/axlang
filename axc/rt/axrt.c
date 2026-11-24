/* axrt.c -- axlang 运行时实现 */
#include "axrt.h"
#include <stdarg.h>
#include <math.h>
#include <inttypes.h>
#include <time.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <sys/stat.h>   /* stat/_stat, S_ISDIR/S_ISREG/_S_IFDIR/_S_IFREG（标准 C 库头，非编译器扩展） */

#ifdef _WIN32
#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0600
#endif
#include <windows.h>
#include <io.h>
#include <direct.h>
#include <process.h>   /* _environ */
#include <sys/utime.h> /* _utimbuf, _utime */
#else
#include <sys/time.h>
#include <pthread.h>
#include <unistd.h>    /* getpid/getppid/gethostname/environ */
#endif

/* ================================================================
 * 标准 C 128 位有符号整数 ax_i128（替代 GCC ax_i128，便于跨平台）
 * 布局：值 = hi * 2^64 + lo，hi 为 int64_t（带符号），lo 为 uint64_t。
 * 所有运算均在标准 C11 中实现，无任何编译器扩展。
 * ================================================================ */

#define AX_I64_HIGH_BIT ((uint64_t)1 << 63)

static ax_i128 i128_make(int64_t hi, uint64_t lo) {
    ax_i128 r; r.hi = hi; r.lo = lo; return r;
}

static ax_i128 i128_from_i64(int64_t v) {
    if (v >= 0) return i128_make(0, (uint64_t)v);
    return i128_make(-1, (uint64_t)v);
}

static int i128_to_i64(ax_i128 v, int64_t* out) {
    if (v.hi == 0) {
        if (v.lo <= (uint64_t)INT64_MAX) { *out = (int64_t)v.lo; return 1; }
        return 0;
    }
    if (v.hi == -1 && v.lo >= AX_I64_HIGH_BIT) {
        *out = (int64_t)(v.lo | AX_I64_HIGH_BIT);
        return 1;
    }
    return 0;
}

static int i128_cmp(ax_i128 a, ax_i128 b) {
    if (a.hi != b.hi) return a.hi < b.hi ? -1 : 1;
    if (a.lo < b.lo) return -1;
    if (a.lo > b.lo) return 1;
    return 0;
}

static ax_i128 i128_neg(ax_i128 v) {
    uint64_t lo_inv = ~v.lo;
    int64_t  hi_inv = ~v.hi;
    uint64_t lo_new = lo_inv + 1;
    int64_t  hi_new = hi_inv + (lo_new == 0 ? 1 : 0);
    return i128_make(hi_new, lo_new);
}

static ax_i128 i128_abs(ax_i128 v) {
    if (v.hi < 0) return i128_neg(v);
    return v;
}

static int i128_add(ax_i128 a, ax_i128 b, ax_i128* out) {
    uint64_t alo = a.lo, blo = b.lo;
    uint64_t lores = alo + blo;
    int carry = lores < alo ? 1 : 0;

    uint64_t ahi = (uint64_t)a.hi, bhi = (uint64_t)b.hi;
    uint64_t hires = ahi + bhi + (uint64_t)carry;

    int a_neg = a.hi < 0;
    int b_neg = b.hi < 0;
    int res_neg = (int64_t)hires < 0;
    int ovf = (a_neg == b_neg) && (res_neg != a_neg);

    *out = i128_make((int64_t)hires, lores);
    return ovf;
}

static int i128_sub(ax_i128 a, ax_i128 b, ax_i128* out) {
    uint64_t alo = a.lo, blo = b.lo;
    uint64_t lores = alo - blo;
    int borrow = lores > alo ? 1 : 0;

    uint64_t ahi = (uint64_t)a.hi, bhi = (uint64_t)b.hi;
    uint64_t hires = ahi - bhi - (uint64_t)borrow;

    int a_neg = a.hi < 0;
    int b_neg = b.hi < 0;
    int res_neg = (int64_t)hires < 0;
    int ovf = (a_neg != b_neg) && (res_neg == b_neg);

    *out = i128_make((int64_t)hires, lores);
    return ovf;
}

static void u128_mul64(uint64_t a, uint64_t b, uint64_t* hi, uint64_t* lo) {
    uint64_t a0 = a & 0xFFFFFFFFu, a1 = a >> 32;
    uint64_t b0 = b & 0xFFFFFFFFu, b1 = b >> 32;
    uint64_t p0 = a0 * b0;
    uint64_t p1 = a1 * b0;
    uint64_t p2 = a0 * b1;
    uint64_t p3 = a1 * b1;
    uint64_t mid = (p0 >> 32) + (p1 & 0xFFFFFFFFu) + (p2 & 0xFFFFFFFFu);
    *lo = p0 + (mid << 32);
    uint64_t c = (mid >> 32) + (p1 >> 32) + (p2 >> 32) + p3;
    if (*lo < p0) c++;
    *hi = c;
}

static int i128_mul(ax_i128 a, ax_i128 b, ax_i128* out) {
    int a_neg = a.hi < 0;
    int b_neg = b.hi < 0;
    ax_i128 aa = a_neg ? i128_neg(a) : a;
    ax_i128 bb = b_neg ? i128_neg(b) : b;

    uint64_t a_hi = (uint64_t)aa.hi, a_lo = aa.lo;
    uint64_t b_hi = (uint64_t)bb.hi, b_lo = bb.lo;

    uint64_t p0_hi, p0_lo, p1_hi, p1_lo, p2_hi, p2_lo, p3_hi, p3_lo;
    u128_mul64(a_lo, b_lo, &p0_hi, &p0_lo);
    u128_mul64(a_hi, b_lo, &p1_hi, &p1_lo);
    u128_mul64(a_lo, b_hi, &p2_hi, &p2_lo);
    u128_mul64(a_hi, b_hi, &p3_hi, &p3_lo);

    uint64_t r_lo = p0_lo;
    uint64_t r_m1 = p0_hi + p1_lo + p2_lo;
    int c1 = (r_m1 < p0_hi) || ((r_m1 == p0_hi) && ((p1_lo < p0_hi) || (p2_lo < p0_hi)));
    uint64_t r_m2 = p1_hi + p2_hi + p3_lo + (uint64_t)c1;
    int c2 = (r_m2 < p1_hi) || ((r_m2 == p1_hi) && ((p2_hi < p1_hi) || (p3_lo < p1_hi)));
    uint64_t r_hi = p3_hi + (uint64_t)c2;

    int res_neg = a_neg != b_neg;
    int ovf = 0;
    if (res_neg) {
        if (r_hi != 0xFFFFFFFFFFFFFFFFu) ovf = 1;
    } else {
        if (r_hi != 0) ovf = 1;
    }

    ax_i128 res = i128_make((int64_t)r_hi, r_lo);
    if (res_neg) res = i128_neg(res);
    *out = res;
    return ovf;
}

static ax_i128 i128_pow10(int n) {
    ax_i128 r = i128_from_i64(1);
    for (int i = 0; i < n; i++) {
        ax_i128 t;
        if (i128_mul(r, i128_from_i64(10), &t)) break;
        r = t;
    }
    return r;
}

/* 128 位有符号除法：num / den -> 商、余数（den != 0，商向零截断） */
static void i128_divmod(ax_i128 num, ax_i128 den, ax_i128* q, ax_i128* r) {
    int neg = (num.hi < 0) != (den.hi < 0);
    ax_i128 n = i128_abs(num);
    ax_i128 d = i128_abs(den);
    ax_i128 quotient = i128_from_i64(0);
    ax_i128 remainder = i128_from_i64(0);
    for (int i = 127; i >= 0; i--) {
        /* 恢复除法：商先左移一位（乘 2），余数也先左移一位并加入当前位 */
        ax_i128 q2;
        if (i128_add(quotient, quotient, &q2)) q2 = quotient;
        quotient = q2;

        ax_i128 doubled;
        if (i128_add(remainder, remainder, &doubled)) doubled = remainder;
        int bit;
        if (i < 64) bit = (int)((n.lo >> i) & 1u);
        else        bit = (int)((n.hi >> (i - 64)) & 1);
        if (bit) {
            ax_i128 tmp;
            if (i128_add(doubled, i128_from_i64(1), &tmp)) tmp = doubled;
            doubled = tmp;
        }
        if (i128_cmp(doubled, d) >= 0) {
            ax_i128 subed;
            if (i128_sub(doubled, d, &subed)) subed = doubled;
            remainder = subed;
            ax_i128 one = i128_from_i64(1);
            if (i128_add(quotient, one, &q2)) q2 = quotient;
            quotient = q2;
        } else {
            remainder = doubled;
        }
    }
    if (neg) quotient = i128_neg(quotient);
    if (num.hi < 0) remainder = i128_neg(remainder);
    *q = quotient;
    *r = remainder;
}

static ax_i128 i128_gcd(ax_i128 a, ax_i128 b) {
    a = i128_abs(a); b = i128_abs(b);
    while (i128_cmp(b, i128_from_i64(0)) != 0) {
        ax_i128 q, r;
        i128_divmod(a, b, &q, &r);
        a = b;
        b = r;
    }
    return a;
}

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
ax_str ax_str_trim_prefix(ax_str s, ax_str prefix) {
    if (prefix.len == 0) return ax_str_from_bytes(s.p, s.len);
    if (prefix.len > s.len) return ax_str_from_bytes(s.p, s.len);
    if (memcmp(s.p, prefix.p, (size_t)prefix.len) != 0) return ax_str_from_bytes(s.p, s.len);
    return ax_str_from_bytes(s.p + prefix.len, s.len - prefix.len);
}

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
int ax_math_isnan(double x) {
    return isnan(x) ? 1 : 0;
}

/* math.IsInf：C11 isinf 宏。sign>0 只判 +Inf，sign<0 只判 -Inf，sign==0 判任意无穷 */
int ax_math_isinf(double x, int sign) {
    if (!isinf(x)) return 0;
    if (sign > 0) return x > 0.0 ? 1 : 0;
    if (sign < 0) return x < 0.0 ? 1 : 0;
    return 1;
}

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
ax_arr ax_bytes_to_upper(ax_arr s) {
    const uint8_t* sp = (const uint8_t*)s.data;
    uint8_t* buf = (uint8_t*)ax_alloc(s.len <= 0 ? 1 : s.len);
    for (int64_t i = 0; i < s.len; i++) buf[i] = u8_to_ascii_upper(sp[i]);
    ax_arr out; ax_arr_init(&out);
    out.data = buf; out.len = s.len; out.cap = s.len;
    return out;
}

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

ax_dec ax_dec_from_i64(int64_t v) {
    ax_dec d; d.coeff = i128_from_i64(v); d.scale = 0; d._pad = 0;
    return d;
}

ax_dec ax_dec_lit(const char* digits, int32_t scale) {
    int neg = 0;
    const char* p = digits;
    if (*p == '-') { neg = 1; p++; }
    ax_i128 c = i128_from_i64(0);
    ax_i128 ten = i128_from_i64(10);
    while (*p) {
        if (*p >= '0' && *p <= '9') {
            ax_i128 t;
            i128_mul(c, ten, &t);
            ax_i128 d;
            i128_add(t, i128_from_i64(*p - '0'), &d);
            c = d;
        }
        p++;
    }
    ax_dec d; d.coeff = neg ? i128_neg(c) : c; d.scale = scale; d._pad = 0;
    return d;
}

ax_dec ax_dec_from_parts(ax_i128 coeff, int32_t scale) {
    ax_dec d; d.coeff = coeff; d.scale = scale; d._pad = 0;
    return d;
}

static int i128_is_zero(ax_i128 v) { return v.hi == 0 && v.lo == 0; }

/* 将 ax_i128 除以 uint64_t（除数 > 0），返回商和余数。 */
static void i128_div_u64(ax_i128 v, uint64_t d, ax_i128* q, uint64_t* r) {
    ax_i128 quotient = i128_from_i64(0);
    uint64_t rem = 0;
    if (v.hi < 0) {
        ax_i128 av = i128_neg(v);
        i128_div_u64(av, d, q, r);
        *q = i128_neg(*q);
        if (*r != 0) { *r = d - *r; ax_i128 one = i128_from_i64(1); ax_i128 q2; i128_add(*q, one, &q2); *q = q2; }
        return;
    }
    uint64_t hi = (uint64_t)v.hi;
    uint64_t lo = v.lo;
    for (int i = 63; i >= 0; i--) {
        rem = (rem << 1) | ((hi >> i) & 1u);
        if (rem >= d) { rem -= d; quotient.lo |= (1uLL << i); }
    }
    for (int i = 63; i >= 0; i--) {
        rem = (rem << 1) | ((lo >> i) & 1u);
        if (rem >= d) { rem -= d; quotient.lo |= (1uLL << i); }
    }
    quotient.hi = 0;
    *q = quotient;
    *r = rem;
}

static ax_dec dec_norm(ax_dec v) {
    while (v.scale > 0 && !i128_is_zero(v.coeff)) {
        ax_i128 qr_q; uint64_t qr_r;
        i128_div_u64(v.coeff, 10, &qr_q, &qr_r);
        if (qr_r != 0) break;
        v.coeff = qr_q;
        v.scale--;
    }
    if (i128_is_zero(v.coeff)) v.scale = 0;
    return v;
}

static void dec_align(ax_dec a, ax_dec b, ax_i128* ac, ax_i128* bc, int32_t* sc,
                      const char* f, int l, int c) {
    int32_t s = a.scale > b.scale ? a.scale : b.scale;
    ax_i128 x, y;
    ax_i128 pa = i128_pow10(s - a.scale);
    ax_i128 pb = i128_pow10(s - b.scale);
    if (i128_mul(a.coeff, pa, &x)) ax_trap(R_OVERFLOW, f, l, c, "decimal add overflow");
    if (i128_mul(b.coeff, pb, &y)) ax_trap(R_OVERFLOW, f, l, c, "decimal add overflow");
    *ac = x; *bc = y; *sc = s;
}

ax_dec ax_dec_add(ax_dec a, ax_dec b) {
    ax_i128 x, y, r;
    int32_t s;
    dec_align(a, b, &x, &y, &s, "<runtime>", 0, 0);
    if (i128_add(x, y, &r)) ax_trap(R_OVERFLOW, "<runtime>", 0, 0, "decimal overflow");
    return dec_norm(ax_dec_from_parts(r, s));
}

ax_dec ax_dec_sub(ax_dec a, ax_dec b) {
    ax_i128 x, y, r;
    int32_t s;
    dec_align(a, b, &x, &y, &s, "<runtime>", 0, 0);
    if (i128_sub(x, y, &r)) ax_trap(R_OVERFLOW, "<runtime>", 0, 0, "decimal overflow");
    return dec_norm(ax_dec_from_parts(r, s));
}

ax_dec ax_dec_mul(ax_dec a, ax_dec b) {
    ax_i128 r;
    if (i128_mul(a.coeff, b.coeff, &r)) ax_trap(R_OVERFLOW, "<runtime>", 0, 0, "decimal overflow");
    return dec_norm(ax_dec_from_parts(r, a.scale + b.scale));
}

ax_dec ax_dec_div_exact(ax_dec a, ax_dec b, const char* f, int l, int c) {
    if (i128_is_zero(b.coeff)) ax_trap(R_DIVZERO, f, l, c, "decimal division by zero");
    ax_i128 g = i128_gcd(a.coeff, b.coeff);
    if (i128_is_zero(g)) g = i128_from_i64(1);
    ax_i128 num, den0;
    i128_divmod(a.coeff, g, &num, &(ax_i128){0});
    i128_divmod(b.coeff, g, &den0, &(ax_i128){0});
    ax_i128 zero = i128_from_i64(0);
    if (i128_cmp(den0, zero) < 0) { den0 = i128_neg(den0); num = i128_neg(num); }
    int p = 0, q = 0;
    ax_i128 d = den0;
    ax_i128 one = i128_from_i64(1);
    while (1) {
        ax_i128 qr_q; uint64_t qr_r;
        i128_div_u64(d, 2, &qr_q, &qr_r);
        if (qr_r != 0) break;
        d = qr_q; p++;
    }
    while (1) {
        ax_i128 qr_q; uint64_t qr_r;
        i128_div_u64(d, 5, &qr_q, &qr_r);
        if (qr_r != 0) break;
        d = qr_q; q++;
    }
    if (i128_cmp(d, one) != 0) {
        ax_trap(R_INEXACT, f, l, c,
                "除不尽（建议用 divide(a, b, digits, Rounding.HalfUp)）");
    }
    int m = p > q ? p : q;
    ax_i128 pm = i128_pow10(m);
    ax_i128 coeff;
    i128_mul(num, pm, &coeff);
    ax_i128 dq, dr;
    i128_divmod(coeff, den0, &dq, &dr);
    coeff = dq;
    int32_t scale = (int32_t)(m - (b.scale - a.scale));
    if (scale < 0) {
        ax_i128 pn = i128_pow10(-scale);
        ax_i128 t;
        if (i128_mul(coeff, pn, &t)) ax_trap(R_OVERFLOW, f, l, c, "decimal overflow");
        coeff = t;
        scale = 0;
    }
    return dec_norm(ax_dec_from_parts(coeff, scale));
}

static ax_i128 div_round_i128(ax_i128 num, ax_i128 den, int mode) {
    if (i128_is_zero(den)) return i128_from_i64(0);
    ax_i128 zero = i128_from_i64(0);
    int neg = (i128_cmp(num, zero) < 0) != (i128_cmp(den, zero) < 0);
    ax_i128 n = i128_abs(num), d = i128_abs(den);
    ax_i128 q, r;
    i128_divmod(n, d, &q, &r);
    if (!i128_is_zero(r)) {
        if (mode == ROUND_DOWN) {
            /* 向零截断 */
        } else if (mode == ROUND_HALF_EVEN) {
            ax_i128 twice;
            i128_add(r, r, &twice);
            ax_i128 qr_q; uint64_t qr_r;
            i128_div_u64(q, 2, &qr_q, &qr_r);
            int q_odd = (qr_r != 0);
            if (i128_cmp(twice, d) > 0 || (i128_cmp(twice, d) == 0 && q_odd)) {
                ax_i128 one = i128_from_i64(1);
                i128_add(q, one, &q);
            }
        } else {
            ax_i128 twice;
            i128_add(r, r, &twice);
            if (i128_cmp(twice, d) >= 0) {
                ax_i128 one = i128_from_i64(1);
                i128_add(q, one, &q);
            }
        }
    }
    return neg ? i128_neg(q) : q;
}

ax_dec ax_dec_div_round(ax_dec a, ax_dec b, int32_t digits, int mode) {
    if (i128_is_zero(b.coeff)) ax_trap(R_DIVZERO, "<runtime>", 0, 0, "decimal division by zero");
    int k = digits + b.scale - a.scale;
    ax_i128 num = a.coeff, den = b.coeff;
    ax_i128 t;
    if (k > 0) {
        ax_i128 pk = i128_pow10(k);
        if (i128_mul(num, pk, &t)) ax_trap(R_OVERFLOW, "<runtime>", 0, 0, "decimal overflow");
        num = t;
    } else if (k < 0) {
        ax_i128 pk = i128_pow10(-k);
        if (i128_mul(den, pk, &t)) ax_trap(R_OVERFLOW, "<runtime>", 0, 0, "decimal overflow");
        den = t;
    }
    ax_i128 q = div_round_i128(num, den, mode);
    return dec_norm(ax_dec_from_parts(q, digits));
}

ax_dec ax_dec_rem(ax_dec a, ax_dec b) {
    if (i128_is_zero(b.coeff)) ax_trap(R_DIVZERO, "<runtime>", 0, 0, "decimal remainder by zero");
    ax_i128 x, y, r;
    int32_t s;
    dec_align(a, b, &x, &y, &s, "<runtime>", 0, 0);
    ax_i128 dq;
    i128_divmod(x, y, &dq, &r);
    return dec_norm(ax_dec_from_parts(r, s));
}

ax_dec ax_dec_round(ax_dec a, int32_t digits, int mode) {
    if (a.scale <= digits) return a;
    int drop = a.scale - digits;
    ax_i128 den = i128_pow10(drop);
    ax_i128 q = div_round_i128(a.coeff, den, mode);
    return dec_norm(ax_dec_from_parts(q, digits));
}

int ax_dec_cmp(ax_dec a, ax_dec b) {
    ax_i128 x, y;
    int32_t s;
    dec_align(a, b, &x, &y, &s, "<runtime>", 0, 0);
    if (i128_cmp(x, y) < 0) return -1;
    if (i128_cmp(x, y) > 0) return 1;
    return 0;
}

int64_t ax_dec_to_i64(ax_dec a, const char* f, int l, int c) {
    ax_i128 den = i128_pow10(a.scale);
    ax_i128 v;
    i128_divmod(a.coeff, den, &v, &(ax_i128){0});
    ax_i128 max_i64 = i128_from_i64(INT64_MAX);
    ax_i128 min_i64 = i128_from_i64(INT64_MIN);
    if (i128_cmp(v, max_i64) > 0 || i128_cmp(v, min_i64) < 0)
        ax_trap(R_CONVERT, f, l, c, "decimal %s 超出 int 范围", "value");
    int64_t out;
    if (!i128_to_i64(v, &out)) ax_trap(R_CONVERT, f, l, c, "decimal %s 超出 int 范围", "value");
    return out;
}

static double i128_to_double(ax_i128 v);

double ax_dec_to_f64(ax_dec v) {
    return i128_to_double(v.coeff) / pow(10.0, (double)v.scale);
}

static double i128_to_double(ax_i128 v) {
    /* value = hi * 2^64 + lo；2^64 为 2 的幂，乘法在 double 中精确 */
    return (double)v.hi * 18446744073709551616.0 + (double)v.lo;
}

double ax_dec_as_double(ax_dec v) { return ax_dec_to_f64(v); }

ax_str ax_dec_to_str(ax_dec v) {
    ax_dec d = dec_norm(v);
    ax_i128 zero = i128_from_i64(0);
    int neg = i128_cmp(d.coeff, zero) < 0;
    ax_i128 a = neg ? i128_neg(d.coeff) : d.coeff;
    char digits[64];
    int n = 0;
    if (i128_is_zero(a)) digits[n++] = '0';
    while (i128_cmp(a, zero) > 0 && n < 60) {
        ax_i128 qr_q; uint64_t qr_r;
        i128_div_u64(a, 10, &qr_q, &qr_r);
        digits[n++] = (char)('0' + (int)qr_r);
        a = qr_q;
    }
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
    ax_i128 coeff = i128_from_i64(0);
    ax_i128 ten = i128_from_i64(10);
    int digits = 0, scale = 0, seen = 0;
    while (i < s.len && s.p[i] >= '0' && s.p[i] <= '9') {
        ax_i128 t1, t2;
        i128_mul(coeff, ten, &t1);
        i128_add(t1, i128_from_i64(s.p[i] - '0'), &t2);
        coeff = t2;
        digits++; i++; seen = 1;
        if (digits > 30) return ax_dec_from_i64(0);
    }
    if (i < s.len && s.p[i] == '.') {
        i++;
        while (i < s.len && s.p[i] >= '0' && s.p[i] <= '9') {
            ax_i128 t1, t2;
            i128_mul(coeff, ten, &t1);
            i128_add(t1, i128_from_i64(s.p[i] - '0'), &t2);
            coeff = t2;
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
        ax_i128 pn = i128_pow10(-scale);
        ax_i128 t;
        i128_mul(coeff, pn, &t);
        coeff = t;
        scale = 0;
    }
    *ok = 1;
    ax_i128 final_coeff = neg ? i128_neg(coeff) : coeff;
    return dec_norm(ax_dec_from_parts(final_coeff, scale));
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

int ax_arr_eq(const ax_arr* a, const ax_arr* b, int64_t esz) {
    if (a->len != b->len) return 0;
    if (a->len == 0) return 1;
    if (a->data == NULL || b->data == NULL) return a->data == b->data;
    for (int64_t i = 0; i < a->len; i++) {
        const void* pa = (const uint8_t*)a->data + i * esz;
        const void* pb = (const uint8_t*)b->data + i * esz;
        if (memcmp(pa, pb, (size_t)esz) != 0) return 0;
    }
    return 1;
}

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
    case 3: { ax_dec d; memcpy(&d, k, sizeof d); uint64_t x = d.coeff.lo ^ ((uint64_t)d.scale << 32); x ^= x >> 33; return x ? x : 1; }
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

ax_arr ax_map_keys(ax_map* m) {
    ax_arr keys;
    ax_arr_init(&keys);
    for (int64_t i = 0; i < m->len; i++) {
        ax_arr_push(&keys, m->entries[i].k, (int64_t)m->ksz);
    }
    return keys;
}

/* 返回所有值的副本组成的 ax_arr（按插入顺序，元素大小为 m->vsz） */
ax_arr ax_map_values(ax_map* m) {
    ax_arr vals;
    ax_arr_init(&vals);
    for (int64_t i = 0; i < m->len; i++) {
        ax_arr_push(&vals, m->entries[i].v, (int64_t)m->vsz);
    }
    return vals;
}

/* 值相等比较：string 用 ax_str_eq，decimal 用 ax_dec_cmp==0，其余按字节 */
static int map_value_eq(const void* a, const void* b, int64_t vsz) {
    if (vsz == (int64_t)sizeof(ax_str)) {
        ax_str x, y;
        memcpy(&x, a, sizeof x);
        memcpy(&y, b, sizeof y);
        return ax_str_eq(x, y);
    }
    if (vsz == (int64_t)sizeof(ax_dec)) {
        ax_dec x, y;
        memcpy(&x, a, sizeof x);
        memcpy(&y, b, sizeof y);
        return ax_dec_cmp(x, y) == 0;
    }
    return memcmp(a, b, (size_t)vsz) == 0;
}

/* 深拷贝 map：新建独立的 entries/idx，键值元素按 ax_map_put 浅拷贝，
 * 返回独立副本（对标 Go maps.Clone）。 */
ax_map ax_map_clone(const ax_map* m) {
    ax_map n;
    ax_map_init(&n, m->khash, m->ksz, m->vsz);
    for (int64_t i = 0; i < m->len; i++) {
        ax_map_put(&n, m->entries[i].k, m->entries[i].v);
    }
    return n;
}

/* 将 src 的全部键值复制到 dst（已存在键被覆盖），对标 Go maps.Copy。 */
void ax_map_copy_into(ax_map* dst, const ax_map* src) {
    for (int64_t i = 0; i < src->len; i++) {
        ax_map_put(dst, src->entries[i].k, src->entries[i].v);
    }
}

/* 判断两个 map 是否相等（同长度、同键且对应值相等），对标 Go maps.Equal。 */
int ax_map_equal(const ax_map* a, const ax_map* b) {
    if (a->len != b->len) return 0;
    if (a->khash != b->khash) return 0;
    for (int64_t i = 0; i < a->len; i++) {
        const void* k = a->entries[i].k;
        const void* v = a->entries[i].v;
        if (!ax_map_has(b, k)) return 0;
        void* bv = ax_map_get(b, k);
        if (!map_value_eq(v, bv, a->vsz)) return 0;
    }
    return 1;
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
static int i64_add_ovf(int64_t a, int64_t b, int64_t* out) {
    if (b > 0 && a > INT64_MAX - b) return 1;
    if (b < 0 && a < INT64_MIN - b) return 1;
    *out = a + b;
    return 0;
}

static int i64_sub_ovf(int64_t a, int64_t b, int64_t* out) {
    if (b < 0 && a > INT64_MAX + b) return 1;
    if (b > 0 && a < INT64_MIN + b) return 1;
    *out = a - b;
    return 0;
}

static int i64_mul_ovf(int64_t a, int64_t b, int64_t* out) {
    if (a == 0 || b == 0) { *out = 0; return 0; }
    if (a == INT64_MIN || b == INT64_MIN) return 1;
    int64_t abs_a = a < 0 ? -a : a;
    int64_t abs_b = b < 0 ? -b : b;
    if (abs_a > INT64_MAX / abs_b) return 1;
    *out = a * b;
    return 0;
}

int64_t ax_add_i64(int64_t a, int64_t b, const char* f, int l, int c) {
    int64_t r;
    if (i64_add_ovf(a, b, &r))
        ax_trap(R_OVERFLOW, f, l, c, "%" PRId64 " + %" PRId64, a, b);
    return r;
}

int64_t ax_sub_i64(int64_t a, int64_t b, const char* f, int l, int c) {
    int64_t r;
    if (i64_sub_ovf(a, b, &r))
        ax_trap(R_OVERFLOW, f, l, c, "%" PRId64 " - %" PRId64, a, b);
    return r;
}

int64_t ax_mul_i64(int64_t a, int64_t b, const char* f, int l, int c) {
    int64_t r;
    if (i64_mul_ovf(a, b, &r))
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

/* errors.Is(err, target)：对标 Go errors.Is。
 * axlang error 是扁平值（code+msg），无错误链，故判定两 error 相等即为 Is。 */
int ax_err_is(ax_err err, ax_err target) {
    if (err.code != target.code) return 0;
    return ax_str_eq(err.msg, target.msg);
}

/* errors.As(err, target)：对标 Go errors.As。
 * 将 err 复制到 target，返回 1。target 必须是 error 类型左值（由 codegen 保证）。 */
int ax_err_as(ax_err err, ax_err* target) {
    *target = err;
    return 1;
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

ax_str ax_time_to_str(int64_t ms) {
    return ax_time_format(ms, "%Y-%m-%d %H:%M:%S");
}

ax_str ax_time_format(int64_t ms, const char* fmt) {
    time_t sec = (time_t)(ms / 1000);
    struct tm tm;
#ifdef _WIN32
    localtime_s(&tm, &sec);
#else
    localtime_r(&sec, &tm);
#endif
    const char* f = fmt && *fmt ? fmt : "%Y-%m-%d %H:%M:%S";
    char buf[64];
    size_t n = strftime(buf, sizeof buf, f, &tm);
    return ax_str_from_bytes((const uint8_t*)buf, (int64_t)n);
}

void ax_time_sleep(int64_t ms) {
#ifdef _WIN32
    Sleep((DWORD)ms);
#else
    if (ms > 0) usleep((useconds_t)(ms * 1000));
#endif
}

/* time.Unix：sec 秒 + nsec 纳秒 -> Unix 毫秒 */
int64_t ax_time_from_unix(int64_t sec, int64_t nsec) {
    return sec * 1000 + nsec / 1000000;
}

/* time.UnixMilli：毫秒 -> Unix 毫秒（与 FromUnixMs 同义） */
int64_t ax_time_from_unix_ms(int64_t ms) {
    return ms;
}

/* time.UnixMicro：微秒 -> Unix 毫秒 */
int64_t ax_time_from_unix_us(int64_t us) {
    return us / 1000;
}

/* time.UnixNano：纳秒 -> Unix 毫秒 */
int64_t ax_time_from_unix_ns(int64_t nsec) {
    return nsec / 1000000;
}

/* time.Since：当前 Unix 毫秒 - t */
int64_t ax_time_since(int64_t t) {
    return ax_now_ms() - t;
}

/* time.Date：按 UTC 组装年月日时分秒纳秒 -> Unix 毫秒（标准 C11，纯手工计算，无时区/闰秒） */
static int ax_time_is_leap(int64_t y) {
    return (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0) ? 1 : 0;
}

int64_t ax_time_date(int64_t year, int64_t month, int64_t day,
                     int64_t hour, int64_t min, int64_t sec, int64_t nsec) {
    static const int64_t mdays[12] = {31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31};
    int64_t days = 0;
    int64_t y;
    for (y = 1970; y < year; y++) days += ax_time_is_leap(y) ? 366 : 365;
    for (int64_t m = 1; m < month; m++) {
        days += mdays[m - 1];
        if (m == 2 && ax_time_is_leap(year)) days += 1;
    }
    days += (day - 1);
    return days * 86400000 +
           (hour * 3600 + min * 60 + sec) * 1000 +
           nsec / 1000000;
}

/* time.Until：t - 当前 Unix 毫秒（t 在将来为正，已过去为负） */
int64_t ax_time_until(int64_t t) {
    return t - ax_now_ms();
}

ax_res_time ax_time_parse_result(ax_str s, ax_str fmt) {
    ax_res_time r;
    int year = 1970, month = 1, day = 1, hour = 0, min = 0, sec = 0;
    const char* sp = (const char*)s.p;
    const char* fp = (const char*)fmt.p;
    int64_t slen = s.len, flen = fmt.len;
    int64_t si = 0, fi = 0;
    int ok = 1;

    while (fi < flen && si < slen && ok) {
        if (fp[fi] == '%' && fi + 1 < flen) {
            char spec = fp[fi + 1];
            fi += 2;
            int val = 0;
            int digits = 0;
            int max_digits = (spec == 'Y') ? 4 : 2;
            while (si < slen && sp[si] >= (uint8_t)'0' && sp[si] <= (uint8_t)'9' && digits < max_digits) {
                val = val * 10 + (sp[si] - (uint8_t)'0');
                si++;
                digits++;
            }
            if (digits == 0) { ok = 0; break; }
            switch (spec) {
                case 'Y': year = val; break;
                case 'm': month = val; break;
                case 'd': day = val; break;
                case 'H': hour = val; break;
                case 'M': min = val; break;
                case 'S': sec = val; break;
                default: ok = 0; break;
            }
            /* 跳过格式串中紧跟的非占位符分隔符 */
            while (fi < flen && fp[fi] != (uint8_t)'%' && fp[fi] >= (uint8_t)'0' && fp[fi] <= (uint8_t)'9') fi++;
        } else {
            if (sp[si] == fp[fi]) { si++; fi++; }
            else { fi++; } /* 宽松匹配：格式字符跳过，不强制输入一一对应 */
        }
    }

    if (!ok || year < 1970 || month < 1 || month > 12 || day < 1 || day > 31 ||
        hour < 0 || hour > 23 || min < 0 || min > 59 || sec < 0 || sec > 59) {
        r.tag = 0;
        r.u.err = ax_err_new(1, AX_S("bad time format"));
        return r;
    }

    struct tm tm;
    memset(&tm, 0, sizeof(tm));
    tm.tm_year = year - 1900;
    tm.tm_mon = month - 1;
    tm.tm_mday = day;
    tm.tm_hour = hour;
    tm.tm_min = min;
    tm.tm_sec = sec;
    time_t t = mktime(&tm);
    if (t == (time_t)-1) {
        r.tag = 0;
        r.u.err = ax_err_new(1, AX_S("bad time format"));
        return r;
    }
    r.tag = 1;
    r.u.ok = (int64_t)t * 1000;
    return r;
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

int ax_parse_bool(ax_str s) {
    if (s.len == 4) {
        if (memcmp(s.p, "true", 4) == 0) return 1;
    } else if (s.len == 5) {
        if (memcmp(s.p, "false", 5) == 0) return 0;
    } else if (s.len == 1) {
        if (s.p[0] == (uint8_t)'1') return 1;
        if (s.p[0] == (uint8_t)'0') return 0;
    }
    return 0;
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

int ax_parse_int_base(ax_str s, int base) {
    char buf[64];
    if (s.len <= 0 || s.len >= 63) return 0;
    memcpy(buf, s.p, (size_t)s.len);
    buf[s.len] = 0;
    char* end = NULL;
    /* base<=0：按前缀自动判断（0x=16, 0=8, 否则=10），与 Go strconv 一致 */
    int b = base;
    if (b <= 0) {
        if (s.len >= 2 && buf[0] == '0' && (buf[1] == 'x' || buf[1] == 'X')) b = 16;
        else if (buf[0] == '0') b = 8;
        else b = 10;
    }
    long long v = strtoll(buf, &end, b);
    if (!end || end == buf) return 0;
    while (*end == ' ' || *end == '\t') end++;
    if (*end != 0) return 0;
    return (int)v;
}

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
static int ax_u8_is_printable(uint8_t c) {
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
static int ax_quote_escape(uint8_t c, char* out) {
    switch (c) {
        case (uint8_t)'\\':  out[0]='\\'; out[1]='\\'; return 2;
        case (uint8_t)'\"':  out[0]='\\'; out[1]='"';  return 2;
        case (uint8_t)'\a':  out[0]='\\'; out[1]='a';  return 2;
        case (uint8_t)'\b':  out[0]='\\'; out[1]='b';  return 2;
        case (uint8_t)'\f':  out[0]='\\'; out[1]='f';  return 2;
        case (uint8_t)'\n':  out[0]='\\'; out[1]='n';  return 2;
        case (uint8_t)'\r':  out[0]='\\'; out[1]='r';  return 2;
        case (uint8_t)'\t':  out[0]='\\'; out[1]='t';  return 2;
        case (uint8_t)'\v':  out[0]='\\'; out[1]='v';  return 2;
        default:
            if (!ax_u8_is_printable(c)) {
                /* 不可打印字符：使用 \xHH */
                out[0]='\\'; out[1]='x';
                static const char hex[] = "0123456789abcdef";
                out[2] = hex[(c >> 4) & 0xF];
                out[3] = hex[c & 0xF];
                return 4;
            }
            out[0] = (char)c;
            return 1;
    }
}

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
static int hexval(uint8_t c);

/* 解析一个转义序列，从 *p 指向反斜杠之后的字符开始。
 * 成功返回写入的字节数（1），并将 *p 前进；失败返回 -1。 */
static int ax_unquote_escape(const uint8_t** p, const uint8_t* end, uint8_t* out) {
    if (*p >= end) return -1;
    uint8_t c = **p;
    (*p)++;
    switch (c) {
        case (uint8_t)'\\': *out = '\\'; return 1;
        case (uint8_t)'"':  *out = '"';  return 1;
        case (uint8_t)'a':  *out = '\a'; return 1;
        case (uint8_t)'b':  *out = '\b'; return 1;
        case (uint8_t)'f':  *out = '\f'; return 1;
        case (uint8_t)'n':  *out = '\n'; return 1;
        case (uint8_t)'r':  *out = '\r'; return 1;
        case (uint8_t)'t':  *out = '\t'; return 1;
        case (uint8_t)'v':  *out = '\v'; return 1;
        case (uint8_t)'x': {
            /* \xHH：两个十六进制字符 */
            if (*p + 2 > end) return -1;
            uint8_t hi = **p; (*p)++;
            uint8_t lo = **p; (*p)++;
            int v1 = hexval(hi), v2 = hexval(lo);
            if (v1 < 0 || v2 < 0) return -1;
            *out = (uint8_t)((v1 << 4) | v2);
            return 1;
        }
        default:
            return -1;
    }
}

/* 十六进制字符转数值，失败返回 -1 */
static int hexval(uint8_t c) {
    if (c >= (uint8_t)'0' && c <= (uint8_t)'9') return (int)(c - '0');
    if (c >= (uint8_t)'a' && c <= (uint8_t)'f') return (int)(c - 'a' + 10);
    if (c >= (uint8_t)'A' && c <= (uint8_t)'F') return (int)(c - 'A' + 10);
    return -1;
}

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

int ax_file_remove(ax_str path) {
    /* ax_str 是 const uint8_t*，需要转为 const char* 传给 remove */
    return remove((const char*)path.p) == 0 ? 0 : 1;
}

int ax_file_rename(ax_str old_path, ax_str new_path) {
    return rename((const char*)old_path.p, (const char*)new_path.p) == 0 ? 0 : 1;
}

int ax_file_exists(ax_str path) {
    /* 用 fopen 尝试打开，成功则存在 */
    FILE* fp = fopen((const char*)path.p, "rb");
    if (fp) {
        fclose(fp);
        return 1;
    }
    return 0;
}

int ax_dir_create(ax_str path) {
#ifdef _WIN32
    return _mkdir((const char*)path.p) == 0 ? 0 : 1;
#else
    return mkdir((const char*)path.p, 0755) == 0 ? 0 : 1;
#endif
}

int ax_file_is_dir(ax_str path) {
    char name[1024];
    int64_t n = path.len < 1023 ? path.len : 1023;
    memcpy(name, path.p, (size_t)n);
    name[n] = 0;
#if defined(_WIN32)
    struct _stat st;
    if (_stat(name, &st) != 0) return 0;
    return (st.st_mode & _S_IFDIR) != 0 ? 1 : 0;
#else
    struct stat st;
    if (stat(name, &st) != 0) return 0;
    return S_ISDIR(st.st_mode) ? 1 : 0;
#endif
}

int ax_file_is_file(ax_str path) {
    char name[1024];
    int64_t n = path.len < 1023 ? path.len : 1023;
    memcpy(name, path.p, (size_t)n);
    name[n] = 0;
#if defined(_WIN32)
    struct _stat st;
    if (_stat(name, &st) != 0) return 0;
    return (st.st_mode & _S_IFREG) != 0 ? 1 : 0;
#else
    struct stat st;
    if (stat(name, &st) != 0) return 0;
    return S_ISREG(st.st_mode) ? 1 : 0;
#endif
}

/* 文件大小（字节），对标 Go os.Stat+FileInfo.Size；失败返回 -1。 */
int64_t ax_file_size(ax_str path) {
    char name[1024];
    int64_t n = path.len < 1023 ? path.len : 1023;
    memcpy(name, path.p, (size_t)n);
    name[n] = 0;
#if defined(_WIN32)
    struct _stat st;
    if (_stat(name, &st) != 0) return -1;
    return (int64_t)st.st_size;
#else
    struct stat st;
    if (stat(name, &st) != 0) return -1;
    return (int64_t)st.st_size;
#endif
}

/* 最后修改时间（Unix 毫秒），对标 Go os.Stat+FileInfo.ModTime；失败返回 -1。 */
int64_t ax_file_modtime(ax_str path) {
    char name[1024];
    int64_t n = path.len < 1023 ? path.len : 1023;
    memcpy(name, path.p, (size_t)n);
    name[n] = 0;
#if defined(_WIN32)
    struct _stat st;
    if (_stat(name, &st) != 0) return -1;
    return (int64_t)st.st_mtime * 1000;
#else
    struct stat st;
    if (stat(name, &st) != 0) return -1;
    return (int64_t)st.st_mtime * 1000;
#endif
}

/* 将整个字符串写入文件（创建/截断），0=成功 1=失败 */
int ax_file_write_all(ax_str path, ax_str s) {
    char name[1024];
    int64_t n = path.len < 1023 ? path.len : 1023;
    memcpy(name, path.p, (size_t)n);
    name[n] = 0;
    FILE* fp = fopen(name, "wb");
    if (!fp) return 1;
    if (s.len > 0 && fwrite(s.p, 1, (size_t)s.len, fp) != (size_t)s.len) {
        fclose(fp);
        return 1;
    }
    fclose(fp);
    return 0;
}

/* 修改文件权限，对标 Go os.Chmod / C chmod；0=成功 1=失败。 */
int ax_file_chmod(ax_str path, int mode) {
    char name[1024];
    int64_t n = path.len < 1023 ? path.len : 1023;
    memcpy(name, path.p, (size_t)n);
    name[n] = 0;
#if defined(_WIN32)
    return _chmod(name, mode) == 0 ? 0 : 1;
#else
    return chmod(name, (mode_t)mode) == 0 ? 0 : 1;
#endif
}

/* 修改访问/修改时间，对标 Go os.Chtimes；atime/mtime 为 Unix 毫秒；0=成功 1=失败。 */
int ax_file_chtimes(ax_str path, int64_t atime, int64_t mtime) {
    char name[1024];
    int64_t n = path.len < 1023 ? path.len : 1023;
    memcpy(name, path.p, (size_t)n);
    name[n] = 0;
#if defined(_WIN32)
    struct _utimbuf times;
    times.actime = (time_t)(atime / 1000);
    times.modtime = (time_t)(mtime / 1000);
    return _utime(name, &times) == 0 ? 0 : 1;
#else
    struct timeval times[2];
    times[0].tv_sec = (time_t)(atime / 1000);
    times[0].tv_usec = (long)((atime % 1000) * 1000);
    times[1].tv_sec = (time_t)(mtime / 1000);
    times[1].tv_usec = (long)((mtime % 1000) * 1000);
    return utimes(name, times) == 0 ? 0 : 1;
#endif
}

/* 创建临时文件，对标 Go os.CreateTemp。dir 为目录，prefix 为文件名前缀。 */
ax_res_file ax_file_tempfile(ax_str dir, ax_str prefix) {
    ax_res_file r;
    char name[1024];
    int64_t dn = dir.len < 1023 ? dir.len : 1023;
    memcpy(name, dir.p, (size_t)dn);
    name[dn] = 0;
    int len = (int)dn;
    if (len > 0 && name[len-1] != '/' && name[len-1] != '\\') {
#if defined(_WIN32)
        name[len++] = '\\';
#else
        name[len++] = '/';
#endif
    }
    static int counter = 0;
    counter++;
    int64_t now = ax_now_ms();
    int pn = prefix.len < 128 ? prefix.len : 128;
    memcpy(name + len, prefix.p, (size_t)pn);
    len += pn;
    len += snprintf(name + len, (size_t)(sizeof name - len), "%lld_%d.tmp",
                    (long long)now, counter);
    if (len < 0 || len >= (int)sizeof name) {
        r.tag = 0;
        r.u.err = ax_err_new(4, ax_str_from_cstr("tempfile name too long"));
        return r;
    }
    name[len] = 0;
    FILE* fp = fopen(name, "wb+");
    if (!fp) {
        r.tag = 0;
        r.u.err = ax_err_new(4, ax_str_from_cstr("cannot create temp file"));
        return r;
    }
    ax_file* f = (ax_file*)ax_alloc((int64_t)sizeof(ax_file));
    f->fp = fp;
    r.tag = 1;
    r.u.ok = f;
    return r;
}

/* ---------------- os 模块 ---------------- */

static int ax_os_path_cstr(ax_str s, char* out, int cap) {
    if (s.len >= (int64_t)cap) return 0;
    memcpy(out, s.p, (size_t)s.len);
    out[s.len] = 0;
    return 1;
}

ax_str ax_os_getenv(ax_str name) {
    char buf[256];
    if (!ax_os_path_cstr(name, buf, (int)sizeof buf)) return ax_str_empty();
    return ax_env_utf8(buf);
}

/* 用户主目录：Windows 读 USERPROFILE，Unix 读 HOME；未设置返回空串 */
ax_str ax_os_user_home_dir(void) {
#if defined(_WIN32)
    return ax_os_getenv(AX_S("USERPROFILE"));
#else
    return ax_os_getenv(AX_S("HOME"));
#endif
}

void ax_os_exit(int code) {
    exit(code);
}

/* 命令行参数全局（由 main 包装器在进入用户 main 前设置） */
static ax_arr g_os_args;
static int  g_os_args_set = 0;

void ax_os_set_args(ax_arr* args) {
    if (!g_os_args_set) {
        g_os_args = *args;      /* 浅拷贝：data 指针由 main 端的 ax_arr 管理 */
        g_os_args_set = 1;
    }
}

ax_arr ax_os_args(void) {
    return g_os_args;
}

ax_str ax_os_getwd(void) {
    char cwd[4096];
#if defined(_WIN32)
    if (!_getcwd(cwd, (int)sizeof cwd)) return ax_str_empty();
#else
    if (!getcwd(cwd, (size_t)sizeof cwd)) return ax_str_empty();
#endif
    return ax_str_from_cstr(cwd);
}

int ax_os_chdir(ax_str path) {
    char buf[4096];
    if (!ax_os_path_cstr(path, buf, (int)sizeof buf)) return 1;
#if defined(_WIN32)
    return _chdir(buf) == 0 ? 0 : 1;
#else
    return chdir(buf) == 0 ? 0 : 1;
#endif
}

/* ---------------- os 进程/环境扩展（对标 Go os） ---------------- */

int ax_os_getpid(void) {
#if defined(_WIN32)
    return (int)GetCurrentProcessId();
#else
    return (int)getpid();
#endif
}

int ax_os_getppid(void) {
#if defined(_WIN32)
    return 0;  /* Windows 暂不提供父进程 ID */
#else
    return (int)getppid();
#endif
}

ax_str ax_os_hostname(void) {
    char buf[256];
#if defined(_WIN32)
    DWORD n = (DWORD)sizeof buf;
    if (!GetComputerNameA(buf, &n)) return ax_str_empty();
    return ax_str_from_cstr(buf);
#else
    if (gethostname(buf, sizeof buf) != 0) return ax_str_empty();
    return ax_str_from_cstr(buf);
#endif
}

ax_arr ax_os_environ(void) {
    ax_arr arr;
    ax_arr_init(&arr);
#if defined(_WIN32)
    /* <process.h> 已声明 _environ（dllimport），此处直接引用，避免重复声明属性冲突 */
    char **env = _environ;
#else
    extern char **environ;
    char **env = environ;
#endif
    for (char **p = env; p && *p; p++) {
        ax_str s = ax_str_from_cstr(*p);
        ax_arr_push(&arr, &s, (int64_t)sizeof(ax_str));
    }
    return arr;
}

int ax_os_setenv(ax_str name, ax_str value) {
    char buf[512];
    if (!ax_os_path_cstr(name, buf, (int)sizeof buf)) return 1;
    int n = (int)strlen(buf);
    if (value.len > 0) {
        if (n + 1 + (int)value.len >= (int)sizeof buf) return 1;
        buf[n] = '=';
        memcpy(buf + n + 1, value.p, (size_t)value.len);
        buf[n + 1 + (int)value.len] = 0;
    } else {
        buf[n] = '=';
        buf[n + 1] = 0;
    }
#if defined(_WIN32)
    return _putenv(buf) == 0 ? 0 : 1;
#else
    char *eq = strchr(buf, '=');
    if (!eq) return 1;
    *eq = 0;
    int rc = setenv(buf, eq + 1, 1);
    *eq = '=';
    return rc == 0 ? 0 : 1;
#endif
}

int ax_os_unsetenv(ax_str name) {
    char buf[256];
    if (!ax_os_path_cstr(name, buf, (int)sizeof buf)) return 1;
#if defined(_WIN32)
    /* Windows 无 unsetenv，把值置空以模拟删除 */
    int n = (int)strlen(buf);
    buf[n] = '=';
    buf[n + 1] = 0;
    return _putenv(buf) == 0 ? 0 : 1;
#else
    return unsetenv(buf) == 0 ? 0 : 1;
#endif
}

/* ---------------- rand 模块（对标 Go math/rand） ---------------- */

void ax_rand_seed(int64_t seed) {
    srand((unsigned int)seed);
}

int64_t ax_rand_int(int64_t n) {
    if (n <= 0) return 0;
    /* 双倍数缩放避免取模偏差，结果落在 [0, n) */
    return (int64_t)((double)rand() / ((double)RAND_MAX + 1.0) * (double)n);
}

double ax_rand_float(void) {
    return (double)rand() / ((double)RAND_MAX + 1.0);
}

/* ---------------- sort 模块（对标 Go sort） ---------------- */

static int ax_sort_int_cmp(const void* a_, const void* b_) {
    int64_t va = *(const int64_t*)a_;
    int64_t vb = *(const int64_t*)b_;
    if (va < vb) return -1;
    if (va > vb) return 1;
    return 0;
}

void ax_sort_int(int64_t* data, int64_t len) {
    if (len <= 1) return;
    qsort(data, (size_t)len, sizeof(int64_t), ax_sort_int_cmp);
}

static int ax_sort_str_cmp(const void* a_, const void* b_) {
    return ax_str_cmp(*(const ax_str*)a_, *(const ax_str*)b_);
}

void ax_sort_str(ax_str* data, int64_t len) {
    if (len <= 1) return;
    qsort(data, (size_t)len, sizeof(ax_str), ax_sort_str_cmp);
}

static int ax_sort_float_cmp(const void* a_, const void* b_) {
    double va = *(const double*)a_;
    double vb = *(const double*)b_;
    if (va < vb) return -1;
    if (va > vb) return 1;
    return 0;
}

void ax_sort_float(double* data, int64_t len) {
    if (len <= 1) return;
    qsort(data, (size_t)len, sizeof(double), ax_sort_float_cmp);
}

int ax_sort_ints_are_sorted(const int64_t* data, int64_t len) {
    int64_t i;
    for (i = 1; i < len; i++) {
        if (data[i - 1] > data[i]) return 0;
    }
    return 1;
}

int ax_sort_strings_are_sorted(const ax_str* data, int64_t len) {
    int64_t i;
    for (i = 1; i < len; i++) {
        if (ax_str_cmp(data[i - 1], data[i]) > 0) return 0;
    }
    return 1;
}

int64_t ax_sort_search_ints(const int64_t* data, int64_t len, int64_t x) {
    int64_t lo = 0, hi = len;
    while (lo < hi) {
        int64_t mid = lo + (hi - lo) / 2;
        if (data[mid] < x) lo = mid + 1;
        else hi = mid;
    }
    if (lo < len && data[lo] == x) return lo;
    return -1;
}

int64_t ax_sort_search_strings(const ax_str* data, int64_t len, ax_str x) {
    int64_t lo = 0, hi = len;
    while (lo < hi) {
        int64_t mid = lo + (hi - lo) / 2;
        if (ax_str_cmp(data[mid], x) < 0) lo = mid + 1;
        else hi = mid;
    }
    if (lo < len && ax_str_eq(data[lo], x)) return lo;
    return -1;
}

/* 数组原地反转，对标 Go slices.Reverse。data 指向 ax_arr.data，len 为元素个数，esz 为元素字节大小。 */
void ax_arr_reverse(void* data, int64_t len, int64_t esz) {
    if (len <= 1) return;
    char* d = (char*)data;
    int64_t i = 0, j = len - 1;
    while (i < j) {
        for (int64_t k = 0; k < esz; k++) {
            char tmp = d[i * esz + k];
            d[i * esz + k] = d[j * esz + k];
            d[j * esz + k] = tmp;
        }
        i++;
        j--;
    }
}

/* 数组深拷贝，返回独立副本（对标 clone 语义，原数组不受影响） */
ax_arr ax_arr_clone(const ax_arr* a, int64_t esz) {
    ax_arr r;
    r.len = a->len;
    r.cap = a->len;
    if (a->len <= 0) { r.data = NULL; return r; }
    r.data = ax_alloc((size_t)(a->len * esz));
    if (a->data != NULL) memcpy(r.data, a->data, (size_t)(a->len * esz));
    return r;
}

/* 在索引 i 处插入元素 v，后续元素后移（对标 slices.Insert） */
void ax_arr_insert(ax_arr* a, int64_t i, const void* v, int64_t esz, const char* f, int l, int c) {
    if (i < 0 || i > a->len)
        ax_trap(R_INDEX, f, l, c, "insert index %" PRId64 ", len %" PRId64, i, a->len);
    if (a->len >= a->cap) {
        int64_t cap = a->cap ? a->cap * 2 : 4;
        void* nd = ax_alloc(cap * esz);
        if (a->len > 0) memcpy(nd, a->data, (size_t)(a->len * esz));
        a->data = nd;
        a->cap = cap;
    }
    if (i < a->len)
        memmove((uint8_t*)a->data + (i + 1) * esz,
                (uint8_t*)a->data + i * esz,
                (size_t)((a->len - i) * esz));
    memcpy((uint8_t*)a->data + i * esz, v, (size_t)esz);
    a->len++;
}

/* 用指定值 v 填充所有元素 */
void ax_arr_fill(ax_arr* a, const void* v, int64_t esz) {
    for (int64_t i = 0; i < a->len; i++)
        memcpy((uint8_t*)a->data + i * esz, v, (size_t)esz);
}

/* 调整长度为 n，不足时零初始化新增元素，超出时截断 */
void ax_arr_resize(ax_arr* a, int64_t n, int64_t esz) {
    if (n < 0) n = 0;
    if (n > a->cap) {
        int64_t cap = a->cap ? a->cap * 2 : 4;
        while (cap < n) cap *= 2;
        void* nd = ax_alloc(cap * esz);
        if (a->len > 0 && a->data != NULL) memcpy(nd, a->data, (size_t)(a->len * esz));
        a->data = nd;
        a->cap = cap;
    }
    if (n > a->len && a->data != NULL)
        memset((uint8_t*)a->data + a->len * esz, 0, (size_t)((n - a->len) * esz));
    a->len = n;
}

/* 数组合并：将 b 的所有元素追加到 a 末尾 */
void ax_arr_merge(ax_arr* a, const ax_arr* b, int64_t esz) {
    if (b->len <= 0) return;
    int64_t need = a->len + b->len;
    if (need > a->cap) {
        int64_t cap = a->cap ? a->cap * 2 : 4;
        while (cap < need) cap *= 2;
        void* nd = ax_alloc(cap * esz);
        if (a->len > 0 && a->data != NULL) memcpy(nd, a->data, (size_t)(a->len * esz));
        a->data = nd;
        a->cap = cap;
    }
    memcpy((uint8_t*)a->data + a->len * esz, b->data, (size_t)(b->len * esz));
    a->len = need;
}

/* int[] 聚合：求最小值，空数组返回 0 */
int64_t ax_arr_min_int(const int64_t* data, int64_t len) {
    if (len <= 0) return 0;
    int64_t m = data[0];
    for (int64_t i = 1; i < len; i++) if (data[i] < m) m = data[i];
    return m;
}

/* int[] 聚合：求最大值，空数组返回 0 */
int64_t ax_arr_max_int(const int64_t* data, int64_t len) {
    if (len <= 0) return 0;
    int64_t m = data[0];
    for (int64_t i = 1; i < len; i++) if (data[i] > m) m = data[i];
    return m;
}

/* int[] 聚合：求和，空数组返回 0 */
int64_t ax_arr_sum_int(const int64_t* data, int64_t len) {
    int64_t s = 0;
    for (int64_t i = 0; i < len; i++) s += data[i];
    return s;
}

/* float[] 聚合：求最小值，空数组返回 0 */
double ax_arr_min_float(const double* data, int64_t len) {
    if (len <= 0) return 0.0;
    double m = data[0];
    for (int64_t i = 1; i < len; i++) if (data[i] < m) m = data[i];
    return m;
}

/* float[] 聚合：求最大值，空数组返回 0 */
double ax_arr_max_float(const double* data, int64_t len) {
    if (len <= 0) return 0.0;
    double m = data[0];
    for (int64_t i = 1; i < len; i++) if (data[i] > m) m = data[i];
    return m;
}

/* float[] 聚合：求和，空数组返回 0 */
double ax_arr_sum_float(const double* data, int64_t len) {
    double s = 0.0;
    for (int64_t i = 0; i < len; i++) s += data[i];
    return s;
}

/* ---------------- csv 模块（对标 Go encoding/csv） ----------------
 * Reader/Writer 对标 Go io.Reader/io.Writer，在 axlang 中均为 File 句柄（ax_file*）。
 * 简化实现：按 '\n' 分行，每行按 ',' 分字段；不处理引号转义与换行字段。 */

ax_arr ax_csv_read_all(ax_file* f) {
    ax_str content;
    if (ax_file_read_all(f, &content) != 0) {
        ax_arr empty;
        ax_arr_init(&empty);
        return empty;
    }

    ax_arr outer;
    ax_arr_init(&outer);

    int64_t nlines = 0;
    ax_str* lines = ax_str_split(content, ax_str_from_cstr("\n"), &nlines);
    for (int64_t i = 0; i < nlines; i++) {
        ax_str line = lines[i];
        /* 去掉行尾的 '\r'（Windows CRLF 兼容） */
        if (line.len > 0 && line.p[line.len - 1] == (uint8_t)'\r') {
            line = ax_str_from_bytes(line.p, line.len - 1);
        }
        /* 跳过空行 */
        if (line.len == 0) continue;

        int64_t nfields = 0;
        ax_str* fields = ax_str_split(line, ax_str_from_cstr(","), &nfields);

        ax_arr inner;
        ax_arr_init(&inner);
        for (int64_t j = 0; j < nfields; j++) {
            ax_arr_push(&inner, &fields[j], (int64_t)sizeof(ax_str));
        }
        ax_arr_push(&outer, &inner, (int64_t)sizeof(ax_arr));
    }
    return outer;
}

int64_t ax_csv_write(ax_file* f, ax_arr record) {
    if (!f || !f->fp) return -1;
    ax_sb b;
    ax_sb_init(&b);
    for (int64_t i = 0; i < record.len; i++) {
        if (i > 0) {
            ax_sb_push(&b, ax_str_from_cstr(","));
        }
        ax_str* field = (ax_str*)((uint8_t*)record.data + (size_t)i * sizeof(ax_str));
        ax_sb_push(&b, *field);
    }
    ax_sb_push(&b, ax_str_from_cstr("\n"));
    ax_str out = ax_sb_finish(&b);
    return ax_file_write(f, out);
}

/* ---------------- slices 模块（对标 Go slices） ---------------- */

int ax_slice_contains_int(const int64_t* data, int64_t len, int64_t x) {
    int64_t i;
    for (i = 0; i < len; i++) {
        if (data[i] == x) return 1;
    }
    return 0;
}

int ax_slice_contains_string(const ax_str* data, int64_t len, ax_str x) {
    int64_t i;
    for (i = 0; i < len; i++) {
        if (ax_str_eq(data[i], x)) return 1;
    }
    return 0;
}

int64_t ax_slice_index_int(const int64_t* data, int64_t len, int64_t x) {
    int64_t i;
    for (i = 0; i < len; i++) {
        if (data[i] == x) return i;
    }
    return -1;
}

int64_t ax_slice_index_string(const ax_str* data, int64_t len, ax_str x) {
    int64_t i;
    for (i = 0; i < len; i++) {
        if (ax_str_eq(data[i], x)) return i;
    }
    return -1;
}

/* ---------------- filepath 模块（对标 Go path/filepath） ---------------- */

static int ax_filepath_has_sep(ax_str s) {
    int64_t i;
    for (i = 0; i < s.len; i++) {
        if (s.p[i] == (uint8_t)AX_FILEPATH_SEP[0]) return 1;
    }
    return 0;
}

static int64_t ax_filepath_last_sep(ax_str s) {
    int64_t i;
    for (i = s.len - 1; i >= 0; i--) {
        if (s.p[i] == (uint8_t)AX_FILEPATH_SEP[0]) return i;
    }
    return -1;
}

ax_str ax_filepath_join(ax_arr parts) {
    ax_sb sb;
    ax_sb_init(&sb);
    int64_t n = parts.len;
    int64_t i;
    for (i = 0; i < n; i++) {
        ax_str* e = (ax_str*)ax_arr_at(&parts, i, (int64_t)sizeof(ax_str), __FILE__, __LINE__, 0);
        if (i > 0) {
            ax_sb_push_cstr(&sb, AX_FILEPATH_SEP);
        }
        ax_sb_push(&sb, *e);
    }
    return ax_sb_finish(&sb);
}

ax_str ax_filepath_dir(ax_str path) {
    int64_t pos = ax_filepath_last_sep(path);
    if (pos < 0) {
        /* 无分隔符：返回当前目录 "." */
        return ax_str_from_cstr(".");
    }
    if (pos == 0) {
        /* 路径以分隔符开头且只有该分隔符：返回 "/" 或 "\\" */
        return ax_str_from_cstr(AX_FILEPATH_SEP);
    }
    return ax_str_slice_cp(path, 0, pos);
}

ax_str ax_filepath_base(ax_str path) {
    int64_t pos = ax_filepath_last_sep(path);
    if (pos < 0) {
        return path;
    }
    if (pos == path.len - 1) {
        /* 以分隔符结尾：返回空字符串 */
        return ax_str_empty();
    }
    return ax_str_slice_cp(path, pos + 1, path.len);
}

/* 返回扩展名（含点，如 ".txt"；无扩展名或以分隔符结尾返回空串） */
ax_str ax_filepath_ext(ax_str path) {
    int64_t pos = ax_filepath_last_sep(path);
    int64_t start = (pos < 0) ? 0 : pos + 1;
    if (start >= path.len) return ax_str_empty();
    int64_t dot = -1;
    for (int64_t i = start; i < path.len; i++) {
        if (path.p[i] == (uint8_t)'.') dot = i;
    }
    if (dot < 0 || dot == start) return ax_str_empty();
    /* 扩展名不能是 "." 或 ".." */
    if (dot == start + 1 && path.p[start] == '.') return ax_str_empty();
    return ax_str_slice_cp(path, dot, path.len);
}

/* 分割路径为 [dir, base] 数组（对标 Go filepath.Split） */
ax_arr ax_filepath_split(ax_str path) {
    ax_arr arr;
    ax_arr_init(&arr);
    int64_t pos = ax_filepath_last_sep(path);
    ax_str dir, base;
    if (pos < 0) {
        dir = ax_str_from_cstr(".");
        base = path;
    } else if (pos == 0) {
        dir = ax_str_from_cstr(AX_FILEPATH_SEP);
        base = ax_str_slice_cp(path, 1, path.len);
    } else {
        dir = ax_str_slice_cp(path, 0, pos);
        base = ax_str_slice_cp(path, pos + 1, path.len);
    }
    ax_arr_push(&arr, &dir, (int64_t)sizeof(ax_str));
    ax_arr_push(&arr, &base, (int64_t)sizeof(ax_str));
    return arr;
}

/* 路径规范化（去除 .、..、重复分隔符，对标 Go filepath.Clean） */
ax_str ax_filepath_clean(ax_str path) {
    int is_abs = (path.len > 0 && path.p[0] == (uint8_t)AX_FILEPATH_SEP[0]);

    /* 按分隔符分割为段 */
    ax_arr parts;
    ax_arr_init(&parts);
    int64_t start = 0;
    for (int64_t i = 0; i <= path.len; i++) {
        if (i == path.len || path.p[i] == (uint8_t)AX_FILEPATH_SEP[0]) {
            if (i > start) {
                ax_str seg = ax_str_slice_cp(path, start, i);
                ax_arr_push(&parts, &seg, (int64_t)sizeof(ax_str));
            }
            start = i + 1;
        }
    }

    /* 处理段：跳过空段和 "."，处理 ".." */
    ax_arr stack;
    ax_arr_init(&stack);
    int64_t n = parts.len;
    for (int64_t i = 0; i < n; i++) {
        ax_str* seg = (ax_str*)ax_arr_at(&parts, i, (int64_t)sizeof(ax_str), __FILE__, __LINE__, 0);
        if (seg->len == 0) continue;
        if (seg->len == 1 && seg->p[0] == (uint8_t)'.') continue;
        if (seg->len == 2 && seg->p[0] == (uint8_t)'.' && seg->p[1] == (uint8_t)'.') {
            int64_t sn = stack.len;
            if (sn > 0) {
                ax_str* top = (ax_str*)ax_arr_at(&stack, sn - 1, (int64_t)sizeof(ax_str), __FILE__, __LINE__, 0);
                if (!(top->len == 2 && top->p[0] == (uint8_t)'.' && top->p[1] == (uint8_t)'.')) {
                    stack.len--; /* 弹出栈顶，抵消 ".." */
                    continue;
                }
            }
            ax_arr_push(&stack, seg, (int64_t)sizeof(ax_str));
        } else {
            ax_arr_push(&stack, seg, (int64_t)sizeof(ax_str));
        }
    }

    /* 构建结果 */
    ax_sb sb;
    ax_sb_init(&sb);
    if (is_abs) ax_sb_push_cstr(&sb, AX_FILEPATH_SEP);
    int64_t sn = stack.len;
    for (int64_t i = 0; i < sn; i++) {
        if (i > 0) ax_sb_push_cstr(&sb, AX_FILEPATH_SEP);
        ax_str* seg = (ax_str*)ax_arr_at(&stack, i, (int64_t)sizeof(ax_str), __FILE__, __LINE__, 0);
        ax_sb_push(&sb, *seg);
    }
    if (sn == 0) {
        if (is_abs) ax_sb_push_cstr(&sb, AX_FILEPATH_SEP);
        else ax_sb_push_cstr(&sb, ".");
    }
    return ax_sb_finish(&sb);
}

/* 判断是否为绝对路径（Unix 以 / 开头，Windows 以 \ 或 盘符: 开头） */
int ax_filepath_is_abs(ax_str path) {
    if (path.len == 0) return 0;
    if (path.p[0] == (uint8_t)AX_FILEPATH_SEP[0]) return 1;
#if defined(_WIN32)
    /* Windows 盘符：C: 或 C:\ */
    if (path.len >= 2 && path.p[1] == (uint8_t)':' &&
        ((path.p[0] >= (uint8_t)'a' && path.p[0] <= (uint8_t)'z') ||
         (path.p[0] >= (uint8_t)'A' && path.p[0] <= (uint8_t)'Z'))) {
        if (path.len == 2 || path.p[2] == (uint8_t)AX_FILEPATH_SEP[0] || path.p[2] == (uint8_t)'/') return 1;
    }
#endif
    return 0;
}

/* 转为绝对路径（对标 Go filepath.Abs）。已是绝对路径则 Clean 后返回；
 * 否则用当前工作目录拼接后 Clean。 */
ax_str ax_filepath_abs(ax_str path) {
    if (ax_filepath_is_abs(path)) return ax_filepath_clean(path);

    /* 获取当前工作目录 */
    char cwd[4096];
#if defined(_WIN32)
    if (!_getcwd(cwd, (int)sizeof cwd)) return ax_filepath_clean(path);
#else
    if (!getcwd(cwd, (size_t)sizeof cwd)) return ax_filepath_clean(path);
#endif
    ax_str cwd_str = ax_str_from_cstr(cwd);

    /* 拼接 cwd + sep + path */
    ax_sb sb;
    ax_sb_init(&sb);
    ax_sb_push(&sb, cwd_str);
    if (cwd_str.len > 0 && cwd_str.p[cwd_str.len - 1] != (uint8_t)AX_FILEPATH_SEP[0]) {
        ax_sb_push_cstr(&sb, AX_FILEPATH_SEP);
    }
    ax_sb_push(&sb, path);
    ax_str joined = ax_sb_finish(&sb);
    return ax_filepath_clean(joined);
}

/* ---------------- arena（区域分配器） ---------------- */
struct ax_arena {
    void**  items;
    int64_t n;
    int64_t cap;
};

ax_arena* ax_arena_create(void) {
    ax_arena* ar = (ax_arena*)ax_alloc((int64_t)sizeof(ax_arena));
    ar->items = NULL;
    ar->n = 0;
    ar->cap = 0;
    return ar;
}

void* ax_arena_adopt(ax_arena* ar, void* p) {
    if (ar->n >= ar->cap) {
        int64_t nc = ar->cap == 0 ? 8 : ar->cap * 2;
        void** ni = (void**)ax_alloc(nc * (int64_t)sizeof(void*));
        if (ar->items && ar->n > 0) {
            memcpy(ni, ar->items, (size_t)ar->n * sizeof(void*));
        }
        ar->items = ni;
        ar->cap = nc;
    }
    ar->items[ar->n] = p;
    ar->n++;
    return p;
}

void ax_arena_free(ax_arena* ar) {
    if (!ar) return;
    for (int64_t i = 0; i < ar->n; i++) {
        free(ar->items[i]);
    }
    free(ar->items);
    free(ar);
}

/* ================================================================ 线程 */
#ifdef _WIN32
struct ax_thread {
    HANDLE h;
};

static DWORD WINAPI ax_thread_proc(LPVOID arg) {
    void (**func)(void*) = (void (**)(void*))arg;
    void* user_arg = ((void**)arg)[1];
    (*func)(user_arg);
    free(arg);
    return 0;
}

ax_thread* ax_thread_create(void (*func)(void*), void* arg) {
    ax_thread* t = (ax_thread*)ax_alloc((int64_t)sizeof(ax_thread));
    void** pack = (void**)ax_alloc(2 * sizeof(void*));
    pack[0] = (void*)func;
    pack[1] = arg;
    t->h = CreateThread(NULL, 0, ax_thread_proc, pack, 0, NULL);
    if (!t->h) ax_oom();
    return t;
}

void ax_thread_join(ax_thread* t) {
    if (!t) return;
    WaitForSingleObject(t->h, INFINITE);
    CloseHandle(t->h);
    free(t);
}
#else
struct ax_thread {
    pthread_t t;
};

static void* ax_thread_proc(void* arg) {
    void (**func)(void*) = (void (**)(void*))arg;
    void* user_arg = ((void**)arg)[1];
    (*func)(user_arg);
    free(arg);
    return NULL;
}

ax_thread* ax_thread_create(void (*func)(void*), void* arg) {
    ax_thread* t = (ax_thread*)ax_alloc((int64_t)sizeof(ax_thread));
    void** pack = (void**)ax_alloc(2 * sizeof(void*));
    pack[0] = (void*)func;
    pack[1] = arg;
    if (pthread_create(&t->t, NULL, ax_thread_proc, pack) != 0) ax_oom();
    return t;
}

void ax_thread_join(ax_thread* t) {
    if (!t) return;
    pthread_join(t->t, NULL);
    free(t);
}
#endif

/* ================================================================ 阻塞队列 */
#define AX_QUEUE_CAP 256

struct ax_queue {
    void*    items[AX_QUEUE_CAP];  /* 元素副本指针 */
    int64_t  head, tail, count;
    int64_t  elem_size;
    int      closed;
#ifdef _WIN32
    CRITICAL_SECTION lock;
    CONDITION_VARIABLE not_empty;
    CONDITION_VARIABLE not_full;
#else
    pthread_mutex_t lock;
    pthread_cond_t  not_empty;
    pthread_cond_t  not_full;
#endif
};

ax_queue* ax_queue_create(int64_t elem_size) {
    ax_queue* q = (ax_queue*)ax_alloc((int64_t)sizeof(ax_queue));
    q->head = 0;
    q->tail = 0;
    q->count = 0;
    q->elem_size = elem_size;
    q->closed = 0;
#ifdef _WIN32
    InitializeCriticalSection(&q->lock);
    InitializeConditionVariable(&q->not_empty);
    InitializeConditionVariable(&q->not_full);
#else
    pthread_mutex_init(&q->lock, NULL);
    pthread_cond_init(&q->not_empty, NULL);
    pthread_cond_init(&q->not_full, NULL);
#endif
    return q;
}

void ax_queue_push(ax_queue* q, const void* data) {
    if (!q) return;
#ifdef _WIN32
    EnterCriticalSection(&q->lock);
    while (q->count >= AX_QUEUE_CAP && !q->closed) {
        SleepConditionVariableCS(&q->not_full, &q->lock, INFINITE);
    }
    if (q->closed) {
        LeaveCriticalSection(&q->lock);
        return;
    }
    void* copy = ax_alloc(q->elem_size);
    memcpy(copy, data, (size_t)q->elem_size);
    q->items[q->tail] = copy;
    q->tail = (q->tail + 1) % AX_QUEUE_CAP;
    q->count++;
    WakeConditionVariable(&q->not_empty);
    LeaveCriticalSection(&q->lock);
#else
    pthread_mutex_lock(&q->lock);
    while (q->count >= AX_QUEUE_CAP && !q->closed) {
        pthread_cond_wait(&q->not_full, &q->lock);
    }
    if (q->closed) {
        pthread_mutex_unlock(&q->lock);
        return;
    }
    void* copy = ax_alloc(q->elem_size);
    memcpy(copy, data, (size_t)q->elem_size);
    q->items[q->tail] = copy;
    q->tail = (q->tail + 1) % AX_QUEUE_CAP;
    q->count++;
    pthread_cond_signal(&q->not_empty);
    pthread_mutex_unlock(&q->lock);
#endif
}

int ax_queue_pop(ax_queue* q, void* out) {
    if (!q) return 1;
#ifdef _WIN32
    EnterCriticalSection(&q->lock);
    while (q->count == 0 && !q->closed) {
        SleepConditionVariableCS(&q->not_empty, &q->lock, INFINITE);
    }
    if (q->count == 0) {
        /* 已关闭且为空 */
        LeaveCriticalSection(&q->lock);
        return 1;
    }
    memcpy(out, q->items[q->head], (size_t)q->elem_size);
    free(q->items[q->head]);
    q->items[q->head] = NULL;
    q->head = (q->head + 1) % AX_QUEUE_CAP;
    q->count--;
    WakeConditionVariable(&q->not_full);
    LeaveCriticalSection(&q->lock);
    return 0;
#else
    pthread_mutex_lock(&q->lock);
    while (q->count == 0 && !q->closed) {
        pthread_cond_wait(&q->not_empty, &q->lock);
    }
    if (q->count == 0) {
        pthread_mutex_unlock(&q->lock);
        return 1;
    }
    memcpy(out, q->items[q->head], (size_t)q->elem_size);
    free(q->items[q->head]);
    q->items[q->head] = NULL;
    q->head = (q->head + 1) % AX_QUEUE_CAP;
    q->count--;
    pthread_cond_signal(&q->not_full);
    pthread_mutex_unlock(&q->lock);
    return 0;
#endif
}

/* 非阻塞弹出：队列有数据返回 0 并写出，队列为空（无论是否已关闭）立即返回 1，不阻塞。 */
int ax_queue_try_pop(ax_queue* q, void* out) {
    if (!q) return 1;
#ifdef _WIN32
    EnterCriticalSection(&q->lock);
    if (q->count == 0) {
        LeaveCriticalSection(&q->lock);
        return 1;
    }
    memcpy(out, q->items[q->head], (size_t)q->elem_size);
    free(q->items[q->head]);
    q->items[q->head] = NULL;
    q->head = (q->head + 1) % AX_QUEUE_CAP;
    q->count--;
    WakeConditionVariable(&q->not_full);
    LeaveCriticalSection(&q->lock);
    return 0;
#else
    pthread_mutex_lock(&q->lock);
    if (q->count == 0) {
        pthread_mutex_unlock(&q->lock);
        return 1;
    }
    memcpy(out, q->items[q->head], (size_t)q->elem_size);
    free(q->items[q->head]);
    q->items[q->head] = NULL;
    q->head = (q->head + 1) % AX_QUEUE_CAP;
    q->count--;
    pthread_cond_signal(&q->not_full);
    pthread_mutex_unlock(&q->lock);
    return 0;
#endif
}

void ax_queue_close(ax_queue* q) {
    if (!q) return;
#ifdef _WIN32
    EnterCriticalSection(&q->lock);
    q->closed = 1;
    WakeAllConditionVariable(&q->not_empty);
    LeaveCriticalSection(&q->lock);
#else
    pthread_mutex_lock(&q->lock);
    q->closed = 1;
    pthread_cond_broadcast(&q->not_empty);
    pthread_mutex_unlock(&q->lock);
#endif
}
