/* axrt_bufio.c -- axlang 运行时模块：缓冲 I/O（对标 Go bufio，P2）
 * ax_file_buf_* 接收并返回 File 句柄（ax_file*）。
 * 与 std/bufio.ax 一一对应。
 */
#include "axrt.h"
#include "axrt_internal.h"

/* ---------------- bufio 模块 ---------------- */

/* bufio.NewReader(f: File) -> File：创建缓冲读取器，对标 Go bufio.NewReader。
 * 本版本建立 API：在 C 层返回同一句柄，缓冲由底层 FILE* 的 stdio 缓冲提供。
 */
ax_file* ax_file_buf_reader(ax_file* f) {
    return f;
}

/* bufio.NewWriter(f: File) -> File：创建缓冲写入器，对标 Go bufio.NewWriter。 */
ax_file* ax_file_buf_writer(ax_file* f) {
    return f;
}

/* bufio.Scanner(f: File) -> File：创建扫描器，对标 Go bufio.NewScanner。 */
ax_file* ax_file_buf_scanner(ax_file* f) {
    return f;
}
