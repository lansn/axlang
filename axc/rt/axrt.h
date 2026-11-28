/* axrt.h -- axlang 运行时（规格书 §14）
 * 纯标准 C11，不使用任何 GNU/Clang 扩展（无 __int128、无 __builtin_*）。
 */
#ifndef AXRT_H
#define AXRT_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include <string.h>
#include <stdlib.h>
#include <stdio.h>
#include <math.h>
#include <stdatomic.h>   /* C11 原子：atomic_load/atomic_store/atomic_compare_exchange_strong */
#include <inttypes.h>    /* PRId64 等格式化宏 */
#include <time.h>        /* 时间函数 */
#include <errno.h>       /* errno */
#include <sys/stat.h>    /* stat/_stat, S_ISDIR/S_ISREG/_S_IFDIR/_S_IFREG */

#ifdef _WIN32
#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0600
#endif
#include <windows.h>
#include <io.h>
#include <direct.h>
#include <process.h>   /* _environ */
#include <sys/utime.h> /* _utimbuf, _utime */
#else
#include <sys/time.h>
#include <pthread.h>
#include <unistd.h>    /* getpid/getppid/gethostname/environ */
#endif

#define AX_I64_HIGH_BIT ((uint64_t)1 << 63)


/* ---------------- arena（区域分配器，规格书 8.4 / O-17…O-20） ----------------
 * 整块同生共死：adopt 只登记指针（所有权移交区域），free 一次性释放
 * 区域内全部对象与 arena 自身。ArenaRef<T> 在 C 层就是 T*。
 */
typedef struct ax_arena ax_arena;
ax_arena* ax_arena_create(void);
void*     ax_arena_adopt(ax_arena* ar, void* p);
void      ax_arena_free(ax_arena* ar);

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

typedef struct { int64_t hi; uint64_t lo; } ax_i128;

typedef struct { ax_i128 coeff; int32_t scale; int32_t _pad; } ax_dec;

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
/* string.ContainsRune：判断 s 是否包含码点 cp，对标 Go strings.ContainsRune，按 UTF-8 码点比较 */
int     ax_str_contains_rune(ax_str s, int32_t cp);
/* 字符集查找（对标 Go strings.ContainsAny / IndexAny，按 UTF-8 码点） */
int     ax_str_contains_any(ax_str s, ax_str chars);  /* s 是否包含 chars 中任意字符，1=是 0=否 */
int64_t ax_str_index_any(ax_str s, ax_str chars);     /* 首次出现 chars 中任意字符的码点索引，未找到返回 -1 */
int64_t ax_str_last_index_any(ax_str s, ax_str chars); /* 最后出现 chars 中任意字符的码点索引，未找到返回 -1 */
/* 子串计数（对标 Go strings.Count，非重叠） */
int64_t ax_str_count(ax_str s, ax_str substr);        /* 非重叠出现次数 */
/* 子串查找：按 UTF-8 码点索引返回，未找到返回 -1；空子串匹配索引 0 */
int64_t ax_str_index_of(ax_str s, ax_str sub);
int64_t ax_str_last_index_of(ax_str s, ax_str sub);
ax_str* ax_str_split(ax_str s, ax_str sep, int64_t* n);
ax_str  ax_str_trim(ax_str s);

/* 去除左侧/右侧字符集中的字符（对标 Go strings.TrimLeft/TrimRight，
 * 按 UTF-8 码点遍历，cutset 中任意字符会被去除；返回新字符串） */
ax_str  ax_str_trim_left(ax_str s, ax_str cutset);
ax_str  ax_str_trim_right(ax_str s, ax_str cutset);

/* 去除前缀/后缀（对标 Go strings.TrimPrefix/TrimSuffix，按字节匹配，
 * 去除后返回剩余部分；不匹配则返回原字符串副本） */
ax_str  ax_str_trim_prefix(ax_str s, ax_str prefix);
ax_str  ax_str_trim_suffix(ax_str s, ax_str suffix);

/* 分割（返回 ax_arr，元素为 ax_str） */
ax_str* ax_str_split_n(ax_str s, ax_str sep, int64_t n, int64_t* out_n);
ax_str* ax_str_split_after(ax_str s, ax_str sep, int64_t* n);

/* 标题大写：每个单词首字母大写（对标 Go strings.Title，仅处理 ASCII） */
ax_str  ax_str_title(ax_str s);

/* 字符映射：对每个字符调用 mapping 函数（对标 Go strings.Map，
 * mapping 为 int->int 的 C 函数指针；axlang int 映射为 int64_t） */
ax_str  ax_str_map(ax_str s, int64_t (*mapping)(int64_t));

/* 大小写转换 */
ax_str  ax_str_to_upper(ax_str s);
ax_str  ax_str_to_lower(ax_str s);

/* 单字符大小写转换（对标 Go strings.ToUpperRune/ToLowerRune / C toupper/tolower，
 * 仅处理 ASCII 范围：a-z<->A-Z，非 ASCII 码点原样返回；标准 C11 实现） */
int64_t ax_str_to_upper_rune(int64_t c);
int64_t ax_str_to_lower_rune(int64_t c);

/* 忽略大小写的字符串相等比较（对标 Go strings.EqualFold，仅处理 ASCII 大小写，
 * 非 ASCII 字符按字节比较；相等返回 1，否则返回 0） */
int     ax_str_equal_fold(ax_str a, ax_str b);

/* 前缀/后缀判断 */
int     ax_str_starts_with(ax_str s, ax_str prefix);
int     ax_str_ends_with(ax_str s, ax_str suffix);

