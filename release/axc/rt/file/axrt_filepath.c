/* axrt_filepath.c -- axlang 运行时模块 */
#include "axrt.h"
#include "axrt_internal.h"


/* ---------------- filepath 模块（对标 Go path/filepath） ---------------- */

static int ax_filepath_has_sep(ax_str s) {
    int64_t i;
    for (i = 0; i < s.len; i++) {
        if (s.p[i] == (uint8_t)AX_FILEPATH_SEP[0]) return 1;
    }
    return 0;
}

static int64_t ax_filepath_last_sep(ax_str s) {
    int64_t i;
    for (i = s.len - 1; i >= 0; i--) {
        if (s.p[i] == (uint8_t)AX_FILEPATH_SEP[0]) return i;
    }
    return -1;
}


ax_str ax_filepath_join(ax_arr parts) {
    ax_sb sb;
    ax_sb_init(&sb);
    int64_t n = parts.len;
    int64_t i;
    for (i = 0; i < n; i++) {
        ax_str* e = (ax_str*)ax_arr_at(&parts, i, (int64_t)sizeof(ax_str), __FILE__, __LINE__, 0);
        if (i > 0) {
            ax_sb_push_cstr(&sb, AX_FILEPATH_SEP);
        }
        ax_sb_push(&sb, *e);
    }
    return ax_sb_finish(&sb);
}


ax_str ax_filepath_dir(ax_str path) {
    int64_t pos = ax_filepath_last_sep(path);
    if (pos < 0) {
        /* 无分隔符：返回当前目录 "." */
        return ax_str_from_cstr(".");
    }
    if (pos == 0) {
        /* 路径以分隔符开头且只有该分隔符：返回 "/" 或 "\\" */
        return ax_str_from_cstr(AX_FILEPATH_SEP);
    }
    return ax_str_slice_cp(path, 0, pos);
}


ax_str ax_filepath_base(ax_str path) {
    int64_t pos = ax_filepath_last_sep(path);
    if (pos < 0) {
        return path;
    }
    if (pos == path.len - 1) {
        /* 以分隔符结尾：返回空字符串 */
        return ax_str_empty();
    }
    return ax_str_slice_cp(path, pos + 1, path.len);
}

/* 返回扩展名（含点，如 ".txt"；无扩展名或以分隔符结尾返回空串） */

/* 返回扩展名（含点，如 ".txt"；无扩展名或以分隔符结尾返回空串） */
ax_str ax_filepath_ext(ax_str path) {
    int64_t pos = ax_filepath_last_sep(path);
    int64_t start = (pos < 0) ? 0 : pos + 1;
    if (start >= path.len) return ax_str_empty();
    int64_t dot = -1;
    for (int64_t i = start; i < path.len; i++) {
        if (path.p[i] == (uint8_t)'.') dot = i;
    }
    if (dot < 0 || dot == start) return ax_str_empty();
    /* 扩展名不能是 "." 或 ".." */
    if (dot == start + 1 && path.p[start] == '.') return ax_str_empty();
    return ax_str_slice_cp(path, dot, path.len);
}

/* 分割路径为 [dir, base] 数组（对标 Go filepath.Split） */

/* 分割路径为 [dir, base] 数组（对标 Go filepath.Split） */
ax_arr ax_filepath_split(ax_str path) {
    ax_arr arr;
    ax_arr_init(&arr);
    int64_t pos = ax_filepath_last_sep(path);
    ax_str dir, base;
    if (pos < 0) {
        dir = ax_str_from_cstr(".");
        base = path;
    } else if (pos == 0) {
        dir = ax_str_from_cstr(AX_FILEPATH_SEP);
        base = ax_str_slice_cp(path, 1, path.len);
    } else {
        dir = ax_str_slice_cp(path, 0, pos);
        base = ax_str_slice_cp(path, pos + 1, path.len);
    }
    ax_arr_push(&arr, &dir, (int64_t)sizeof(ax_str));
    ax_arr_push(&arr, &base, (int64_t)sizeof(ax_str));
    return arr;
}

/* 路径规范化（去除 .、..、重复分隔符，对标 Go filepath.Clean） */

