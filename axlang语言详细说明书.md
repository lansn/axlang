# axlang 语言详细说明书

**版本**：0.0.28  
**编译器**：axc（Rust 实现，生成 C11 代码，再由系统 gcc 编译）  
**真相源**：`D:/Project/axlang/rust/src/` 编译器源码  
**适用**：以编译器源码为准，与之前的设计文稿和实现规格书同步更新

---

## 1. 语言概览

axlang 是一种静态类型、编译型、面向对象的系统级编程语言，编译器为单 exe（约 1.1 MB），产物为 C11 源码，再由系统 C 编译器（gcc）生成最终可执行文件。

### 1.1 编译管线

```
.ax 源码 (UTF-8)
   │ frontend::lexer   字符流 → Token 流（码点列号、插值片段化）
   ▼
Token 流
   │ frontend::parser  Token 流 → AST（错误恢复、歧义消解）
   ▼
AST Module ───────────► sema：检查结论全部写入 Sema::slots（键=节点 id）
   │
   ▼
AST + slots
   │ backend（mod / codegen_fn / codegen_expr）
   ▼
单个 C 翻译单元（纯 ASCII）+ axrt.h
   │ cli：调用系统 cc（gcc，可 --cc 指定）
   ▼
可执行文件 / 共享库
```

### 1.2 退出码契约（对外承诺，勿改）

| 码 | 含义 |
|----|------|
| 0  | 成功 |
| 1  | 有 error 级诊断 |
| 2  | 目标程序运行时崩溃 |

### 1.3 CLI 子命令与选项

```
axc [check|build|run|env] <file.ax> [选项]
```

不写子命令时，等价于 `axc run <file.ax>`（构建后立即执行）。

| 子命令 | 含义 |
|--------|------|
| `check` | 只做词法→语法→语义检查，报诊断，不生成 C（必须显式写出） |
| `build` | 生成 C11 源码并调用 cc 产出可执行文件 |
| `run`   | build 后立即执行，产物放临时目录，跑完即删（省略子命令时的默认行为） |
| `precompile` | 单独预编译 axrt 运行时和标准库为 .o 缓存，后续 `axc run`/`axc build` 直接复用，无需重新编译库代码。发行包 `dist/` 已随版本打包全部预编译的 `.o` + `.hash` 缓存，用户直接使用即可，无需首次运行 `axc precompile`。axc 启动时不自动执行预编译，若缓存缺失会回退到在 unit.c 中合并标准库 C 代码，由 gcc 一次性编译 |
| `env`   | 显示或设置编译器环境（类似 `go env`），不需要 file 参数 |

通用选项：

| 选项 | 别名 | 含义 |
|------|------|------|
| `-I <dir>` | `--include` | 追加模块搜索根（可多次；入口文件目录优先） |
| `-o <path>` | `--output` | 输出文件名 |
| `--emit=c` | `-c` | 只输出生成的 C 源码到 stdout |
| `--dump=ast\|types\|c` | | 打印中间产物 |
| `--diag=text\|json` | | 诊断格式，json 为 JSONL |
| `-Werror` | | 警告升级为错误 |
| `--cc=<path>` | | 指定 C 编译器（默认取 `$AX_CC` 或 gcc） |
| `--lang=en\|zh` | | 诊断文案语言（默认 en；也可用 `AXC_LANG` 环境变量或 `axc.toml`） |
| `-k` | `--keep` | 保留构建目录（排查生成代码用） |
| `--kind=exe\|shared\|static` | | 产物类型：exe（默认可执行文件）、shared（动态链接库 .dll/.so/.dylib）或 static（静态链接库 .lib/.a） |
| `-v` / `-V` / `--version` | | 打印版本号后退出 |
| `-h` / `--help` | | 打印帮助 |

语言解析顺序：`--lang` > 环境变量 `AXC_LANG` > `axc.toml`（顶层 `lang = "zh"`）> 默认英文。

---

## 2. 词法规则

### 2.1 源码规范化

- 源码必须为 UTF-8 编码。
- `\r\n` 和 `\r` 都规范化为 `\n`。
- 可选的 BOM (`\u{feff}`) 会被剥离。
- 列号按 **码点（Unicode code point）** 计算，非按字节，以避免中文内容上列号错误。

### 2.2 空白与注释

- 空白字符：空格、制表符、`\u{0b}`、`\u{0c}` 均被跳过。
- 换行 `\n` 产生 `NL` token，作为语句分隔。
- 行注释：`//` 到行尾。
- 块注释：`/* ... */`，支持嵌套。块注释内的换行仍产生 `NEWLINE`。

### 2.3 关键字（共 48 个）

| 分组 | 关键字 |
|------|--------|
| 声明 | `var` `const` `class` `struct` `interface` `extend` `implement` `override` `parent` `this` `enum` `type` |
| 控制流 | `if` `else` `for` `while` `in` `match` `switch` `case` `default` `fallthrough` `return` `try` `catch` `finally` `throw` `defer` `import` |
| 可见性 | `public` `protected` `private` `static` `new` `as` `break` `continue` `constructor` |
| FFI/特殊 | `extern` `unsafe` `abstract` `export` `final` `ref` `is` |
| 字面量 | `true` `false` `null` |

### 2.4 保留类型名

词法上是 `IDENT`，语义上是根模块内建类型，不可重新声明：

```
bool byte int int64 int8 int16 int32 uint uint8 uint16 uint32
float float64 float32 decimal string object void result error
func map Self range Rounding Arena ArenaRef
```

### 2.5 标识符规则

- 仅允许 `[A-Za-z_][A-Za-z0-9_]*`，不允许非 ASCII 字符。
- `_` 作为独立标识符（丢弃值），可用于忽略。
- 命名风格：成员名（方法/字段/枚举成员等）**强烈建议 camelCase**，编译器不做强制性检查，不再报 `E3043`。

### 2.6 运算符（最长匹配）

| 运算符 | 说明 |
|--------|------|
| `..=` `..` | 区间（闭区间 / 半开区间） |
| `?.` `??` | 安全访问 / 空合并 |
| `<<` `>>` | 移位 |
| `<=` `>=` `==` `!=` | 关系/相等 |
| `&&` `||` | 逻辑与/或 |
| `+=` `-=` `*=` `/=` `%=` | 复合赋值 |
| `=>` | match 分支箭头 |
| `++` `--` | 自增/自减（仅语句级） |
| `+` `-` `*` `/` `%` `&` `\|` `^` `~` `!` `<` `>` | 算术/位/一元 |
| `=` `?` `:` `.` `,` `;` `(` `)` `[` `]` `{` `}` `@` | 分隔与标记 |

### 2.7 字面量

