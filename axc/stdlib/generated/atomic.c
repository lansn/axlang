/* generated/atomic.c —— axlang 标准库 atomic 模块（预转译自 axc/stdlib/atomic.ax）
 * 纯标准 C11，随编译器发布，通过 gcc 参与编译。
 * 底层依赖 axrt.h / axrt.c 提供的 C 运行时原语。
 */

#include "axrt.h"
#include "axc_stdlib.h"

/* atomic.AddInt */
int64_t ax_std_atomic_add_int(volatile int64_t* addr, int64_t delta) {
    return ax_atomic_add_int(addr, delta);
}

/* atomic.LoadInt */
int64_t ax_std_atomic_load_int(volatile int64_t* addr) {
    return ax_atomic_load_int(addr);
}

/* atomic.StoreInt */
void ax_std_atomic_store_int(volatile int64_t* addr, int64_t val) {
    ax_atomic_store_int(addr, val);
}

/* atomic.CompareAndSwapInt */
int ax_std_atomic_compare_and_swap_int(volatile int64_t* addr, int64_t old, int64_t new) {
    return ax_atomic_compare_and_swap_int(addr, old, new);
}