/* 字符属性判断（对标 C <ctype.h> isdigit/isalpha/isspace/islower/isupper/isalnum，
 * 作用于整个字符串：所有字符均满足返回 1，否则返回 0；空字符串返回 0） */
int     ax_str_is_digit(ax_str s);
int     ax_str_is_alpha(ax_str s);
int     ax_str_is_space(ax_str s);
int     ax_str_is_lower(ax_str s);
int     ax_str_is_upper(ax_str s);
int     ax_str_is_alnum(ax_str s);

/* 连接数组（用 s 作为分隔符） */
ax_str  ax_str_join(ax_str sep, ax_str* parts, int64_t n);

/* 替换所有出现 */
ax_str  ax_str_replace(ax_str s, ax_str old, ax_str new);

/* 重复 n 次 */
ax_str  ax_str_repeat(ax_str s, int64_t n);
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
ax_dec ax_dec_from_parts(ax_i128 coeff, int32_t scale);
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
int   ax_arr_eq(const ax_arr* a, const ax_arr* b, int64_t esz);

/* ---------------- 字节数组（Rt-7，对标 Go bytes，元素为 uint8_t，C 层为 ax_arr） ---------------- */
/* 判断 s 是否包含子片 sub（按字节比较） */
bool    ax_bytes_contains(ax_arr s, ax_arr sub);
/* 判断两个字节数组是否相等（长度相同且所有字节相同） */
bool    ax_bytes_equal(ax_arr a, ax_arr b);
/* 查找子片 sub 在 s 中首次出现的索引，未找到返回 -1 */
int64_t ax_bytes_index(ax_arr s, ax_arr sub);
/* 按分隔符 sep 分割 s，返回 ax_arr（元素为 ax_arr，每个元素是一个子片） */
ax_arr  ax_bytes_split(ax_arr s, ax_arr sep);
/* 去除 s 首尾的空白字节，返回新数组 */
ax_arr  ax_bytes_trim_space(ax_arr s);
/* 将 s 中的 ASCII 小写字母转为大写，返回新数组 */
ax_arr  ax_bytes_to_upper(ax_arr s);
/* 将 s 中的 ASCII 大写字母转为小写，返回新数组 */
ax_arr  ax_bytes_to_lower(ax_arr s);
/* 将 s 重复 count 次，返回新数组 */
ax_arr  ax_bytes_repeat(ax_arr s, int64_t count);

/* 字符串分割（对标 Go strings.Fields，按空白符分割，返回 ax_arr，元素为 ax_str） */
ax_arr ax_str_fields(ax_str s);

/* math 扩展（对标 Go math.Sign / math.Mod，纯 C11，无 GNU/Clang 扩展） */
int   ax_math_sign(double x);   /* x>0→1，x<0→-1，否则（含 0/NaN）→0 */
double ax_math_mod(double x, double y); /* 浮点余数，语义同 C fmod：符号同 x，幅值 < |y| */
/* math.Log10 / math.Exp2 直接映射到 C11 log10/exp2，无需运行时封装；
 * math.IsNaN / math.IsInf 需布尔/符号判断，以下给 C11 封装。 */
int   ax_math_isnan(double x);   /* C11 isnan：x 为 NaN 返回 1，否则 0 */
int   ax_math_isinf(double x, int sign); /* C11 isinf：sign>0 只判 +Inf，sign<0 只判 -Inf，sign==0 判任意无穷 */

/* math 扩展（对标 Go math.Signbit / math.Copysign / math.Dim / math.Ldexp，纯 C11，无 GNU/Clang 扩展） */
int   ax_math_signbit(float x);  /* C11 signbit：x 为负数（含 -0 / -NaN）返回 1，否则 0 */
float ax_math_copysign(float x, float y); /* C11 copysign：x 的幅值、y 的符号 */
float ax_math_dim(float x, float y);      /* C11 fdim：max(x - y, 0) */
float ax_math_ldexp(float x, int exp);    /* C11 ldexp：x * 2^exp */
float ax_math_frexp_frac(double x);       /* math.Frexp：返回尾数 */
int   ax_math_frexp_exp(double x);        /* math.Frexp：返回 2 的幂指数 */

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
void  ax_map_delete_func(ax_map* m, int (*f)(const void* k, void* ctx), void* ctx);
/* 返回所有键的副本组成的 ax_arr（按插入顺序，元素大小为 m->ksz） */
ax_arr ax_map_keys(ax_map* m);

/* maps 模块（对标 Go maps，标准 C11 实现，Rt-8 续） */
/* 返回所有值的副本组成的 ax_arr（按插入顺序，元素大小为 m->vsz） */
ax_arr ax_map_values(ax_map* m);
/* 深拷贝 map：新建独立的 entries/idx，键值元素按 ax_map_put 浅拷贝，
 * 返回独立副本（对标 Go maps.Clone）。 */
ax_map ax_map_clone(const ax_map* m);
/* 将 src 的全部键值复制到 dst（已存在键被覆盖），对标 Go maps.Copy。 */
void ax_map_copy_into(ax_map* dst, const ax_map* src);
/* 判断两个 map 是否相等（同长度、同键且对应值相等），对标 Go maps.Equal。
 * 值比较：string 走 ax_str_eq，decimal 走 ax_dec_cmp==0，其余按字节 memcmp。 */
int ax_map_equal(const ax_map* a, const ax_map* b);

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
int     ax_err_is(ax_err err, ax_err target);
int     ax_err_as(ax_err err, ax_err* target);

