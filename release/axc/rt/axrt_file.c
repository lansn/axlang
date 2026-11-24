/* axrt_file.c -- axlang 运行时模块 */
#include "axrt.h"
#include "axrt_internal.h"


int ax_read_line(ax_str* out) {
    size_t cap = 128, len = 0;
    char* buf = (char*)ax_alloc((int64_t)cap);
    int ch;
    while ((ch = fgetc(stdin)) != EOF && ch != '\n') {
        if (len + 2 >= cap) {
            cap *= 2;
            char* nb = (char*)ax_alloc((int64_t)cap);
            memcpy(nb, buf, len);
            buf = nb;
        }
        buf[len++] = (char)ch;
    }
    if (ch == EOF && len == 0) return 1;
    if (len > 0 && buf[len-1] == '\r') len--;
    *out = ax_str_from_bytes((const uint8_t*)buf, (int64_t)len);
    return 0;
}


int ax_read_file(ax_str path, ax_str* out) {
    char name[1024];
    int64_t n = path.len < 1023 ? path.len : 1023;
    memcpy(name, path.p, (size_t)n);
    name[n] = 0;
    FILE* fp = fopen(name, "rb");
    if (!fp) return 1;
    fseek(fp, 0, SEEK_END);
    long sz = ftell(fp);
    fseek(fp, 0, SEEK_SET);
    if (sz < 0) { fclose(fp); return 1; }
    uint8_t* buf = (uint8_t*)ax_alloc((int64_t)sz + 1);
    size_t rd = fread(buf, 1, (size_t)sz, fp);
    fclose(fp);
    *out = ax_str_from_bytes(buf, (int64_t)rd);
    return 0;
}


/* ---------------- 文件（Rt-13） ---------------- */

/* ax_str → 以 NUL 结尾的栈缓冲；路径过长返回 0（与 ax_read_file 同一上限）。 */
int ax_path_cstr(ax_str path, char* out, int cap) {
    if (path.len >= (int64_t)cap) return 0;
    memcpy(out, path.p, (size_t)path.len);
    out[path.len] = 0;
    return 1;
}


ax_res_file ax_file_open_result(ax_str path, int mode) {
    ax_res_file r;
    char name[1024];
    /* 一律二进制模式：Windows 文本模式会把 '\n' 改写成 CRLF，导致同一份代码
     * 在 Linux 与 Windows 上写出的字节不同、读回的字符串长度也不同。 */
    const char* m = mode == 1 ? "wb" : (mode == 2 ? "ab" : "rb");
    FILE* fp = NULL;
    if (ax_path_cstr(path, name, (int)sizeof name)) fp = fopen(name, m);
    if (!fp) {
        r.tag = 0;
        r.u.err = ax_err_new(4, ax_str_from_cstr("cannot open file"));
        return r;
    }
    ax_file* f = (ax_file*)ax_alloc((int64_t)sizeof(ax_file));
    f->fp = fp;
    r.tag = 1;
    r.u.ok = f;
    return r;
}

ax_file* ax_file_open(ax_str path) {
    ax_res_file r = ax_file_open_result(path, 0);
    if (r.tag != 1) return NULL;
    return r.u.ok;
}

ax_file* ax_file_create(ax_str path) {
    ax_res_file r = ax_file_open_result(path, 1);
    if (r.tag != 1) return NULL;
    return r.u.ok;
}

ax_file* ax_file_append(ax_str path) {
    ax_res_file r = ax_file_open_result(path, 2);
    if (r.tag != 1) return NULL;
    return r.u.ok;
}


int ax_file_read_all(ax_file* f, ax_str* out) {
    if (!f || !f->fp) return 1;
    ax_sb b;
    ax_sb_init(&b);
    char buf[4096];
    size_t n;
    while ((n = fread(buf, 1, sizeof buf, f->fp)) > 0) {
        ax_sb_push(&b, ax_str_from_bytes((const uint8_t*)buf, (int64_t)n));
    }
    if (ferror(f->fp)) return 1;
    *out = ax_sb_finish(&b);
    return 0;
}


ax_str ax_file_read_all_val(ax_file f) {
    ax_str out = AX_S("");
    ax_file_read_all(&f, &out);
    return out;
}


