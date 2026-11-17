/* axrt.h -- axlang 运行时（规格书 §14）
 * C11，除 __int128 外不使用 GNU 扩展。
 */
#ifndef AXRT_H
#define AXRT_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include <string.h>
#include <stdlib.h>
#include <stdio.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ---------------- 崩溃（Rt-5） ---------------- */
#define R_INDEX   101
#define R_DIVZERO 102
#define R_INEXACT 103
#define R_OVERFLOW 104
#define R_SHIFT   105
#define R_FLOAT   106
#define R_CONVERT 107
#define R_OOM     100

_Noreturn void ax_trap(int code, const char* file, int line, int col,
                       const char* fmt, ...);
_Noreturn void ax_oom(void);
void* ax_alloc(int64_t size);
void* ax_calloc(int64_t n, int64_t size);

typedef struct { __int128 coeff; int32_t scale; int32_t _pad; } ax_dec;

#define ROUND_HALF_UP   0
#define ROUND_HALF_EVEN 1
#define ROUND_DOWN      2

/* ---------------- 字符串（Rt-6） ---------------- */
typedef struct { const uint8_t* p; int64_t len; } ax_str;

#define AX_S(lit) ((ax_str){ (const uint8_t*)(lit), (int64_t)(sizeof(lit) - 1) })

ax_str  ax_str_from_bytes(const uint8_t* p, int64_t n);
ax_str  ax_str_from_cstr(const char* s);
ax_str  ax_env_utf8(const char* name);      /* 环境变量 → UTF-8（Windows 走宽字符） */
void    ax_console_utf8(void);              /* main 入口调用：Windows 控制台切 UTF-8 */
ax_str  ax_str_empty(void);
int64_t ax_str_len_cp(ax_str s);
int64_t ax_str_byte_len(ax_str s);
ax_str  ax_str_concat(ax_str a, ax_str b);
int     ax_str_eq(ax_str a, ax_str b);
int     ax_str_cmp(ax_str a, ax_str b);
ax_str  ax_str_slice_cp(ax_str s, int64_t from, int64_t to);
int     ax_str_at(ax_str s, int64_t i, ax_str* out);
int     ax_str_codepoint_at(ax_str s, int64_t i, int32_t* out);
int     ax_str_from_codepoint(int32_t cp, ax_str* out);
int     ax_str_contains(ax_str s, ax_str sub);
ax_str* ax_str_split(ax_str s, ax_str sep, int64_t* n);
ax_str  ax_str_trim(ax_str s);
ax_str  ax_str_of_int(int64_t v);
ax_str  ax_str_of_uint(uint64_t v);
ax_str  ax_str_of_bool(int v);
ax_str  ax_str_of_f64(double v);
ax_str  ax_str_of_dec(double v);
void    ax_write_stdout(ax_str s);
void    ax_write_stderr(ax_str s);

/* 字符串构建器（B-13） */
typedef struct { uint8_t* buf; int64_t len, cap; } ax_sb;
void   ax_sb_init(ax_sb* b);
void   ax_sb_push(ax_sb* b, ax_str s);
void   ax_sb_push_cstr(ax_sb* b, const char* s);
void   ax_sb_push_int(ax_sb* b, int64_t v);
void   ax_sb_push_uint(ax_sb* b, uint64_t v);
void   ax_sb_push_f64(ax_sb* b, double v);
void   ax_sb_push_f64_n(ax_sb* b, double v, int32_t digits);
void   ax_sb_push_dec(ax_sb* b, ax_dec v);
void   ax_sb_push_dec_n(ax_sb* b, ax_dec v, int32_t digits);
ax_str ax_sb_finish(ax_sb* b);

/* ---------------- decimal（Rt-7） ---------------- */
ax_dec ax_dec_from_i64(int64_t v);
ax_dec ax_dec_from_parts(__int128 coeff, int32_t scale);
ax_dec ax_dec_lit(const char* digits, int32_t scale);
ax_dec ax_dec_parse(ax_str s, int* ok);
ax_str ax_dec_to_str(ax_dec v);
ax_dec ax_dec_add(ax_dec a, ax_dec b);
ax_dec ax_dec_sub(ax_dec a, ax_dec b);
ax_dec ax_dec_mul(ax_dec a, ax_dec b);
ax_dec ax_dec_div_exact(ax_dec a, ax_dec b, const char* f, int l, int c);
ax_dec ax_dec_div_round(ax_dec a, ax_dec b, int32_t digits, int mode);
ax_dec ax_dec_rem(ax_dec a, ax_dec b);
ax_dec ax_dec_round(ax_dec a, int32_t digits, int mode);
int    ax_dec_cmp(ax_dec a, ax_dec b);
int64_t ax_dec_to_i64(ax_dec a, const char* f, int l, int c);
double ax_dec_to_f64(ax_dec v);
double ax_dec_as_double(ax_dec v);

/* ---------------- 数组（Rt-8） ---------------- */
typedef struct { int64_t len, cap; void* data; } ax_arr;

void  ax_arr_init(ax_arr* a);
void  ax_arr_push(ax_arr* a, const void* v, int64_t esz);
void* ax_arr_at(ax_arr* a, int64_t i, int64_t esz, const char* f, int l, int c);
int   ax_arr_try_at(ax_arr* a, int64_t i, int64_t esz, void* out);
void  ax_arr_pop(ax_arr* a, void* out, int64_t esz, const char* f, int l, int c);
void  ax_arr_remove(ax_arr* a, int64_t i, int64_t esz, const char* f, int l, int c);
void  ax_arr_clear(ax_arr* a);
void  ax_arr_from(void* out, const void* src, int64_t n, int64_t esz);
ax_arr ax_arr_of(const void* src, int64_t n, int64_t esz);