/* ---------------- 标准库 intrinsic ---------------- */
typedef struct { ax_str* kv; int64_t n; } ax_row;   /* 极简 Row：扁平键值对 */
typedef struct { uint8_t tag; union { ax_row ok; ax_err err; } u; } ax_res_row;
ax_row  ax_db_query(ax_str sql, ax_str arg);
ax_res_row ax_db_query_result(ax_str sql, int64_t arg);
int     ax_row_empty(ax_row r);
ax_str  ax_row_get(ax_row r, ax_str key);
ax_dec  ax_dec_parse_or(ax_str s, ax_dec fallback);
ax_str  ax_json_stringify(ax_box* v);

/* json.MarshalIndent / json.Valid（对标 Go encoding/json，标准 C11 实现）：
 * ax_json_marshal_indent：带缩进序列化 JSON（prefix/indent 可为空串）；
 * ax_json_valid：判断 JSON 是否合法，1=合法 0=非法。 */
ax_str  ax_json_marshal_indent(ax_box* v, ax_str prefix, ax_str indent);
int     ax_json_valid(ax_str s);

int64_t ax_now_ms(void);
int64_t ax_time_now(void);  /* 当前 Unix 毫秒（time 类型，与 ax_now_ms 同值） */

/* 时间（time 类型，Unix 毫秒）：
 * ax_time_to_str 按 "YYYY-MM-DD HH:MM:SS" 格式化；
 * ax_time_format 指定 strftime 风格格式串（%Y %m %d %H %M %S 等）。 */
ax_str ax_time_to_str(int64_t ms);
ax_str ax_time_format(int64_t ms, const char* fmt);

/* 毫秒睡眠（对标 C <unistd.h> usleep / Windows Sleep） */
void ax_time_sleep(int64_t ms);

/* time.Unix / UnixMilli / Since（对标 Go time，time 为 Unix 毫秒，标准 C11 实现）： */
int64_t ax_time_from_unix(int64_t sec, int64_t nsec);   /* sec 秒 + nsec 纳秒 -> Unix 毫秒 */
int64_t ax_time_from_unix_ms(int64_t ms);               /* 毫秒 -> Unix 毫秒（与 FromUnixMs 同义） */
/* time.UnixMicro / time.UnixNano：微秒/纳秒 -> Unix 毫秒（整数除法，向零截断） */
int64_t ax_time_from_unix_us(int64_t us);               /* 微秒 -> Unix 毫秒 */
int64_t ax_time_from_unix_ns(int64_t nsec);             /* 纳秒 -> Unix 毫秒 */
int64_t ax_time_since(int64_t t);                       /* 当前 Unix 毫秒 - t */

/* 时间解析（对标 Go time.Parse / C strptime，标准 C11 实现）：
 * 解析字符串 s 按格式 fmt（%Y %m %d %H %M %S 等），返回 Unix 毫秒；
 * 失败时 tag=0 + error（code=1，msg="bad time format"）。 */
typedef struct { uint8_t tag; union { int64_t ok; ax_err err; } u; } ax_res_time;
ax_res_time ax_time_parse_result(ax_str s, ax_str fmt);

/* time.Date / time.Until（对标 Go time，标准 C11 实现，time 为 Unix 毫秒）：
 * ax_time_date 按 UTC 组装年月日时分秒纳秒 -> Unix 毫秒；
 * ax_time_until 返回 t - 当前 Unix 毫秒（t 在将来为正，已过去为负）。 */
int64_t ax_time_date(int64_t year, int64_t month, int64_t day,
                     int64_t hour, int64_t min, int64_t sec, int64_t nsec);
int64_t ax_time_until(int64_t t);   /* t - now */

/* time.Round / time.Truncate（对标 Go time.Round/Truncate，按 d 毫秒间隔舍入，
 * d <= 0 时返回 t 本身；标准 C11，无 GNU/Clang 扩展）： */
int64_t ax_time_round(int64_t t, int64_t d);   /* 四舍五入到 d 的最近整数倍 */
int64_t ax_time_truncate(int64_t t, int64_t d); /* 向零截断到 d 的整数倍 */

/* json.Parse 的 result<map<string, object>> 形态（文稿 11.3）：
   ok 携带 ax_map（string → ax_box*），fail 携带解析错误（code=1 + 位置信息）。 */
typedef struct { uint8_t tag; union { ax_map ok; ax_err err; } u; } ax_res_map;
ax_res_map ax_json_parse_result(ax_str s);

/* 输入 / 解析 */
int   ax_read_line(ax_str* out);          /* 0=读到, 1=EOF */
int   ax_read_file(ax_str path, ax_str* out);

/* ---------------- 文件（Rt-13）
 * File 是内建句柄类型：语言层只允许 Read / ReadLine / Write / WriteLine / Close
 * 五个方法（文稿 11.3），不暴露 FILE*、不支持定位与模式切换。
 */
typedef struct { FILE* fp; } ax_file;

/* ---------------- bufio 模块（对标 Go bufio，P2） ----------------
 * axlang 的 Reader/Writer 即 File 句柄（ax_file*），bufio 函数接收并返回 File。
 * 本版本建立 API：在 C 层返回同一句柄，缓冲由底层 FILE* 的 stdio 缓冲提供。
 * 后续版本可在此基础上增加显式 ax_buf_reader/ax_buf_writer 结构体与可配置缓冲大小。
 */
ax_file* ax_file_buf_reader(ax_file* f);  /* 创建缓冲读取器，对标 Go bufio.NewReader */
ax_file* ax_file_buf_writer(ax_file* f);  /* 创建缓冲写入器，对标 Go bufio.NewWriter */
ax_file* ax_file_buf_scanner(ax_file* f); /* 创建扫描器，对标 Go bufio.NewScanner */

/* 模式：0=只读 1=写（新建或截断） 2=追加。失败时 tag=0 + error.IO。 */
typedef struct { uint8_t tag; union { ax_file* ok; ax_err err; } u; } ax_res_file;
ax_res_file ax_file_open_result(ax_str path, int mode);

