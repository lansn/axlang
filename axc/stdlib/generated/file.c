/* generated/file.c —— axlang 标准库 file 模块（预转译自 axc/stdlib/file.ax）
 * 纯标准 C11，随编译器发布，通过 gcc 参与编译。
 * 底层依赖 axrt.h / axrt.c 提供的 C 运行时原语。
 */

#include "axrt.h"
#include "axc_stdlib.h"

/* file.Open — 模式 0=只读 */
ax_file ax_std_file_open(ax_str path) {
    ax_res_file res = ax_file_open_result(path, 0);
    if (res.tag == 1) {
        return *res.u.ok;
    }
    ax_file empty = { NULL };
    return empty;
}

/* file.Create — 模式 1=写（新建或截断） */
ax_file ax_std_file_create(ax_str path) {
    ax_res_file res = ax_file_open_result(path, 1);
    if (res.tag == 1) {
        return *res.u.ok;
    }
    ax_file empty = { NULL };
    return empty;
}

/* file.Append — 模式 2=追加 */
ax_file ax_std_file_append(ax_str path) {
    ax_res_file res = ax_file_open_result(path, 2);
    if (res.tag == 1) {
        return *res.u.ok;
    }
    ax_file empty = { NULL };
    return empty;
}

/* file.Remove */
int ax_std_file_remove(ax_str path) {
    return ax_file_remove(path);
}

/* file.Rename */
int ax_std_file_rename(ax_str old_path, ax_str new_path) {
    return ax_file_rename(old_path, new_path);
}

/* file.Exists */
int ax_std_file_exists(ax_str path) {
    return ax_file_exists(path);
}

/* file.Mkdir */
int ax_std_file_mkdir(ax_str path) {
    return ax_dir_create(path);
}

/* file.IsDir */
int ax_std_file_is_dir(ax_str path) {
    return ax_file_is_dir(path);
}

/* file.IsFile */
int ax_std_file_is_file(ax_str path) {
    return ax_file_is_file(path);
}

/* file.Size */
int64_t ax_std_file_size(ax_str path) {
    return ax_file_size(path);
}

/* file.ModTime */
int64_t ax_std_file_modtime(ax_str path) {
    return ax_file_modtime(path);
}

/* file.WriteAll */
int ax_std_file_write_all(ax_str path, ax_str s) {
    return ax_file_write_all(path, s);
}

/* file.Chmod */
int ax_std_file_chmod(ax_str path, int mode) {
    return ax_file_chmod(path, mode);
}

/* file.Chtimes */
int ax_std_file_chtimes(ax_str path, int64_t atime, int64_t mtime) {
    return ax_file_chtimes(path, atime, mtime);
}

/* file.TempFile */
ax_file ax_std_file_tempfile(ax_str dir, ax_str prefix) {
    ax_res_file res = ax_file_tempfile(dir, prefix);
    if (res.tag == 1) {
        return *res.u.ok;
    }
    ax_file empty = { NULL };
    return empty;
}

/* File 实例方法：ReadAll */
ax_str ax_std_file_read_all(ax_file f) {
    return ax_file_read_all_val(f);
}

/* File 实例方法：Write */
int64_t ax_std_file_write(ax_file f, ax_str s) {
    return ax_file_write(&f, s);
}

/* File 实例方法：WriteLine */
int64_t ax_std_file_write_line(ax_file f, ax_str s) {
    return ax_file_write_line(&f, s);
}

/* File 实例方法：Close */
void ax_std_file_close(ax_file f) {
    ax_file_close(&f);
}
