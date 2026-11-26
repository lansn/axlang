/* axrt_math.c -- axlang 运行时模块：数学扩展（对标 Go math，P2）
 * 纯标准 C11，无 GNU/Clang 扩展。
 * 与 std/math.ax 一一对应。
 */
#include "axrt.h"
#include "axrt_internal.h"
#include <math.h>

/* ---------------- math 模块 ---------------- */

/* math.Sign：x>0→1，x<0→-1，否则（含 0/NaN）→0 */
int ax_math_sign(double x) {
    if (x > 0) return 1;
    if (x < 0) return -1;
    return 0;
}

/* math.Mod：浮点余数，语义同 C fmod：符号同 x，幅值 < |y| */
double ax_math_mod(double x, double y) {
    return fmod(x, y);
}

/* math.IsNaN：C11 isnan，x 为 NaN 返回 1，否则 0 */
int ax_math_isnan(double x) {
    return isnan(x);
}

/* math.IsInf：C11 isinf，sign>0 只判 +Inf，sign<0 只判 -Inf，sign==0 判任意无穷 */
int ax_math_isinf(double x, int sign) {
    if (!isinf(x)) return 0;
    if (sign > 0) return x > 0 ? 1 : 0;
    if (sign < 0) return x < 0 ? 1 : 0;
    return 1;
}

/* math.Signbit：C11 signbit，x 为负数（含 -0 / -NaN）返回 1，否则 0 */
int ax_math_signbit(float x) {
    return signbit(x);
}

/* math.Copysign：C11 copysign，x 的幅值、y 的符号 */
float ax_math_copysign(float x, float y) {
    return copysignf(x, y);
}

/* math.Dim：C11 fdim，max(x - y, 0) */
float ax_math_dim(float x, float y) {
    return fdimf(x, y);
}

/* math.Ldexp：C11 ldexp，x * 2^exp */
float ax_math_ldexp(float x, int exp) {
    return ldexpf(x, exp);
}

/* math.Frexp：C11 frexp，将 x 分解为尾数和 2 的幂指数。
 * ax 语言层无输出指针，故拆为两个原语：ax_math_frexp_frac 返回尾数，
 * ax_math_frexp_exp 返回指数；ax 层再组装为 (frac, int) 元组返回。 */
float ax_math_frexp_frac(double x) {
    int exp;
    return (float)frexp(x, &exp);
}
int ax_math_frexp_exp(double x) {
    int exp;
    (void)frexp(x, &exp);
    return exp;
}
