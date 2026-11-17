<!-- 本文件由 tools/gen_errdoc.py 生成，请勿手改；修改诊断文案后重新运行。 -->

# axlang 错误码清单（多语言）

诊断码按组件分段：`E1xxx` 词法 / `E2xxx` 语法 / `E3xxx` 声明与模块 /
`E4xxx` 类型与表达式 / `E5xxx` 转换与装箱 / `E6xxx` match 与错误传播 /
`E7xxx` 所有权与借用。输出语言由 `--lang` / `AXC_LANG` / `axc.toml`（`lang = "zh"`）决定，默认英文。

共 129 个码、329 条文案。

## E1（词法）

| 码 | 中文 | English |
|---|---|---|
| `E1001` | 禁止前导零十进制（八进制请写 `0o17`）（L-8） | leading-zero decimal is not allowed (write octal as `0o17`) |

| `E1002` | 单引号不是字符串定界符；一个字符就是长度为 1 的 string，请写 "{0}"（L-14） | a single quote is not a string delimiter; a one-character string is written "{0}" |

| `E1003` | 原始字符串未闭合（缺少 `"`）（L-15） | raw string not closed (missing `"`) |
| `E1003` | 块注释未闭合（缺少 `*/`）（L-20） | block comment not closed (missing `*/`) |
| `E1003` | 多行字符串未闭合（缺少 `"""`）（L-15） | multi-line string not closed (missing `"""`) |
| `E1003` | 字符串未闭合（换行出现前缺少 `"`）（L-15） | string not closed (missing `"` before end of line) |
| `E1003` | 字符串未闭合（缺少 `"`）（L-15） | string not closed (missing `"`) |

| `E1004` | 非法转义序列 `\{0}`（L-13） | invalid escape sequence `\{0}` |
| `E1004` | 非法转义序列 `\{0}`（只允许 \<br> \	 \r \\ \" \0 与 \u{{...}}） | invalid escape sequence `\{0}` (only \<br> \	 \r \\ \" \0 and \u{{...}} are allowed) |

| `E1005` | `1.e3` 不合法：小数点后必须有数字（L-9） | `1.e3` is invalid: digits are required after the decimal point |
| `E1005` | 下划线只能出现在两个数字之间（L-6） | underscores may only appear between two digits |
| `E1005` | 指数部分缺少数字（L-9） | missing digits in the exponent |
| `E1005` | 数值字面量缺少数字（`0{0}` 后必须有数字）（L-7） | numeric literal is missing digits (`0{0}` must be followed by digits) |
| `E1005` | 数字 {0} 不属于 {1} 进制（L-7） | digit {0} is not valid in base {1} |
| `E1005` | 非法整数字面量 `{0}`（L-6） | invalid integer literal `{0}` |
| `E1005` | 非法浮点字面量 `{0}`（L-9） | invalid float literal `{0}` |

| `E1006` | 源文件中出现 NUL 字符（U+0000）（L-3） | NUL character (U+0000) found in source file |
| `E1006` | 非法字符 {0}（L-1） | invalid character {0} |
| `E1006` | 非法字符 {0}（标识符只允许 [A-Za-z_][A-Za-z0-9_]*）（L-22） | invalid character {0} (identifiers only allow [A-Za-z_][A-Za-z0-9_]*) |

| `E1007` | `\u` 后必须紧跟 `{十六进制}`（L-13） | `\u` must be immediately followed by `{hex digits}` |
| `E1007` | `\u{...}` 未闭合（L-13） | `\u{...}` not closed |
| `E1007` | `\u{...}` 需要 1~6 位十六进制数字（L-13） | `\u{...}` requires 1-6 hex digits |
| `E1007` | 非法 Unicode 码点 U+{0}（L-13） | invalid Unicode code point U+{0} |

| `E1008` | 插值 `{` 未闭合（L-18） | unclosed `{` in interpolation |
| `E1008` | 插值表达式为空 | interpolation expression is empty |

## E2（语法）

| 码 | 中文 | English |
|---|---|---|
| `E2001` | `@` 后必须是属性名（`@os` 或 `@arch`）（L-23） | an attribute name is required after `@` (`@os` or `@arch`) |
| `E2001` | `catch` 的捕获类型必须是 `error` 或 `error.变体`（文稿 10.2） | the catch type must be `error` or `error.Variant` |
| `E2001` | `func` 类型请写成 `func(参数) 返回`（G-1） | `func` types are written `func(params) return` |
| `E2001` | `throw` 后必须是 `error.变体(消息)`（文稿 10.1） | `throw` must be followed by `error.Variant(message)` |
| `E2001` | `throw` 本身是语句，不能作为 return 的表达式（G-15） | `throw` is a statement and cannot be used as a return expression |
| `E2001` | `try` 之后必须跟 `catch` 或 `finally`（文稿 10.2） | `try` must be followed by `catch` or `finally` |
| `E2001` | `void?` 不合法（G-1） | `void?` is not valid |
| `E2001` | 关键字 `{0}` 不能作{1}（G-7） | keyword `{0}` cannot be used as {1} |
| `E2001` | 插值表达式后有多余内容（L-18） | unexpected content after the interpolation expression |
| `E2001` | 期望 `>`，得到 `{0}`（G-1） | expected `>`, found `{0}` |
| `E2001` | 期望 {0}，得到 {1}（G-1） | expected {0}, found {1} |
| `E2001` | 期望{0}，得到 `{1}`（G-1） | expected {0}, found `{1}` |
| `E2001` | 非法的 match 模式（G-1） | invalid match pattern |
| `E2001` | 非法的表达式起点 `{0}`（G-1） | invalid expression start `{0}` |

| `E2002` | 语句未以换行或 `;` 结束（同一行不能写两条语句）（G-2） | a statement must end with a newline or `;` (two statements cannot share one line) |

| `E2003` | 行首不能是运算符 `{0}`（上一行已是完整语句；续行请把运算符留在行末） | `{0}` cannot start a line (the previous line is already a complete statement; keep the operator at the end of the line when continuing) |

| `E2004` | {0} 必须用 `{{}}` 括起来（即使只有一条语句）（G-1） | {0} must be wrapped in `{{}}` (even for a single statement) |

