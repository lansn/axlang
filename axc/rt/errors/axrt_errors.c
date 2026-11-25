/* axrt_errors.c -- axlang 运行时模块：错误处理（对标 Go errors，P2）
 * ax_err_* 操作 ax_err 扁平值（code+msg）。
 * 与 std/errors.ax 一一对应。
 */
#include "axrt.h"
#include "axrt_internal.h"

/* ---------------- errors 模块 ---------------- */

/* errors.New：创建一个错误值 */
ax_err ax_err_new(int32_t code, ax_str msg) {
    ax_err e; e.code = code; e.msg = msg; return e;
}

/* errors.Is(err, target)：判断两个错误是否相等。
 * axlang error 是扁平值（code+msg），无错误链，故判定两 error 相等即为 Is。 */
int ax_err_is(ax_err err, ax_err target) {
    if (err.code != target.code) return 0;
    return ax_str_eq(err.msg, target.msg);
}

/* errors.As(err, target)：将 err 复制到 target，返回 1。 */
int ax_err_as(ax_err err, ax_err* target) {
    *target = err;
    return 1;
}