/* 便捷打开函数：成功返回 ax_file* 句柄，失败返回 NULL（ax 标准库 ax_std_* 包装层使用）。
 * 与 ax_file_open_result 等价，但直接返回句柄而不暴露 result 结构。 */
ax_file* ax_file_open(ax_str path);       /* mode=0 只读 */
ax_file* ax_file_create(ax_str path);     /* mode=1 写（新建或截断） */
ax_file* ax_file_append(ax_str path);     /* mode=2 追加 */

int     ax_file_read_all(ax_file* f, ax_str* out);   /* 0=读到末尾 1=读失败 */
ax_str  ax_file_read_all_val(ax_file f);             /* 按值返回 ax_str，供 ax 标准库调用 */
int     ax_file_read_line(ax_file* f, ax_str* out);  /* 0=读到一行 1=EOF 或失败 */
int64_t ax_file_write(ax_file* f, ax_str s);         /* >=0 写入字节数，<0 失败 */
int64_t ax_file_write_line(ax_file* f, ax_str s);    /* 同上，末尾补 '\n' */
void    ax_file_close(ax_file* f);                   /* 幂等；之后句柄不可再用 */

/* 文件/目录操作（对标 C <stdio.h>/<stdlib.h> remove/rename/mkdir） */
int     ax_file_remove(ax_str path);                 /* 0=成功 1=失败 */
int     ax_file_rename(ax_str old_path, ax_str new_path); /* 0=成功 1=失败 */
int     ax_file_exists(ax_str path);                 /* 1=存在 0=不存在 */
int     ax_dir_create(ax_str path);                  /* 0=成功 1=失败 */
/* 文件/目录类型判断（对标 Go os.Stat+FileInfo.IsDir）：1=是 0=否 */
int     ax_file_is_dir(ax_str path);                 /* 1=目录 0=否或不存在 */
int     ax_file_is_file(ax_str path);                /* 1=常规文件 0=否或不存在 */
/* 文件大小（字节），对标 Go os.Stat+FileInfo.Size；失败返回 -1（标准 C11 stat/st_size）。 */
int64_t ax_file_size(ax_str path);                   /* 字节数，失败返回 -1 */
/* 最后修改时间（Unix 毫秒），对标 Go os.Stat+FileInfo.ModTime；失败返回 -1。 */
int64_t ax_file_modtime(ax_str path);                /* Unix 毫秒，失败返回 -1 */
/* 将整个字符串写入文件（创建/截断），0=成功 1=失败 */
int     ax_file_write_all(ax_str path, ax_str s);

/* 修改文件权限，对标 Go os.Chmod / C chmod；0=成功 1=失败。 */
int     ax_file_chmod(ax_str path, int mode);
/* 修改访问/修改时间，对标 Go os.Chtimes；0=成功 1=失败。 */
int     ax_file_chtimes(ax_str path, int64_t atime, int64_t mtime);
/* 创建临时文件，对标 Go os.CreateTemp；返回 result<File>。 */
ax_res_file ax_file_tempfile(ax_str dir, ax_str prefix);

/* ---------------- os 模块（对标 Go os/stdLib stdlib） ---------------- */
/* 读环境变量，未设置返回空字符串 */
ax_str  ax_os_getenv(ax_str name);
/* 用户主目录，对标 Go os.UserHomeDir：Windows 读 USERPROFILE，Unix 读 HOME；未设置返回空串 */
ax_str  ax_os_user_home_dir(void);
/* 终止进程，对标 C exit() */
void    ax_os_exit(int code);
/* 命令行参数：main 包装器会在进入 main 前调用 ax_os_set_args 设置；
 * ax_os_args 返回 argv[1..] 组成的 ax_arr（元素为 ax_str，浅拷贝）。 */
void    ax_os_set_args(ax_arr* args);
ax_arr  ax_os_args(void);
/* 当前工作目录：失败返回空字符串 */
ax_str  ax_os_getwd(void);
/* 改变当前工作目录：0=成功，1=失败 */
int     ax_os_chdir(ax_str path);

/* os 进程/环境扩展（对标 Go os）： */
int     ax_os_getpid(void);     /* 当前进程 ID */
int     ax_os_getppid(void);    /* 父进程 ID（Windows 下返回 0） */
ax_str  ax_os_hostname(void);   /* 主机名，失败返回空串 */
ax_arr  ax_os_environ(void);    /* 环境变量列表 "KEY=VALUE"，元素为 ax_str */
/* 环境变量读写（对标 Go os.Setenv/os.Unsetenv）：0=成功，1=失败 */
int     ax_os_setenv(ax_str name, ax_str value);  /* 设置环境变量 */
int     ax_os_unsetenv(ax_str name);              /* 删除环境变量 */

/* os 缓存目录 / 页面 / 用户身份 / 可执行文件扩展（对标 Go os.UserCacheDir 等，
 * Windows 下 uid/gid/getgroups 暂无对应，返回 0 / 空数组；标准 C11 + 平台 API，无 GNU/Clang 扩展） */
ax_str  ax_os_user_cache_dir(void);    /* 用户缓存目录：Windows LOCALAPPDATA/USERPROFILE，Unix XDG_CACHE_HOME/$HOME/.cache */
int     ax_os_getpagesize(void);       /* 系统页面大小（字节） */
int     ax_os_getuid(void);            /* 用户 ID（Windows 返回 0） */
int     ax_os_getgid(void);            /* 组 ID（Windows 返回 0） */
int     ax_os_geteuid(void);           /* 有效用户 ID（Windows 返回 0） */
int     ax_os_getegid(void);           /* 有效组 ID（Windows 返回 0） */
ax_arr  ax_os_getgroups(void);         /* 用户所属组 ID 列表（Windows 返回空数组） */
ax_str  ax_os_executable(void);        /* 当前可执行文件路径 */