| 类型 | 语法 | 说明 |
|------|------|------|
| 整数 | `123` `0x1F` `0b1010` `0o17` | 支持 `_` 分隔，禁止前导零十进制 |
| 浮点 | `3.14` `1.5e2` `3.14f32` | `f32`/`F32` 后缀为 `float32`（32 位单精度），C 层对应 `float` |
| 十进制 | `19.99d` | `d`/`D` 后缀为 `decimal`（定点货币类型） |
| 布尔 | `true` `false` | |
| 空值 | `null` | |
| 字符串 | `"hello"` `r"raw"` `"""multi"""` | 支持转义 `\n \t \r \\ \" \0 \u{xxxx}` |
| 字符串插值 | `"val={x:.2}"` | 插值片段支持 `:.N` 小数位格式 |

### 2.8 字符串转义

| 转义 | 含义 |
|------|------|
| `\n` | 换行 |
| `\t` | 制表 |
| `\r` | 回车 |
| `\\` | 反斜杠 |
| `\"` | 双引号 |
| `\0` | NUL |
| `\u{1-6位十六进制}` | Unicode 码点（U+0000 ~ U+10FFFF，排除代理区） |

单引号 `'` 不是字符串定界符；一个字符就是长度为 1 的 `string`，请用双引号。

### 2.9 属性

`@` 后跟属性名，用于函数/方法修饰：`@pure` `@nothrow` `@nogc` `@deprecated`、`@os`、`@arch` 等。

---

## 3. 语法结构

### 3.1 模块与文件

- 一个 `.ax` 文件 = 一个模块。
- 模块路径即目录结构：`import a.b` 对应入口文件同目录的 `a/b.ax`。
- 循环导入是编译错误（`E3026`）。
- 模块级只能是声明或 `import`，不能出现语句（`E2011`）。
- 入口文件的顶层语句会被自动包装成隐式 `main` 函数（`E3024` 限制：顶层不能声明变量，只能 `const`）。
- 被 import 的文件不能有顶层语句（`E3024`）。
- `main` 签名必须是 `int main()` 或 `int main(string[] args)`（`E3025`）。
- 一个模块只能有一个 `main`（`E3025`）。
- `main` 只能出现在程序入口文件（`E3028`）。

### 3.2 声明

#### 3.2.1 变量声明

```ax
var x = 42;           // 类型推断
int y = 10;           // 显式类型：类型 名 = 值（C 风格）
int? z = null;        // 可空
```

- `var` 只能用于局部变量（`E3006`）。
- 变量声明必须有初值（`E3002`）。
- `var` 不能从 `null` 推断类型（`E4007`）。
- 未使用的局部变量报警告 `W1005`。

#### 3.2.2 常量声明

```ax
const MAX = 100;
const float PI = 3.14;
```

- `const` 初始化器必须是常量表达式（`E3003`）。
- 不能给 `const` 赋值（`E3005`）。
- 模块级 `const` 可用；语句级 `const` 与模块级同构。

#### 3.2.3 类型别名

```ax
type MyInt = int;
type Pair<K, V> = map<K, V>;
```

- 类型别名不能循环引用（`E3003`）。
- 泛型别名支持，不变型。

#### 3.2.4 枚举

```ax
enum Color { Red, Green, Blue }
```

- 枚举成员不能带初值或载荷（`E3031`）。
- 枚举成员名重复报 `E3044`。
- 枚举是 `uint32_t`，不能从整数转换，也不能转成整数（`E4019`）。
- 枚举成员名**强烈建议 camelCase**，编译器不做强制性检查。

#### 3.2.5 结构体

```ax
struct Point {
    int x;
    int y;
}
```

- 结构体成员一律 public，不能写可见性修饰词（`E3011`）。
- 结构体不能 `extend` 或 `implement`（`E3021`）。
- 结构体字段不能含 class 引用（包括容器元素）（`E3023`）。
- 结构体递归内联包含自身报 `E3022`。

#### 3.2.6 类

```ax
class Person extend Entity implement Drawable {
    string name;
    int age;

    constructor(string name, int age) {
        this.name = name;
        this.age = age;
    }

    public method greet() {
        return "Hello, " + this.name;
    }
}
```

- 类支持单继承 `extend`（`E3020`：一个类只能 extend 一个父类）。
- 类支持多 interface `implement`（`E3019`：implement 后必须是 interface）。
- 有字段但没有 constructor 的类报 `E3008`。
- constructor 必须给所有字段赋值（`E3009`）。
- constructor 名重复报 `E3010`。
- constructor 不能写可见性修饰词（默认 public，`E3011`）。
- constructor 不能是 `static` 或 `override`（`E3011`）。
- 子类 constructor 第一条语句必须是 `parent(...)`（`E3035`）。
- `parent(...)` 只能在子类 constructor 内使用（`E3034`）。
- `this` 只能在 class/struct 方法体内使用（`E3034`）。
- 静态方法体内没有 `this`（`E3038`）。
- 类成员缺少可见性修饰符报 `E3037`（必须显式写 `public`/`private`/`protected`）。
- 成员名**强烈建议 camelCase**，编译器不做强制性检查。
- `final` 类不能被 `extend`（`E3046`）。
- 抽象方法必须在子类中实现（`E3047`）。
- 非抽象类不能声明抽象方法（`E3048`）。
- 抽象方法不能是 `static`（`E3049`）。
- 方法覆写必须写 `override`（`E3017`）。
- `override` 签名必须与父类一致（`E3019`）。
- `override` 没有对应的父类或 interface 方法报 `E3016`。

#### 3.2.7 接口

```ax
interface Drawable {
    void draw();
    float area();
}
```

- 接口只能 extend interface（`E3019`）。
- 方法与 interface 签名不一致报 `E3019`。
- 类必须实现所有 interface 方法（`E3019`）。
- 接口成员不能写可见性修饰词（`E3011`）。
- 接口不能有 constructor（`E3032`）。
- 接口支持默认方法体（`MethodSig.default_body`）。

#### 3.2.8 外部函数（FFI）

```ax
extern "C" {
    int puts(string s);
    int printf(string fmt, ...);
}
```

- `extern` 函数不支持泛型（`E3033`）。
- 仅声明签名，无函数体。
- 调用时需包裹在 `unsafe { ... }` 块中。

### 3.3 表达式

