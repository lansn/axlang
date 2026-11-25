/* axrt_sort.c -- axlang 运行时模块 */
#include "axrt.h"
#include "axrt_internal.h"


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

