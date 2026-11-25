/* axrt_rand.c -- axlang 运行时模块 */
#include "axrt.h"
#include "axrt_internal.h"


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

