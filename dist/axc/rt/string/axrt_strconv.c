/* axrt_strconv.c -- axlang 运行时模块 */
#include "axrt.h"
#include "axrt_internal.h"


/* 将一个字节写入 Go 转义序列，返回写入的字符数（不含 NUL）。
 * buf 必须至少有 6 字节空间（\UXXXXXXXX 最长 10 字节，这里保守按 6 足够）。 */
int ax_quote_escape(uint8_t c, char* out) {
    switch (c) {
        case (uint8_t)'\\':  out[0]='\\'; out[1]='\\'; return 2;
        case (uint8_t)'\"':  out[0]='\\'; out[1]='"';  return 2;
        case (uint8_t)'\a':  out[0]='\\'; out[1]='a';  return 2;
        case (uint8_t)'\b':  out[0]='\\'; out[1]='b';  return 2;
        case (uint8_t)'\f':  out[0]='\\'; out[1]='f';  return 2;
        case (uint8_t)'\n':  out[0]='\\'; out[1]='n';  return 2;
        case (uint8_t)'\r':  out[0]='\\'; out[1]='r';  return 2;
        case (uint8_t)'\t':  out[0]='\\'; out[1]='t';  return 2;
        case (uint8_t)'\v':  out[0]='\\'; out[1]='v';  return 2;
        default:
            if (!ax_u8_is_printable(c)) {
                /* 不可打印字符：使用 \xHH */
                out[0]='\\'; out[1]='x';
                static const char hex[] = "0123456789abcdef";
                out[2] = hex[(c >> 4) & 0xF];
                out[3] = hex[c & 0xF];
                return 4;
            }
            out[0] = (char)c;
            return 1;
    }
}


/* 十六进制字符转数值，失败返回 -1（前向声明，供 ax_unquote_escape 调用） */
int hexval(uint8_t c);

/* 解析一个转义序列，从 *p 指向反斜杠之后的字符开始。
 * 成功返回写入的字节数（1），并将 *p 前进；失败返回 -1。 */

/* 解析一个转义序列，从 *p 指向反斜杠之后的字符开始。
 * 成功返回写入的字节数（1），并将 *p 前进；失败返回 -1。 */
int ax_unquote_escape(const uint8_t** p, const uint8_t* end, uint8_t* out) {
    if (*p >= end) return -1;
    uint8_t c = **p;
    (*p)++;
    switch (c) {
        case (uint8_t)'\\': *out = '\\'; return 1;
        case (uint8_t)'"':  *out = '"';  return 1;
        case (uint8_t)'a':  *out = '\a'; return 1;
        case (uint8_t)'b':  *out = '\b'; return 1;
        case (uint8_t)'f':  *out = '\f'; return 1;
        case (uint8_t)'n':  *out = '\n'; return 1;
        case (uint8_t)'r':  *out = '\r'; return 1;
        case (uint8_t)'t':  *out = '\t'; return 1;
        case (uint8_t)'v':  *out = '\v'; return 1;
        case (uint8_t)'x': {
            /* \xHH：两个十六进制字符 */
            if (*p + 2 > end) return -1;
            uint8_t hi = **p; (*p)++;
            uint8_t lo = **p; (*p)++;
            int v1 = hexval(hi), v2 = hexval(lo);
            if (v1 < 0 || v2 < 0) return -1;
            *out = (uint8_t)((v1 << 4) | v2);
            return 1;
        }
        default:
            return -1;
    }
}

/* 十六进制字符转数值，失败返回 -1 */

/* 十六进制字符转数值，失败返回 -1 */
int hexval(uint8_t c) {
    if (c >= (uint8_t)'0' && c <= (uint8_t)'9') return (int)(c - '0');
    if (c >= (uint8_t)'a' && c <= (uint8_t)'f') return (int)(c - 'a' + 10);
    if (c >= (uint8_t)'A' && c <= (uint8_t)'F') return (int)(c - 'A' + 10);
    return -1;
}

