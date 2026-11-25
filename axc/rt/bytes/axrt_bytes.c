/* axrt_bytes.c -- axlang 运行时模块：字节数组（对标 Go bytes，P2）
 * ax_bytes_* 操作 ax_arr（元素为 uint8_t），C 层对应 ax_arr。
 * 与 std/bytes.ax 一一对应。
 */
#include "axrt.h"
#include "axrt_internal.h"

/* ---------------- 字节辅助（ASCII 大小写转换，供 ax_bytes_* 使用） ---------------- */

static int u8_is_ascii_lower(uint8_t c) { return c >= 'a' && c <= 'z'; }
static int u8_is_ascii_upper(uint8_t c) { return c >= 'A' && c <= 'Z'; }

static uint8_t u8_to_ascii_upper(uint8_t c) {
    return u8_is_ascii_lower(c) ? (uint8_t)(c - 'a' + 'A') : c;
}

static uint8_t u8_to_ascii_lower(uint8_t c) {
    return u8_is_ascii_upper(c) ? (uint8_t)(c - 'A' + 'a') : c;
}

/* ---------------- bytes 模块 ---------------- */

/* bytes.Contains：判断 s 是否包含子片 sub（按字节比较） */
bool ax_bytes_contains(ax_arr s, ax_arr sub) {
    if (sub.len == 0) return 1;
    if (sub.len > s.len) return 0;
    const uint8_t* sp = (const uint8_t*)s.data;
    const uint8_t* subp = (const uint8_t*)sub.data;
    for (int64_t i = 0; i + sub.len <= s.len; i++) {
        if (memcmp(sp + i, subp, (size_t)sub.len) == 0) return 1;
    }
    return 0;
}

/* bytes.Equal：判断两个字节数组是否相等（长度相同且所有字节相同） */
bool ax_bytes_equal(ax_arr a, ax_arr b) {
    if (a.len != b.len) return 0;
    if (a.len == 0) return 1;
    return memcmp(a.data, b.data, (size_t)a.len) == 0;
}

/* bytes.Index：查找子片 sub 在 s 中首次出现的索引，未找到返回 -1 */
int64_t ax_bytes_index(ax_arr s, ax_arr sub) {
    if (sub.len == 0) return 0;
    if (sub.len > s.len) return -1;
    const uint8_t* sp = (const uint8_t*)s.data;
    const uint8_t* subp = (const uint8_t*)sub.data;
    for (int64_t i = 0; i + sub.len <= s.len; i++) {
        if (memcmp(sp + i, subp, (size_t)sub.len) == 0) return i;
    }
    return -1;
}

/* bytes.Split：按分隔符 sep 分割 s，返回 ax_arr（元素为 ax_arr，每个元素是一个子片）。 */
ax_arr ax_bytes_split(ax_arr s, ax_arr sep) {
    ax_arr out; ax_arr_init(&out);
    if (sep.len == 0) {
        ax_arr_push(&out, &s, (int64_t)sizeof(ax_arr));
        return out;
    }
    const uint8_t* sp = (const uint8_t*)s.data;
    const uint8_t* sepp = (const uint8_t*)sep.data;
    int64_t start = 0;
    int64_t i = 0;
    while (i + sep.len <= s.len) {
        if (memcmp(sp + i, sepp, (size_t)sep.len) == 0) {
            ax_arr part; ax_arr_init(&part);
            int64_t partlen = i - start;
            uint8_t* buf = (uint8_t*)ax_alloc(partlen <= 0 ? 1 : partlen);
            if (partlen > 0) memcpy(buf, sp + start, (size_t)partlen);
            part.data = buf; part.len = partlen; part.cap = partlen;
            ax_arr_push(&out, &part, (int64_t)sizeof(ax_arr));
            i += sep.len;
            start = i;
        } else {
            i++;
        }
    }
    ax_arr part; ax_arr_init(&part);
    int64_t lastlen = s.len - start;
    uint8_t* buf = (uint8_t*)ax_alloc(lastlen <= 0 ? 1 : lastlen);
    if (lastlen > 0) memcpy(buf, sp + start, (size_t)lastlen);
    part.data = buf; part.len = lastlen; part.cap = lastlen;
    ax_arr_push(&out, &part, (int64_t)sizeof(ax_arr));
    return out;
}

/* bytes.TrimSpace：去除 s 首尾的空白字节，返回新数组。 */
ax_arr ax_bytes_trim_space(ax_arr s) {
    const uint8_t* sp = (const uint8_t*)s.data;
    int64_t start = 0, end = s.len;
    while (start < end &&
           (sp[start] == ' ' || sp[start] == '\t' || sp[start] == '\n' ||
            sp[start] == '\r' || sp[start] == '\v' || sp[start] == '\f')) start++;
    while (end > start &&
           (sp[end - 1] == ' ' || sp[end - 1] == '\t' || sp[end - 1] == '\n' ||
            sp[end - 1] == '\r' || sp[end - 1] == '\v' || sp[end - 1] == '\f')) end--;
    ax_arr out; ax_arr_init(&out);
    int64_t len = end - start;
    uint8_t* buf = (uint8_t*)ax_alloc(len <= 0 ? 1 : len);
    if (len > 0) memcpy(buf, sp + start, (size_t)len);
    out.data = buf; out.len = len; out.cap = len;
    return out;
}

/* bytes.ToUpper：将 s 中的 ASCII 小写字母转为大写，返回新数组。 */
ax_arr ax_bytes_to_upper(ax_arr s) {
    const uint8_t* sp = (const uint8_t*)s.data;
    uint8_t* buf = (uint8_t*)ax_alloc(s.len <= 0 ? 1 : s.len);
    for (int64_t i = 0; i < s.len; i++) buf[i] = u8_to_ascii_upper(sp[i]);
    ax_arr out; ax_arr_init(&out);
    out.data = buf; out.len = s.len; out.cap = s.len;
    return out;
}

/* bytes.ToLower：将 s 中的 ASCII 大写字母转为小写，返回新数组。 */
ax_arr ax_bytes_to_lower(ax_arr s) {
    const uint8_t* sp = (const uint8_t*)s.data;
    uint8_t* buf = (uint8_t*)ax_alloc(s.len <= 0 ? 1 : s.len);
    for (int64_t i = 0; i < s.len; i++) buf[i] = u8_to_ascii_lower(sp[i]);
    ax_arr out; ax_arr_init(&out);
    out.data = buf; out.len = s.len; out.cap = s.len;
    return out;
}

/* bytes.Repeat：将 s 重复 count 次，返回新数组。count <= 0 返回空数组。 */
ax_arr ax_bytes_repeat(ax_arr s, int64_t count) {
    if (count <= 0) {
        ax_arr out; ax_arr_init(&out);
        return out;
    }
    const uint8_t* sp = (const uint8_t*)s.data;
    int64_t total = s.len * count;
    uint8_t* buf = (uint8_t*)ax_alloc(total <= 0 ? 1 : total);
    for (int64_t i = 0; i < count; i++) {
        if (s.len > 0) memcpy(buf + i * s.len, sp, (size_t)s.len);
    }
    ax_arr out; ax_arr_init(&out);
    out.data = buf; out.len = total; out.cap = total;
    return out;
}