/* ---------------- rand 模块（对标 Go math/rand）------------ */
/* 设置随机种子 */
void    ax_rand_seed(int64_t seed);
/* [0, n) 范围内的随机整数；n<=0 时返回 0 */
int64_t ax_rand_int(int64_t n);
/* [0, 1) 范围内的随机浮点数 */
double  ax_rand_float(void);

/* ---------------- sort 模块（对标 Go sort） ---------------- */
/* 原地升序排序（标准 C qsort）；data 指向 ax_arr.data，len 为元素个数 */
void    ax_sort_int(int64_t* data, int64_t len);
void    ax_sort_str(ax_str* data, int64_t len);
/* 浮点数切片升序排序，对标 Go sort.Float64s；标准 C qsort，NaN 位置未指定（与 Go 一致）。 */
void    ax_sort_float(double* data, int64_t len);
/* 判断整数/字符串切片是否已按升序排列，对标 Go sort.IntsAreSorted / sort.StringsAreSorted；
 * 返回 1=已排序 0=未排序；空/单元素切片返回 1。标准 C11 实现。 */
int     ax_sort_ints_are_sorted(const int64_t* data, int64_t len);
int     ax_sort_strings_are_sorted(const ax_str* data, int64_t len);
/* 二分查找：在已升序排列的整数/字符串切片中查找 x，对标 Go sort.SearchInts / sort.SearchStrings；
 * 找到返回下标，未找到返回 -1。要求输入切片已按升序排序，否则结果未定义。标准 C11 实现。 */
int64_t ax_sort_search_ints(const int64_t* data, int64_t len, int64_t x);
int64_t ax_sort_search_strings(const ax_str* data, int64_t len, ax_str x);
/* 数组原地反转，对标 Go slices.Reverse。data 指向 ax_arr.data，len 为元素个数，esz 为元素字节大小。 */
void    ax_arr_reverse(void* data, int64_t len, int64_t esz);

/* 数组深拷贝，返回独立副本（对标 clone 语义，原数组不受影响） */
ax_arr  ax_arr_clone(const ax_arr* a, int64_t esz);
/* 在索引 i 处插入元素 v，后续元素后移（对标 slices.Insert） */
void    ax_arr_insert(ax_arr* a, int64_t i, const void* v, int64_t esz, const char* f, int l, int c);
/* 用指定值 v 填充所有元素（对标 slices.Fill 语义） */
void    ax_arr_fill(ax_arr* a, const void* v, int64_t esz);
/* 调整长度为 n，不足时零初始化新增元素，超出时截断 */
void    ax_arr_resize(ax_arr* a, int64_t n, int64_t esz);
/* 数组合并：将 b 的所有元素追加到 a 末尾 */
void    ax_arr_merge(ax_arr* a, const ax_arr* b, int64_t esz);
void    ax_arr_delete_range(ax_arr* a, int64_t i, int64_t j, int64_t esz, const char* f, int l, int c);
/* int[] 聚合：求最小/最大/求和，空数组分别返回 0/0/0 */
int64_t ax_arr_min_int(const int64_t* data, int64_t len);
int64_t ax_arr_max_int(const int64_t* data, int64_t len);
int64_t ax_arr_sum_int(const int64_t* data, int64_t len);
/* float[] 聚合：求最小/最大/求和，空数组分别返回 0/0/0 */
double  ax_arr_min_float(const double* data, int64_t len);
double  ax_arr_max_float(const double* data, int64_t len);
double  ax_arr_sum_float(const double* data, int64_t len);

/* ---------------- csv 模块（对标 Go encoding/csv） ----------------
 * Reader/Writer 对标 Go io.Reader/io.Writer，在 axlang 中均为 File 句柄（ax_file*）。
 * ax_csv_read_all：读取整个 CSV 文件，返回 string[][]（ax_arr 的 ax_arr，内层元素为 ax_str）。
 * ax_csv_write：将 string[] 记录按逗号连接并写入一行，返回写入字节数。 */
ax_arr  ax_csv_read_all(ax_file* f);              /* 外层 ax_arr，每个元素为 ax_arr（内层 ax_str） */
int64_t ax_csv_write(ax_file* f, ax_arr record);  /* record 元素为 ax_str，返回写入字节数 */

/* ---------------- slices 模块（对标 Go slices） ---------------- */
/* 在 int[] 中查找 x：找到返回 1，否则 0 */
int     ax_slice_contains_int(const int64_t* data, int64_t len, int64_t x);
/* 在 string[] 中查找 x：找到返回 1，否则 0（按 ax_str_cmp 字节序比较） */
int     ax_slice_contains_string(const ax_str* data, int64_t len, ax_str x);
/* 在 int[] 中查找 x：返回下标，未找到返回 -1 */
int64_t ax_slice_index_int(const int64_t* data, int64_t len, int64_t x);
/* 在 string[] 中查找 x：返回下标，未找到返回 -1（按 ax_str_cmp 字节序比较） */
int64_t ax_slice_index_string(const ax_str* data, int64_t len, ax_str x);

/* ---------------- filepath 模块（对标 Go path/filepath） ---------------- */
/* 平台路径分隔符：Unix "/"，Windows "\\" */
#if defined(_WIN32)
#define AX_FILEPATH_SEP "\\"
#else
#define AX_FILEPATH_SEP "/"
#endif