| 表达式 | 语法 | 说明 |
|--------|------|------|
| 整数字面量 | `42` `0xFF` | `int`（64 位有符号） |
| 浮点字面量 | `3.14` `3.14f32` | `float` 或 `float32` |
| 十进制字面量 | `19.99d` | `decimal` |
| 布尔字面量 | `true` `false` | `bool` |
| 空字面量 | `null` | `null` |
| 字符串 | `"hello"` | `string` |
| 路径 | `a.b.c` | 模块/类型/符号访问 |
| this | `this` | 当前实例 |
| parent | `parent` | 父类引用 |
| 元组 | `(1, 2)` | 元组字面量 |
| 数组 | `[1, 2, 3]` | 数组字面量 |
| record | `{"k": 1}` | 记录字面量（键必须是字符串字面量） |
| 调用 | `f(a, b)` | 函数调用 |
| 方法调用 | `obj.method(a)` | 实例方法调用 |
| 字段访问 | `obj.field` | 字段访问 |
| 安全访问 | `obj?.field` | 可空安全访问 |
| 安全调用 | `obj?.method(a)` | 可空安全调用 |
| 下标 | `arr[i]` | 数组/ map 下标 |
| 一元 | `-x` `!x` `~x` `&x` `++x` `--x` | 一元运算 |
| 二元 | `+` `-` `*` `/` `%` `==` `!=` `<` `>` `<=` `>=` `&&` `||` `&` `\|` `^` `<<` `>>` | 二元运算 |
| 三目 | `c ? a : b` | 三目运算 |
| 空合并 | `a ?? b` | 左侧非 null 则取左，否则取右 |
| 区间 | `0..10` `0..=10` | 区间表达式 |
| try | `try expr` | result<T> 隐式展开 |
| new | `new ClassName(args)` | 类实例化 |
| if 表达式 | `if c { a } else { b }` | if 作表达式，必须有 else |
| match 表达式 | `match v { ... => ... }` | match 作表达式 |
| 匿名函数 | `(int x) { return x; }` | 需要目标类型（`E4043`） |
| is 类型查询 | `is(T == Type)` | 编译期类型查询，返回 bool |

#### 3.3.1 运算符优先级与结合性

（按编译器 parser 实现，从高到低：成员访问/调用 > 一元 > 乘除 > 加减 > 移位 > 关系 > 相等 > 按位与 > 按位异或 > 按位或 > 逻辑与 > 逻辑或 > 三目 > 空合并 > 赋值）

#### 3.3.2 字符串拼接

`+` 两边必须都是 string（拼接）；数值请用 `{x}{y}` 插值（`E4010`）。

#### 3.3.3 字符串下标

string 不支持 `s[i]`（`E4006`）；请用 `s.at(i)`（返回 `string?`）或切片方法。

### 3.4 语句

| 语句 | 语法 | 说明 |
|------|------|------|
| 块 | `{ ... }` | 语句块 |
| 变量声明 | `var x = 1;` | 局部变量 |
| 常量 | `const X = 1;` | 局部常量 |
| 表达式/赋值 | `x = 1; x += 1;` | 赋值、后缀 ++/-- |
| if | `if c { } else { }` | 条件分支 |
| match | `match v { ... }` | 模式匹配 |
| for | `for i in 0..10 { }` | 循环 |
| while | `while c { }` | 循环 |
| return | `return expr;` | 返回 |
| throw | `throw error.Variant(msg);` | 抛出错误 |
| try/catch/finally | `try { } catch (e) { } finally { }` | 错误处理 |
| defer | `defer f();` | 延迟调用 |
| break | `break;` | 跳出循环 |
| continue | `continue;` | 继续循环 |
| switch | `switch (e) { case ... }` | 分支选择（向 Go 靠齐，见 3.4.5） |
| unsafe | `unsafe { }` | 不安全块 |

#### 3.4.1 for 循环变体

```ax
for i in 0..10 { }           // 范围循环（半开）
for i in 0..=10 { }          // 范围循环（闭区间）
for i in arr { }             // 迭代数组
for (k, v) in m { }          // 遍历 map（键值对）
for i in (0..100).Step(2) { } // 带步长（Range.Step）
while c { }                  // 条件循环
```

- `for` 遍历只支持数组、map、string、range（`E4025`）。
- 不能在遍历期间修改长度（`E4026`）。
- `(k, v) in` 只能遍历 map（`E4025`）。
- `break`/`continue` 不在循环内报 `E6009`。
- `break`/`continue` 会跳出 try 块（finally 不会执行，`E6013`）。

#### 3.4.2 try/catch/finally

```ax
try {
    let f = try file.open("a.txt");
} catch (error e) {
    println("error: " + e.msg);
} catch (error.NotFound e) {
    println("not found");
} finally {
    cleanup();
}
```

- `try` 之后必须跟 `catch` 或 `finally`（`E2001`）。
- `catch` 的捕获类型必须是 `error` 或 `error.变体`（`E2001`）。
- `catch (error e)` 已捕获全部错误，后面的 catch 永远不会执行（`E6012`）。
- `catch (error.X ...)` 重复报 `E6012`。
- `throw` 只能出现在返回 `result<...>` 的函数内或 try 块内（main 除外，`E6010`）。
- `try` 只能作用于 `result<T>`（`E6011`）。
- `nothrow` 函数内不能 `throw`（`E6014`）。
- `try` 绑定最紧：`try` 之后不能再接 `.` 或 `[`（`E2009`）。

#### 3.4.3 defer

```ax
defer file.close();
defer println("cleanup");
```

- `defer` 后必须是一次调用（`E2013`）。

#### 3.4.4 match 模式

| 模式 | 语法 | 说明 |
|------|------|------|
| 字面量 | `1`, `"a"`, `true` | 字面量匹配 |
| 区间 | `1..5`, `1..=5` | 区间匹配（只能用于 int，两端必须是常量，下界 ≤ 上界） |
| enum | `Color.Red` | 枚举变体匹配 |
| result | `ok(x)`, `fail(e)` | result 解构模式 |
| 类型 | `Type x` | 类型模式（class 或 interface，`object` 必须有 `_` 分支） |
| 通配符 | `_` | 匹配任意 |
| 绑定 | `x` | 绑定到变量 |
| 多值 | `1,3,6 =>` | 任意一个匹配即可 |

- match 非穷尽报 `E6001`。
- match 分支类型不一致报 `E4014`。
- match 作表达式时分支不能是 void（`E6007`）。
- `_` 之后还有分支不可达（`E6006`）。
- 类型模式不能是 interface（用具体类，`E4001`）。
- 对 object 的 match 必须有 `_` 分支（`E6002`）。

#### 3.4.5 switch 语句（向 Go 靠齐）

**值 switch**：

```ax
switch (expr) {
    case v1, v2, ...: { ... }   // 多值 case，任意一个匹配即可
    case expr: { ... }           // 任意表达式求值（函数调用、算术表达式等）
    default: { ... }             // 兜底，至多一个
}

// 无表达式 switch（等价 if 链）
switch {
    case cond1: { ... }
    case cond2: { ... }
    default: { ... }
}
```

- 支持的判断表达式类型：`int`、`string`、`bool`、`enum`、`result`
- `int`/`enum` 且全部 case 为 int 字面量时，生成 C 原生 `switch` 语句（性能最优）
- 否则退化为 if 链（`sv == expr` / `ax_str_eq(sv, expr)`，多值用 `||`）
- 每个 case 隐式 `break`；`fallthrough` 仅在 int/enum C switch 路径允许，其他路径报 `E4040`
- 多个 `default` 报 `E4029`
- `case` 值类型与判断表达式不匹配报 `E4030`
- 重复的 int 字面量 `case` 值报 `E4031`
- 无表达式 switch 的 `case` 条件必须是 `bool`，否则 `E4030`

