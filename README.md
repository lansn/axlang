# axc — axlang 编译器（v0.0.27，初具雏形）

axlang 是纯AI生成的静态类型编译型语言，包括编译器/库/文档

依据 `axlang编译器实现规格书.md` 实现；语言真相唯一来源是 `axlang语法设计文稿.md`。
缺口与偏差见 `GAP-REPORT.md`。

> **实现语言：Rust**（`rust/`，单个 exe 约 1.1 MB，零运行时依赖）。
> 不再参与构建与验收；迁移过程与逐阶段验收记录见 `RUST-MIGRATION.md`。
>
> **接手本项目**：先读 `ARCHITECTURE.md`（架构与开发指南），版本历史见 `CHANGELOG.md`。

## 用法

```bash
dist\axc.exe check samples\01_hello.ax        # 词法 / 语法 / 语义检查
dist\axc.exe build samples\01_hello.ax -o hi  # 生成 C11 + 调 gcc 产出可执行文件
dist\axc.exe run   samples\01_hello.ax        # build 后立即执行
```

选项：`-o <path>`、`--emit=c`、`--dump=ast|types|c`、`--diag=text|json`、`-Werror`、`--cc=<path>`、`--keep`、`--lang=en|zh`（诊断语言，默认英文；亦可 `AXC_LANG` 或 `axc.toml`）、`-V/--version`。
退出码：`0` 成功、`1` 有 error 级诊断、`2` 目标程序运行时崩溃（R101–R107）。

## 构建

```bash
cd rust && cargo build --release    # → rust/target/release/axc.exe（约 1.1 MB）
cp target/release/axc.exe ../dist/axc.exe
```

`rust\build.cmd` 是 cmd 下的等价脚本（先查 PATH，查不到回落 `~/.cargo/bin`——Windows
的环境变量在进程启动时就固定了，rustup 装完后**已开着的**终端里 `cargo` 仍不可见，
详见 `RUST-MIGRATION.md`）。工具链配置（本机缺 `link.exe`，改用 GNU target + `rust-lld`）也记在那里。

## 示例

`samples/` 下有 12 个可直接运行的程序 + `samples/demo/` 下的多模块工程，覆盖类型、控制流、类与接口、错误处理（`result` + `match`、`try`/`catch`/`finally`）、容器、金额、`defer`、崩溃演示和完整业务示例：

```bash
python samples/run_all.py     # 一次跑完，打印每个程序的输出
```

详见 `samples/README.md`。

## 结构

```
axc/rt/axrt.{h,c}           运行时：字符串、decimal、数组、map（插入序）、Row/db 夹具、
                            控制台 UTF-8（ax_console_utf8，仅交互式终端切换代码页）
rust/src/main.rs            入口 + crate 级导览（管线、模块职责、退出码契约）
rust/src/cli.rs             子命令、C 编译、执行、退出码、运行时目录探测
rust/src/diag.rs            诊断收集 + text/JSONL 输出（排序键 path,line,col,code）
rust/src/types.rs           类型表示、canonical、assignable 唯一入口
rust/src/frontend/          前端：lexer.rs 词法（39 关键字：新增 extern/unsafe，三类字符串、插值片段化、
                            码点列号）、parser.rs 语法（nl/expect_term 分工、G-10…G-16
                            消解与恢复）、ast.rs AST 节点 + --dump=ast 序列化
rust/src/sema/              语义：mod.rs 主类与语义槽 slots、stmt.rs 语句级（含
                            try/catch 检查）、expr.rs 表达式级与 coerce、member.rs
                            成员访问/方法调用/常量求值
rust/src/backend/           C11 后端：mod.rs 程序级（类拉平、vtable、main 包装）、
                            codegen_fn.rs 语句级（defer/try/catch 展开）、
                            codegen_expr.rs 表达式级 + match
dist/axc.exe                分发产物（release 构建的拷贝）
legacy/                     已归档的 Python 参照实现（zip，不参与构建）
samples/                    可直接运行的示例 + run_all.py
tests/runtests.py           §16 用例驱动器
tests/{lex,parse,sema,own,run,mod}（mod 为多文件模块用例：`tests/mod/<ID>/main.ax`）
```

每个源文件头部的 `//!` 注释写明职责、输入输出与关键不变量，是深入了解的第一入口。

运行时 `axc/rt/` 与实现语言无关：生成代码 `#include "axrt.h"`，`build` 时会把它拷进临时目录一起编译。

## §15 FFI / unsafe

axlang 支持 `extern "C" { ... }` 声明外部 C 函数，以及 `unsafe { ... }` 块调用它们。设计遵循“一看代码就能懂”原则：`extern` 函数仅签名声明、无函数体；`unsafe` 块在 C 后端直接展开块内容（C 本身即不安全），无额外运行时开销。

