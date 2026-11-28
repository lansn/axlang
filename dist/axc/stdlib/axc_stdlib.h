#ifndef AXC_STDLIB_H
#define AXC_STDLIB_H

/* axc_stdlib.h -- axlang 标准库 C 接口头文件
 *
 * 声明所有 ax_std_* 前缀的标准库 C 接口函数。
 * 底层仍依赖 axrt.h / axrt.c 提供的 C 运行时原语。
 * 纯标准 C11，不使用任何 GNU/Clang 扩展。
 */

#include "axrt.h"

/* ================================================================
 * sync 模块（对标 Go sync）
 * ax_std_* 函数由 axrt 原语包装
 * ================================================================ */

void ax_std_mutex_lock(void* m);
void ax_std_mutex_unlock(void* m);
void ax_std_rwmutex_lock(void* rw);
void ax_std_rwmutex_unlock(void* rw);
void ax_std_rwmutex_rlock(void* rw);
void ax_std_rwmutex_runlock(void* rw);
void ax_std_waitgroup_add(void* wg, int64_t delta);
void ax_std_waitgroup_done(void* wg);
void ax_std_waitgroup_wait(void* wg);
void ax_std_once_do(void* o);

/* ================================================================
 * atomic 模块（对标 Go sync/atomic）
 * ================================================================ */

int64_t ax_std_atomic_add_int(void* addr, int64_t delta);
int64_t ax_std_atomic_load_int(void* addr);
void ax_std_atomic_store_int(void* addr, int64_t val);
int ax_std_atomic_compare_and_swap_int(void* addr, int64_t old, int64_t new);

/* ================================================================
 * string 模块（对标 Go strings / C string.h / C ctype.h）
 * ================================================================ */

int ax_std_string_compare(ax_str a, ax_str b);
int ax_std_string_contains(ax_str s, ax_str sub);
int64_t ax_std_string_index_of(ax_str s, ax_str sub);
int64_t ax_std_string_last_index_of(ax_str s, ax_str sub);
int64_t ax_std_string_count(ax_str s, ax_str substr);
ax_str ax_std_string_trim(ax_str s);
ax_str ax_std_string_to_upper(ax_str s);
ax_str ax_std_string_to_lower(ax_str s);
ax_str ax_std_string_repeat(ax_str s, int64_t n);
ax_str ax_std_string_replace(ax_str s, ax_str old, ax_str new);
int ax_std_string_starts_with(ax_str s, ax_str prefix);
int ax_std_string_ends_with(ax_str s, ax_str suffix);
int ax_std_string_is_digit(ax_str s);
int ax_std_string_is_alpha(ax_str s);
int ax_std_string_is_space(ax_str s);
int ax_std_string_is_lower(ax_str s);
int ax_std_string_is_upper(ax_str s);
int ax_std_string_is_alnum(ax_str s);

/* ================================================================
 * file 模块（对标 Go os / C stdio/stdlib）
 * ================================================================ */

ax_file ax_std_file_open(ax_str path);
ax_file ax_std_file_create(ax_str path);
ax_file ax_std_file_append(ax_str path);
int ax_std_file_remove(ax_str path);
int ax_std_file_rename(ax_str old_path, ax_str new_path);
int ax_std_file_exists(ax_str path);
int ax_std_file_mkdir(ax_str path);
int ax_std_file_is_dir(ax_str path);
int ax_std_file_is_file(ax_str path);
int64_t ax_std_file_size(ax_str path);
int64_t ax_std_file_modtime(ax_str path);
int ax_std_file_write_all(ax_str path, ax_str s);
int ax_std_file_chmod(ax_str path, int mode);
int ax_std_file_chtimes(ax_str path, int64_t atime, int64_t mtime);
ax_file ax_std_file_tempfile(ax_str dir, ax_str prefix);

/* File 实例方法 */
ax_str ax_std_file_read_all(ax_file f);
int64_t ax_std_file_write(ax_file f, ax_str s);
int64_t ax_std_file_write_line(ax_file f, ax_str s);
void ax_std_file_close(ax_file f);

/* ================================================================
 * slices 模块（对标 Go slices，ax_arr 形式）
 * ================================================================ */

int ax_std_slice_contains_int(ax_arr s, int64_t x);
int ax_std_slice_contains_string(ax_arr s, ax_str x);
int64_t ax_std_slice_index_int(ax_arr s, int64_t x);
int64_t ax_std_slice_index_string(ax_arr s, ax_str x);
void ax_std_slice_reverse_int(ax_arr* s);
void ax_std_slice_reverse_str(ax_arr* s);
ax_arr ax_std_slice_clone_int(ax_arr s);
ax_arr ax_std_slice_clone_str(ax_arr s);
void ax_std_slice_fill_int(ax_arr* s, int64_t v);
void ax_std_slice_fill_str(ax_arr* s, ax_str v);
void ax_std_slice_insert_int(ax_arr* s, int64_t i, int64_t v);
void ax_std_slice_insert_str(ax_arr* s, int64_t i, ax_str v);
void ax_std_slice_delete_range_int(ax_arr* s, int64_t i, int64_t j);
void ax_std_slice_delete_range_str(ax_arr* s, int64_t i, int64_t j);
int64_t ax_std_slice_min_int(ax_arr s);
int64_t ax_std_slice_max_int(ax_arr s);
int64_t ax_std_slice_sum_int(ax_arr s);
double ax_std_slice_min_float(ax_arr s);
double ax_std_slice_max_float(ax_arr s);
double ax_std_slice_sum_float(ax_arr s);

/* ================================================================
 * base64 模块（对标 Go encoding/base64，RFC 4648）
 * ================================================================ */

ax_str ax_base64_encode(ax_arr input);
ax_arr ax_base64_decode(ax_str input);

/* ================================================================
 * hex 模块（对标 Go encoding/hex，小写十六进制）
 * ================================================================ */

ax_str ax_hex_encode(ax_arr input);
ax_arr ax_hex_decode(ax_str input);

/* ================================================================
 * crypto/md5 模块（对标 Go crypto/md5，RFC 1321，摘要 16 字节）
 * ================================================================ */

ax_arr ax_md5_sum(ax_arr data);

/* ================================================================
 * crypto/sha1 模块（对标 Go crypto/sha1，RFC 3174，摘要 20 字节）
 * ================================================================ */

ax_arr ax_sha1_sum(ax_arr data);

/* ================================================================
 * crypto/sha256 模块（对标 Go crypto/sha256，FIPS 180-4，摘要 32 字节）
 * ================================================================ */

ax_arr ax_sha256_sum(ax_arr data);

/* ================================================================
 * flag 模块（对标 Go flag）
 * ================================================================ */

int ax_std_flag_define_bool(ax_str name, int64_t def_val, ax_str usage);
int ax_std_flag_define_int(ax_str name, int64_t def_val, ax_str usage);
int ax_std_flag_define_string(ax_str name, ax_str def_val, ax_str usage);
int ax_std_flag_bool(ax_str name);
int64_t ax_std_flag_int(ax_str name);
ax_str ax_std_flag_string(ax_str name);
ax_str ax_std_flag_arg(int64_t i);

/* ================================================================
 * env 模块（对标 Go os 环境变量增值功能）
 * ================================================================ */

ax_str ax_std_env_expand(ax_str s);
int ax_std_env_clearenv(void);
int ax_std_env_exists(ax_str key);
ax_arr ax_std_env_all(void);

#endif /* AXC_STDLIB_H */
