/* axrt_os.c -- axlang 运行时模块 */
#include "axrt.h"
#include "axrt_internal.h"


#ifdef _WIN32
static UINT ax_cp_out_saved = 0;
static UINT ax_cp_in_saved = 0;

static void ax_console_restore(void) {
    if (ax_cp_out_saved) SetConsoleOutputCP(ax_cp_out_saved);
    if (ax_cp_in_saved) SetConsoleCP(ax_cp_in_saved);
}
#endif


void ax_console_utf8(void) {
#ifdef _WIN32
    if (_isatty(_fileno(stdout)) || _isatty(_fileno(stderr))) {
        UINT cp = GetConsoleOutputCP();
        if (cp != 0 && cp != CP_UTF8 && SetConsoleOutputCP(CP_UTF8)) {
            ax_cp_out_saved = cp;
            atexit(ax_console_restore);
        }
    }
    if (_isatty(_fileno(stdin))) {
        UINT cp = GetConsoleCP();
        if (cp != 0 && cp != CP_UTF8 && SetConsoleCP(CP_UTF8)) {
            ax_cp_in_saved = cp;
        }
    }
#endif
}

/* 读环境变量并统一成 UTF-8。
   Windows 的窄字符 getenv 返回 ANSI 代码页字节，中文夹具会变成 GBK，
   与源码里的 UTF-8 字面量混在一起就是乱码 —— 这里走宽字符接口再转 UTF-8。 */
ax_str ax_env_utf8(const char* name) {
#ifdef _WIN32
    wchar_t wname[64];
    int i = 0;
    for (; name[i] && i < 63; i++) wname[i] = (wchar_t)(unsigned char)name[i];
    wname[i] = 0;
    wchar_t wval[4096];
    DWORD n = GetEnvironmentVariableW(wname, wval, 4096);
    if (n == 0 || n >= 4096) return ax_str_empty();
    char buf[8192];
    int m = WideCharToMultiByte(CP_UTF8, 0, wval, (int)n, buf, (int)sizeof buf, NULL, NULL);
    if (m <= 0) return ax_str_empty();
    return ax_str_from_bytes((const uint8_t*)buf, (int64_t)m);
#else
    const char* v = getenv(name);
    return v ? ax_str_from_cstr(v) : ax_str_empty();
#endif
}


int ax_parse_i64(ax_str s, int64_t* out) {
    char buf[64];
    if (s.len <= 0 || s.len >= 63) return 0;
    memcpy(buf, s.p, (size_t)s.len);
    buf[s.len] = 0;
    char* end = NULL;
    long long v = strtoll(buf, &end, 10);
    if (!end || end == buf) return 0;
    while (*end == ' ' || *end == '\t') end++;
    if (*end != 0) return 0;
    *out = (int64_t)v;
    return 1;
}


int ax_parse_bool(ax_str s) {
    if (s.len == 4) {
        if (memcmp(s.p, "true", 4) == 0) return 1;
    } else if (s.len == 5) {
        if (memcmp(s.p, "false", 5) == 0) return 0;
    } else if (s.len == 1) {
        if (s.p[0] == (uint8_t)'1') return 1;
        if (s.p[0] == (uint8_t)'0') return 0;
    }
    return 0;
}


int ax_parse_f64(ax_str s, double* out) {
    char buf[64];
    if (s.len <= 0 || s.len >= 63) return 0;
    memcpy(buf, s.p, (size_t)s.len);
    buf[s.len] = 0;
    char* end = NULL;
    double v = strtod(buf, &end);
    if (!end || end == buf) return 0;
    while (*end == ' ' || *end == '\t') end++;
    if (*end != 0) return 0;
    *out = v;
    return 1;
}

int64_t str_to_i64(ax_str s) {
    int64_t v = 0;
    if (!ax_parse_i64(s, &v)) return 0;
    return v;
}


int ax_parse_int_base(ax_str s, int base) {
    char buf[64];
    if (s.len <= 0 || s.len >= 63) return 0;
    memcpy(buf, s.p, (size_t)s.len);
    buf[s.len] = 0;
    char* end = NULL;
    /* base<=0：按前缀自动判断（0x=16, 0=8, 否则=10），与 Go strconv 一致 */
    int b = base;
    if (b <= 0) {
        if (s.len >= 2 && buf[0] == '0' && (buf[1] == 'x' || buf[1] == 'X')) b = 16;
        else if (buf[0] == '0') b = 8;
        else b = 10;
    }
    long long v = strtoll(buf, &end, b);
    if (!end || end == buf) return 0;
    while (*end == ' ' || *end == '\t') end++;
    if (*end != 0) return 0;
    return (int)v;
}