```ax
extern "C" {
    int puts(string s);
}

int main() {
    unsafe {
        puts("hello from axlang FFI")
    }
    return 0
}
```

- `extern "C" { int foo(string s); }`：声明外部 C 函数，`ret` 与 `params` 按 axlang 类型系统解析；函数不得有函数体（否则 `E3046`）。
- `unsafe { ... }`：包裹对 extern 函数的调用，直接生成 C 层调用，无包装、无运行时检查。
- 目前 `extern` 函数生成本地空函数体（stub），未接入真实 C 符号链接，调用时运行无实际输出（见 `tests/run/FFI-1.out`）。
- 测试：`tests/run/FFI-1.ax` + `FFI-1.out`。

## 测试

```bash
python tests/runtests.py            # 全量（自动探测编译器）
python tests/runtests.py run -v     # 只看端到端，逐条打印
python tests/runtests.py --axc=xxx  # 指定编译器（默认 rust/target/release → dist）
```

- 负例（`lex|parse|sema|own`）：首行 `// EXPECT: E4001`，断言**恰好一条**诊断且 code 相符；`// EXPECT: PASS` 表示应无错误。
- 正例（`run`）：同目录放 `<ID>.fixture`（`AX_DB_FIXTURE`）、`<ID>.out`（期望 stdout，逐字节）、`<ID>.exit`、`<ID>.errsub`、`<ID>.args`。

当前：**192/192 通过**（含 try/catch、可见性、constructor 通行证、静态 `this` 拦截、多文件模块、匿名函数与其作用域隔离、数组可变方法落盘、`print`/`println` 换行差异与多实参空格拼接、`fmt.Printf` 编译期占位符核对、`File` 读写与 `Append`)，含 §16.10 的 `RUN-1`（`折后小计 53.97` / `柴犬粮 ¥19.99（库存 10）`，退出码 0）、`RUN-2`、`RUN-3`、`RUN-4`，`Rt-01…Rt-16` 七类崩溃与数值/顺序/精度用例，`TRY-1…TRY-4` / `Try-1…Try-3` 错误处理正反例，`Vis-1…Vis-3` 可见性显式负例，以及 `Ctor-1…Ctor-2` / run `Ctor-1` 无 constructor 不可 new 正反例、`Scope-1` 匿名函数不污染外层作用域、`Mut-1` 数组可变方法生效、`IO-1` 输出换行差异、`File-1` 全流程读写、`Out-1` 旧名 `Print` 迁移提示、`Top-1`/`Top-2` 顶层语句与顶层变量的明确诊断、`Mod-neg-builtin` 内建模块名保留（`E3039`）、`Mod-3` 三级模块路径下钻、`Null-1` 可空值插值（有值显示值 / `null` 显示 `null`，含 `decimal?`/`float?` 字面量落地）、`Interp-1` 复合类型插值报 `E5006`、`Mod-4` 具名导入绑定函数/常量/类型、`Mod-neg-named` 具名导入白名单负例、`Main-1` `void main()` 签名负例（`check` 即报 `E3025`）、`Mod-5` 导入末段短名（`import net.http` 后 `http.ping()`）、`Mod-neg-short` 短名冲突 `E3040`、`Mod-6` 跨模块实例化（3 段路径 / `new` / 静态工厂 / struct 位置构造）、`Struct-1` struct 值语义、`Top-3` 隐式 `main`（顶层语句免写 `main`）、`Mod-neg-top` 模块顶层语句负例、`demo/OrderDemo` 多模块示例工程（`samples/demo/OrderDemo`：`catalog.product` / `catalog.inventory` / `checkout.bill` 具名导入 / `checkout.wholesale` 跨模块继承，逐字节比对 `main.out`）、`Mod-7` 裸类型名（`import model` 后 `new Product(...)` / `Product.Sample()` / `Point(3,4)` / `Color.Red`）、`Mod-neg-ambig` 同名类型歧义 `E3042`、`Mod-8` 跨模块 interface 多态（`implement model.Describable` / interface 变量再赋子类实例派发到 override / interface 作跨模块参数）、`Upcast-1` 上转赋值显式强转（声明/传参/return 三处，`-Werror` 零警告）、`Iface-1` interface 多态（interface 变量/参数/返回值/数组，胖指针动态派发）、`Iface-2` interface extend（父 interface 方法经子 interface 变量调用，实现子 interface 即隐式实现父 interface）、`Box-1` object[] 异构数组（装箱/自动解箱/object 中转，标签不符 `R107` 终止）、`Sort-1` 数组 `Sort(cmp)` 三种比较器（匿名函数/命名函数/类元素）、`Printable-1` 实现 `Printable.ToString` 后 class 直接插值（接口变量动态派发 / 可空有值 / `null`）、`Comp-1` `Comparable.Compare(Self)` 经 shim 虚表派发 + `Hashable.Hash()`、`Printable-neg-1/2/3` 内建 interface 重名声明 `E3018` / 未实现插值 `E5006` / 缺实现 `E3019`、`Json-1` json/time 实化（`json.Parse` 解析 + 各类型解箱 + JSON null 键缺席 + 语法错误 fail + `json.Stringify` 往返含转义 + `time.NowMs`）、`Pat-1` 类型模式四种来源（object 箱上的类模式按装箱 tag、类引用上的类模式按对象头真实 tag、object 箱上的标量模式）、`Pat-neg-1` 标量值上用类型模式 `E4001`、`Print-1` `print`/`println` 实参可直接是数字/布尔/枚举/实现 `Printable` 的类（等价补花括号的插值）、`Print-neg-1` 复合类型实参 `E4001`、`Typeof-1` `typeof(表达式)` 编译期类型名（标量/类/数组/可空 + 插值 + const）、`Typeof-neg-1` 裸 `typeof` 提示 `E3003`、`Lit-1` `0x`/`0b`/`0o` 进制字面量、`Str-1` 码点往返（`CodepointAt` 越界 `null` / `FromCodepoint` / 逐码点迭代）、`Sizeof-1` `sizeof(表达式)` 十一种类型的存储大小（class 按引用给指针大小）、`Sizeof-neg-1` 裸 `sizeof` 提示 `E3003`、`GX-1` 泛型函数（推断/显式实参/多实例共存/嵌套实例化/实例体内匿名函数）、`GX-2` 泛型类与 struct（字段/方法/嵌套实例/实例类实现 interface 胖指针派发）、`GX-3` 泛型类型别名、`GX-neg-1..4` 约束拒绝（E4036）/不变型（E4001）/递归上限（E4038）、`O-01…O-12` 所有权负例（move 后使用 / 借用冲突 / 借用期 move / 借用目标 / const 借用 / 分支与循环保守合并 / arena 逃逸）、`Borrow-1` 借用正例（多 `&const` 共存 / 顺序借用 / 复活 / `W1003`）、`Branch-1` 分支循环合并正例、`Arena-1` arena 与 ArenaRef 图结构（循环引用 / 同区域别名 / 整块释放）、`ArenaRef-neg-1..3` 逃逸与 `Alloc` 类型负例、`Math-1` 模块成员 PascalCase（`math.Compare`/`math.Equals`）、`IO-2` `io.ReadFile` 回读、`Name-neg-1` 小写旧写法 `io.readLine()` 的点名提示（`E3003`）、`Name-neg-2` 小写成员声明 `read` 的点名提示（`E3043`）、`G-10` `print(a++)` 恰好一条 `E2010`（修复恢复刷屏）、`Print-neg-2` 多实参 `print` 恰好一条 `E4031`、`Fmt-1` `fmt.Printf`/`fmt.Print`/`fmt.Println` 正例、`Fmt-neg-1..5` 占位符类型/个数与非法占位符负例、`StrSlice-1` `string.Left`/`Right`/`Mid` 截取（含越界安全截断）。

