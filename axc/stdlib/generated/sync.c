/* generated/sync.c —— axlang 标准库 sync 模块（预转译自 axc/stdlib/sync.ax）
 * 纯标准 C11，随编译器发布，通过 gcc 参与编译。
 * 底层依赖 axrt.h / axrt.c 提供的 C 运行时原语。
 */

#include "axrt.h"
#include "axc_stdlib.h"

/* sync.Mutex.Lock / Unlock */
void ax_std_mutex_lock(void* m) {
    ax_mutex_lock((ax_mutex*)m);
}
void ax_std_mutex_unlock(void* m) {
    ax_mutex_unlock((ax_mutex*)m);
}

/* sync.RWMutex.Lock / Unlock / RLock / Runlock */
void ax_std_rwmutex_lock(void* rw) {
    ax_rwmutex_lock((ax_rwmutex*)rw);
}
void ax_std_rwmutex_unlock(void* rw) {
    ax_rwmutex_unlock((ax_rwmutex*)rw);
}
void ax_std_rwmutex_rlock(void* rw) {
    ax_rwmutex_rlock((ax_rwmutex*)rw);
}
void ax_std_rwmutex_runlock(void* rw) {
    ax_rwmutex_runlock((ax_rwmutex*)rw);
}

/* sync.WaitGroup.Add / Done / Wait */
void ax_std_waitgroup_add(void* wg, int64_t delta) {
    ax_waitgroup_add((ax_waitgroup*)wg, delta);
}
void ax_std_waitgroup_done(void* wg) {
    ax_waitgroup_done((ax_waitgroup*)wg);
}
void ax_std_waitgroup_wait(void* wg) {
    ax_waitgroup_wait((ax_waitgroup*)wg);
}

/* sync.Once.Do */
void ax_std_once_do(void* o) {
    ax_once_do((ax_once*)o);
}