| `E2005` | `if` 用作表达式时必须有 `else`（G-1） | `if` used as an expression must have an `else` |

| `E2006` | 函数 `{0}` 存在没有 return 的路径（文稿 9.4） | function `{0}` has a path that does not return |

| `E2009` | `try` 之后不能再接 `.` 或 `[`（`try` 绑定最紧）（文稿 10.1） | `.` or `[` cannot follow `try` (`try` binds tightest) |

| `E2010` | `{0}` 只能单独成语句，不能出现在表达式内部（文稿 11.1） | `{0}` can only stand alone as a statement; it cannot appear inside an expression |
| `E2010` | 不支持前缀 `{0}`（文稿 11.1） | prefix `{0}` is not supported |
| `E2010` | 不支持前缀 `{0}`，请写成语句 `x {1}`（文稿 11.1） | prefix `{0}` is not supported; write the statement as `x {1}` |

| `E2011` | 模块级只能是声明或 import，不能出现语句（文稿 9.1） | only declarations and imports are allowed at module level, not statements |

| `E2012` | `new` 不能用于内建类型 `{0}`（G-9） | `new` cannot be used on built-in type `{0}` |
| `E2012` | `new` 只能用于 class（struct/enum/interface 不需要 new）（G-9） | `new` is only for classes (struct/enum/interface do not need new) |

| `E2013` | `defer` 后必须是一次调用（文稿 5.4） | `defer` must be followed by a call |

| `E2014` | 没有名为 `{0}` 的循环标签（文稿 5.2） | no loop label named `{0}` |

| `E2015` | 语法错误过多（已达 {0} 条），停止解析（G-16） | too many syntax errors ({0} so far); stopping analysis |

## E3（声明 / 模块）

| 码 | 中文 | English |
|---|---|---|
| `E3001` | 变量 `{0}` 遮蔽了外层同名变量（第 {1} 行）（N-3） | variable `{0}` shadows an outer variable of the same name (line {1}) |

| `E3002` | 变量声明必须有初值（文稿 3.3） | a variable declaration must have an initializer |

| `E3003` | Rounding 没有成员 `{0}`（N-17） | Rounding has no member `{0}` |
| `E3003` | 内置库函数名一律小写：`{0}` 请写 `{1}`（文稿 2.3） | built-in function names are lowercase: write `{1}` instead of `{0}` |
| `E3003` | `{0}.{1}` 不存在（N-17） | `{0}.{1}` does not exist |
| `E3003` | `{0}.{1}` 是常量，不可调用（文稿 9.2） | `{0}.{1}` is a constant and cannot be called |
| `E3003` | `{0}` 不是 enum（N-18） | `{0}` is not an enum |
| `E3003` | `{0}` 之后不能继续取成员（`模块.类型.方法(...)` 最多三层）（文稿 9.2） | member access cannot continue after `{0}` (`Module.Type.Method(...)` is at most three levels) |
| `E3003` | `{0}` 没有字段 `{1}`（N-7） | `{0}` has no field `{1}` |
| `E3003` | `{0}` 没有方法 `{1}`（N-7） | `{0}` has no method `{1}` |
| `E3003` | `{0}` 没有静态成员 `{1}`（N-13） | `{0}` has no static member `{1}` |
| `E3003` | `{0}` 没有静态方法 `{1}`（N-3） | `{0}` has no static method `{1}` |
| `E3003` | `{0}` 里没有类型 `{1}`（N-7） | `{0}` has no type `{1}` |
| `E3003` | const 初始化器必须是常量表达式（S-2） | const initializers must be constant expressions |
| `E3003` | enum `{0}` 没有成员 `{1}`（文稿 6.4） | enum `{0}` has no member `{1}` |
| `E3003` | error 只有 `msg` 字段（N-17） | error has only a `msg` field |
| `E3003` | string 没有 parse（N-17） | string has no parse |
| `E3003` | {0} 没有字段 `{1}`（N-7） | {0} has no field `{1}` |
| `E3003` | {0} 没有方法 `{1}`（N-7） | {0} has no method `{1}` |
| `E3003` | 未定义的标识符 `{0}`（N-5） | undefined identifier `{0}` |
| `E3003` | 未定义的类型 `{0}`（N-7） | undefined type `{0}` |
| `E3003` | 模块 `{0}` 没有成员 `{1}`（模块函数要写成 `{2}.{3}(...)`）（文稿 9.2） | module `{0}` has no member `{1}` (module functions are called as `{2}.{3}(...)`) |
| `E3003` | 模块 `{0}` 没有符号 `{1}`（文稿 9.2） | module `{0}` has no symbol `{1}` |
| `E3003` | 父类 `{0}` 没有方法 `{1}`（N-7） | parent class `{0}` has no method `{1}` |
| `E3003` | 类型别名循环引用 `{0}`（N-21） | circular type alias reference `{0}` |

| `E3004` | struct 字段重名（N-4） | duplicate struct field names |
| `E3004` | 参数 `{0}` 重复（N-4） | duplicate parameter `{0}` |
| `E3004` | 字段 `{0}` 重复声明（N-4） | field `{0}` is declared more than once |
| `E3004` | 常量 `{0}` 重复声明（N-4） | constant `{0}` is declared more than once |
| `E3004` | 方法 `{0}` 重复声明（N-4） | method `{0}` is declared more than once |

| `E3005` | 不能给 const `{0}` 赋值（文稿 3.3） | cannot assign to const `{0}` |

| `E3006` | `var` 只能用于局部变量（T-13） | `var` is only allowed for local variables |

| `E3007` | 字段不能有初值（在 constructor 里赋值）（文稿 3.3） | fields cannot have initializers (assign them in the constructor) |

| `E3008` | 类 `{0}` 有字段但没有 constructor（文稿 6.1） | class `{0}` has fields but no constructor |

| `E3009` | constructor 没有给字段 `{0}` 赋值（文稿 6.1） | the constructor never assigns field `{0}` |

| `E3010` | 一个类只能有一个 constructor（文稿 6.1） | a class can have at most one constructor |