## 注意

- 需要 `gcc`（可用 `--cc` 或 `$AX_CC` 指定）。`dist\axc.exe` 是 Rust 编译的独立程序，不依赖 Python。
- 跑 `tests/runtests.py` / `samples/run_all.py` 需要 Python 3（只是驱动脚本，不是编译器实现）。
- `decimal` 用 32 字节 `__int128`（`CONFLICT-5` 收口），语义是 128 位十进制定点。
- 未实现清单（泛型单态化、完整借用生命周期、arena、匿名函数后端、json/file/time 模块）见 `GAP-REPORT.md` §3.2。

## Windows 中文显示

生成的可执行文件在 Windows 控制台里输出中文不需要手动 `chcp`：`main` 入口会调用
`ax_console_utf8()`，把控制台代码页从系统默认（中文系统是 936/GBK）切到 UTF-8，
退出时（含运行时崩溃 `exit(2)`）自动恢复原值。

- 只在**真正连着控制台**时切换（`_isatty` 判断）。重定向到文件或管道时字节原样写出，
  代码页不动，所以 `axc run x.ax > out.txt` 得到的是纯 UTF-8 文本，测试里的逐字节比对也不受影响。
- 若某个终端仍显示方块/问号，那是**控制台字体**没有中文字形（选 Consolas 或新宋体即可），不是编码问题。
- **源文件必须是 UTF-8**。用记事本「ANSI」或 GBK 保存的 `.ax` 会直接报
  `axc: 源文件不是合法 UTF-8`（这是有意的：不猜编码，避免静默产出乱码字符串）。