int ax_file_read_line(ax_file* f, ax_str* out) {
    if (!f || !f->fp) return 1;
    ax_sb b;
    ax_sb_init(&b);
    int ch;
    int any = 0;
    while ((ch = fgetc(f->fp)) != EOF && ch != '\n') {
        any = 1;
        uint8_t c = (uint8_t)ch;
        ax_sb_push(&b, ax_str_from_bytes(&c, 1));
    }
    if (ch == EOF && !any) return 1;
    *out = ax_sb_finish(&b);
    /* 去掉 CRLF 的 '\r'：与 ax_read_line 同语义（文稿 11.3） */
    if (out->len > 0 && out->p[out->len - 1] == '\r') {
        *out = ax_str_from_bytes(out->p, out->len - 1);
    }
    return 0;
}

int64_t ax_file_write(ax_file* f, ax_str s) {
    if (!f || !f->fp) return -1;
    if (s.len == 0) return 0;
    size_t n = fwrite(s.p, 1, (size_t)s.len, f->fp);
    if (n != (size_t)s.len) return -1;
    return (int64_t)n;
}

int64_t ax_file_write_line(ax_file* f, ax_str s) {
    int64_t n = ax_file_write(f, s);
    if (n < 0) return -1;
    if (fputc('\n', f->fp) == EOF) return -1;
    return n + 1;
}


void ax_file_close(ax_file* f) {
    if (f && f->fp) {
        fclose(f->fp);
        f->fp = NULL;
    }
}


int ax_file_remove(ax_str path) {
    /* ax_str 是 const uint8_t*，需要转为 const char* 传给 remove */
    return remove((const char*)path.p) == 0 ? 0 : 1;
}


int ax_file_rename(ax_str old_path, ax_str new_path) {
    return rename((const char*)old_path.p, (const char*)new_path.p) == 0 ? 0 : 1;
}


int ax_file_exists(ax_str path) {
    /* 用 fopen 尝试打开，成功则存在 */
    FILE* fp = fopen((const char*)path.p, "rb");
    if (fp) {
        fclose(fp);
        return 1;
    }
    return 0;
}


int ax_dir_create(ax_str path) {
#ifdef _WIN32
    return _mkdir((const char*)path.p) == 0 ? 0 : 1;
#else
    return mkdir((const char*)path.p, 0755) == 0 ? 0 : 1;
#endif
}


int ax_file_is_dir(ax_str path) {
    char name[1024];
    int64_t n = path.len < 1023 ? path.len : 1023;
    memcpy(name, path.p, (size_t)n);
    name[n] = 0;
#if defined(_WIN32)
    struct _stat st;
    if (_stat(name, &st) != 0) return 0;
    return (st.st_mode & _S_IFDIR) != 0 ? 1 : 0;
#else
    struct stat st;
    if (stat(name, &st) != 0) return 0;
    return S_ISDIR(st.st_mode) ? 1 : 0;
#endif
}


int ax_file_is_file(ax_str path) {
    char name[1024];
    int64_t n = path.len < 1023 ? path.len : 1023;
    memcpy(name, path.p, (size_t)n);
    name[n] = 0;
#if defined(_WIN32)
    struct _stat st;
    if (_stat(name, &st) != 0) return 0;
    return (st.st_mode & _S_IFREG) != 0 ? 1 : 0;
#else
    struct stat st;
    if (stat(name, &st) != 0) return 0;
    return S_ISREG(st.st_mode) ? 1 : 0;
#endif
}

/* 文件大小（字节），对标 Go os.Stat+FileInfo.Size；失败返回 -1。 */
int64_t ax_file_size(ax_str path) {
    char name[1024];
    int64_t n = path.len < 1023 ? path.len : 1023;
    memcpy(name, path.p, (size_t)n);
    name[n] = 0;
#if defined(_WIN32)
    struct _stat st;
    if (_stat(name, &st) != 0) return -1;
    return (int64_t)st.st_size;
#else
    struct stat st;
    if (stat(name, &st) != 0) return -1;
    return (int64_t)st.st_size;
#endif
}

/* 最后修改时间（Unix 毫秒），对标 Go os.Stat+FileInfo.ModTime；失败返回 -1。 */
int64_t ax_file_modtime(ax_str path) {
    char name[1024];
    int64_t n = path.len < 1023 ? path.len : 1023;
    memcpy(name, path.p, (size_t)n);
    name[n] = 0;
#if defined(_WIN32)
    struct _stat st;
    if (_stat(name, &st) != 0) return -1;
    return (int64_t)st.st_mtime * 1000;
#else
    struct stat st;
    if (stat(name, &st) != 0) return -1;
    return (int64_t)st.st_mtime * 1000;
#endif
}