| `E3011` | `constructor` 不能写可见性修饰词（默认 public）（N-11） | `constructor` cannot have a visibility modifier (it is public by default) |
| `E3011` | `constructor` 不能是 `static` 或 `override`（N-11） | `constructor` cannot be `static` or `override` |
| `E3011` | interface 成员不能写可见性修饰词（N-10） | interface members cannot have visibility modifiers |
| `E3011` | struct 成员不能写可见性修饰词（一律 public）（N-10） | struct members cannot have visibility modifiers (they are always public) |

| `E3012` | 修饰符顺序错误：可见性应写在 `static`/`override` 之前（文稿 6.1） | modifier order is wrong: visibility must come before `static`/`override` |

| `E3013` | `{0}` 不是 public，不能跨模块使用（N-8） | `{0}` is not public and cannot be used across modules |
| `E3013` | `{0}` 不是静态方法，请用实例调用（N-13） | `{0}` is not a static method; call it on an instance |
| `E3013` | `{0}` 是 protected，只能在本类或子类内访问（N-9） | `{0}` is protected and can only be accessed inside the class or its subclasses |
| `E3013` | `{0}` 是 protected，只能在本类或子类内调用（N-9） | `{0}` is protected and can only be called inside the class or its subclasses |
| `E3013` | `{0}` 是私有的（只在本类内可见）（N-9） | `{0}` is private (visible only inside its own class) |
| `E3013` | `{0}` 是静态方法，只能用 `{1}.{2}(...)` 调用（N-13） | `{0}` is a static method; call it as `{1}.{2}(...)` |

| `E3014` | `protected` 只能用于类成员，模块级声明不合法（N-12） | `protected` is only allowed on class members; it is invalid at module level |

| `E3016` | `override` 没有对应的父类或 interface 方法（文稿 6.2） | `override` has no matching parent or interface method |

| `E3017` | 方法 `{0}` 覆写了父类方法但没写 `override`（文稿 6.2） | method `{0}` overrides a parent method but is not marked `override` |

| `E3018` | `{0}` 与已声明的{1}重名（类型名与函数名共用一个命名空间）（N-14） | `{0}` conflicts with an existing {1} (type names and function names share one namespace) |
| `E3018` | `{0}` 是内建类型名，不能重新声明（N-15） | `{0}` is a built-in type name and cannot be redeclared |
| `E3018` | 变量与类型同名 `{0}`，`T(x)` 无法判定是转换还是调用（N-16） | variable and type share the name `{0}`; `T(x)` is ambiguous between a conversion and a call |

| `E3019` | `implement` 后面必须是 interface（文稿 6.3） | `implement` must be followed by an interface |
| `E3019` | interface 只能 extend interface（文稿 6.3） | an interface can only extend another interface |
| `E3019` | 方法 `{0}` 与 interface 签名不一致（文稿 6.3） | method `{0}` does not match the interface signature |
| `E3019` | 类 `{0}` 没有实现 interface 方法 `{1}`（文稿 6.3） | class `{0}` does not implement interface method `{1}` |
| `E3019` | 覆写 `{0}` 的签名与父类不一致（文稿 6.2） | the signature of the override for `{0}` differs from the parent |

| `E3020` | `extend` 后面必须是 class（文稿 6.2） | `extend` must be followed by a class |
| `E3020` | 一个类只能 `extend` 一个父类（文稿 6.2） | a class can only `extend` one parent |

| `E3021` | struct 不能 `extend` 或 `implement`（文稿 6.5） | struct cannot `extend` or `implement` |

| `E3022` | struct `{0}` 递归内联包含自身（T-30） | struct `{0}` recursively contains itself by value |

| `E3023` | struct 字段不能含 class 引用（包括容器元素）：`{0}` 是 {1}（T-29） | struct fields cannot contain class references (including in containers): `{0}` is {1} |

| `E3024` | 模块级不能声明变量（只能 const）；入口文件的顶层语句会被包成 main，不能带可见性修饰词（文稿 9.4） | variables cannot be declared at module level (only const); top-level statements of the entry file are wrapped into main and cannot carry visibility modifiers |
| `E3024` | 被 import 的文件不能有顶层语句（语句只能写在函数体内）；顶层代码只允许出现在程序入口文件（文稿 9.4） | an imported file cannot have top-level statements (statements belong inside function bodies); top-level code is only allowed in the program entry file |

| `E3025` | `main` 签名必须是 `int main()` 或 `int main(string[] args)`（文稿 9.4） | `main` must have the signature `int main()` or `int main(string[] args)` |
| `E3025` | 有多个 `main`（文稿 9.4） | multiple `main` functions |
| `E3025` | 缺少 `int main()` 入口（文稿 9.4） | missing `int main()` entry point |

| `E3026` | 循环导入：{0} | circular import: {0} |

| `E3027` | 找不到模块 `{0}`（已在 {1} 下查找） | module `{0}` not found (searched under {1}) |
| `E3027` | 找不到模块 `{0}`（模块路径即目录结构：`import a.b` 对应入口文件同目录的 `a/b.ax`）（文稿 9.2） | module `{0}` not found (a module path is a directory structure: `import a.b` maps to `a/b.ax` next to the entry file) |
| `E3027` | 未知的错误变体 `error.{0}`（可用：NotFound/Invalid/Money/IO）（文稿 10.1） | unknown error variant `error.{0}` (available: NotFound/Invalid/Money/IO) |
| `E3027` | 模块文件不是合法 UTF-8：{0} | module file is not valid UTF-8: {0} |
| `E3027` | 模块文件读取失败：{0} | failed to read module file: {0} |

| `E3028` | `main` 只能出现在程序入口文件（被导入的模块不能声明 main）（文稿 9.4） | `main` can only appear in the program entry file (imported modules cannot declare main) |

| `E3029` | 未知属性 `@{0}`（只支持 @os 与 @arch）（L-23） | unknown attribute `@{0}` (only @os and @arch are supported) |