/* 将多个路径元素用平台分隔符连接（按值传 ax_arr，函数内部取地址） */
ax_str  ax_filepath_join(ax_arr parts);    /* parts 元素为 ax_str，esz = sizeof(ax_str) */
/* 返回目录部分（去掉最后一个分隔符及其后的内容；无分隔符返回 "."） */
ax_str  ax_filepath_dir(ax_str path);
/* 返回最后一个分隔符后的内容（无分隔符返回原字符串） */
ax_str  ax_filepath_base(ax_str path);
/* 返回扩展名（含点，如 ".txt"；无扩展名或以分隔符结尾返回空串） */
ax_str  ax_filepath_ext(ax_str path);
/* 分割路径为 [dir, base] 数组（对标 Go filepath.Split） */
ax_arr  ax_filepath_split(ax_str path);
/* 路径规范化（去除 .、..、重复分隔符） */
ax_str  ax_filepath_clean(ax_str path);
/* 是否为绝对路径（Unix 以 / 开头，Windows 以 盘符: 或 \\ 开头） */
int     ax_filepath_is_abs(ax_str path);
/* 转为绝对路径（需当前工作目录，失败返回原路径） */
ax_str  ax_filepath_abs(ax_str path);

/* 计算从 basepath 到 targpath 的相对路径（对标 Go filepath.Rel，纯 C11） */
ax_str  ax_filepath_rel(ax_str basepath, ax_str targpath);

/* 解析符号链接，返回规范路径（对标 Go filepath.EvalSymlinks，C realpath）。
 * 递归解析路径中所有软链接组件；失败时返回原路径副本。 */
ax_str  ax_filepath_eval_symlinks(ax_str path);

/* 按模式匹配文件名，返回匹配的字符串数组（对标 Go filepath.Glob，C glob）。
 * 支持 * ? [...] 通配符；结果按字节序排序，不重复。 */
ax_arr  ax_filepath_glob(ax_str pattern);

/* 递归遍历目录树，返回目录下所有条目的完整路径数组（对标 Go filepath.Walk，C nftw）。
 * axlang 不支持闭包回调，故直接返回全部条目路径。 */
ax_arr  ax_filepath_walk(ax_str root);

int   ax_parse_i64(ax_str s, int64_t* out);
int   ax_parse_f64(ax_str s, double* out);
int   ax_parse_bool(ax_str s);          /* "true"/"1" → 1，"false"/"0" → 0，否则 0 */
int64_t str_to_i64(ax_str s);

/* strconv 进制解析/格式化（对标 Go strconv.ParseInt/FormatInt，标准 C11 实现）：
 * base 支持 2、8、10、16；base<=0 时按字符串前缀自动判断（0x=16, 0=8, 否则=10）。 */
int   ax_parse_int_base(ax_str s, int base);   /* 失败返回 0，不设置错误码（与 Atoi 保持一致） */
ax_str ax_str_of_int_base(int64_t v, int base); /* base<=0 时按 10 进制输出 */

/* strconv 无符号进制解析/格式化（对标 Go strconv.ParseUint/FormatUint，标准 C11 实现）：
 * axlang 无 uint 类型，故返回/接收 int（int64），值范围受 int64 限制；
 * base 支持 2、8、10、16；base<=0 时按字符串前缀自动判断。 */
int64_t ax_parse_uint_base(ax_str s, int base); /* 失败或超出 int64 范围返回 0 */
ax_str  ax_str_of_uint_base(int64_t v, int base); /* v 按无符号解释并格式化 */

/* strconv 字符串引用处理（对标 Go strconv.Quote/Unquote/CanBackquote，标准 C11 实现）：
 * Quote 返回双引号括起来的 Go 字符串字面量，使用 Go 转义序列；
 * Unquote 解释双引号括起来的 Go 字符串字面量，失败返回空串；
 * CanBackquote 报告字符串能否作为单行反引号字符串表示（不含除 tab 外的控制字符）。 */
int     ax_str_can_backquote(ax_str s);   /* 1=可作为反引号字符串 0=不可 */
ax_str  ax_str_quote(ax_str s);           /* 双引号括起来的 Go 字符串字面量 */
ax_str  ax_str_unquote(ax_str s);         /* 解释双引号字符串字面量，失败返回空串 */


/* ---------------- arena（区域分配器，规格书 8.4 / O-17…O-20） ----------------
 * 整块同生共死：adopt 只登记指针（所有权移交区域），free 一次性释放
 * 区域内全部对象与 arena 自身。ArenaRef<T> 在 C 层就是 T*。
 */
typedef struct ax_arena ax_arena;
ax_arena* ax_arena_create(void);
void*     ax_arena_adopt(ax_arena* ar, void* p);
void      ax_arena_free(ax_arena* ar);

/* ---------------- 并发（线程 + 阻塞队列）----------------
 * axlang 并发模型：显式线程 + 线程安全阻塞队列，线程间不共享可变对象，
 * 仅通过 queue 传递消息。避免 async/await 状态机和手动锁。
 */
typedef struct ax_thread ax_thread;
typedef struct ax_queue  ax_queue;

/* 启动并发执行体：func 会在新线程中执行，arg 作为唯一实参。返回线程句柄。 */
typedef void (*ax_thread_fn)(void*);
ax_thread* ax_thread_create(ax_thread_fn func, void* arg);
/* 等待线程结束（阻塞）。 */
void       ax_thread_join(ax_thread* t);

