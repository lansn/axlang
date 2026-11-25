/* axrt_filepath.c -- axlang runtime module */
#include "axrt.h"
#include "axrt_internal.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#if defined(_WIN32)
#include <windows.h>
#include <io.h>
#else
#include <dirent.h>
#include <sys/stat.h>
#include <unistd.h>
#include <limits.h>
#endif


/* ---------------- filepath module (on par with Go path/filepath) ---------------- */

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
        /* no separator: return current dir "." */
        return ax_str_from_cstr(".");
    }
    if (pos == 0) {
        /* path starts with separator and only that separator: return "/" or "\\" */
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
        /* ends with separator: return empty string */
        return ax_str_empty();
    }
    return ax_str_slice_cp(path, pos + 1, path.len);
}

/* return extension (with dot, like ".txt"; empty if no extension or ends with separator) */
/* return extension (with dot, like ".txt"; empty if no extension or ends with separator) */
ax_str ax_filepath_ext(ax_str path) {
    int64_t pos = ax_filepath_last_sep(path);
    int64_t start = (pos < 0) ? 0 : pos + 1;
    if (start >= path.len) return ax_str_empty();
    int64_t dot = -1;
    for (int64_t i = start; i < path.len; i++) {
        if (path.p[i] == (uint8_t)'.') dot = i;
    }
    if (dot < 0 || dot == start) return ax_str_empty();
    /* extension cannot be "." or ".." */
    if (dot == start + 1 && path.p[start] == '.') return ax_str_empty();
    return ax_str_slice_cp(path, dot, path.len);
}

/* split path into [dir, base] array (on par with Go filepath.Split) */
/* split path into [dir, base] array (on par with Go filepath.Split) */
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

/* path normalization (remove ., .., duplicate separators, on par with Go filepath.Clean) */
/* path normalization (remove ., .., duplicate separators, on par with Go filepath.Clean) */
ax_str ax_filepath_clean(ax_str path) {
    int is_abs = (path.len > 0 && path.p[0] == (uint8_t)AX_FILEPATH_SEP[0]);

    /* split by separator into segments */
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

    /* process segments: skip empty and ".", handle ".." */
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
                    stack.len--; /* pop top to cancel ".." */
                    continue;
                }
            }
            ax_arr_push(&stack, seg, (int64_t)sizeof(ax_str));
        } else {
            ax_arr_push(&stack, seg, (int64_t)sizeof(ax_str));
        }
    }

    /* build result */
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

/* is absolute path (Unix starts with /, Windows starts with \ or drive letter:) */

/* is absolute path (Unix starts with /, Windows starts with \ or drive letter:) */
int ax_filepath_is_abs(ax_str path) {
    if (path.len == 0) return 0;
    if (path.p[0] == (uint8_t)AX_FILEPATH_SEP[0]) return 1;
#if defined(_WIN32)
    /* Windows drive letter: C: or C:\ */
    if (path.len >= 2 && path.p[1] == (uint8_t)':' &&
        ((path.p[0] >= (uint8_t)'a' && path.p[0] <= (uint8_t)'z') ||
         (path.p[0] >= (uint8_t)'A' && path.p[0] <= (uint8_t)'Z'))) {
        if (path.len == 2 || path.p[2] == (uint8_t)AX_FILEPATH_SEP[0] || path.p[2] == (uint8_t)'/') return 1;
    }
#endif
    return 0;
}

/* convert to absolute path (on par with Go filepath.Abs).
 * if already absolute, Clean and return;
 * otherwise join with cwd and Clean. */

/* convert to absolute path (on par with Go filepath.Abs).
 * if already absolute, Clean and return;
 * otherwise join with cwd and Clean. */