| `E3030` | `{0}` 不在 `import {1}.{{...}}` 的具名导入清单里（文稿 9.2） | `{0}` is not in the named-import list of `import {1}.{{...}}` |
| `E3030` | 模块 `{0}` 没有公开成员 `{1}`（文件内符号默认私有，导出需写 `public`）（文稿 9.1） | module `{0}` has no public member `{1}` (symbols inside a file are private by default; export them with `public`) |
| `E3030` | 模块 `{0}` 没有公开类型 `{1}`（文稿 9.1） | module `{0}` has no public type `{1}` |
| `E3030` | 模块 `{0}` 没有公开类型 `{1}`（文件内符号默认私有，导出需写 `public`）（文稿 9.1） | module `{0}` has no public type `{1}` (symbols inside a file are private by default; export them with `public`) |

| `E3031` | enum 成员不能带初值或载荷（文稿 6.4） | enum members cannot have initializers or payloads |
| `E3031` | enum 成员名重复（文稿 6.4） | duplicate enum member name |

| `E3032` | interface 不能有 constructor（文稿 6.3） | an interface cannot have a constructor |

| `E3034` | `parent(...)` 只能在子类 constructor 内使用（G-8） | `parent(...)` can only be used inside a subclass constructor |
| `E3034` | `parent` 只能用于有父类的类内（G-8） | `parent` can only be used inside a class that has a parent class |
| `E3034` | `this` 只能在 class/struct 方法体内使用（G-8） | `this` can only be used inside class/struct method bodies |

| `E3035` | 子类 constructor 的第一条语句必须是 `parent(...)`（文稿 6.2） | the first statement of a subclass constructor must be `parent(...)` |

| `E3036` | `static`/`override` 不能用于字段（文稿 6.1） | `static`/`override` cannot be applied to fields |

| `E3037` | 嵌套类型缺少可见性修饰符：必须显式写 `public`、`private` 或 `protected`（文稿 6.1） | nested type is missing a visibility modifier: write `public`, `private`, or `protected` explicitly |
| `E3037` | 类成员缺少可见性修饰符：必须显式写 `public`、`private` 或 `protected`（文稿 6.1） | class member is missing a visibility modifier: write `public`, `private`, or `protected` explicitly |

| `E3038` | 静态方法体内没有 `this`（静态方法不属于任何对象）<br>  = 访问实例成员请改为实例方法，或通过参数把实例传进来（N-21） | there is no `this` inside a static method body (a static method does not belong to any object)<br>  = to access instance members, make it an instance method, or pass the instance in as a parameter |

| `E3039` | 模块名 `{0}` 与内建模块重名，文件 `{1}` 不会被加载（内建模块优先）；请重命名该文件 | module name `{0}` conflicts with a built-in module; file `{1}` will not be loaded (built-ins win); rename the file |

| `E3040` | `{0}` 这个名字已经被 `import {1}` 占用，`import {2}` 不能再使用；请写成 `import {3} as 别名` 加以区分（文稿 9.2） | the name `{0}` is already taken by `import {1}`; `import {2}` can no longer use it; write `import {3} as alias` to disambiguate |

| `E3041` | 既有 `int main()` 又有顶层语句（顶层语句会自动包成 `main`）；二者只留一个——要么把语句搬进 `main`，要么删掉 `main`（文稿 9.4） | both an explicit `int main()` and top-level statements exist (top-level statements are wrapped into an implicit `main`); keep only one - either move the statements into `main`, or delete `main` |

| `E3042` | `{0}` 有歧义：{1} 都导出了同名类型；请写全路径 `{2}.{3}` 或改用 `import {4}.{{{5}}}`（文稿 9.2） | `{0}` is ambiguous: both {1} export a type with that name; write the full path `{2}.{3}` or use `import {4}.{{{5}}}` |

| `E3043` | 成员名一律 PascalCase：{0} `{1}` 应写作 `{2}`（文稿 2.3） | member names are always PascalCase: {0} `{1}` should be written `{2}` |

## E4（类型与表达式）

| 码 | 中文 | English |
|---|---|---|
| `E4001` | `fail` 的实参必须是 error，得到 {0}（文稿 10.1） | the argument to `fail` must be error, found {0} |
| `E4001` | `null` 只能与可空类型或引用类型比较（T-19） | `null` can only be compared with nullable or reference types |
| `E4001` | `throw` 的消息必须是 string（T-5） | the message of `throw` must be a string |
| `E4001` | `{0}` 不能赋给 {1}（T-14） | `{0}` cannot be assigned to {1} |
| `E4001` | `{0}` 两边类型不一致：{1} vs {2}（T-5） | the two sides of `{0}` have different types: {1} vs {2} |
| `E4001` | `{0}` 左侧必须是可写位置（文稿 11.1） | the left side of `{0}` must be a writable place |
| `E4001` | `{0}` 是方法，调用要写 `(...)`（N-13） | `{0}` is a method; call it with `(...)` |
| `E4001` | map 键类型是 {0}，得到 {1}（T-5） | map key type is {0}, found {1} |
| `E4001` | void 函数不能返回值（T-5） | a void function cannot return a value |
| `E4001` | {0} 不能下标访问（文稿 3.3） | {0} cannot be indexed |
| `E4001` | 函数需要返回 {0}，但 `return` 没有值（T-5） | the function must return {0}, but `return` has no value |
| `E4001` | 区间模式下界大于上界（S-1） | in a range pattern the lower bound exceeds the upper bound |
| `E4001` | 区间模式两端必须是常量（S-1） | both ends of a range pattern must be constants |
| `E4001` | 区间模式只能用于 int（文稿 5.3） | a range pattern can only be used on int |
| `E4001` | 匿名函数有 {0} 个参数，目标类型需要 {1} 个（文稿 4.1） | the anonymous function has {0} parameters but the target type requires {1} |
| `E4001` | 可空值 `{0}` 不能直接赋给 `{1}`，请先判空（T-19） | a nullable value `{0}` cannot be assigned directly to `{1}`; check for null first |
| `E4001` | 定长数组 `{0}` 需要 {1} 个元素，得到 {2}（T-15） | fixed-size array `{0}` requires {1} elements, found {2} |
| `E4001` | 数组元素类型不一致：{0} vs {1}（T-15） | array element types are inconsistent: {0} vs {1} |
| `E4001` | 模式 `{0}.{1}` 与 match 值类型 {2} 不一致（文稿 5.3） | pattern `{0}.{1}` does not match the match value type {2} |
| `E4001` | 模式类型 {0} 与 match 值 {1} 不一致（文稿 5.3） | pattern type {0} does not match the match value {1} |
| `E4001` | 比较两边类型不一致：{0} vs {1}（T-5） | the two sides of the comparison have different types: {0} vs {1} |
| `E4001` | 类型不匹配：期望 {0}，得到 float（T-5） | type mismatch: expected {0}, found float |
| `E4001` | 类型不匹配：期望 {0}，得到 {1}（{2}）（T-5） | type mismatch: expected {0}, found {1} ({2}) |
| `E4001` | 类型模式不能是 interface（用具体类）（文稿 5.3） | a type pattern cannot be an interface (use a concrete class) |
| `E4001` | 类型模式只能用于 object 或类引用，得到 {0}（文稿 5.3） | a type pattern can only be used on object or a class reference, found {0} |
| `E4001` | 类型模式必须是 class 或 interface | a type pattern must be a class or interface |
| `E4001` | 赋值左侧必须是可写位置（文稿 11.1） | the left side of an assignment must be a writable place |

