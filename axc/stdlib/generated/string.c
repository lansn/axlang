/* generated/string.c —— axlang 标准库 string 模块（预转译自 axc/stdlib/string.ax）
 * 纯标准 C11，随编译器发布，通过 gcc 参与编译。
 * 底层依赖 axrt.h / axrt.c 提供的 C 运行时原语。
 */

#include "axrt.h"
#include "axc_stdlib.h"

/* string.Compare */
int ax_std_string_compare(ax_str a, ax_str b) {
    return ax_str_cmp(a, b);
}

/* string.Contains */
int ax_std_string_contains(ax_str s, ax_str sub) {
    return ax_str_contains(s, sub);
}

/* string.IndexOf */
int64_t ax_std_string_index_of(ax_str s, ax_str sub) {
    return ax_str_index_of(s, sub);
}

/* string.LastIndexOf */
int64_t ax_std_string_last_index_of(ax_str s, ax_str sub) {
    return ax_str_last_index_of(s, sub);
}

/* string.Count */
int64_t ax_std_string_count(ax_str s, ax_str substr) {
    return ax_str_count(s, substr);
}

/* string.Trim */
ax_str ax_std_string_trim(ax_str s) {
    return ax_str_trim(s);
}

/* string.ToUpper */
ax_str ax_std_string_to_upper(ax_str s) {
    return ax_str_to_upper(s);
}

/* string.ToLower */
ax_str ax_std_string_to_lower(ax_str s) {
    return ax_str_to_lower(s);
}

/* string.Repeat */
ax_str ax_std_string_repeat(ax_str s, int64_t n) {
    return ax_str_repeat(s, n);
}

/* string.Replace */
ax_str ax_std_string_replace(ax_str s, ax_str old, ax_str new) {
    return ax_str_replace(s, old, new);
}

/* string.StartsWith */
int ax_std_string_starts_with(ax_str s, ax_str prefix) {
    return ax_str_starts_with(s, prefix);
}

/* string.EndsWith */
int ax_std_string_ends_with(ax_str s, ax_str suffix) {
    return ax_str_ends_with(s, suffix);
}

/* string.IsDigit */
int ax_std_string_is_digit(ax_str s) {
    return ax_str_is_digit(s);
}

/* string.IsAlpha */
int ax_std_string_is_alpha(ax_str s) {
    return ax_str_is_alpha(s);
}

/* string.IsSpace */
int ax_std_string_is_space(ax_str s) {
    return ax_str_is_space(s);
}

/* string.IsLower */
int ax_std_string_is_lower(ax_str s) {
    return ax_str_is_lower(s);
}

/* string.IsUpper */
int ax_std_string_is_upper(ax_str s) {
    return ax_str_is_upper(s);
}

/* string.IsAlnum */
int ax_std_string_is_alnum(ax_str s) {
    return ax_str_is_alnum(s);
}