/* 创建元素大小为 elem_size 的有界阻塞队列（内部固定 256 槽）。 */
ax_queue*  ax_queue_create(int64_t elem_size);
/* 推入元素：队列满时阻塞，直到有空位。 */
void       ax_queue_push(ax_queue* q, const void* data);
/* 弹出元素：队列空且未关闭时阻塞；成功返回 0，队列已关闭且为空返回 1。 */
int        ax_queue_pop(ax_queue* q, void* out);
/* 非阻塞弹出：队列有数据返回 0 并写出，队列为空（无论是否已关闭）立即返回 1，不阻塞。 */
int        ax_queue_try_pop(ax_queue* q, void* out);
/* 关闭队列：后续 pop 在队列耗尽后返回 1，作为结束信号。 */
void       ax_queue_close(ax_queue* q);

/* ---------------- sync 模块（对标 Go sync，P2） ----------------
 * 基于平台原语：Windows CRITICAL_SECTION / SRWLOCK / CONDITION_VARIABLE，
 * 非 Windows pthread_mutex / pthread_rwlock / pthread_cond。
 * axlang 尚无闭包/函数值，Once.Do 的回调由上层在 C 层之外处理，
 * 这里仅提供原语绑定。 */

/* 互斥锁（不可重入）：Lock/Unlock。 */
typedef struct {
#ifdef _WIN32
    CRITICAL_SECTION cs;
#else
    pthread_mutex_t mu;
#endif
    int initialized;  /* 懒初始化标志：零值结构体不能直接加锁，首次锁定时平台初始化 */
} ax_mutex;
void ax_mutex_lock(ax_mutex* m);
void ax_mutex_unlock(ax_mutex* m);

/* 读写锁：Lock/Unlock（写），RLock/Runlock（读）。 */
typedef struct {
#ifdef _WIN32
    SRWLOCK srw;
#else
    pthread_rwlock_t rw;
#endif
    int initialized;  /* 懒初始化标志：pthread_rwlock 零值不可直接用，首次锁定时初始化 */
} ax_rwmutex;
void ax_rwmutex_lock(ax_rwmutex* rw);
void ax_rwmutex_unlock(ax_rwmutex* rw);
void ax_rwmutex_rlock(ax_rwmutex* rw);
void ax_rwmutex_runlock(ax_rwmutex* rw);

/* 等待组：Add(delta)/Done()/Wait()。 */
typedef struct {
#ifdef _WIN32
    CRITICAL_SECTION cs;
    CONDITION_VARIABLE cv;
#else
    pthread_mutex_t mu;
    pthread_cond_t cv;
#endif
    int64_t counter;
    int initialized;  /* 懒初始化标志：零值结构体不能直接用，首次操作时平台初始化 */
} ax_waitgroup;
void ax_waitgroup_add(ax_waitgroup* wg, int64_t delta);
void ax_waitgroup_done(ax_waitgroup* wg);
void ax_waitgroup_wait(ax_waitgroup* wg);

/* Once：Do() 保证初始化逻辑仅执行一次。done 标志由 axlang 上层串行化访问，
 * C 层仅提供原语绑定（无闭包时无回调可传）。 */
typedef struct {
#ifdef _WIN32
    INIT_ONCE io;
#else
    pthread_once_t once;
#endif
    int64_t done;
} ax_once;
void ax_once_do(ax_once* o);

/* ---------------- sync/atomic 模块（对标 Go sync/atomic，P2） ----------------
 * 原子整数操作，基于标准 C11 <stdatomic.h>（axrt.c 内部包含）。
 * addr 在 axlang 层传 *int 变量的地址，C 层形参为 volatile int64_t*，
 * 内部经标准 C11 atomic_* 操作。 */
int64_t ax_atomic_add_int(volatile int64_t* addr, int64_t delta);
int64_t ax_atomic_load_int(volatile int64_t* addr);
void    ax_atomic_store_int(volatile int64_t* addr, int64_t val);
int     ax_atomic_compare_and_swap_int(volatile int64_t* addr, int64_t old, int64_t new);

/* ax_std_atomic_*：ax 编译器 codegen 的 module_call 入口（void* 兼容 ax_stdlib.h 声明） */
int64_t ax_std_atomic_add_int(void* addr, int64_t delta);
int64_t ax_std_atomic_load_int(void* addr);
void    ax_std_atomic_store_int(void* addr, int64_t val);
int     ax_std_atomic_compare_and_swap_int(void* addr, int64_t old, int64_t new);

/* ---------------- slices 模块扩展（对标 Go slices，ax_arr 形式） ---------------- */
/* 在 int[] 中判断是否包含 x：1=包含 0=不包含 */
int     ax_std_slice_contains_int(ax_arr s, int64_t x);
/* 在 string[] 中判断是否包含 x：1=包含 0=不包含 */
int     ax_std_slice_contains_string(ax_arr s, ax_str x);
/* 在 int[] 中查找 x 的下标：找到返回下标，未找到返回 -1 */
int64_t ax_std_slice_index_int(ax_arr s, int64_t x);
/* 在 string[] 中查找 x 的下标：找到返回下标，未找到返回 -1 */
int64_t ax_std_slice_index_string(ax_arr s, ax_str x);
/* 原地反转数组（对标 slices.Reverse）。int[] 与 string[] 分别有独立入口，C 内部硬编码 esz，ax 层无需传入。s 必须是左值 */
void    ax_std_slice_reverse_int(ax_arr* s);
void    ax_std_slice_reverse_str(ax_arr* s);
/* 返回数组的独立副本（对标 slices.Clone） */
ax_arr  ax_std_slice_clone_int(ax_arr s);
ax_arr  ax_std_slice_clone_str(ax_arr s);
/* 将所有元素设为指定值 v（对标 slices.Fill） */
void    ax_std_slice_fill_int(ax_arr* s, int64_t v);
void    ax_std_slice_fill_str(ax_arr* s, ax_str v);
/* 在索引 i 处插入元素 v，后续元素后移（对标 slices.Insert） */
void    ax_std_slice_insert_int(ax_arr* s, int64_t i, int64_t v);
void    ax_std_slice_insert_str(ax_arr* s, int64_t i, ax_str v);
/* 删除索引 [i, j) 范围的元素 */
void    ax_std_slice_delete_range_int(ax_arr* s, int64_t i, int64_t j);
void    ax_std_slice_delete_range_str(ax_arr* s, int64_t i, int64_t j);
/* int[] 聚合：求最小/最大/求和，空数组返回 0/0/0 */
int64_t ax_std_slice_min_int(ax_arr s);
int64_t ax_std_slice_max_int(ax_arr s);
int64_t ax_std_slice_sum_int(ax_arr s);
/* float[] 聚合：求最小/最大/求和，空数组返回 0/0/0 */
double  ax_std_slice_min_float(ax_arr s);
double  ax_std_slice_max_float(ax_arr s);
double  ax_std_slice_sum_float(ax_arr s);