| `E4002` | `{0}` 的操作数必须是 bool，得到 {1}（文稿 11.1） | the operand of `{0}` must be bool, found {1} |
| `E4002` | `{0}` 的条件必须是 bool，得到 {1}（文稿 5.1） | the condition of `{0}` must be bool, found {1} |

| `E4003` | decimal 字面量只能赋给 decimal（T-9） | a decimal literal can only be assigned to decimal |
| `E4003` | float 字面量不能直接赋给 decimal（C-6）；金额请写 `19.99d`（T-9） | a float literal cannot be assigned directly to decimal (C-6); write money as `19.99d` |
| `E4003` | {0} 与 {1} 之间没有隐式转换，请写 `{2}(x)`（T-8） | no implicit conversion between {0} and {1}; write `{2}(x)` |
| `E4003` | 字面量 {0} 超出 {1} 的范围 [{2}, {3}]（T-9） | literal {0} is out of range for {1} [{2}, {3}] |
| `E4003` | 整数字面量 {0} 超出 int 范围（T-9） | integer literal {0} is out of range for int |
| `E4003` | 整数类型不一致：{0} 与 {1}（T-8） | integer types are inconsistent: {0} and {1} |
| `E4003` | 整数类型不一致：{0} 与 {1}（无隐式转换，请写 `{2}(x)`）（T-8） | integer types are inconsistent: {0} and {1} (no implicit conversion; write `{2}(x)`) |
| `E4003` | 需要显式转换：写 `{0}({1})`（T-8） | an explicit conversion is required: write `{0}({1})` |

| `E4004` | 定长数组的长度必须是常量表达式（S-1） | the length of a fixed-size array must be a constant expression |
| `E4004` | 定长数组长度必须是正整数（S-1） | the length of a fixed-size array must be a positive integer |
| `E4004` | 定长数组长度超出上限（S-1） | the length of a fixed-size array exceeds the limit |

| `E4005` | 定长数组长度为 {0}，常量下标 {1} 越界（合法范围 0..{2}）（S-1） | the fixed-size array has length {0}; constant index {1} is out of bounds (valid range 0..{2}) |

| `E4006` | string 不支持 `s[i]`；请用 `s.at(i)`（返回 string?）或 `s[i..j]` 之外的切片方法（文稿 11.2） | string does not support `s[i]`; use `s.at(i)` (returns string?) or the slice methods |

| `E4007` | `var` 不能从 null 推断类型（T-11） | `var` cannot infer a type from null |
| `E4007` | `var` 不能从 void 表达式推断类型（T-11） | `var` cannot infer a type from a void expression |
| `E4007` | `var` 不能持有 range（range 只能用于 for 头）（T-11） | `var` cannot hold a range (ranges are only allowed in for headers) |
| `E4007` | 无法从初始化器推断 const 类型（T-12） | the const type cannot be inferred from the initializer |
| `E4007` | 无法推断 const 类型（T-12） | the const type cannot be inferred |
| `E4007` | 空 `{}` 需要目标类型（T-14） | empty `{}` requires a target type |
| `E4007` | 空数组 `[]` 需要目标类型（T-15） | empty array `[]` requires a target type |

| `E4008` | record 字面量重复键 `{0}`（T-26） | duplicate key `{0}` in record literal |
| `E4008` | record 没有键 `{0}`（键名固定；要加键请用 map）（T-26） | record has no key `{0}` (keys are fixed; use a map to add keys dynamically) |
| `E4008` | record 的下标必须是字符串字面量（键在类型里固定）（T-26） | the index of a record must be a string literal (keys are fixed by the type) |

| `E4009` | `!` 只能用于 bool，得到 {0}（文稿 11.1） | `!` can only be applied to bool, found {0} |
| `E4009` | `%` 只能用于整数（文稿 11.1） | `%` can only be applied to integers |
| `E4009` | `-` 只能用于数值，得到 {0}（文稿 11.1） | `-` can only be applied to numbers, found {0} |
| `E4009` | `decimal` 不支持 `%`（除不尽会崩），请用 `a - trunc(a/b)*b`（文稿 3.2） | `decimal` does not support `%` (division may not terminate); use `a - trunc(a/b)*b` |
| `E4009` | `{0}` 只能用于数值（文稿 11.1） | `{0}` can only be applied to numbers |
| `E4009` | `{0}` 只能用于整数，得到 {1} 与 {2}（文稿 11.1） | `{0}` can only be applied to integers, found {1} and {2} |
| `E4009` | `{0}` 的操作数类型不允许：{1} 与 {2}（文稿 11.1） | operand types of `{0}` are not allowed: {1} and {2} |
| `E4009` | `~` 只能用于整数，得到 {0}（文稿 11.1） | `~` can only be applied to integers, found {0} |
| `E4009` | range 两端必须是 int（文稿 5.2） | both ends of a range must be int |
| `E4009` | 常量除零（S-3） | division by zero in a constant expression |
| `E4009` | 数组下标必须是 int，得到 {0}（文稿 3.3） | array index must be int, found {0} |
| `E4009` | 无符号整数不能取负（文稿 11.1） | unsigned integers cannot be negated |
| `E4009` | 未知运算符 `{0}`（文稿 11.1） | unknown operator `{0}` |
| `E4009` | 移位只能用于整数（文稿 11.1） | shift can only be applied to integers |