**类型 switch**（按运行时类型 tag 匹配）：

```ax
switch (obj) {           // obj 必须是 object 或 类/具名类型
    case TypeName binding: { ... }  // tag 匹配后 ax_unbox_* 解包为 binding 变量
    case TypeName: { ... }           // 只做类型判断，不绑定变量
    default: { ... }
}
```

- 判断表达式必须是 `object` 或 类/具名类型，否则 `E4028`
- 无表达式的类型 switch 报 `E4028`
- 类型 switch 中出现 `case 值:`（Pat::Lit）报 `E4030`
- 类型 switch 中 `fallthrough` 报 `E4040`
- 多值类型 `case` 不能带绑定变量，报 `E4030`
- 仅支持 `RESERVED_TYPES` 中的内建类型（`string`/`int`/`bool` 等）作为 `Pat::Type`；用户自定义类名按值表达式处理

关键字：`switch`、`case`、`default`、`fallthrough`

#### 3.5 模式

| 模式 | 语法 |
|------|------|
| Lit | `1`, `"a"` |
| Range | `1..5` |
| Enum | `Color.Red` |
| Result | `ok(x)`, `fail(e)` |
| Type | `Type x` |
| Wild | `_` |
| Bind | `x` |
| Multi | `1,3,6` |

---

## 4. 类型系统

### 4.1 基础类型

| 类型 | C 映射 | 说明 |
|------|--------|------|
| `bool` | `bool` | 布尔 |
| `byte` | `uint8_t` | 字节（uint8 别名） |
| `int`/`int64` | `int64_t` | 64 位有符号整数 |
| `int8`/`int16`/`int32` | `int8_t`/`int16_t`/`int32_t` | 定宽整数 |
| `uint` | `uint64_t` | 64 位无符号整数 |
| `uint8`/`uint16`/`uint32` | `uint8_t`/`uint16_t`/`uint32_t` | 定宽无符号 |
| `float`/`float64` | `double` | 64 位双精度浮点 |
| `float32` | `float` | 32 位单精度浮点，字面量后缀 `f32`/`F32` |
| `decimal` | `ax_dec` | 定点十进制，适合金额，字面量后缀 `d`/`D` |
| `string` | `ax_str` | UTF-8 字符串 |
| `object` | `ax_box*` | 装箱引用类型 |
| `void` | `void` | 空类型 |
| `null` | — | 空值字面量 |
| `never` | — | 不返回类型 |
| `range` | `int64_t` | 区间类型（仅 for 头） |
| `error` | `ax_err` | 错误类型 |
| `func` | `axfn_*` | 函数类型 |
| `map` | `ax_map` | 映射类型 |
| `Self` | — | 当前类/struct/interface |
| `ArenaRef<T>` | T* | arena 托管引用 |

### 4.2 复合类型

| 类型 | 语法 | C 映射 |
|------|------|--------|
| 数组 | `T[]` | `ax_arr` |
| 定长数组 | `T[N]` | `axfix_T_N` |
| Map | `map<K, V>` | `ax_map` |
| 元组 | `(T1, T2)` | `axtup_*` |
| Record | `{k: T, ...}` | `axrec_*` |
| Nullable | `T?` | `axopt_T` |
| Result | `result<T>` | `axres_T` |
| Func | `func(P...) R` | `axfn_*` |

### 4.3 类型可赋值性（assignable）

全项目类型判定唯一入口：`types.rs` `assignable(src, dst, ctx, allow_unbox)`。

**隐式转换规则**：

1. 同类型直接赋值：`Yes`
2. `Unknown` / `Never` 与任何类型相容：`Yes`
3. `null` 可赋给可空类型：`Yes`
4. 非可空 → 可空：自动 nullwrap（`Conv("nullwrap")`）
5. 可空 → 非可空：不可隐式，需先判空（`No`）
6. 标量装箱到 `object`：`box`（`boxable` 标量：`bool` `int` `float` `float32` `decimal` `string`）
7. `object` 解箱：仅允许 P1..P8 位置自动解箱（`allow_unbox`）
8. 子类 → 父类：向上转型 `Yes`
9. 父类 → 子类：向下转型 `Conv("downcast")`
10. 类实现接口：赋给接口类型 `Yes`
11. `float32` → `float`：隐式扩大 `Conv("f32_to_f64")`（C 层 float 自动提升为 double）
12. 定长数组不能直接赋给可变长数组（`C-16`，需逐元素复制）
13. 数值类型间无隐式转换（需显式转换，`E4003`）

**禁止的转换**：
- `bool` 与其他类型之间没有转换（`E5001`）
- `float` 不能直接转 `decimal`（`C-6`，请用 `decimal.Parse(s)`）
- `string` 不能用 `string(x)` 转换，请用 `x.toString()`
- 枚举不能从整数转换，也不能转成整数（`E4019`）
- `record` 不能装箱到 `object`（`E4029`）

### 4.4 可空类型 `T?`

- `T?` 表示可为 null 的 T。
- `null` 只能赋给可空类型或引用类型（`E4001`）。
- 可空值不能直接用 `.` 成员访问，需先判空或用 `?.`（`E4022`）。
- `??` 左侧必须是可空值（`E4022`）。
- 可空值不能直接加格式说明符 `:.N`，先判空取出再格式化（`E4030`）。
- `void?` 不合法（`E2001`）。

### 4.5 Result 类型 `result<T>`

- `result<T>` 表示可能成功（`ok`）或失败（`fail`）的计算。
- `throw error.Variant(msg)` 抛出错误，函数需返回 `result<...>`。
- `try expr` 对 `result<T>` 做隐式展开，失败时提前返回。
- `match` 可用 `ok(x)` / `fail(e)` 模式解构。
- `fail` 需要 1 个实参（`E4031`）。
- `ok` 需要 1 个实参（`E4031`）。

### 4.6 装箱与解箱

- `BOXABLE_SCALARS = ["bool", "int", "float", "float32", "decimal", "string"]`
- `boxable` 还包括 `Array`、`Map`、`Named`（class 实例）。
- `unboxable` 包括上述标量、`Array`、`Map`、`Nullable`、`Named`。

---

## 5. 语义规则

### 5.1 作用域与可见性

- 可见性修饰词：`public`、`protected`、`private`。
- 类成员缺少可见性修饰符报 `E3037`（必须显式写）。
- `protected` 只能用于类成员，模块级声明不合法（`E3014`）。
- `private` 只在本类内可见（`E3013`）。
- `protected` 只能在本类或子类内访问（`E3013`）。
- 跨模块使用非 public 符号报 `E3013`。
- 文件内符号默认私有，导出需写 `public`（`E3030`）。
- 接口成员不能写可见性修饰词（`E3011`）。
- 结构体成员不能写可见性修饰词（一律 public，`E3011`）。