/* ---------------- base64 模块（对标 Go encoding/base64，标准 C11 实现） ---------------- */
/* Base64 标准编码（RFC 4648，无换行）：输入原始字节，输出 Base64 字符串 */
ax_str  ax_base64_encode(ax_arr input);
/* Base64 标准解码：输入 Base64 字符串，输出原始字节；失败返回空数组 */
ax_arr  ax_base64_decode(ax_str input);

/* ---------------- hex 模块（对标 Go encoding/hex，标准 C11 实现） ---------------- */
/* Hex 编码（小写）：输入原始字节，输出十六进制字符串 */
ax_str  ax_hex_encode(ax_arr input);
/* Hex 解码：输入十六进制字符串，输出原始字节；失败返回空数组 */
ax_arr  ax_hex_decode(ax_str input);

/* ---------------- crypto/md5 模块（对标 Go crypto/md5，RFC 1321，标准 C11 实现） ---------------- */
/* MD5 哈希：输入数据，返回 16 字节摘要（ax_arr，元素为 uint8_t） */
ax_arr  ax_md5_sum(ax_arr data);

/* ---------------- crypto/sha1 模块（对标 Go crypto/sha1，RFC 3174，标准 C11 实现） ---------------- */
/* SHA1 哈希：输入数据，返回 20 字节摘要（ax_arr，元素为 uint8_t） */
ax_arr  ax_sha1_sum(ax_arr data);

/* ---------------- crypto/sha256 模块（对标 Go crypto/sha256，FIPS 180-4，标准 C11 实现） ---------------- */
/* SHA256 哈希：输入数据，返回 32 字节摘要（ax_arr，元素为 uint8_t） */
ax_arr  ax_sha256_sum(ax_arr data);

/* ---------------- flag 模块（对标 Go flag，标准 C11 实现） ---------------- */
/* 标志类型 */
#define AX_FLAG_BOOL    1
#define AX_FLAG_INT     2
#define AX_FLAG_STRING  3
#define AX_FLAG_MAX     64

/* 标志定义描述符（内部使用，ax 层不直接访问） */
typedef struct {
    int     kind;       /* AX_FLAG_BOOL / AX_FLAG_INT / AX_FLAG_STRING */
    ax_str  name;
    ax_str  usage;
    int64_t def_int;
    ax_str  def_str;
    int     def_bool;
    int64_t val_int;
    ax_str  val_str;
    int     val_bool;
    int     is_set;     /* 是否已由命令行设置 */
} ax_flag_def;

/* 注册 bool 标志：0=成功，非 0=失败（如重复注册或超出上限） */
int     ax_std_flag_define_bool(ax_str name, int64_t def_val, ax_str usage);
/* 注册 int 标志：0=成功，非 0=失败 */
int     ax_std_flag_define_int(ax_str name, int64_t def_val, ax_str usage);
/* 注册 string 标志：0=成功，非 0=失败 */
int     ax_std_flag_define_string(ax_str name, ax_str def_val, ax_str usage);
/* 解析命令行参数（从 ax_os_args() 获取）：0=成功，非 0=失败（如未知标志） */
int     ax_flag_parse(void);
/* 获取 bool 标志的当前值，未找到返回 0 */
int     ax_std_flag_bool(ax_str name);
/* 获取 int 标志的当前值，未找到返回 0 */
int64_t ax_std_flag_int(ax_str name);
/* 获取 string 标志的当前值，未找到返回空字符串 */
ax_str  ax_std_flag_string(ax_str name);
/* 返回已通过命令行设置的标志数量 */
int     ax_flag_n_flag(void);
/* 返回第 i 个非标志位置参数，索引越界返回空字符串 */
ax_str  ax_std_flag_arg(int64_t i);
/* 返回非标志位置参数的数量 */
int64_t ax_flag_n_arg(void);
/* 返回所有非标志位置参数（ax_arr，元素为 ax_str） */
ax_arr  ax_flag_args(void);

/* ---------------- env 模块（对标 Go os 环境变量增值功能，标准 C11 实现） ---------------- */
/* 展开字符串中的环境变量引用（$VAR 或 ${VAR}，对标 Go os.ExpandEnv，
 * 未设置的变量展开为空字符串） */
ax_str  ax_env_expand(ax_str s);
/* 清除所有环境变量（对标 Go os.Clearenv）：0=成功，非 0=失败 */
int     ax_env_clearenv(void);
/* 判断环境变量是否存在：1=存在 0=不存在 */
int     ax_env_exists(ax_str key);
/* 返回所有环境变量 "KEY=VALUE" 数组（对标 Go os.Environ，ax_arr 元素为 ax_str） */
ax_arr  ax_env_all(void);

#ifdef __cplusplus
}
#endif
#endif /* AXRT_H */