| `E4010` | `+` 两边必须都是 string（拼接）；数值请用 `{x}{y}` 插值（文稿 11.2） | both sides of `+` must be string (for concatenation); for numbers use `{x}{y}` interpolation |

| `E4011` | `float` 不能用 `{0}`（浮点比较请用 math.Equals）（文稿 3.7） | `float` cannot use `{0}` (compare floats with math.Equals) |

| `E4012` | 常量移位量 {0} 越界（0 <= n < {1}）（S-1） | constant shift amount {0} is out of range (0 <= n < {1}) |

| `E4013` | `if` 两分支类型不一致：{0} vs {1}（文稿 5.1） | the two branches of `if` have different types: {0} vs {1} |
| `E4013` | 三目两分支类型不一致：{0} vs {1}（文稿 5.1） | the two branches of the ternary have different types: {0} vs {1} |

| `E4014` | `match` 分支类型不一致：{0} vs {1}（文稿 5.3） | `match` arms have different types: {0} vs {1} |

| `E4015` | `??` 右侧类型 {0} 不能赋给 {1}（T-17） | the right side of `??` has type {0} which cannot be assigned to {1} |

| `E4016` | 比较不能连写（`a < b < c` 请写成 `a < b && b < c`）（文稿 11.1） | comparisons cannot be chained (write `a < b < c` as `a < b && b < c`) |

| `E4017` | `float` 不实现 Comparable（浮点比较请用 math.Compare）（文稿 7.1） | `float` does not implement Comparable (compare floats with math.Compare) |
| `E4017` | {0} 不实现 Comparable（文稿 7.1） | {0} does not implement Comparable |

| `E4018` | map 键类型必须是 Hashable + Comparable（不能是 {0}）（文稿 3.3） | map key types must be Hashable + Comparable ({0} is not allowed) |

| `E4019` | Rounding 是 enum，不能从数值转换（C-17） | Rounding is an enum and cannot be converted from a number |
| `E4019` | enum 不能从整数/其他类型转换（C-17） | an enum cannot be converted from int or other types |
| `E4019` | enum 不能转成整数（C-17） | an enum cannot be converted to an integer |

| `E4020` | 元组不能用 `{0}`（T-37） | tuples do not support `{0}` |

| `E4022` | `?.` 左侧必须是可空类型，得到 {0}（T-20） | the left side of `?.` must be a nullable type, found {0} |
| `E4022` | `??` 左侧必须是可空值，得到 {0}（T-17） | the left side of `??` must be a nullable value, found {0} |
| `E4022` | `{0}` 可能为 null，不能直接用 `.{1}`；请先判空收窄或用 `?.`（T-20） | `{0}` may be null and cannot be used directly with `.{1}`; narrow with a null check or use `?.` |
| `E4022` | `{0}` 可能为空，先判空或用 `?.`（T-20） | `{0}` may be null; check first or use `?.` |

| `E4024` | 函数值不能用 `{0}`（文稿 4.1） | function values do not support `{0}` |

| `E4025` | `(k, v) in` 只能遍历 map，得到 {0}（文稿 5.2） | `(k, v) in` can only iterate over a map, found {0} |
| `E4025` | {0} 不能遍历（只支持数组、map、string、range）（文稿 5.2） | {0} is not iterable (only arrays, maps, strings, and ranges are supported) |

| `E4026` | 不能在遍历 `{0}` 期间修改它的长度（`{1}`）（文稿 5.2） | the length of `{0}` cannot change while it is being iterated (`{1}`) |

| `E4027` | 元组元素个数不符：{0} 个变量，{1} 个元素（T-38） | tuple element count mismatch: {0} variables, {1} elements |
| `E4027` | 解构右侧必须是元组（T-38） | the right side of a destructuring must be a tuple |

| `E4028` | `object` 不能下标访问；先解箱到具体类型（文稿 6.7） | `object` cannot be indexed; unbox it to a concrete type first |
| `E4028` | `object` 没有成员方法；先解箱到具体类型（文稿 6.7） | `object` has no member methods; unbox it to a concrete type first |
| `E4028` | `object` 没有成员（文稿 6.7）；先解箱到具体类型（文稿 6.7） | `object` has no members (6.7); unbox it to a concrete type first |

| `E4029` | record 不能装箱到 object（T-28） | a record cannot be boxed into object |
| `E4029` | {0} 不能装箱到 object（T-6） | {0} cannot be boxed into object |

| `E4030` | 可空值 `{0}` 不能直接加格式说明符 `:.{1}`；先判空取出再格式化（文稿 11.2） | a nullable value `{0}` cannot take a format specifier `:.{1}` directly; check for null and format the unwrapped value |
| `E4030` | 格式说明符 `:.{0}` 只能用于数值类型，得到 {1}（文稿 11.2） | the format specifier `:.{0}` is only allowed on numeric types, found {1} |

| `E4031` | `fail` 需要 1 个实参（文稿 10.1） | `fail` takes exactly 1 argument |
| `E4031` | `ok` 需要 1 个实参（文稿 10.1） | `ok` takes exactly 1 argument |
| `E4031` | `parent(...)` 需要 {0} 个实参，得到 {1}（文稿 6.2） | `parent(...)` takes {0} arguments, found {1} |
| `E4031` | `{0}(...)` 缺少实参（N-20） | `{0}(...)` is missing arguments |
| `E4031` | `{0}` 的 constructor 需要 {1} 个实参，得到 {2}（N-20） | the constructor of `{0}` takes {1} arguments, found {2} |
| `E4031` | `{0}` 需要 {1} 个实参，得到 {2}（N-20） | `{0}` takes {1} arguments, found {2} |

