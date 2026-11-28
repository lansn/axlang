/* axrt_env.c -- axlang env 模块 C 运行时实现（对标 Go os 环境变量增值功能）
 * 纯标准 C11，不使用任何 GNU/Clang 扩展。
 */

#include "axrt.h"
#include "axrt_internal.h"

/* ---------------- env_expand：展开 $VAR 和 ${VAR} ---------------- */

ax_str ax_env_expand(ax_str s) {
    ax_sb sb;
    ax_sb_init(&sb);

    int64_t i = 0;
    while (i < s.len) {
        if (s.p[i] == (uint8_t)'$') {
            /* 检查是否为 ${VAR} 形式 */
            if (i + 1 < s.len && s.p[i + 1] == (uint8_t)'{') {
                int64_t j = i + 2;
                while (j < s.len && s.p[j] != (uint8_t)'}') j++;
                if (j < s.len) {
                    /* 找到闭合的 } */
                    int64_t name_len = j - i - 2;
                    char namebuf[256];
                    if (name_len > 0 && (int64_t)sizeof namebuf - 1 > name_len) {
                        memcpy(namebuf, s.p + i + 2, (size_t)name_len);
                        namebuf[name_len] = 0;
                        ax_str val = ax_env_utf8(namebuf);
                        ax_sb_push(&sb, val);
                    }
                    i = j + 1;
                    continue;
                }
                /* 没有闭合的 }，将 $ 原样输出 */
            } else {
                /* $VAR 形式：读取后面的标识符字符 */
                int64_t j = i + 1;
                while (j < s.len &&
                       ((s.p[j] >= (uint8_t)'a' && s.p[j] <= (uint8_t)'z') ||
                        (s.p[j] >= (uint8_t)'A' && s.p[j] <= (uint8_t)'Z') ||
                        (s.p[j] >= (uint8_t)'0' && s.p[j] <= (uint8_t)'9') ||
                        s.p[j] == (uint8_t)'_')) {
                    j++;
                }
                if (j > i + 1) {
                    int64_t name_len = j - i - 1;
                    char namebuf[256];
                    if ((int64_t)sizeof namebuf - 1 > name_len) {
                        memcpy(namebuf, s.p + i + 1, (size_t)name_len);
                        namebuf[name_len] = 0;
                        ax_str val = ax_env_utf8(namebuf);
                        ax_sb_push(&sb, val);
                    }
                    i = j;
                    continue;
                }
            }
            /* 不是合法的环境变量引用，将 $ 原样输出 */
            {
                ax_str one = { s.p + i, 1 };
                ax_sb_push(&sb, one);
            }
            i++;
        } else {
            /* 累积连续的非 $ 字符块 */
            int64_t j = i + 1;
            while (j < s.len && s.p[j] != (uint8_t)'$') j++;
            ax_str block = { s.p + i, j - i };
            ax_sb_push(&sb, block);
            i = j;
        }
    }

    return ax_sb_finish(&sb);
}

/* ---------------- env_clearenv：清除所有环境变量 ---------------- */

int ax_env_clearenv(void) {
#if defined(_WIN32)
    /* Windows 没有 clearenv，需要遍历 _environ 并逐个删除 */
    char **env = _environ;
    /* 先收集所有变量名，避免在遍历时修改 _environ 导致问题 */
    /* 简单做法：逐个读取并删除（_environ 会随 _putenv 变化，但通常可行） */
    for (char **p = env; p && *p; ) {
        char *eq = strchr(*p, '=');
        if (eq) {
            size_t namelen = (size_t)(eq - *p);
            char *namebuf = (char*)malloc(namelen + 1);
            if (namebuf) {
                memcpy(namebuf, *p, namelen);
                namebuf[namelen] = 0;
                /* 构造 "NAME=" 以删除 */
                size_t buflen = namelen + 2;
                char *delbuf = (char*)malloc(buflen);
                if (delbuf) {
                    memcpy(delbuf, namebuf, namelen);
                    delbuf[namelen] = '=';
                    delbuf[namelen + 1] = 0;
                    _putenv(delbuf);
                    free(delbuf);
                }
                free(namebuf);
            }
            /* 删除后 _environ 可能变化，从头重新开始（简单但低效，正确） */
            p = _environ;
        } else {
            p++;
        }
    }
    return 0;
#else
    /* Unix/POSIX：使用 clearenv()（POSIX.1-2008） */
    extern char **environ;
    /* 逐个 unsetenv 比 clearenv() 更可移植（clearenv 并非所有系统都有） */
    for (char **p = environ; p && *p; ) {
        char *eq = strchr(*p, '=');
        if (eq) {
            size_t namelen = (size_t)(eq - *p);
            char *namebuf = (char*)malloc(namelen + 1);
            if (namebuf) {
                memcpy(namebuf, *p, namelen);
                namebuf[namelen] = 0;
                unsetenv(namebuf);
                free(namebuf);
            }
            p = environ;
        } else {
            p++;
        }
    }
    return 0;
#endif
}

/* ---------------- env_exists：判断环境变量是否存在 ---------------- */

int ax_env_exists(ax_str key) {
    char buf[512];
    if (!ax_os_path_cstr(key, buf, (int)sizeof buf)) return 0;
#if defined(_WIN32)
    wchar_t wbuf[512];
    int i = 0;
    for (; buf[i] && i < 511; i++) wbuf[i] = (wchar_t)(unsigned char)buf[i];
    wbuf[i] = 0;
    DWORD n = GetEnvironmentVariableW(wbuf, NULL, 0);
    return n > 0 ? 1 : 0;
#else
    return getenv(buf) != NULL ? 1 : 0;
#endif
}

/* ---------------- env_all：返回所有环境变量 ---------------- */

ax_arr ax_env_all(void) {
    return ax_os_environ();
}
