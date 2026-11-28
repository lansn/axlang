/* axrt_csv.c -- axlang 运行时模块 */
#include "axrt.h"
#include "axrt_internal.h"


/* ---------------- csv 模块（对标 Go encoding/csv） ----------------
 * Reader/Writer 对标 Go io.Reader/io.Writer，在 axlang 中均为 File 句柄（ax_file*）。
 * 简化实现：按 '\n' 分行，每行按 ',' 分字段；不处理引号转义与换行字段。 */

ax_arr ax_csv_read_all(ax_file* f) {
    ax_str content;
    if (ax_file_read_all(f, &content) != 0) {
        ax_arr empty;
        ax_arr_init(&empty);
        return empty;
    }

    ax_arr outer;
    ax_arr_init(&outer);

    int64_t nlines = 0;
    ax_str* lines = ax_str_split(content, ax_str_from_cstr("\n"), &nlines);
    for (int64_t i = 0; i < nlines; i++) {
        ax_str line = lines[i];
        /* 去掉行尾的 '\r'（Windows CRLF 兼容） */
        if (line.len > 0 && line.p[line.len - 1] == (uint8_t)'\r') {
            line = ax_str_from_bytes(line.p, line.len - 1);
        }
        /* 跳过空行 */
        if (line.len == 0) continue;

        int64_t nfields = 0;
        ax_str* fields = ax_str_split(line, ax_str_from_cstr(","), &nfields);

        ax_arr inner;
        ax_arr_init(&inner);
        for (int64_t j = 0; j < nfields; j++) {
            ax_arr_push(&inner, &fields[j], (int64_t)sizeof(ax_str));
        }
        ax_arr_push(&outer, &inner, (int64_t)sizeof(ax_arr));
    }
    return outer;
}

int64_t ax_csv_write(ax_file* f, ax_arr record) {
    if (!f || !f->fp) return -1;
    ax_sb b;
    ax_sb_init(&b);
    for (int64_t i = 0; i < record.len; i++) {
        if (i > 0) {
            ax_sb_push(&b, ax_str_from_cstr(","));
        }
        ax_str* field = (ax_str*)((uint8_t*)record.data + (size_t)i * sizeof(ax_str));
        ax_sb_push(&b, *field);
    }
    ax_sb_push(&b, ax_str_from_cstr("\n"));
    ax_str out = ax_sb_finish(&b);
    return ax_file_write(f, out);
}

/* ---------------- slices 模块（对标 Go slices） ---------------- */