| `E4033` | `{...}` 的键必须是字符串字面量（GAP-1） | keys of `{...}` must be string literals |
| `E4033` | `{...}` 的键必须是字符串字面量（不能含插值）（GAP-1） | keys of `{...}` must be string literals (interpolation is not allowed) |

| `E4034` | record 不能作为参数/返回值/赋给 {0}；请定义命名 struct（T-27） | a record cannot be used as an argument/return value/assigned to {0}; define a named struct instead |
| `E4034` | record 与 struct `{0}` 字段不匹配（需要 {1}，得到 {2}）（C-15） | record fields do not match struct `{0}` (expected {1}, found {2}) |
| `E4034` | 字段 `{0}` 类型不匹配：{1} vs {2}（C-15） | field `{0}` type mismatch: {1} vs {2} |

| `E4035` | `ArenaRef` 需要 1 个类型实参（G-1） | `ArenaRef` takes exactly 1 type argument |
| `E4035` | `map` 需要 2 个类型实参（键、值）（G-1） | `map` takes exactly 2 type arguments (key, value) |
| `E4035` | `result` 需要 1 个类型实参（G-1） | `result` takes exactly 1 type argument |
| `E4035` | `{0}` 是模块，不是类型（N-20） | `{0}` is a module, not a type |
| `E4035` | `{0}` 需要 {1} 个类型实参（G-1） | `{0}` takes {1} type arguments |

| `E4036` | `float` 不实现 Comparable（T-33） | `float` does not implement Comparable |
| `E4036` | {0} 不满足约束 `{1}`（T-36） | {0} does not satisfy the constraint `{1}` |
| `E4036` | 类型参数 `{0}` 缺少 `Comparable` 约束（T-32） | type parameter `{0}` is missing the `Comparable` constraint |
| `E4036` | 约束 `{0}` 必须是 interface（T-31） | a constraint `{0}` must be an interface |

| `E4037` | 无法推断 `{0}` 的类型实参，请显式写 `{1}<...>(...)`（T-16） | cannot infer the type arguments of `{0}`; write `{1}<...>(...)` explicitly |

| `E4038` | 泛型实例化递归超过 64 层：`{0}`（T-37） | generic instantiation recursion exceeds 64 levels: `{0}` |

| `E4039` | `Self` 只能用在 class/struct/interface 体内（N-19） | `Self` can only be used inside a class/struct/interface body |

| `E4041` | 定长数组过大（{0} × {1} 字节 > 8 MiB）（GAP-10） | fixed-size array too large ({0} x {1} bytes > 8 MiB) |

| `E4042` | `{0}` 没有 constructor，不能 new<br>  = 它的静态方法请直接用类名调用：`{1}.xxx(...)`<br>  = 如果确实需要实例，请先为 `{2}` 添加 constructor（N-20） | `{0}` has no constructor and cannot be new'd<br>  = call its static methods directly on the class: `{1}.xxx(...)`<br>  = if an instance is really needed, add a constructor to `{2}` first |

| `E4043` | 匿名函数需要目标类型：先声明 `func(参数) 返回` 类型的变量或参数，再把匿名函数赋给它<br>  = 例：`func(int) int f = (int x) { return x; }`（文稿 4.1） | an anonymous function needs a target type: declare a variable or parameter of type `func(params) ret` first, then assign the anonymous function to it<br>  = e.g. `func(int) int f = (int x) { return x; }` |

## E5（转换 / 装箱）

| 码 | 中文 | English |
|---|---|---|
| `E5001` | bool 与其他类型之间没有转换（C-10） | no conversion exists between bool and other types |
| `E5001` | float 不能直接转 decimal（C-6）；请用 `decimal.Parse(s)`（C-6） | float cannot convert to decimal directly (C-6); use `decimal.Parse(s)` |
| `E5001` | 不能用 `string(x)` 转字符串（C-8） | cannot convert to string with `string(x)` |
| `E5001` | 不能用 `string(x)` 转字符串；请写 `x.ToString()`，码点用 `string.FromCodepoint`（C-8） | cannot convert to string with `string(x)`; write `x.ToString()`, and `string.FromCodepoint` for code points |
| `E5001` | 不能用 `{0}(x)` 解析字符串；请用 `{1}.Parse(s)`（返回 result）（C-9） | cannot parse a string with `{0}(x)`; use `{1}.Parse(s)` (returns result) |

| `E5002` | `object` 不能直接作为条件；先落到 bool 变量或写 `bool(o)`（T-10） | `object` cannot be used as a condition directly; bind it to a bool variable or write `bool(o)` |
| `E5002` | `object` 不能直接参与运算；先落到具体类型的变量，或写 `int(o)`（T-10） | `object` cannot participate in arithmetic directly; bind it to a variable of a concrete type, or write `int(o)` |
| `E5002` | `object` 不能直接参与逻辑运算（T-10） | `object` cannot participate in logical operations directly |
| `E5002` | `object` 不能直接参与逻辑运算；先落到 bool（T-10） | `object` cannot participate in logical operations directly; bind it to bool first |
| `E5002` | `object` 不能直接当 {0} 用；先落到具体类型的变量，或写 `{1}(o)`（T-10） | `object` cannot be used as {0} directly; bind it to a variable of a concrete type, or write `{1}(o)` |
| `E5002` | 插值里不能直接用 object；先落成具体类型（T-10） | object cannot be used inside interpolation directly; bind it to a concrete type first |

| `E5003` | interface 只能从实现它的类构造（C-14） | an interface can only be constructed from a class that implements it |
| `E5003` | 向下转型的目标必须是子类（`{0}` 不是 `{1}` 的子类） | the target of a downcast must be a subclass (`{0}` is not a subclass of `{1}`) |

| `E5004` | `{0}(x)` 的实参类型 {1} 不能转换（C-1） | the argument type {1} of `{0}(x)` cannot be converted |
| `E5004` | `{0}` 不是可构造类型（N-20） | `{0}` is not a constructible type |
| `E5004` | `{0}` 不是可调用对象（N-20） | `{0}` is not callable |
| `E5004` | `{0}` 不是类型（N-20） | `{0}` is not a type |
| `E5004` | 未知类型转换（N-20） | unknown type conversion |
| `E5004` | 调用目标不是函数（N-20） | the call target is not a function |

