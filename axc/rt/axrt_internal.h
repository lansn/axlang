#ifndef AXRT_INTERNAL_H
#define AXRT_INTERNAL_H

#include "axrt.h"

/* ================================================================
 * 内部辅助函数（跨模块共享，非对外公开 API）
 * ================================================================ */

/* ax_i128 128 位整数运算（axrt_core.c 定义，axrt_decimal.c 调用） */
ax_i128 i128_make(int64_t hi, uint64_t lo);
ax_i128 i128_from_i64(int64_t v);
int  i128_to_i64(ax_i128 v, int64_t* out);
int  i128_cmp(ax_i128 a, ax_i128 b);
ax_i128 i128_neg(ax_i128 v);
ax_i128 i128_abs(ax_i128 v);
int  i128_add(ax_i128 a, ax_i128 b, ax_i128* out);
int  i128_sub(ax_i128 a, ax_i128 b, ax_i128* out);
void u128_mul64(uint64_t a, uint64_t b, uint64_t* hi, uint64_t* lo);
int  i128_mul(ax_i128 a, ax_i128 b, ax_i128* out);
ax_i128 i128_pow10(int n);
void i128_divmod(ax_i128 num, ax_i128 den, ax_i128* q, ax_i128* r);
ax_i128 i128_gcd(ax_i128 a, ax_i128 b);
int  i128_is_zero(ax_i128 v);
void i128_div_u64(ax_i128 v, uint64_t d, ax_i128* q, uint64_t* r);
double i128_to_double(ax_i128 v);

/* 文件路径转换（axrt_file.c 定义，axrt_os.c 调用） */
int ax_path_cstr(ax_str path, char* out, int cap);
int ax_os_path_cstr(ax_str s, char* out, int cap);

/* strconv 内部函数（axrt_strconv.c 定义，axrt_str.c 调用） */
int ax_quote_escape(uint8_t c, char* out);
int hexval(uint8_t c);
int ax_unquote_escape(const uint8_t** p, const uint8_t* end, uint8_t* out);
int ax_u8_is_printable(uint8_t c);

#endif /* AXRT_INTERNAL_H */
