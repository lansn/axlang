/* axrt_atomic.c -- axlang 运行时模块 */
#include "axrt.h"
#include "axrt_internal.h"


void ax_atomic_store_int(volatile int64_t* addr, int64_t val) {
    if (!addr) return;
    _Atomic int64_t* p = (_Atomic int64_t*)addr;
    atomic_store(p, val);
}


int ax_atomic_compare_and_swap_int(volatile int64_t* addr, int64_t old, int64_t new) {
    if (!addr) return 0;
    _Atomic int64_t* p = (_Atomic int64_t*)addr;
    int64_t expected = old;
    return atomic_compare_exchange_strong(p, &expected, new) ? 1 : 0;
}