/* 路径规范化（去除 .、..、重复分隔符，对标 Go filepath.Clean） */
ax_str ax_filepath_clean(ax_str path) {
    int is_abs = (path.len > 0 && path.p[0] == (uint8_t)AX_FILEPATH_SEP[0]);

    /* 按分隔符分割为段 */
    ax_arr parts;
    ax_arr_init(&parts);
    int64_t start = 0;
    for (int64_t i = 0; i <= path.len; i++) {
        if (i == path.len || path.p[i] == (uint8_t)AX_FILEPATH_SEP[0]) {
            if (i > start) {
                ax_str seg = ax_str_slice_cp(path, start, i);
                ax_arr_push(&parts, &seg, (int64_t)sizeof(ax_str));
            }
            start = i + 1;
        }
    }

    /* 处理段：跳过空段和 "."，处理 ".." */
    ax_arr stack;
    ax_arr_init(&stack);
    int64_t n = parts.len;
    for (int64_t i = 0; i < n; i++) {
        ax_str* seg = (ax_str*)ax_arr_at(&parts, i, (int64_t)sizeof(ax_str), __FILE__, __LINE__, 0);
        if (seg->len == 0) continue;
        if (seg->len == 1 && seg->p[0] == (uint8_t)'.') continue;
        if (seg->len == 2 && seg->p[0] == (uint8_t)'.' && seg->p[1] == (uint8_t)'.') {
            int64_t sn = stack.len;
            if (sn > 0) {
                ax_str* top = (ax_str*)ax_arr_at(&stack, sn - 1, (int64_t)sizeof(ax_str), __FILE__, __LINE__, 0);
                if (!(top->len == 2 && top->p[0] == (uint8_t)'.' && top->p[1] == (uint8_t)'.')) {
                    stack.len--; /* 弹出栈顶，抵消 ".." */
                    continue;
                }
            }
            ax_arr_push(&stack, seg, (int64_t)sizeof(ax_str));
        } else {
            ax_arr_push(&stack, seg, (int64_t)sizeof(ax_str));
        }
    }

    /* 构建结果 */
    ax_sb sb;
    ax_sb_init(&sb);
    if (is_abs) ax_sb_push_cstr(&sb, AX_FILEPATH_SEP);
    int64_t sn = stack.len;
    for (int64_t i = 0; i < sn; i++) {
        if (i > 0) ax_sb_push_cstr(&sb, AX_FILEPATH_SEP);
        ax_str* seg = (ax_str*)ax_arr_at(&stack, i, (int64_t)sizeof(ax_str), __FILE__, __LINE__, 0);
        ax_sb_push(&sb, *seg);
    }
    if (sn == 0) {
        if (is_abs) ax_sb_push_cstr(&sb, AX_FILEPATH_SEP);
        else ax_sb_push_cstr(&sb, ".");
    }
    return ax_sb_finish(&sb);
}

/* 判断是否为绝对路径（Unix 以 / 开头，Windows 以 \ 或 盘符: 开头） */

/* 判断是否为绝对路径（Unix 以 / 开头，Windows 以 \ 或 盘符: 开头） */
int ax_filepath_is_abs(ax_str path) {
    if (path.len == 0) return 0;
    if (path.p[0] == (uint8_t)AX_FILEPATH_SEP[0]) return 1;
#if defined(_WIN32)
    /* Windows 盘符：C: 或 C:\ */
    if (path.len >= 2 && path.p[1] == (uint8_t)':' &&
        ((path.p[0] >= (uint8_t)'a' && path.p[0] <= (uint8_t)'z') ||
         (path.p[0] >= (uint8_t)'A' && path.p[0] <= (uint8_t)'Z'))) {
        if (path.len == 2 || path.p[2] == (uint8_t)AX_FILEPATH_SEP[0] || path.p[2] == (uint8_t)'/') return 1;
    }
#endif
    return 0;
}

/* 转为绝对路径（对标 Go filepath.Abs）。已是绝对路径则 Clean 后返回；
 * 否则用当前工作目录拼接后 Clean。 */

/* 转为绝对路径（对标 Go filepath.Abs）。已是绝对路径则 Clean 后返回；
 * 否则用当前工作目录拼接后 Clean。 */
ax_str ax_filepath_abs(ax_str path) {
    if (ax_filepath_is_abs(path)) return ax_filepath_clean(path);

    /* 获取当前工作目录 */
    char cwd[4096];
#if defined(_WIN32)
    if (!_getcwd(cwd, (int)sizeof cwd)) return ax_filepath_clean(path);
#else
    if (!getcwd(cwd, (size_t)sizeof cwd)) return ax_filepath_clean(path);
#endif
    ax_str cwd_str = ax_str_from_cstr(cwd);

    /* 拼接 cwd + sep + path */
    ax_sb sb;
    ax_sb_init(&sb);
    ax_sb_push(&sb, cwd_str);
    if (cwd_str.len > 0 && cwd_str.p[cwd_str.len - 1] != (uint8_t)AX_FILEPATH_SEP[0]) {
        ax_sb_push_cstr(&sb, AX_FILEPATH_SEP);
    }
    ax_sb_push(&sb, path);
    ax_str joined = ax_sb_finish(&sb);
    return ax_filepath_clean(joined);
}

