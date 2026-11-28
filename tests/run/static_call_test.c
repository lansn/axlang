/* static_call_test.c —— 从外部 C 程序链接并调用 axc 编译的静态库 static_test.lib
 *
 * 静态库中导出的函数（C 调用约定）：
 *   int64_t ax_export_static_test__add(int64_t a, int64_t b)
 *   int64_t ax_export_static_test__sub(int64_t a, int64_t b)
 *   ax_str   ax_export_static_test__greet(ax_str name)
 *
 * 编译（MinGW gcc）：
 *   dist/gcc/bin/gcc.exe static_call_test.c static_test.lib -o static_call_test.exe -I dist/axc/rt -I dist/axc/stdlib
 *
 * 运行：
 *   ./static_call_test.exe
 */

#include <stdio.h>
#include <stdint.h>

/* 与 axrt.h 中 ax_str 定义保持一致 */
typedef struct { const uint8_t* p; int64_t len; } ax_str;

static void print_ax_str(ax_str s) {
    for (int64_t i = 0; i < s.len; i++) {
        putchar((int)s.p[i]);
    }
}

/* 声明静态库导出函数 */
int64_t ax_export_static_test__add(int64_t a, int64_t b);
int64_t ax_export_static_test__sub(int64_t a, int64_t b);
ax_str    ax_export_static_test__greet(ax_str name);

int main(void) {
    /* 调用 add */
    int64_t r_add = ax_export_static_test__add(100, 200);
    printf("add(100, 200) = %lld\n", (long long)r_add);

    /* 调用 sub */
    int64_t r_sub = ax_export_static_test__sub(100, 200);
    printf("sub(100, 200) = %lld\n", (long long)r_sub);

    /* 调用 greet —— 传入 ax_str 字符串，接收 ax_str 返回值 */
    ax_str name = { (const uint8_t*)"StaticLib", 9 };
    ax_str r_greet = ax_export_static_test__greet(name);

    printf("greet(\"StaticLib\") = ");
    print_ax_str(r_greet);
    printf("\n");

    printf("外部程序链接静态库调用导出函数：全部成功\n");
    return 0;
}
