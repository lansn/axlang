/* axrt_sync.c -- axlang 运行时模块 */
#include "axrt.h"
#include "axrt_internal.h"


#ifdef _WIN32
struct ax_thread {
    HANDLE h;
};

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

/* ================================================================ sync 模块（对标 Go sync，P2） */

/* --- 互斥锁 --- */

/* ================================================================ sync 模块（对标 Go sync，P2） */

/* --- 互斥锁 --- */
void ax_mutex_lock(ax_mutex* m) {
    if (!m) return;
    if (!m->initialized) {
#ifdef _WIN32
        InitializeCriticalSection(&m->cs);
#else
        pthread_mutex_init(&m->mu, NULL);
#endif
        m->initialized = 1;
    }
#ifdef _WIN32
    EnterCriticalSection(&m->cs);
#else
    pthread_mutex_lock(&m->mu);
#endif
}


void ax_mutex_unlock(ax_mutex* m) {
    if (!m) return;
    if (!m->initialized) {
        return;  /* 未曾加锁，无需解锁 */
    }
#ifdef _WIN32
    LeaveCriticalSection(&m->cs);
#else
    pthread_mutex_unlock(&m->mu);
#endif
}

/* --- 读写锁 --- */

/* --- 读写锁 --- */
void ax_rwmutex_lock(ax_rwmutex* rw) {
    if (!rw) return;
    if (!rw->initialized) {
#ifndef _WIN32
        pthread_rwlock_init(&rw->rw, NULL);
#endif
        rw->initialized = 1;
    }
#ifdef _WIN32
    AcquireSRWLockExclusive(&rw->srw);
#else
    pthread_rwlock_wrlock(&rw->rw);
#endif
}


void ax_rwmutex_unlock(ax_rwmutex* rw) {
    if (!rw) return;
    if (!rw->initialized) {
        return;
    }
#ifdef _WIN32
    ReleaseSRWLockExclusive(&rw->srw);
#else
    pthread_rwlock_unlock(&rw->rw);
#endif
}


void ax_rwmutex_rlock(ax_rwmutex* rw) {
    if (!rw) return;
    if (!rw->initialized) {
#ifndef _WIN32
        pthread_rwlock_init(&rw->rw, NULL);
#endif
        rw->initialized = 1;
    }
#ifdef _WIN32
    AcquireSRWLockShared(&rw->srw);
#else
    pthread_rwlock_rdlock(&rw->rw);
#endif
}


void ax_rwmutex_runlock(ax_rwmutex* rw) {
    if (!rw) return;
    if (!rw->initialized) {
        return;
    }
#ifdef _WIN32
    ReleaseSRWLockShared(&rw->srw);
#else
    pthread_rwlock_unlock(&rw->rw);
#endif
}

/* --- 等待组 --- */

/* --- 等待组 --- */
void ax_waitgroup_add(ax_waitgroup* wg, int64_t delta) {
    if (!wg) return;
    if (!wg->initialized) {
#ifdef _WIN32
        InitializeCriticalSection(&wg->cs);
        InitializeConditionVariable(&wg->cv);
#else
        pthread_mutex_init(&wg->mu, NULL);
        pthread_cond_init(&wg->cv, NULL);
#endif
        wg->initialized = 1;
    }
#ifdef _WIN32
    EnterCriticalSection(&wg->cs);
#else
    pthread_mutex_lock(&wg->mu);
#endif
    wg->counter += delta;
    if (wg->counter <= 0) {
#ifdef _WIN32
        WakeAllConditionVariable(&wg->cv);
#else
        pthread_cond_broadcast(&wg->cv);
#endif
    }
#ifdef _WIN32
    LeaveCriticalSection(&wg->cs);
#else
    pthread_mutex_unlock(&wg->mu);
#endif
}


void ax_waitgroup_done(ax_waitgroup* wg) {
    ax_waitgroup_add(wg, -1);
}


void ax_waitgroup_wait(ax_waitgroup* wg) {
    if (!wg) return;
    if (!wg->initialized) {
        return;  /* 未 Add 过，计数器为 0，无需等待 */
    }
#ifdef _WIN32
    EnterCriticalSection(&wg->cs);
    while (wg->counter > 0) {
        SleepConditionVariableCS(&wg->cv, &wg->cs, INFINITE);
    }
    LeaveCriticalSection(&wg->cs);
#else
    pthread_mutex_lock(&wg->mu);
    while (wg->counter > 0) {
        pthread_cond_wait(&wg->cv, &wg->mu);
    }
    pthread_mutex_unlock(&wg->mu);
#endif
}

/* --- Once --- */
/* axlang 尚无闭包/函数值，Once.Do 无可传回调；C 层仅提供 done 标志快路径，
 * 上层在串行化上下文中调用。 */

/* --- Once --- */
/* axlang 尚无闭包/函数值，Once.Do 无可传回调；C 层仅提供 done 标志快路径，
 * 上层在串行化上下文中调用。 */
void ax_once_do(ax_once* o) {
    if (!o) return;
    if (o->done) return;
#ifdef _WIN32
    /* INIT_ONCE 需静态初始化，对堆分配结构体不直接适用；此处以 done 标志
     * 作快路径，并由调用方保证串行化（axlang 单线程或用户自行加锁）。 */
    o->done = 1;
#else
    pthread_once(&o->once, NULL);
    o->done = 1;
#endif
}

/* ================================================================ ax_std_* 标准库包装层 */

/* ax_std_* sync 模块 C 接口，封装 axrt 同步原语。
 * 参数为 void* 以兼容 ax_stdlib.h 声明，内部转回对应句柄类型。 */
void ax_std_mutex_lock(void* m) {
    ax_mutex_lock((ax_mutex*)m);
}

void ax_std_mutex_unlock(void* m) {
    ax_mutex_unlock((ax_mutex*)m);
}

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

void ax_std_waitgroup_add(void* wg, int64_t delta) {
    ax_waitgroup_add((ax_waitgroup*)wg, delta);
}

void ax_std_waitgroup_done(void* wg) {
    ax_waitgroup_done((ax_waitgroup*)wg);
}

void ax_std_waitgroup_wait(void* wg) {
    ax_waitgroup_wait((ax_waitgroup*)wg);
}

void ax_std_once_do(void* o) {
    ax_once_do((ax_once*)o);
}
