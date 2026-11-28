/* axrt_flag.c -- axlang flag 模块 C 运行时实现（对标 Go flag）
 * 纯标准 C11，不使用任何 GNU/Clang 扩展。
 */

#include "axrt.h"
#include "axrt_internal.h"

/* ---------------- 标志注册表 ---------------- */

static ax_flag_def g_flags[AX_FLAG_MAX];
static int         g_flag_count = 0;

/* 位置参数列表（由 parse() 填充） */
static ax_arr      g_pos_args;
static int         g_pos_args_init = 0;

/* 查找指定名称的标志，返回索引，未找到返回 -1 */
static int ax_flag_find_index(ax_str name) {
    for (int i = 0; i < g_flag_count; i++) {
        if (ax_str_eq(g_flags[i].name, name)) return i;
    }
    return -1;
}

/* 注册 bool 标志 */
int ax_std_flag_define_bool(ax_str name, int64_t def_val, ax_str usage) {
    if (g_flag_count >= AX_FLAG_MAX) return 1;
    if (ax_flag_find_index(name) >= 0) return 1; /* 重复注册 */
    g_flags[g_flag_count].kind = AX_FLAG_BOOL;
    g_flags[g_flag_count].name = name;
    g_flags[g_flag_count].usage = usage;
    g_flags[g_flag_count].def_bool = (int)(def_val != 0 ? 1 : 0);
    g_flags[g_flag_count].val_bool = g_flags[g_flag_count].def_bool;
    g_flags[g_flag_count].is_set = 0;
    g_flag_count++;
    return 0;
}

/* 注册 int 标志 */
int ax_std_flag_define_int(ax_str name, int64_t def_val, ax_str usage) {
    if (g_flag_count >= AX_FLAG_MAX) return 1;
    if (ax_flag_find_index(name) >= 0) return 1; /* 重复注册 */
    g_flags[g_flag_count].kind = AX_FLAG_INT;
    g_flags[g_flag_count].name = name;
    g_flags[g_flag_count].usage = usage;
    g_flags[g_flag_count].def_int = def_val;
    g_flags[g_flag_count].val_int = def_val;
    g_flags[g_flag_count].is_set = 0;
    g_flag_count++;
    return 0;
}

/* 注册 string 标志 */
int ax_std_flag_define_string(ax_str name, ax_str def_val, ax_str usage) {
    if (g_flag_count >= AX_FLAG_MAX) return 1;
    if (ax_flag_find_index(name) >= 0) return 1; /* 重复注册 */
    g_flags[g_flag_count].kind = AX_FLAG_STRING;
    g_flags[g_flag_count].name = name;
    g_flags[g_flag_count].usage = usage;
    g_flags[g_flag_count].def_str = def_val;
    g_flags[g_flag_count].val_str = def_val;
    g_flags[g_flag_count].is_set = 0;
    g_flag_count++;
    return 0;
}

