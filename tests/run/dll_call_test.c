/* dll_call_test.c —— 从外部 C 程序调用 shared_test.dll 中的导出函数
 *
 * 通过 LoadLibrary + GetProcAddress 动态加载 DLL，调用：
 *   int64_t ax_export_shared_test__add(int64_t a, int64_t b)
 *   int64_t ax_export_shared_test__sub(int64_t a, int64_t b)
 *   ax_str   ax_export_shared_test__greet(ax_str name)
 *
 * 编译：dist/gcc/bin/gcc.exe dll_call_test.c -o dll_call_test.exe
 * 运行：./dll_call_test.exe   （shared_test.dll 必须在同目录）
 */

#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <windows.h>

/* 与 axrt.h 中 ax_str 定义保持一致 */
typedef struct { const uint8_t* p; int64_t len; } ax_str;

typedef int64_t (*add_fn)(int64_t a, int64_t b);
typedef int64_t (*sub_fn)(int64_t a, int64_t b);
typedef ax_str  (*greet_fn)(ax_str name);

static void print_ax_str(ax_str s) {
    for (int64_t i = 0; i < s.len; i++) {
        putchar((int)s.p[i]);
    }
}

int main(void) {
    /* DLL 与可执行文件同目录，LoadLibrary 可直接按名加载 */
    HMODULE hDll = LoadLibraryA("shared_test.dll");
    if (hDll == NULL) {
        fprintf(stderr, "LoadLibrary 失败，错误码: %lu\n", GetLastError());
        return 1;
    }

    add_fn  add = (add_fn) GetProcAddress(hDll, "ax_export_shared_test__add");
    sub_fn  sub = (sub_fn) GetProcAddress(hDll, "ax_export_shared_test__sub");
    greet_fn greet = (greet_fn) GetProcAddress(hDll, "ax_export_shared_test__greet");

    if (add == NULL || sub == NULL || greet == NULL) {
        fprintf(stderr, "GetProcAddress 失败\n");
        fprintf(stderr, "  add  = %p\n", (void*)add);
        fprintf(stderr, "  sub  = %p\n", (void*)sub);
        fprintf(stderr, "  greet= %p\n", (void*)greet);
        FreeLibrary(hDll);
        return 1;
    }

    /* 调用 add */
    int64_t r_add = add(10, 20);
    printf("add(10, 20) = %lld\n", (long long)r_add);

    /* 调用 sub */
    int64_t r_sub = sub(10, 20);
    printf("sub(10, 20) = %lld\n", (long long)r_sub);

    /* 调用 greet —— 传入 ax_str 字符串，接收 ax_str 返回值 */
    ax_str name = { (const uint8_t*)"World", 5 };
    ax_str r_greet = greet(name);

    printf("greet(\"World\") = ");
    print_ax_str(r_greet);
    printf("\n");

    /* 额外：greet 内部会调用 io.writeLine 向 stdout 写日志，此处已随 stdout 输出 */

    FreeLibrary(hDll);
    printf("外部程序调用 DLL 导出函数：全部成功\n");
    return 0;
}