/* 计算从 basepath 到 targpath 的相对路径（对标 Go filepath.Rel，纯 C11） */
ax_str ax_filepath_rel(ax_str basepath, ax_str targpath) {
    ax_str base = ax_filepath_clean(basepath);
    ax_str targ = ax_filepath_clean(targpath);

    /* 分割 basepath 为段（同时视 / 和 \ 为分隔符，跨平台兼容；跳过 "." 段） */
    ax_arr base_parts;
    ax_arr_init(&base_parts);
    int64_t start = 0;
    for (int64_t i = 0; i <= base.len; i++) {
        uint8_t c = base.p[i];
        if (i == base.len || c == (uint8_t)AX_FILEPATH_SEP[0] || c == (uint8_t)'/') {
            if (i > start) {
                ax_str seg = ax_str_slice_cp(base, start, i);
                /* 跳过 "." 段（如 clean(".") 产生的单段） */
                if (seg.len == 1 && seg.p[0] == '.') { start = i + 1; continue; }
                ax_arr_push(&base_parts, &seg, (int64_t)sizeof(ax_str));
            }
            start = i + 1;
        }
    }

    /* 分割 targpath 为段（同时视 / 和 \ 为分隔符，跨平台兼容；跳过 "." 段） */
    ax_arr targ_parts;
    ax_arr_init(&targ_parts);
    start = 0;
    for (int64_t i = 0; i <= targ.len; i++) {
        uint8_t c = targ.p[i];
        if (i == targ.len || c == (uint8_t)AX_FILEPATH_SEP[0] || c == (uint8_t)'/') {
            if (i > start) {
                ax_str seg = ax_str_slice_cp(targ, start, i);
                if (seg.len == 1 && seg.p[0] == '.') { start = i + 1; continue; }
                ax_arr_push(&targ_parts, &seg, (int64_t)sizeof(ax_str));
            }
            start = i + 1;
        }
    }

    /* 找到公共前缀段数 */
    int64_t bn = base_parts.len, tn = targ_parts.len;
    int64_t common = 0;
    int64_t n = bn < tn ? bn : tn;
    for (int64_t i = 0; i < n; i++) {
        ax_str* bs = (ax_str*)ax_arr_at(&base_parts, i, (int64_t)sizeof(ax_str), __FILE__, __LINE__, 0);
        ax_str* ts = (ax_str*)ax_arr_at(&targ_parts, i, (int64_t)sizeof(ax_str), __FILE__, __LINE__, 0);
        if (bs->len == ts->len && memcmp(bs->p, ts->p, bs->len) == 0) {
            common++;
        } else {
            break;
        }
    }

    /* 生成相对路径：base 剩余段各输出 ".."，targ 剩余段直接追加 */
    ax_sb sb;
    ax_sb_init(&sb);
    for (int64_t i = common; i < bn; i++) {
        if (sb.len > 0) ax_sb_push_cstr(&sb, AX_FILEPATH_SEP);
        ax_sb_push_cstr(&sb, "..");
    }
    for (int64_t i = common; i < tn; i++) {
        if (sb.len > 0) ax_sb_push_cstr(&sb, AX_FILEPATH_SEP);
        ax_str* seg = (ax_str*)ax_arr_at(&targ_parts, i, (int64_t)sizeof(ax_str), __FILE__, __LINE__, 0);
        ax_sb_push(&sb, *seg);
    }

    ax_str result;
    if (sb.len == 0) {
        result = ax_str_from_cstr(".");
    } else {
        result = ax_sb_finish(&sb);
    }
    return result;
}

/* ---------------- arena（区域分配器） ---------------- */
struct ax_arena {
    void**  items;
    int64_t n;
    int64_t cap;
};

ax_arena* ax_arena_create(void) {
    ax_arena* ar = (ax_arena*)ax_alloc((int64_t)sizeof(ax_arena));
    ar->items = NULL;
    ar->n = 0;
    ar->cap = 0;
    return ar;
}

void* ax_arena_adopt(ax_arena* ar, void* p) {
    if (ar->n >= ar->cap) {
        int64_t nc = ar->cap == 0 ? 8 : ar->cap * 2;
        void** ni = (void**)ax_alloc(nc * (int64_t)sizeof(void*));
        if (ar->items && ar->n > 0) {
            memcpy(ni, ar->items, (size_t)ar->n * sizeof(void*));
        }
        ar->items = ni;
        ar->cap = nc;
    }
    ar->items[ar->n] = p;
    ar->n++;
    return p;
}