| `E5005` | `{0}(x)` 只能有一个实参（构造请用构造调用）（C-1） | `{0}(x)` takes exactly one argument (use a constructor call to construct) |

| `E5006` | 插值暂不支持 `{0}` 类型；先取其中的标量或自己拼接 | interpolation does not support type `{0}` yet; extract a scalar from it or concatenate manually |

## E6（match / 错误传播）

| 码 | 中文 | English |
|---|---|---|
| `E6001` | `match` 非穷尽：缺少 fail(...)（文稿 5.3） | `match` is not exhaustive: missing fail(...) |
| `E6001` | `match` 非穷尽：缺少 ok 与 fail（文稿 5.3） | `match` is not exhaustive: missing ok and fail |
| `E6001` | `match` 非穷尽：缺少 ok(...)（文稿 5.3） | `match` is not exhaustive: missing ok(...) |
| `E6001` | `match` 非穷尽：缺少 {0}（文稿 5.3） | `match` is not exhaustive: missing {0} |

| `E6002` | 对 `object` 的 `match` 必须有 `_` 分支（文稿 5.3） | a `match` on `object` must have a `_` arm |
| `E6002` | 对 {0} 的 `match` 必须有 `_` 分支（文稿 5.3） | a `match` on {0} must have a `_` arm |

| `E6004` | `ok(...)`/`fail(...)` 模式只能用于 result，得到 {0}（文稿 10.1） | `ok(...)`/`fail(...)` patterns can only be used on result, found {0} |

| `E6005` | 类型模式缺少绑定名（写 `{0} x`）（G-11） | type pattern is missing a binding name (write `{0} x`) |

| `E6006` | `_` 之后还有分支，不可达（文稿 5.3） | there are arms after `_`, which are unreachable |
| `E6006` | 该分支不可达（`{0}.{1}` 已覆盖）（文稿 5.3） | this arm is unreachable (`{0}.{1}` is already covered) |
| `E6006` | 该分支不可达（`{0}` 已覆盖）（文稿 5.3） | this arm is unreachable (`{0}` is already covered) |
| `E6006` | 该分支不可达（重复的字面量 {0}）（文稿 5.3） | this arm is unreachable (duplicate literal {0}) |

| `E6007` | `match` 作表达式时分支不能是 void（文稿 5.3） | when `match` is used as an expression its arms cannot be void |

| `E6008` | 循环标签 `{0}` 重复（文稿 5.2） | duplicate loop label `{0}` |

| `E6009` | `break` 不在循环内（文稿 5.2） | `break` is not inside a loop |
| `E6009` | `continue` 不在循环内（文稿 5.2） | `continue` is not inside a loop |

| `E6010` | `throw` 只能出现在返回 result<...> 的函数内或 try 块内（main 除外）（文稿 10.1） | `throw` can only appear in functions returning result<...> or inside try blocks (main excepted) |
| `E6010` | `try` 只能出现在返回 result<...> 的函数内或 try 块内（main 除外）（文稿 10.1） | `try` can only appear in functions returning result<...> or inside try blocks (main excepted) |
| `E6010` | `{0}(...)` 只能用于返回 result<...> 的函数内（main 除外）（文稿 10.1） | `{0}(...)` can only be used in functions returning result<...> (main excepted) |

| `E6011` | `try` 只能作用于 result<T>，得到 {0}（文稿 10.1） | `try` can only apply to result<T>, found {0} |

| `E6012` | `catch (error e)` 已捕获全部错误，后面的 catch 永远不会执行（文稿 10.2） | `catch (error e)` catches every error; the catch clauses after it will never run |
| `E6012` | `catch (error.{0} ...)` 重复，前一个已经接住这类错误（文稿 10.2） | `catch (error.{0} ...)` is duplicated; an earlier catch already handles it |

| `E6013` | `break` 会跳出 try 块（finally 不会执行）；把循环移到 try 外面（文稿 10.2） | `break` would leave the try block (finally would not run); move the loop outside the try |
| `E6013` | `continue` 会跳出 try 块（finally 不会执行）；把循环移到 try 外面（文稿 10.2） | `continue` would leave the try block (finally would not run); move the loop outside the try |

## E7（所有权 / 借用）

| 码 | 中文 | English |
|---|---|---|
| `E7001` | `{0}` 已被移走，此处不可再用（重新赋一个值可复活）（O-2） | `{0}` has been moved and can no longer be used here (assign it a new value to revive it) |

| `E7002` | 同一语句内对 `{0}` 有多个借用（至少一个是可变的）（O-7） | multiple borrows of `{0}` within one statement (at least one is mutable) |

| `E7004` | `{0}` 正在被借用，同一语句内不能移动或写入它（O-12） | `{0}` is borrowed and cannot be moved or written in the same statement |

| `E7005` | `arena.Alloc` 只收引用类型（class 实例），得到 {0}（O-17） | `arena.Alloc` only accepts reference types (class instances), found {0} |

| `E7006` | `ArenaRef` 不能逃出 arena 的作用域（{0}）（O-18） | `ArenaRef` cannot escape the arena's scope ({0}) |

| `E7007` | `&` 只能作用于可写位置（变量、字段、元素）（O-8） | `&` can only be applied to writable places (variables, fields, elements) |

| `E7009` | 不能对 const `{0}` 取可变借用（写 `&const`）（O-9） | cannot take a mutable borrow of const `{0}` (write `&const`) |

## FILE_NOT_FOUND（CLI）

| 码 | 中文 | English |
|---|---|---|
| `FILE_NOT_FOUND` | 找不到文件 {0}（CLI） | file not found: {0} |

## SUMMARY（CLI）

| 码 | 中文 | English |
|---|---|---|
| `SUMMARY` | {0} 个错误（CLI） | {0} error(s) |

## W1003（W1003）

| 码 | 中文 | English |
|---|---|---|
| `W1003` | 对 copy 类型 `{0}` 写借用等价于传值，`&` 是多余的 | a borrow of copy type `{0}` is equivalent to pass-by-value; `&` is redundant |
