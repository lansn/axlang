/* axrt_core.c -- axlang 运行时模块 */
#include "axrt.h"
#include "axrt_internal.h"


ax_i128 i128_make(int64_t hi, uint64_t lo) {
    ax_i128 r; r.hi = hi; r.lo = lo; return r;
}


ax_i128 i128_from_i64(int64_t v) {
    if (v >= 0) return i128_make(0, (uint64_t)v);
    return i128_make(-1, (uint64_t)v);
}


int i128_to_i64(ax_i128 v, int64_t* out) {
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


int i128_cmp(ax_i128 a, ax_i128 b) {
    if (a.hi != b.hi) return a.hi < b.hi ? -1 : 1;
    if (a.lo < b.lo) return -1;
    if (a.lo > b.lo) return 1;
    return 0;
}


ax_i128 i128_neg(ax_i128 v) {
    uint64_t lo_inv = ~v.lo;
    int64_t  hi_inv = ~v.hi;
    uint64_t lo_new = lo_inv + 1;
    int64_t  hi_new = hi_inv + (lo_new == 0 ? 1 : 0);
    return i128_make(hi_new, lo_new);
}


ax_i128 i128_abs(ax_i128 v) {
    if (v.hi < 0) return i128_neg(v);
    return v;
}


int i128_add(ax_i128 a, ax_i128 b, ax_i128* out) {
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


int i128_sub(ax_i128 a, ax_i128 b, ax_i128* out) {
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

void u128_mul64(uint64_t a, uint64_t b, uint64_t* hi, uint64_t* lo) {
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


int i128_mul(ax_i128 a, ax_i128 b, ax_i128* out) {
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


ax_i128 i128_pow10(int n) {
    ax_i128 r = i128_from_i64(1);
    for (int i = 0; i < n; i++) {
        ax_i128 t;
        if (i128_mul(r, i128_from_i64(10), &t)) break;
        r = t;
    }
    return r;
}

/* 128 位有符号除法：num / den -> 商、余数（den != 0，商向零截断） */

/* 128 位有符号除法：num / den -> 商、余数（den != 0，商向零截断） */
void i128_divmod(ax_i128 num, ax_i128 den, ax_i128* q, ax_i128* r) {
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


ax_i128 i128_gcd(ax_i128 a, ax_i128 b) {
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
void ax_write_stdout(ax_str s) {
    if (s.len > 0) fwrite(s.p, 1, (size_t)s.len, stdout);
}

void ax_write_stderr(ax_str s) {
    if (s.len > 0) fwrite(s.p, 1, (size_t)s.len, stderr);
}

/* ================================================================ builder */

int i128_is_zero(ax_i128 v) { return v.hi == 0 && v.lo == 0; }

/* 将 ax_i128 除以 uint64_t（除数 > 0），返回商和余数。 */

/* 将 ax_i128 除以 uint64_t（除数 > 0），返回商和余数。 */
void i128_div_u64(ax_i128 v, uint64_t d, ax_i128* q, uint64_t* r) {
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


void ax_arr_delete_range(ax_arr* a, int64_t i, int64_t j, int64_t esz, const char* f, int l, int c) {
    if (i < 0 || i > a->len)
        ax_trap(R_INDEX, f, l, c, "delete start index %" PRId64 ", len %" PRId64, i, a->len);
    if (j < i || j > a->len)
        ax_trap(R_INDEX, f, l, c, "delete end index %" PRId64 ", start %" PRId64 ", len %" PRId64, j, i, a->len);
    if (i == j) return;
    int64_t n = j - i;
    memmove((uint8_t*)a->data + i * esz,
            (uint8_t*)a->data + j * esz,
            (size_t)((a->len - j) * esz));
    a->len -= n;
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

/* 将 src 的全部键值复制到 dst（已存在键被覆盖），对标 Go maps.Copy。 */
void ax_map_copy_into(ax_map* dst, const ax_map* src) {
    for (int64_t i = 0; i < src->len; i++) {
        ax_map_put(dst, src->entries[i].k, src->entries[i].v);
    }
}

/* 判断两个 map 是否相等（同长度、同键且对应值相等），对标 Go maps.Equal。 */

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

/* ================================================================ 标准库 */

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

/* 用指定值 v 填充所有元素 */
void ax_arr_fill(ax_arr* a, const void* v, int64_t esz) {
    for (int64_t i = 0; i < a->len; i++)
        memcpy((uint8_t*)a->data + i * esz, v, (size_t)esz);
}

/* 调整长度为 n，不足时零初始化新增元素，超出时截断 */

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


struct ax_arena {
    void**  items;
    int64_t n;
    int64_t cap;
};

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
#endif