/* 解析单个参数 "-name=value" 或 "-name value" 或 "--name=value" */
/* 返回 1 表示成功消费了参数（可能消费 1 或 2 个 argv 元素），0 表示未识别 */
static int ax_flag_parse_one(ax_str* args, int64_t n, int64_t* idx) {
    ax_str raw = args[*idx];
    /* 必须以 "-" 开头，且不只是 "-" */
    if (raw.len < 2 || raw.p[0] != (uint8_t)'-') return 0;

    /* 跳过前导 '-' */
    int64_t start = 1;
    if (raw.len >= 2 && raw.p[1] == (uint8_t)'-') start = 2; /* "--name" */

    /* 提取标志名（到 "=" 或结束） */
    int64_t eq = -1;
    for (int64_t j = start; j < raw.len; j++) {
        if (raw.p[j] == (uint8_t)'=') { eq = j; break; }
    }

    ax_str name;
    ax_str value;
    int    has_value = 0;

    if (eq >= 0) {
        /* "-name=value" */
        name.p = raw.p + start;
        name.len = eq - start;
        value.p = raw.p + eq + 1;
        value.len = raw.len - eq - 1;
        has_value = 1;
    } else {
        /* "-name" 或 "--name"，值可能在下一个 argv */
        name.p = raw.p + start;
        name.len = raw.len - start;
    }

    int fi = ax_flag_find_index(name);
    if (fi < 0) return 0; /* 未知标志，不消费 */

    ax_flag_def* fd = &g_flags[fi];
    fd->is_set = 1;

    switch (fd->kind) {
    case AX_FLAG_BOOL:
        if (has_value) {
            fd->val_bool = ax_parse_bool(value) ? 1 : 0;
        } else {
            /* 无值形式：-bool 表示 true */
            fd->val_bool = 1;
        }
        break;
    case AX_FLAG_INT:
        if (has_value) {
            int64_t v = 0;
            ax_parse_i64(value, &v);
            fd->val_int = v;
        } else {
            /* 无值形式：尝试从下一个 argv 读取 */
            if (*idx + 1 < n) {
                ax_str next = args[*idx + 1];
                /* 如果下一个参数不以 "-" 开头，当作值 */
                if (next.len > 0 && next.p[0] != (uint8_t)'-') {
                    int64_t v = 0;
                    ax_parse_i64(next, &v);
                    fd->val_int = v;
                    (*idx)++; /* 消费下一个 argv */
                }
            }
        }
        break;
    case AX_FLAG_STRING:
        if (has_value) {
            fd->val_str = value;
        } else {
            /* 无值形式：尝试从下一个 argv 读取 */
            if (*idx + 1 < n) {
                ax_str next = args[*idx + 1];
                if (next.len > 0 && next.p[0] != (uint8_t)'-') {
                    fd->val_str = next;
                    (*idx)++; /* 消费下一个 argv */
                }
            }
        }
        break;
    }
    return 1;
}

/* 解析命令行参数 */
int ax_flag_parse(void) {
    ax_arr args = ax_os_args();
    /* args 元素为 ax_str，esz = sizeof(ax_str) */
    ax_str* argv = (ax_str*)args.data;
    int64_t argc = args.len;

    /* 重置位置参数 */
    if (!g_pos_args_init) {
        ax_arr_init(&g_pos_args);
        g_pos_args_init = 1;
    } else {
        ax_arr_clear(&g_pos_args);
    }

    int64_t i = 0;
    while (i < argc) {
        if (!ax_flag_parse_one(argv, argc, &i)) {
            /* 非标志参数，作为位置参数 */
            ax_arr_push(&g_pos_args, &argv[i], (int64_t)sizeof(ax_str));
        }
        i++;
    }
    return 0;
}

/* 获取 bool 标志的当前值 */
int ax_std_flag_bool(ax_str name) {
    int fi = ax_flag_find_index(name);
    if (fi < 0) return 0;
    return g_flags[fi].val_bool;
}

/* 获取 int 标志的当前值 */
int64_t ax_std_flag_int(ax_str name) {
    int fi = ax_flag_find_index(name);
    if (fi < 0) return 0;
    return g_flags[fi].val_int;
}

/* 获取 string 标志的当前值 */
ax_str ax_std_flag_string(ax_str name) {
    int fi = ax_flag_find_index(name);
    if (fi < 0) return ax_str_empty();
    return g_flags[fi].val_str;
}

/* 返回已通过命令行设置的标志数量 */
int ax_flag_n_flag(void) {
    int n = 0;
    for (int i = 0; i < g_flag_count; i++) {
        if (g_flags[i].is_set) n++;
    }
    return n;
}

/* 返回第 i 个非标志位置参数 */
ax_str ax_std_flag_arg(int64_t i) {
    if (i < 0 || i >= g_pos_args.len) return ax_str_empty();
    ax_str* p = (ax_str*)((char*)g_pos_args.data + (size_t)i * sizeof(ax_str));
    return *p;
}

/* 返回非标志位置参数的数量 */
int64_t ax_flag_n_arg(void) {
    return g_pos_args.len;
}

/* 返回所有非标志位置参数 */
ax_arr ax_flag_args(void) {
    return g_pos_args;
}