### 5.2 命名规则（camelCase，建议不强制）

- 成员名（方法、字段、枚举成员等）**强烈建议 camelCase**，编译器不做强制性检查，不再报 `E3043`。
- 不在检查范围：`constructor` 关键字、类型名、模块级函数、局部变量、参数、常量。
- 模块函数调用（如 `utils.add(2, 3)`）不受成员名命名规则影响，仅需匹配 `module_sig` 注册的标准库成员名（如 `io.readLine`、`math.abs`）。

### 5.3 常量折叠

- 编译期可求值的表达式会进行常量折叠。
- 除零、溢出等在常量表达式中报 `E4009`。

### 5.4 隐式转换位置 P1..P8

自动解箱只允许在 P1..P8 位置。

### 5.5 所有权与借用

- `&` 只能作用于可写位置（变量、字段、元素）（`E7007`）。
- `&` 作为前缀运算符。
- 借用不能作为返回值、赋给变量或存入容器（`E7003`）。
- 不能对 `const` 取可变借用，写 `&const`（`E7009`）。
- 返回局部变量的引用会悬垂（`E7010`）。
- `arena.Alloc` 只收引用类型（class 实例）（`E7005`）。
- `ArenaRef` 不能逃出 arena 作用域（`E7006`）。
- Copy 类型写借用等价于传值，`&` 冗余（`W1003`）。

---

## 6. 标准库

axlang 标准库分为两层，**严格互斥，不得并存**：

1. **编译器内建模块**（7 个）：无法用纯 ax 语法独立实现，长期保留在编译器 Rust 端内建，由 codegen 硬编码 axrt 调用。不涉及 `axc/std/` 下的任何 `.ax` 源码。
2. **标准库模块**（16 个）：用 ax 语法编写，位于 `axc/std/`，预转译为标准 C，通过 gcc 参与编译。ax 层仅包装 axrt C 运行时原语，通过 `extern "C"` 调用，不引入 ax 编译器内建能力。

### 6.1 编译器内建模块（INTERNAL_BUILTINS，7 个）

以下 7 个模块为编译器 Rust 端内建，无 `axc/std/` 下的 `.ax` 源码，由 codegen 硬编码 axrt 调用。用裸模块名直接使用，无需 `import`：

| 模块 | 函数 | 签名 | 说明 |
|------|------|------|------|
| db | `query(sql)` | `(string) -> result<Row>` | 数据库查询（planned，需运行时类型反射 / 数据库驱动设计） |
| fmt | `print(s)` | `(string) -> void` | 将字符串输出到标准输出，不换行 |
| fmt | `println(s)` | `(string) -> void` | 将字符串输出到标准输出并换行 |
| fmt | `printf(format, args...)` | `(string, any...) -> void` | 格式化输出，编译期核对占位符（%d/%f/%s/%.Nf/%t/%v/%%） |
| fmt | `sprintf(format, args...)` | `(string, any...) -> string` | 格式化并返回字符串 |
| fmt | `sprint(args...)` | `(any...) -> string` | 将任意参数格式化为字符串返回 |
| fmt | `fprintf(w, format, args...)` | `(File, string, any...) -> int` | 将格式化内容写入文件句柄 |
| fmt | `errorf(format, args...)` | `(string, any...) -> error` | 格式化错误信息并返回 error |
| fmt | `scanf(format, args...)` | `(string, any...) -> int` | 从 stdin 按格式读取 |
| log | `print(args...)` | 多实参 `-> void` | 打印日志到标准输出 |
| log | `printf(fmt, args...)` | `(string, ...) -> void` | 格式化日志 |
| log | `fatal(args...)` | 多实参 `-> void` | 致命日志，输出后终止进程 |
| maps | `*` | — | Map 集合工具函数（planned，运行时类型反射相关） |
| reflect | `typeOf(o)` | `(object) -> string` | 编译期类型查询，返回静态类型显示名 |
| reflect | `numField(o)` | `(object) -> int` | 结构体字段数量 |
| sort | `ints(arr)` | `(int[]) -> void` | 对 int 数组升序排序 |
| sort | `strings(arr)` | `(string[]) -> void` | 对 string 数组升序排序 |
| sort | `float64s(arr)` | `(float[]) -> void` | 对 float 数组升序排序 |
| sort | `intsAreSorted(arr)` | `(int[]) -> bool` | 判断 int 数组是否已排序 |
| sort | `stringsAreSorted(arr)` | `(string[]) -> bool` | 判断 string 数组是否已排序 |
| sort | `searchInts(arr, x)` | `(int[], int) -> int` | 二分查找 int |
| sort | `searchStrings(arr, x)` | `(string[], string) -> int` | 二分查找 string |
| sort | `slice(arr, cmp)` | `(object[], func(int,int)int) -> void` | 通用切片排序 |
| sort | `sliceStable(arr, cmp)` | `(object[], func(int,int)int) -> void` | 稳定通用切片排序 |
| unique | `Handle[T]` | 泛型类型 | 唯一句柄类型 |

### 6.2 标准库模块（STDLIB_MODULES，16 个）

所有模块位于 `axc/std/`，用 ax 语法编写，通过 `extern "C"` + `unsafe` 调用 axrt C 原语：

| 模块 | 文件 | 对应 C 运行时 |
|------|------|--------------|
| std.string | `string.ax` | `axc/rt/string/axrt_str.c` |
| std.file | `file.ax` | `axc/rt/file/axrt_file.c` |
| std.sync | `sync.ax` | `axc/rt/sync/axrt_sync.c` |
| std.atomic | `atomic.ax` | `axc/rt/atomic/axrt_atomic.c` |
| std.io | `io.ax` | `axc/rt/io/axrt_io.c` |
| std.json | `json.ax` | `axc/rt/json/axrt_json.c` |
| std.os | `os.ax` | `axc/rt/os/axrt_os.c` |
| std.strconv | `strconv.ax` | `axc/rt/string/axrt_strconv.c` |
| std.math | `math.ax` | `axc/rt/math/axrt_math.c` |
| std.errors | `errors.ax` | `axc/rt/errors/axrt_errors.c` |
| std.time | `time.ax` | `axc/rt/time/axrt_time.c` |
| std.rand | `rand.ax` | `axc/rt/rand/axrt_rand.c` |
| std.bytes | `bytes.ax` | `axc/rt/bytes/axrt_bytes.c` |
| std.bufio | `bufio.ax` | `axc/rt/bytes/axrt_bufio.c` |
| std.csv | `csv.ax` | `axc/rt/csv/axrt_csv.c` |
| std.filepath | `filepath.ax` | `axc/rt/file/axrt_filepath.c` |

**导出函数命名**：C 风格，全部小写，下划线分隔（如 `read_line`、`write_line`）。ax 层通过 camelCase 包装调用。

**示例（std/io.ax）**：