/* 将整个字符串写入文件（创建/截断），0=成功 1=失败 */

/* 将整个字符串写入文件（创建/截断），0=成功 1=失败 */
int ax_file_write_all(ax_str path, ax_str s) {
    char name[1024];
    int64_t n = path.len < 1023 ? path.len : 1023;
    memcpy(name, path.p, (size_t)n);
    name[n] = 0;
    FILE* fp = fopen(name, "wb");
    if (!fp) return 1;
    if (s.len > 0 && fwrite(s.p, 1, (size_t)s.len, fp) != (size_t)s.len) {
        fclose(fp);
        return 1;
    }
    fclose(fp);
    return 0;
}

/* 修改文件权限，对标 Go os.Chmod / C chmod；0=成功 1=失败。 */

/* 修改文件权限，对标 Go os.Chmod / C chmod；0=成功 1=失败。 */
int ax_file_chmod(ax_str path, int mode) {
    char name[1024];
    int64_t n = path.len < 1023 ? path.len : 1023;
    memcpy(name, path.p, (size_t)n);
    name[n] = 0;
#if defined(_WIN32)
    return _chmod(name, mode) == 0 ? 0 : 1;
#else
    return chmod(name, (mode_t)mode) == 0 ? 0 : 1;
#endif
}

/* 修改访问/修改时间，对标 Go os.Chtimes；atime/mtime 为 Unix 毫秒；0=成功 1=失败。 */

/* 修改访问/修改时间，对标 Go os.Chtimes；atime/mtime 为 Unix 毫秒；0=成功 1=失败。 */
int ax_file_chtimes(ax_str path, int64_t atime, int64_t mtime) {
    char name[1024];
    int64_t n = path.len < 1023 ? path.len : 1023;
    memcpy(name, path.p, (size_t)n);
    name[n] = 0;
#if defined(_WIN32)
    struct _utimbuf times;
    times.actime = (time_t)(atime / 1000);
    times.modtime = (time_t)(mtime / 1000);
    return _utime(name, &times) == 0 ? 0 : 1;
#else
    struct timeval times[2];
    times[0].tv_sec = (time_t)(atime / 1000);
    times[0].tv_usec = (long)((atime % 1000) * 1000);
    times[1].tv_sec = (time_t)(mtime / 1000);
    times[1].tv_usec = (long)((mtime % 1000) * 1000);
    return utimes(name, times) == 0 ? 0 : 1;
#endif
}

/* 创建临时文件，对标 Go os.CreateTemp。dir 为目录，prefix 为文件名前缀。 */

/* 创建临时文件，对标 Go os.CreateTemp。dir 为目录，prefix 为文件名前缀。 */
ax_res_file ax_file_tempfile(ax_str dir, ax_str prefix) {
    ax_res_file r;
    char name[1024];
    int64_t dn = dir.len < 1023 ? dir.len : 1023;
    memcpy(name, dir.p, (size_t)dn);
    name[dn] = 0;
    int len = (int)dn;
    if (len > 0 && name[len-1] != '/' && name[len-1] != '\\') {
#if defined(_WIN32)
        name[len++] = '\\';
#else
        name[len++] = '/';
#endif
    }
    static int counter = 0;
    counter++;
    int64_t now = ax_now_ms();
    int pn = prefix.len < 128 ? prefix.len : 128;
    memcpy(name + len, prefix.p, (size_t)pn);
    len += pn;
    len += snprintf(name + len, (size_t)(sizeof name - len), "%lld_%d.tmp",
                    (long long)now, counter);
    if (len < 0 || len >= (int)sizeof name) {
        r.tag = 0;
        r.u.err = ax_err_new(4, ax_str_from_cstr("tempfile name too long"));
        return r;
    }
    name[len] = 0;
    FILE* fp = fopen(name, "wb+");
    if (!fp) {
        r.tag = 0;
        r.u.err = ax_err_new(4, ax_str_from_cstr("cannot create temp file"));
        return r;
    }
    ax_file* f = (ax_file*)ax_alloc((int64_t)sizeof(ax_file));
    f->fp = fp;
    r.tag = 1;
    r.u.ok = f;
    return r;
}

/* ---------------- os 模块 ---------------- */


/* ---------------- os 模块 ---------------- */

int ax_os_path_cstr(ax_str s, char* out, int cap) {
    if (s.len >= (int64_t)cap) return 0;
    memcpy(out, s.p, (size_t)s.len);
    out[s.len] = 0;
    return 1;
}

