/* axrt_decimal.c -- axlang 运行时模块 */
#include "axrt.h"
#include "axrt_internal.h"


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

double i128_to_double(ax_i128 v);

double ax_dec_to_f64(ax_dec v) {
    return i128_to_double(v.coeff) / pow(10.0, (double)v.scale);
}

double i128_to_double(ax_i128 v) {
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