ax_str ax_os_getenv(ax_str name) {
    char buf[256];
    if (!ax_os_path_cstr(name, buf, (int)sizeof buf)) return ax_str_empty();
    return ax_env_utf8(buf);
}

/* 用户主目录：Windows 读 USERPROFILE，Unix 读 HOME；未设置返回空串 */

/* 用户主目录：Windows 读 USERPROFILE，Unix 读 HOME；未设置返回空串 */
ax_str ax_os_user_home_dir(void) {
#if defined(_WIN32)
    return ax_os_getenv(AX_S("USERPROFILE"));
#else
    return ax_os_getenv(AX_S("HOME"));
#endif
}


void ax_os_exit(int code) {
    exit(code);
}

/* 命令行参数全局（由 main 包装器在进入用户 main 前设置） */

/* 命令行参数全局（由 main 包装器在进入用户 main 前设置） */
static ax_arr g_os_args;
static int  g_os_args_set = 0;


void ax_os_set_args(ax_arr* args) {
    if (!g_os_args_set) {
        g_os_args = *args;      /* 浅拷贝：data 指针由 main 端的 ax_arr 管理 */
        g_os_args_set = 1;
    }
}


ax_arr ax_os_args(void) {
    return g_os_args;
}


ax_str ax_os_getwd(void) {
    char cwd[4096];
#if defined(_WIN32)
    if (!_getcwd(cwd, (int)sizeof cwd)) return ax_str_empty();
#else
    if (!getcwd(cwd, (size_t)sizeof cwd)) return ax_str_empty();
#endif
    return ax_str_from_cstr(cwd);
}


int ax_os_chdir(ax_str path) {
    char buf[4096];
    if (!ax_os_path_cstr(path, buf, (int)sizeof buf)) return 1;
#if defined(_WIN32)
    return _chdir(buf) == 0 ? 0 : 1;
#else
    return chdir(buf) == 0 ? 0 : 1;
#endif
}

/* ---------------- os 进程/环境扩展（对标 Go os） ---------------- */


/* ---------------- os 进程/环境扩展（对标 Go os） ---------------- */

int ax_os_getpid(void) {
#if defined(_WIN32)
    return (int)GetCurrentProcessId();
#else
    return (int)getpid();
#endif
}


int ax_os_getppid(void) {
#if defined(_WIN32)
    return 0;  /* Windows 暂不提供父进程 ID */
#else
    return (int)getppid();
#endif
}


ax_str ax_os_hostname(void) {
    char buf[256];
#if defined(_WIN32)
    DWORD n = (DWORD)sizeof buf;
    if (!GetComputerNameA(buf, &n)) return ax_str_empty();
    return ax_str_from_cstr(buf);
#else
    if (gethostname(buf, sizeof buf) != 0) return ax_str_empty();
    return ax_str_from_cstr(buf);
#endif
}


ax_arr ax_os_environ(void) {
    ax_arr arr;
    ax_arr_init(&arr);
#if defined(_WIN32)
    /* <process.h> 已声明 _environ（dllimport），此处直接引用，避免重复声明属性冲突 */
    char **env = _environ;
#else
    extern char **environ;
    char **env = environ;
#endif
    for (char **p = env; p && *p; p++) {
        ax_str s = ax_str_from_cstr(*p);
        ax_arr_push(&arr, &s, (int64_t)sizeof(ax_str));
    }
    return arr;
}


int ax_os_setenv(ax_str name, ax_str value) {
    char buf[512];
    if (!ax_os_path_cstr(name, buf, (int)sizeof buf)) return 1;
    int n = (int)strlen(buf);
    if (value.len > 0) {
        if (n + 1 + (int)value.len >= (int)sizeof buf) return 1;
        buf[n] = '=';
        memcpy(buf + n + 1, value.p, (size_t)value.len);
        buf[n + 1 + (int)value.len] = 0;
    } else {
        buf[n] = '=';
        buf[n + 1] = 0;
    }
#if defined(_WIN32)
    return _putenv(buf) == 0 ? 0 : 1;
#else
    char *eq = strchr(buf, '=');
    if (!eq) return 1;
    *eq = 0;
    int rc = setenv(buf, eq + 1, 1);
    *eq = '=';
    return rc == 0 ? 0 : 1;
#endif
}


int ax_os_unsetenv(ax_str name) {
    char buf[256];
    if (!ax_os_path_cstr(name, buf, (int)sizeof buf)) return 1;
#if defined(_WIN32)
    /* Windows 无 unsetenv，把值置空以模拟删除 */
    int n = (int)strlen(buf);
    buf[n] = '=';
    buf[n + 1] = 0;
    return _putenv(buf) == 0 ? 0 : 1;
#else
    return unsetenv(buf) == 0 ? 0 : 1;
#endif
}

/* ---------------- rand 模块（对标 Go math/rand） ---------------- */