ax_str ax_filepath_abs(ax_str path) {
    if (ax_filepath_is_abs(path)) return ax_filepath_clean(path);

    /* get current working directory */
    char cwd[4096];
#if defined(_WIN32)
    if (!_getcwd(cwd, (int)sizeof cwd)) return ax_filepath_clean(path);
#else
    if (!getcwd(cwd, (size_t)sizeof cwd)) return ax_filepath_clean(path);
#endif
    ax_str cwd_str = ax_str_from_cstr(cwd);

    /* join cwd + sep + path */
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

/* compute relative path from basepath to targpath (on par with Go filepath.Rel, pure C11) */
ax_str ax_filepath_rel(ax_str basepath, ax_str targpath) {
    ax_str base = ax_filepath_clean(basepath);
    ax_str targ = ax_filepath_clean(targpath);

    /* split basepath into segments (treat / and \ as separators, skip "." segments) */
    ax_arr base_parts;
    ax_arr_init(&base_parts);
    int64_t start = 0;
    for (int64_t i = 0; i <= base.len; i++) {
        uint8_t c = base.p[i];
        if (i == base.len || c == (uint8_t)AX_FILEPATH_SEP[0] || c == (uint8_t)'/') {
            if (i > start) {
                ax_str seg = ax_str_slice_cp(base, start, i);
                /* skip "." segments (like single segment from clean(".")) */
                if (seg.len == 1 && seg.p[0] == '.') { start = i + 1; continue; }
                ax_arr_push(&base_parts, &seg, (int64_t)sizeof(ax_str));
            }
            start = i + 1;
        }
    }

    /* split targpath into segments (treat / and \ as separators, skip "." segments) */
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

    /* find common prefix segment count */
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

    /* generate relative path: remaining base segments output "..", remaining targ segments appended */
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


/* ---------------- helper: ax_str -> NUL-terminated stack buffer ---------------- */

/* copy ax_str to stack buffer and add NUL terminator; return 0 if buffer too small */
static int ax_filepath_str_to_cstr(ax_str s, char* out, int cap) {
    if (s.len >= (int64_t)cap - 1) return 0;
    memcpy(out, s.p, (size_t)s.len);
    out[s.len] = 0;
    return 1;
}

/* append ax_str to ax_arr (elements are ax_str values, each sizeof(ax_str) bytes) */
static void ax_filepath_arr_push_str(ax_arr* arr, ax_str s) {
    ax_arr_push(arr, &s, (int64_t)sizeof(ax_str));
}

/* comparison function for ax_arr elements (ax_str values), used by qsort: byte order sort */
static int ax_filepath_str_ptr_cmp(const void* a, const void* b) {
    const ax_str* sa = (const ax_str*)a;
    const ax_str* sb = (const ax_str*)b;
    int64_t la = sa->len, lb = sb->len;
    int64_t n = la < lb ? la : lb;
    int c = memcmp(sa->p, sb->p, (size_t)n);
    if (c != 0) return c;
    if (la < lb) return -1;
    if (la > lb) return 1;
    return 0;
}

/* sort ax_arr (elements are ax_str values) by byte order */
static void ax_filepath_arr_sort_str(ax_arr* arr) {
    if (arr->len > 1 && arr->data) {
        qsort(arr->data, (size_t)arr->len, (size_t)sizeof(ax_str), ax_filepath_str_ptr_cmp);
    }
}


/* ---------------- eval_symlinks ---------------- */

/* resolve symlinks, return canonical path (on par with Go filepath.EvalSymlinks, C realpath).
 * recursively resolve all symlink components in path; on failure return a copy of the original path. */
ax_str ax_filepath_eval_symlinks(ax_str path) {
    char buf[4096];
    if (!ax_filepath_str_to_cstr(path, buf, (int)sizeof buf)) {
        return ax_str_slice_cp(path, 0, path.len);
    }

#if defined(_WIN32)
    /* Windows: prefer GetFinalPathNameByHandle to resolve reparse points (symlinks/junctions) */
    char resolved[MAX_PATH];
    HANDLE h = CreateFileA(buf, GENERIC_READ,
                           FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                           NULL, OPEN_EXISTING,
                           FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT, NULL);
    if (h != INVALID_HANDLE_VALUE) {
        DWORD n = GetFinalPathNameByHandleA(h, resolved, (DWORD)sizeof resolved, VOLUME_NAME_DOS);
        CloseHandle(h);
        if (n > 0 && n < (DWORD)sizeof resolved) {
            /* strip \\?\ prefix */
            if (strncmp(resolved, "\\\\?\\", 4) == 0) {
                return ax_str_from_cstr(resolved + 4);
            }
            return ax_str_from_cstr(resolved);
        }
    }
    /* fallback: _fullpath (does not resolve symlinks, but canonicalizes path) */
    char resolved2[MAX_PATH];
    if (_fullpath(resolved2, buf, (int)sizeof resolved2)) {
        return ax_str_from_cstr(resolved2);
    }
    return ax_str_slice_cp(path, 0, path.len);
#else
    /* POSIX: use realpath to resolve all symlinks (requires existing path components to resolve) */
    char resolved[4096];
    if (realpath(buf, resolved)) {
        return ax_str_from_cstr(resolved);
    }
    /* realpath fails (e.g. path does not fully exist), return a copy of the original path */
    return ax_str_slice_cp(path, 0, path.len);
#endif
}


/* ---------------- glob ---------------- */

/* wildcard matching: supports * ? [...] (does not support [!...] syntax, [!...] as negation).
 * return 1 for match, 0 for no match. */
static int ax_filepath_glob_match(const char* pattern, const char* name) {
    while (*pattern) {
        if (*pattern == '*') {
            pattern++;
            if (!*pattern) return 1;          /* trailing * matches everything */
            while (*name) {
                if (ax_filepath_glob_match(pattern, name)) return 1;
                name++;
            }
            return 0;
        } else if (*pattern == '?') {
            pattern++;
            if (!*name) return 0;
            name++;
        } else if (*pattern == '[') {
            pattern++;
            int negated = 0;
            if (*pattern == '!') { negated = 1; pattern++; }
            int match = 0;
            if (*pattern == ']') { pattern++; match = 1; }   /* [] matches ] itself */
            while (*pattern && *pattern != ']') {
                if (*name == *pattern) match = 1;
                pattern++;
            }
            if (*pattern != ']') return 0;   /* unclosed [...] treated as no match */
            pattern++;                       /* skip ] */
            if (negated) match = !match;
            if (!match) return 0;
            name++;
        } else {
            if (*pattern != *name) return 0;
            pattern++;
            name++;
        }
    }
    return *name == 0;
}

/* match filenames by pattern, return array of matching strings (on par with Go filepath.Glob, C glob).
 * supports * ? [...] wildcards; results sorted by byte order, no duplicates.
 * handles single-level directory wildcard only (e.g. dir/*.txt, *.log), not nested wildcards (dir slash-star slash file). */
ax_arr ax_filepath_glob(ax_str pattern) {
    ax_arr result;
    ax_arr_init(&result);

    char patbuf[4096];
    if (!ax_filepath_str_to_cstr(pattern, patbuf, (int)sizeof patbuf)) return result;

    /* find last directory separator, split into dir + namepat */
    char* last_sep = NULL;
    for (char* p = patbuf; *p; p++) {
        if (*p == '/' || *p == '\\') last_sep = p;
    }

    char dirbuf[4096];
    char namepat[4096];
    if (last_sep) {
        int dirlen = (int)(last_sep - patbuf);
        memcpy(dirbuf, patbuf, (size_t)dirlen);
        dirbuf[dirlen] = 0;
        strcpy(namepat, last_sep + 1);
    } else {
        strcpy(dirbuf, ".");
        strcpy(namepat, patbuf);
    }

#if defined(_WIN32)
    char search[4096];
    int n = snprintf(search, (size_t)sizeof search, "%s\\*", dirbuf);
    if (n < 0 || n >= (int)sizeof search) return result;
    WIN32_FIND_DATA fd;
    HANDLE h = FindFirstFileA(search, &fd);
    if (h == INVALID_HANDLE_VALUE) return result;
    do {
        if (strcmp(fd.cFileName, ".") == 0 || strcmp(fd.cFileName, "..") == 0) continue;
        if (ax_filepath_glob_match(namepat, fd.cFileName)) {
            char full[4096];
            snprintf(full, (size_t)sizeof full, "%s\\%s", dirbuf, fd.cFileName);
            ax_filepath_arr_push_str(&result, ax_str_from_cstr(full));
        }
    } while (FindNextFileA(h, &fd));
    FindClose(h);
#else
    DIR* dp = opendir(dirbuf);
    if (!dp) return result;
    struct dirent* de;
    while ((de = readdir(dp)) != NULL) {
        if (strcmp(de->d_name, ".") == 0 || strcmp(de->d_name, "..") == 0) continue;
        if (ax_filepath_glob_match(namepat, de->d_name)) {
            char full[4096];
            int n = snprintf(full, (size_t)sizeof full, "%s/%s", dirbuf, de->d_name);
            if (n < 0 || n >= (int)sizeof full) continue;
            ax_filepath_arr_push_str(&result, ax_str_from_cstr(full));
        }
    }
    closedir(dp);
#endif

    ax_filepath_arr_sort_str(&result);
    return result;
}


/* ---------------- walk ---------------- */

/* recursively traverse directory, append all entry full paths to result.
 * on par with Go filepath.Walk / C nftw, but axlang does not support closure callback,
 * so directly return all entry paths in the directory tree as a string array. */
static void ax_filepath_walk_dir(const char* dirpath, ax_arr* result) {
#if defined(_WIN32)
    char search[4096];
    int n = snprintf(search, (size_t)sizeof search, "%s\\*", dirpath);
    if (n < 0 || n >= (int)sizeof search) return;
    WIN32_FIND_DATA fd;
    HANDLE h = FindFirstFileA(search, &fd);
    if (h == INVALID_HANDLE_VALUE) return;
    do {
        if (strcmp(fd.cFileName, ".") == 0 || strcmp(fd.cFileName, "..") == 0) continue;
        char full[4096];
        n = snprintf(full, (size_t)sizeof full, "%s\\%s", dirpath, fd.cFileName);
        if (n < 0 || n >= (int)sizeof full) continue;
        ax_filepath_arr_push_str(result, ax_str_from_cstr(full));
        if (fd.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) {
            ax_filepath_walk_dir(full, result);
        }
    } while (FindNextFileA(h, &fd));
    FindClose(h);
#else
    DIR* dp = opendir(dirpath);
    if (!dp) return;
    struct dirent* de;
    while ((de = readdir(dp)) != NULL) {
        if (strcmp(de->d_name, ".") == 0 || strcmp(de->d_name, "..") == 0) continue;
        char full[4096];
        int n = snprintf(full, (size_t)sizeof full, "%s/%s", dirpath, de->d_name);
        if (n < 0 || n >= (int)sizeof full) continue;
        ax_filepath_arr_push_str(result, ax_str_from_cstr(full));
        struct stat st;
        if (lstat(full, &st) == 0 && S_ISDIR(st.st_mode)) {
            ax_filepath_walk_dir(full, result);
        }
    }
    closedir(dp);
#endif
}

/* recursively traverse directory tree, return array of all entry full paths under root
 * (on par with Go filepath.Walk, C nftw).
 * axlang does not support closure callback, so directly return all entry paths. */
ax_arr ax_filepath_walk(ax_str root) {
    ax_arr result;
    ax_arr_init(&result);

    char rootbuf[4096];
    if (!ax_filepath_str_to_cstr(root, rootbuf, (int)sizeof rootbuf)) return result;

    /* append root directory itself first */
    ax_filepath_arr_push_str(&result, ax_str_from_cstr(rootbuf));

    /* then recursively append child entries */
    ax_filepath_walk_dir(rootbuf, &result);

    ax_filepath_arr_sort_str(&result);
    return result;
}

/* ---------------- arena (region allocator) ---------------- */
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
