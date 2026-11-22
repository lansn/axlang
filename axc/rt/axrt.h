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
/* 子串查找：按 UTF-8 码点索引返回，未找到返回 -1；空子串匹配索引 0 */
int64_t ax_str_index_of(ax_str s, ax_str sub);
int64_t ax_str_last_index_of(ax_str s, ax_str sub);
ax_str* ax_str_split(ax_str s, ax_str sep, int64_t* n);
ax_str  ax_str_trim(ax_str s);

/* 大小写转换 */
ax_str  ax_str_to_upper(ax_str s);
ax_str  ax_str_to_lower(ax_str s);

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
/* 字符串分割（对标 Go strings.Fields，按空白符分割，返回 ax_arr，元素为 ax_str） */
ax_arr ax_str_fields(ax_str s);

/* math 扩展（对标 Go math.Sign / math.Mod，纯 C11，无 GNU/Clang 扩展） */
int   ax_math_sign(double x);   /* x>0→1，x<0→-1，否则（含 0/NaN）→0 */
double ax_math_mod(double x, double y); /* 浮点余数，语义同 C fmod：符号同 x，幅值 < |y| */

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
/* 返回所有键的副本组成的 ax_arr（按插入顺序，元素大小为 m->ksz） */
ax_arr ax_map_keys(ax_map* m);

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
int64_t ax_time_since(int64_t t);                       /* 当前 Unix 毫秒 - t */

/* 时间解析（对标 Go time.Parse / C strptime，标准 C11 实现）：
 * 解析字符串 s 按格式 fmt（%Y %m %d %H %M %S 等），返回 Unix 毫秒；
 * 失败时 tag=0 + error（code=1，msg="bad time format"）。 */
typedef struct { uint8_t tag; union { int64_t ok; ax_err err; } u; } ax_res_time;
ax_res_time ax_time_parse_result(ax_str s, ax_str fmt);

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

/* 模式：0=只读 1=写（新建或截断） 2=追加。失败时 tag=0 + error.IO。 */
typedef struct { uint8_t tag; union { ax_file* ok; ax_err err; } u; } ax_res_file;
ax_res_file ax_file_open_result(ax_str path, int mode);

int     ax_file_read_all(ax_file* f, ax_str* out);   /* 0=读到末尾 1=读失败 */
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
/* 将整个字符串写入文件（创建/截断），0=成功 1=失败 */
int     ax_file_write_all(ax_str path, ax_str s);

/* ---------------- os 模块（对标 Go os/stdLib stdlib） ---------------- */
/* 读环境变量，未设置返回空字符串 */
ax_str  ax_os_getenv(ax_str name);
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

#ifdef __cplusplus
}
#endif
#endif /* AXRT_H */