/* ---------------- map（Rt-8，插入序） ---------------- */
typedef struct { uint64_t hash; void* k; void* v; } ax_mentry;

typedef struct {
    int64_t   len, cap;
    ax_mentry* entries;
    int32_t*  idx;        /* 开放寻址：存 entry 下标 +1，0 表示空 */
    int64_t   idxcap;
    uint64_t  seed;
    int64_t   ksz, vsz;
    int       khash;      /* 0=int64 1=string 2=bool 3=decimal 4=enum 5=ptr */
} ax_map;

void  ax_map_init(ax_map* m, int khash, int64_t ksz, int64_t vsz);
void* ax_map_get(ax_map* m, const void* k);
void* ax_map_get_chk(ax_map* m, const void* k, const char* f, int l, int c,
                     const char* keydesc);
int   ax_map_has(ax_map* m, const void* k);
void  ax_map_put(ax_map* m, const void* k, const void* v);
void  ax_map_remove(ax_map* m, const void* k);
void  ax_map_clear(ax_map* m);

/* ---------------- 装箱（Rt-9） ---------------- */
typedef struct ax_box {
    uint32_t tag;
    int64_t  size;
    void*    payload;
} ax_box;

#define AX_TAG_BOOL 1
#define AX_TAG_INT 2
#define AX_TAG_INT8 3
#define AX_TAG_INT16 4
#define AX_TAG_FLOAT 6
#define AX_TAG_DEC 7
#define AX_TAG_INT32 9
#define AX_TAG_UINT 10
#define AX_TAG_UINT8 11
#define AX_TAG_UINT16 12
#define AX_TAG_UINT32 13
#define AX_TAG_STRING 14

ax_box* ax_box_new(uint32_t tag, const void* payload, int64_t size);
int64_t ax_unbox_int(ax_box* b, const char* f, int l, int c);
double  ax_unbox_f64(ax_box* b, const char* f, int l, int c);
ax_dec  ax_unbox_dec(ax_box* b, const char* f, int l, int c);
ax_str  ax_unbox_str(ax_box* b, const char* f, int l, int c);
int     ax_unbox_bool(ax_box* b, const char* f, int l, int c);
void*   ax_unbox_ref(ax_box* b, uint32_t want, const char* f, int l, int c);

/* ---------------- 整数运算（带溢出检查，R104） ---------------- */
int64_t ax_add_i64(int64_t a, int64_t b, const char* f, int l, int c);
int64_t ax_sub_i64(int64_t a, int64_t b, const char* f, int l, int c);
int64_t ax_mul_i64(int64_t a, int64_t b, const char* f, int l, int c);
int64_t ax_div_i64(int64_t a, int64_t b, const char* f, int l, int c);
int64_t ax_mod_i64(int64_t a, int64_t b, const char* f, int l, int c);
int64_t ax_shl_i64(int64_t a, int64_t b, const char* f, int l, int c);
int64_t ax_shr_i64(int64_t a, int64_t b, const char* f, int l, int c);
int64_t ax_neg_i64(int64_t a, const char* f, int l, int c);
int64_t ax_narrow(int64_t v, int bits, int is_signed, const char* f, int l, int c);
double  ax_fadd(double a, double b);
double  ax_fsub(double a, double b);
double  ax_fmul(double a, double b);
double  ax_fdiv(double a, double b, const char* f, int l, int c);

/* ---------------- 错误 ---------------- */
typedef struct { int32_t code; ax_str msg; } ax_err;
ax_err ax_err_new(int32_t code, ax_str msg);

/* ---------------- 标准库 intrinsic ---------------- */
typedef struct { ax_str* kv; int64_t n; } ax_row;   /* 极简 Row：扁平键值对 */
typedef struct { uint8_t tag; union { ax_row ok; ax_err err; } u; } ax_res_row;
ax_row  ax_db_query(ax_str sql, ax_str arg);
ax_res_row ax_db_query_result(ax_str sql, int64_t arg);
int     ax_row_empty(ax_row r);
ax_str  ax_row_get(ax_row r, ax_str key);
ax_dec  ax_dec_parse_or(ax_str s, ax_dec fallback);
ax_str  ax_json_stringify(ax_box* v);

int64_t ax_now_ms(void);

/* 输入 / 解析 */
int   ax_read_line(ax_str* out);          /* 0=读到, 1=EOF */
int   ax_read_file(ax_str path, ax_str* out);

/* ---------------- 文件（Rt-13）
 * File 是内建句柄类型：语言层只允许 Read / ReadLine / Write / WriteLine / Close
 * 五个方法（文稿 11.3），不暴露 FILE*、不支持定位与模式切换。
 */
typedef struct { FILE* fp; } ax_file;

/* 模式：0=只读 1=写（新建或截断） 2=追加。失败时 tag=0 + error.IO。 */
typedef struct { uint8_t tag; union { ax_file* ok; ax_err err; } u; } ax_res_file;
ax_res_file ax_file_open_result(ax_str path, int mode);

int     ax_file_read_all(ax_file* f, ax_str* out);   /* 0=读到末尾 1=读失败 */
int     ax_file_read_line(ax_file* f, ax_str* out);  /* 0=读到一行 1=EOF 或失败 */
int64_t ax_file_write(ax_file* f, ax_str s);         /* >=0 写入字节数，<0 失败 */
int64_t ax_file_write_line(ax_file* f, ax_str s);    /* 同上，末尾补 '\n' */
void    ax_file_close(ax_file* f);                   /* 幂等；之后句柄不可再用 */

int   ax_parse_i64(ax_str s, int64_t* out);
int   ax_parse_f64(ax_str s, double* out);
int64_t str_to_i64(ax_str s);

#ifdef __cplusplus
}
#endif
#endif /* AXRT_H */