```ax
extern "C" {
    int ax_read_line(ref string out);
    void ax_write_stdout(string s);
}

public int readLine(ref string out) {
    unsafe {
        return ax_read_line(out)
    }
}

public void write(string s) {
    unsafe {
        ax_write_stdout(s)
    }
}
```

ax 调用端：`io.readLine(ref line)`、`io.writeLine("hello")`。


## 7. fmt 模块与格式化输出

### 7.1 fmt 模块（内建模块）

`fmt` 是编译器内建模块（`INTERNAL_BUILTINS`，无 `axc/std/fmt.ax`），**无需 `import std.fmt`**，直接用裸模块名 `fmt` 调用即可。与标准库模块严格互斥，不得并存。

| 函数 | 签名 | 说明 |
|------|------|------|
| `fmt.print` | `(string) -> void` | 将字符串输出到标准输出，不换行 |
| `fmt.println` | `(string) -> void` | 将字符串输出到标准输出并换行 |
| `fmt.printf` | `(string, any...) -> void` | 格式化输出，编译期核对占位符（%d/%f/%s/%.Nf/%t/%v/%%） |
| `fmt.sprintf` | `(string, any...) -> string` | 格式化并返回字符串 |
| `fmt.sprint` | `(any...) -> string` | 将任意参数格式化为字符串返回 |
| `fmt.fprintf` | `(File, string, any...) -> int` | 将格式化内容写入文件句柄 |
| `fmt.errorf` | `(string, any...) -> error` | 格式化错误信息并返回 error |
| `fmt.scanf` | `(string, any...) -> int` | 从 stdin 按格式读取 |

### 7.2 格式化占位符

`fmt.printf` / `fmt.sprintf` / `fmt.fprintf` / `fmt.errorf` / `fmt.scanf` 的首个参数是格式字符串，编译器在编译期对占位符与实参类型进行核对，不匹配时报 `E4032`（格式串/占位符错误）或 `E4031`（参数个数错误）。

| 占位符 | 含义 | 示例 |
|--------|------|------|
| `%d` | 有符号整数（int/int64/定宽整数） | `fmt.printf("count = %d\n", 42)` |
| `%f` | 浮点（float/float64，默认 6 位小数） | `fmt.printf("pi = %f\n", 3.14159)` |
| `%.Nf` | 浮点保留 N 位小数 | `fmt.printf("pi = %.2f\n", 3.14159)` |
| `%s` | 字符串 | `fmt.printf("name = %s\n", "axlang")` |
| `%t` | 布尔 | `fmt.printf("flag = %t\n", true)` |
| `%v` | 通用值（自动按类型格式化） | `fmt.printf("val = %v\n", x)` |
| `%%` | 字面量 `%` | `fmt.printf("100%%\n")` |

### 7.3 格式化输出示例

```ax
var pi = 3.14159
var count = 42
var name = "axlang"

fmt.print("hello\n")                   // 不换行输出
fmt.println("world")                   // 换行输出
fmt.printf("pi = %.2f\n", pi)          // 输出：pi = 3.14
fmt.printf("count = %d\n", count)      // 输出：count = 42
fmt.printf("name = %s\n", name)        // 输出：name = axlang
var s = fmt.sprintf("price = %.2f\n", 19.99d)
fmt.print(s)
```

字符串插值（§2.7）也可用于简单格式化，如 `fmt.print("pi = {pi:.2}\n")`，与 `%.2f` 占位符等价；两者可根据需要选择。

---

## 8. 诊断码

### 8.1 诊断码分级

| 前缀 | 含义 | 示例 |
|------|------|------|
| E1xxx | 词法错误 | E1001 禁止前导零 |
| E2xxx | 语法错误 | E2001 期望某 token |
| E3xxx | 语义错误（命名/可见性/模块） | E3003 未定义标识符 |
| E4xxx | 类型错误 | E4001 类型不匹配，E4003 无隐式转换 |
| E5xxx | 转换/object 错误 | E5001 bool 无转换，E5002 object 不能直接用 |
| E6xxx | 控制流/匹配错误 | E6001 match 非穷尽，E6010 throw 位置 |
| E7xxx | 所有权/借用错误 | E7001 移动后使用，E7003 借用逃逸 |
| W1xxx | 警告 | W1005 未使用局部变量 |

### 8.2 主要诊断码（部分列举，完整列表见 `docs/ERRORS.md`）

