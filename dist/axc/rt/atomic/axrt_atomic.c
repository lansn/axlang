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


int64_t ax_atomic_add_int(volatile int64_t* addr, int64_t delta) {
    if (!addr) return 0;
    _Atomic int64_t* p = (_Atomic int64_t*)addr;
    return atomic_fetch_add(p, delta) + delta;
}


int64_t ax_atomic_load_int(volatile int64_t* addr) {
    if (!addr) return 0;
    _Atomic int64_t* p = (_Atomic int64_t*)addr;
    return atomic_load(p);
}


/* ================================================================ ax_std_atomic_* 包装层 */
/* 供 ax 编译器 codegen 的 module_call 调用，参数用 void* 兼容 ax_stdlib.h 声明。 */

int64_t ax_std_atomic_add_int(void* addr, int64_t delta) {
    return ax_atomic_add_int((volatile int64_t*)addr, delta);
}


int64_t ax_std_atomic_load_int(void* addr) {
    return ax_atomic_load_int((volatile int64_t*)addr);
}


void ax_std_atomic_store_int(void* addr, int64_t val) {
    ax_atomic_store_int((volatile int64_t*)addr, val);
}


int ax_std_atomic_compare_and_swap_int(void* addr, int64_t old, int64_t new) {
    return ax_atomic_compare_and_swap_int((volatile int64_t*)addr, old, new);
}