| 码 | 中文含义 | 英文含义 |
|----|----------|----------|
| E1001 | 禁止前导零十进制 | leading-zero decimal not allowed |
| E1002 | 单引号不是字符串定界符 | single quote not string delimiter |
| E1003 | 字符串/注释未闭合 | string/comment not closed |
| E1004 | 非法转义序列 | invalid escape sequence |
| E1005 | 非法数值字面量 | invalid numeric literal |
| E1006 | 非法字符 | invalid character |
| E1007 | 非法 Unicode 码点 | invalid Unicode code point |
| E1008 | 插值未闭合或为空 | unclosed/empty interpolation |
| E2001 | 语法期望项缺失 | expected token missing |
| E2002 | 语句未以换行或 ; 结束 | statement must end with newline or ; |
| E2003 | 行首不能是运算符 | operator cannot start line |
| E2004 | 必须用 {} 括起来 | must be wrapped in {} |
| E2005 | if 作表达式必须有 else | if as expression needs else |
| E2006 | 函数存在没有 return 的路径 | function has path without return |
| E2009 | try 绑定最紧 | . or [ cannot follow try |
| E2010 | 只能单独成语句 | can only stand alone |
| E2011 | 模块级只能是声明或 import | only declarations/imports at module level |
| E2012 | new 不能用于内建类型 | new cannot be used on built-in type |
| E2013 | defer 后必须是调用 | defer must be followed by call |
| E2014 | 无名为 xxx 的循环标签 | no loop label named xxx |
| E3001 | 变量遮蔽 | variable shadows outer |
| E3002 | 变量声明必须有初值 | variable declaration needs initializer |
| E3003 | 未定义标识符/类型/成员 | undefined identifier/type/member |
| E3004 | 重复声明 | duplicate declaration |
| E3005 | 不能给 const 赋值 | cannot assign to const |
| E3006 | var 只能用于局部变量 | var only for locals |
| E3007 | 字段不能有初值 | fields cannot have initializers |
| E3008 | 类有字段但无 constructor | class has fields but no constructor |
| E3009 | constructor 未给字段赋值 | constructor never assigns field |
| E3010 | 构造函数名重复 | duplicate constructor name |
| E3011 | 修饰符错误 | modifier order/visibility error |
| E3013 | 可见性错误 | not public / protected / private |
| E3016 | override 无对应父方法 | override has no matching parent |
| E3017 | 覆写未写 override | override not marked |
| E3018 | 重名冲突 | name conflict |
| E3019 | implement/override 签名错误 | interface signature mismatch |
| E3020 | extend 后必须是 class | extend must follow class |
| E3021 | struct 不能 extend/implement | struct cannot extend/implement |
| E3022 | struct 递归内联 | struct recursively contains itself |
| E3023 | struct 字段含 class 引用 | struct field cannot contain class ref |
| E3024 | 模块级不能声明变量 | variables not at module level |
| E3025 | main 签名/多个 main/缺少 main | main signature issues |
| E3026 | 循环导入 | circular import |
| E3027 | 找不到模块 | module not found |
| E3028 | main 只能在入口文件 | main only in entry file |
| E3029 | 未知属性 | unknown attribute |
| E3030 | 模块无公开成员 | module has no public member |
| E3031 | enum 成员不能带初值 | enum members no initializers |
| E3032 | interface 不能有 constructor | interface no constructor |
| E3034 | parent/this 使用位置错误 | parent/this usage error |
| E3035 | 子类 constructor 首句必须 parent | first stmt must be parent(...) |
| E3036 | static/override 不能用于字段 | static/override not for fields |
| E3037 | 缺少可见性修饰符 | missing visibility modifier |
| E3038 | 静态方法无 this | no this in static method |
| E3039 | 模块名与内建重名 | module name conflicts with built-in |
| E3040 | 导入名称冲突 | import name conflict |
| E3041 | 主函数与顶层语句冲突 | main vs top-level statements |
| E3042 | 歧义导出类型 | ambiguous exported type |
| E3043 | ~~成员名非 camelCase~~ **已废止**：编译器不再对用户定义的成员名做 camelCase 强制性检查 | ~~member names must be camelCase~~ |
| E3044 | enum 成员名重复 | duplicate enum member |
| E3045 | 具名导入歧义 | ambiguous named import |
| E3046 | 不能 extend final 类 | cannot extend final class |
| E3047 | 抽象方法未实现 | abstract method not implemented |
| E3048 | 非抽象类不能声明抽象方法 | non-abstract class no abstract method |
| E3049 | 抽象方法不能是 static | abstract method cannot be static |
| E4001 | 类型不匹配 | type mismatch |
| E4002 | 操作数/条件必须是 bool | operand/condition must be bool |
| E4003 | 无隐式转换 | no implicit conversion |
| E4004 | 定长数组长度必须是常量 | array length must be constant |
| E4005 | 数组下标越界 | array index out of bounds |
| E4006 | string 不支持 s[i] | string no index access |
| E4007 | 无法推断类型 | cannot infer type |
| E4008 | record 键错误 | record key error |
| E4009 | 运算符类型错误 | operator type error |
| E4010 | + 两边必须都是 string | + both sides must be string |
| E4011 | float 不能用比较运算符 | float cannot use comparison |
| E4012 | 常量移位量越界 | constant shift out of range |
| E4013 | if/三目分支类型不一致 | branch type mismatch |
| E4014 | match 分支类型不一致 | match arm type mismatch |
| E4015 | ?? 右侧类型不能赋给左侧 | ?? right type not assignable |
| E4016 | 比较不能连写 | comparisons cannot chain |
| E4017 | 不实现 Comparable | does not implement Comparable |
| E4018 | map 键必须 Hashable + Comparable | map key constraints |
| E4019 | enum 不能和数值转换 | enum no numeric conversion |
| E4020 | 元组不支持某操作 | tuples do not support op |
| E4022 | ?. / ?? 左侧必须可空 | left side must be nullable |
| E4024 | 函数值不支持某操作 | function values do not support op |
| E4025 | 只能遍历数组/map/string/range | not iterable |
| E4026 | 遍历期间不能修改长度 | cannot modify length during iteration |
| E4027 | 元组元素个数不符 | tuple element count mismatch |
| E4028 | object 不能下标/成员访问 | object cannot be indexed |
| E4029 | record 不能装箱到 object | record cannot box to object |
| E4030 | 可空值不能加格式说明符 | nullable cannot take format spec |
| E4031 | 参数个数错误 | wrong argument count |
| E4032 | 格式串/占位符错误 | format string/placeholder error |
| E4033 | record 键必须是字符串字面量 | record keys must be string literals |
| E4034 | record 不能作为参数/返回值 | record cannot be arg/return |
| E4035 | 类型参数个数错误 | wrong type argument count |
| E4036 | 类型参数不满足约束 | type param constraint unsatisfied |
| E4037 | 无法推断类型实参 | cannot infer type arguments |
| E4038 | 泛型实例化递归超过 64 层 | generic recursion too deep |
| E4039 | Self 只能用在类体内 | Self only inside class/struct/interface |
| E4041 | 定长数组过大 | fixed array too large |
| E4042 | 无 constructor 不能 new | no constructor cannot new |
| E4043 | 匿名函数需要目标类型 | anonymous function needs target type |
| E4044 | 不支持的格式占位符 | unsupported format placeholder |
| E5001 | bool 与其他类型无转换 | no conversion between bool and others |
| E5002 | object 不能直接作为条件/运算 | object cannot be used directly |
| E5003 | interface 只能从实现类构造 | interface only from implementing class |
| E5004 | 类型转换/调用错误 | type conversion/call error |
| E5005 | 单实参限制 | takes exactly one argument |
| E5006 | 插值不支持该类型 | interpolation does not support type |
| E6001 | match 非穷尽 | match not exhaustive |
| E6002 | object match 必须有 _ 分支 | match on object needs _ arm |
| E6004 | ok/fail 模式只能用于 result | ok/fail only for result |
| E6005 | 类型模式缺少绑定名 | type pattern missing binding |
| E6006 | 不可达分支 | unreachable arm |
| E6007 | match 作表达式分支不能是 void | match arms cannot be void |
| E6008 | 循环标签重复 | duplicate loop label |
| E6009 | break/continue 不在循环内 | break/continue not in loop |
| E6010 | throw/try 位置限制 | throw/try location restriction |
| E6011 | try 只能作用于 result<T> | try only on result<T> |
| E6012 | catch 重复/全捕获 | catch duplicated/all caught |
| E6013 | break/continue 跳出 try | break/continue leaves try |
| E6014 | nothrow 函数不能 throw | nothrow cannot throw |
| E6015 | pure 函数不能调用非纯函数 | pure cannot call non-pure |
| E6016 | 编译期断言失败 | compile-time assert failed |
| E6017 | @nogc 函数内不得 new | @nogc no heap allocation |
| E7001 | 移动后不可再用 | moved value cannot be used |
| E7002 | 多个借用（至少一个可变） | multiple borrows |
| E7003 | 借用不能逃逸 | borrow cannot escape |
| E7004 | 被借用时不能移动/写入 | borrowed cannot be moved/written |
| E7005 | arena.Alloc 只收引用类型 | arena.Alloc only ref types |
| E7006 | ArenaRef 不能逃出 arena | ArenaRef cannot escape arena |
| E7007 | & 只能作用于可写位置 | & only on writable places |
| E7009 | 不能对 const 取可变借用 | mutable borrow of const |
| E7010 | 返回局部变量引用会悬垂 | dangling reference to local |
| W1003 | Copy 类型借用冗余 | borrow of copy type redundant |
| W1005 | 局部变量未使用 | local variable unused |

### 8.3 诊断输出格式

- **text 模式**：1 行头 + 1 行源码 + 1 行脱字符指向错误位置。
- **json 模式**：JSONL，每行一个 JSON 对象，字段固定：`code`、`msg`、`path`、`line`、`col`、`eline`、`ecol`、`rule`、`severity`、`notes`、`suggestion`。
- 排序：`(path, line, col, code)`，去重：同一位置同一码只保留第一条。

---

## 9. 代码生成约定

### 9.1 C 类型映射

| ax 类型 | C 类型 |
|---------|--------|
| `bool` | `bool` |
| `int`/`int64` | `int64_t` |
| `uint`/`uint64` | `uint64_t` |
| `int8`/`uint8`/`int16`/`uint16`/`int32`/`uint32` | `int8_t`/`uint8_t`/`int16_t`/`uint16_t`/`int32_t`/`uint32_t` |
| `float` | `double` |
| `float32` | `float` |
| `decimal` | `ax_dec` |
| `string` | `ax_str` |
| `object` | `ax_box*` |
| `void` | `void` |
| `error` | `ax_err` |
| `range` | `int64_t` |
| `enum` | `uint32_t` |
| `T[]` | `ax_arr` |
| `T[N]` | `axfix_T_N` |
| `map<K, V>` | `ax_map` |
| `T?` | `axopt_T` |
| `result<T>` | `axres_T` |
| `func(P...) R` | `axfn_*` (`typedef R (*name)(P...)`) |
| 类实例 | `ClassName*` |
| interface | `Iface_Name`（胖指针 `{obj, vt}`） |
| 结构体 | `struct Name` |

### 9.2 类布局

- 类引用在 C 层是 `T*` 指针。
- 类结构首两字段固定为 `vt`（虚表指针）与 `__tag`（运行时类型标签）。
- 所有实例方法经 vtable 调用；vtable 条目首参 `void*`，避免函数指针类型不兼容。
- 继承链字段按「父类在前」拉平，保证 C 布局前缀兼容。

### 9.3 运行时类型标签

- 基础类型标签 1–14：`bool=1`、`int64=2`、`int8=3`、`int16=4`、`int32=9`、`float/float64=6`、`float32=15`、`decimal=7`、`string=14`、`uint64=10`、`uint8=11`、`uint16=12`、`uint32=13`。
- 复合类型：`1000 + fnv1a32(canonical) % 100000`。

### 9.4 生成的 C 源码特征

- 纯 ASCII，非 ASCII 字符串字面量经 `cstr()` 转义为 `\ooo` 八进制（避免 `\x` 贪婪匹配问题）。
- 浮点最短往返表示，无小数点/指数时补 `.0`。
- `#include "axrt.h"` 和 `#include "axc_stdlib.h"`。
- 临时变量命名 `v_`，参数命名 `p_`，常量命名 `k_`。
- 共享库模式（`--kind=shared`）：跳过 main 包装器，导出函数加 `ax_export_` 前缀。产物为 .dll/.so/.dylib，可被外部程序动态加载调用。
- 静态库模式（`--kind=static`）：跳过 main 包装器，导出函数加 `ax_export_` 前缀。产物为 .lib/.a，通过 ar 打包，可被外部 C 程序静态链接调用。源码中不应包含 main 函数，避免与外部程序的入口点冲突。

### 9.5 泛型类型擦除（GTS）

- 引用类型在 C 层均为指针，可擦除为 `void*`，多种引用类型可共享同一套泛型代码。
- 值类型按大小分组共享（`i8`/`i16`/`i32`/`i64`）。
- 共享类名规则：`{模板名}_{类别键}`，如 `Box_ref`、`Box_i64`。

---

## 10. 内存管理

- ax 使用 arena 分配器进行内存管理。
- `arena.Alloc` 只接收引用类型（class 实例）。
- `ArenaRef<T>` 表示 arena 托管的引用，不能逃出 arena 作用域。
- 类实例在堆上分配（`ax_calloc`），通过 `new` 构造。
- 结构体按值传递。
- 字符串 `ax_str` 在 C 层按值传递结构体。

---

## 11. 与设计文稿/规格书的同步

本说明书基于编译器 v0.0.28 源码生成，与以下文档保持一致：

- `axlang语法设计文稿.md`：§2.7.1（44 个关键字）、§3.3（44 个保留字）、§4.1（float32 类型）、§8.3（interface 默认方法体）
- `axlang编译器实现规格书.md`：§1.2 CLI 契约、§3.2 Token 全集、§3.4 关键字、§7.6 转换矩阵（C-21 float32→float）、§12.3 大小与对齐
- `CHANGELOG.md`：[0.0.28] 条目
- `ARCHITECTURE.md` §8：版本历史延长至 0.0.28

---

## 12. 示例程序

### 12.1 Hello World

```ax
import std.io;

int main() {
    io.writeLine("Hello, axlang!");
    return 0;
}
```

### 12.2 变量与类型

```ax
var x = 42;
float y = 3.14;
float32 z = 2.5f32;
decimal money = 19.99d;
string name = "axlang";
int? opt = null;

if (opt != null) {
    println(opt);
}
```

### 12.3 类与继承

```ax
class Animal {
    string name;
    constructor(string name) {
        this.name = name;
    }
    public method speak() {
        return "???";
    }
}

class Dog extend Animal {
    constructor(string name) {
        parent(name);
    }
    public override method speak() {
        return "Woof!";
    }
}

int main() {
    var d = new Dog("Buddy");
    println(d.speak());
    return 0;
}
```

### 12.4 错误处理

```ax
import std.file;
import std.io;

int main() {
    var f = try file.readAll("config.txt");
    io.writeLine(f);
    return 0;
}
```

### 12.5 格式化输出

```ax
int main() {
    var pi = 3.14159;
    var count = 42;
    var name = "axlang";

    fmt.print("hello\n");
    fmt.println("world");
    fmt.printf("pi = %.2f\n", pi);      // 输出：pi = 3.14
    fmt.printf("count = %d\n", count);  // 输出：count = 42
    fmt.printf("name = %s\n", name);    // 输出：name = axlang
    return 0;
}
```

> 说明：`fmt` 是编译器内建模块，**无需 `import std.fmt`**，直接使用 `fmt.printf`/`fmt.println` 等即可。`fmt.printf` 支持编译期占位符核对（%d/%f/%s/%.Nf/%t/%v/%%）；字符串插值 `"{pi:.2}"` 也可用于简单格式化（见 §2.7 字符串字面量）。

---

*本说明书由 axlang 编译器 v0.0.28 源码生成，如与编译器实际行为有出入，以编译器源码为准。*
