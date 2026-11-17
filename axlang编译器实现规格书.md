# axlang 编译器实现规格书（AI 执行版）

> 版本：v0.16　配套：`axlang语法设计文稿.md` v0.40
> 读者：具备代码生成能力的 AI（及人类实现者）
> 用途：**唯一目的**是从零产出可运行的 `axc` 编译器（词法 → 语法 → 语义 → 代码生成 → 运行时）
> 本文件不是语言设计稿，不新增任何语言特性；语言事实一律以文稿为准

---

## 0. 如何使用本文档

### 0.1 三条铁律

| # | 铁律 | 说明 |
|---|---|---|
| 1 | **不得发明语言** | 不新增关键字、运算符、语法形式、隐式转换、标准库函数。文稿没有一个写法，你就不能造一个 |
| 2 | **文稿是唯一的语言真相** | 本文档与文稿冲突时以文稿为准，并在实现报告里登记 `CONFLICT-n`（见 §17.2） |
| 3 | **缺口必须显式化** | 文稿没写、本文档 §17 也没收口的情况，**不得猜语义**：登记 `GAP-REPORT`，选择"最保守实现"（能静态报错就静态报错，绝不静默放行） |

### 0.2 本文档的信息分层（务必按层区分）

| 层 | 内容 | 可改性 |
|---|---|---|
| **A 层：语言事实** | 词法、语法、类型规则、语义检查、崩溃清单 | 不可改，改就是改语言 |
| **B 层：实现决策（D-n）** | 实现语言、后端目标、内存布局、mangle、运行时 API、诊断格式 | 可改，但必须全项目一致，改一处必须回改全部引用点 |
| **C 层：缺口收口（GAP-n / CONFLICT-n）** | 文稿未定义、实现必须选一个答案的点 | 可选，但替换后必须同步 §16 用例 |

三者混在一起是编译器项目最常见的失败原因：把 B 层当成语言规则写进文档，下一代实现就会以为那是语法。

### 0.3 规则编号体系

本文档所有可检查的规则都有编号，测试用例直接引用编号（§16）：

| 前缀 | 含义 | 所在章节 |
|---|---|---|
| `L-n` | 词法规则 | §3 |
| `G-n` | 语法规则 | §4 |
| `N-n` | 名称解析 / 作用域 / 可见性 | §6 |
| `T-n` | 类型规则（等价、可赋值、推断） | §7 |
| `C-n` | 转换规则 | §7.6 |
| `S-n` | 语义检查 | §9 |
| `O-n` | 所有权与借用 | §11 |
| `B-n` | 后端与布局 | §12、§13 |
| `Rt-n` | 运行时行为 | §10、§14 |

诊断码（`E####` / `W####` / `R###`）是这些规则的**执行出口**：一条规则可以对应多个诊断码，一个诊断码必须能指回唯一规则。

### 0.4 工作方式（对 AI 的硬性要求）

1. **按里程碑推进**（§1.3），每个里程碑必须跑通 §16 中标注为该阶段的用例，才算完成。
2. **每个阶段结束输出一份自检报告**：已实现规则编号清单、未实现清单、`GAP-REPORT` 清单。
3. **不得跳过语义检查去生成能跑的代码**。本语言的价值全在"错的东西编译不过"，能跑但漏检查的实现等于没实现。
4. **诊断信息是产品的一部分**：每个 `E####` 必须给出文件、行、列、规则编号、一句人话解释。禁止 `internal error` 式输出。

---

## 1. 交付物、实现语言与里程碑

### 1.1 交付物（D-01）

| 项 | 内容 |
|---|---|
| 可执行文件 | `axc`（单文件，无运行时外部依赖，除系统 C 编译器与 libc） |
| 实现语言 | **Rust 2021**（默认选择）。理由：`enum` + `match` 与 AST 天然同构、无 GC、错误处理适合编译器、生成的诊断代码可控 |
| 备选实现语言 | Go（可用，代价是 AST 需大量接口断言）、C（不推荐，手工内存管理会淹没语义逻辑）。**开工前可改一次，之后不得变更** |
| 后端 | **C11**（D-02）。`axc` 生成 `.c` + 链接 `axrt.c`，调用系统 `cc` 产出可执行文件。理由：无 LLVM 依赖、AI 生成的代码可读可查、可直接用 clang/gcc 做一致性验证 |
| 运行时 | `axrt`（C11，§14），静态链接进产物 |
| 标准库 | 根模块 intrinsic（§15），少量模块用 `.ax` 写成 |

> 后端选择不影响语言语义。禁用 LLVM 不是设计判断，只是把"实现风险"降到 AI 可控范围；后续若换 LLVM IR，只需替换 §13 的代码生成层。

### 1.2 CLI 契约（D-03）

```
axc check <file.ax> [flags]         # 只做词法/语法/语义检查，不产码
axc build <file.ax> [flags]         # 产出 <file> 同名可执行文件
axc build <file.ax> -o app          # 指定输出
axc run   <file.ax> [-- args...]    # build 到临时目录后执行，透传退出码
```

| flag | 含义 |
|---|---|
| `-I <dir>` | 模块搜索根（可多次），默认：入口文件所在目录 |
| `--emit=c` | 只生成 C 源，不调用 cc；输出到 `<file>.c` |
| `--emit=exe` | 默认 |
| `--dump=ast\|types\|hir\|c` | 调试输出，格式见 §5.3 |
| `--diag=text\|json` | 诊断格式，默认 `text`（§2.1） |
| `--os=linux\|macos\|windows` | 覆盖目标平台（影响 `@os` 选择） |
| `--arch=x86_64\|arm64` | 覆盖目标架构 |
| `--lang=en\|zh` | 诊断文案语言，默认 `en`；回退顺序：`--lang` > 环境变量 `AXC_LANG` > 工作目录 `axc.toml`（顶层 `lang = "zh"`） |
| `-Werror` | 警告视为错误 |
| `-O0\|-O1\|-O2` | 传给 cc，默认 `-O1` |

### 1.3 里程碑与验收（严格顺序，不得并行跳阶）

| 阶段 | 内容 | 验收标准（必须全绿） |
|---|---|---|
| **M0** | 骨架：CLI、源文件读入、诊断输出、`--diag=json` | `axc check` 对空文件报 `E3025 无 main`；诊断格式通过 §2.1 快照 |
| **M1** | 词法：全部 token、字面量、注释、NEWLINE、位置 | §16 的 L 组用例；`--dump=ast` 可打印 token 流 |
| **M2** | 语法：完整 EBNF → AST，含全部歧义消解算法 | §16 的 G 组用例；文稿 §12 完整示例可解析（`--dump=ast` 无错） |
| **M3** | 名称解析 + 标量类型检查（表达式、if/for、赋值、const） | §16 的 N 组、T 组前半 |
| **M4** | 聚合类型：数组 / map / record / struct / class / interface / enum | §16 的 T 组后半、S 组 |
| **M5** | `result` / `try` / `throw` / `match` 穷尽 / 可空收窄 | §16 的 E 组（错误处理） |
| **M6** | 泛型：约束检查 + 单态化 | §16 的 GX 组 |
| **M7** | 所有权 / 借用 / `defer` / `arena` | §16 的 O 组 |
| **M8** | C11 代码生成 + `axrt` 最小集 | 文稿 §12 示例端到端跑通，输出与期望文本逐字节一致 |
| **M9** | 根模块标准库最小面 + linter 警告 + 全量用例 | §16 全部用例；`-Werror` 下自身示例零警告 |

> 判据不是"能编译 Hello World"，而是 M8 结束时 **§12 示例程序（Goods/DiscountedGoods/main）跑出的三行输出与文稿一致**。那是本语言所有特性的联合验收。

### 1.4 推荐源码结构（D-04）

```
axc/
  Cargo.toml
  src/main.rs             # CLI 入口
  src/driver.rs           # 阶段编排、模块加载缓存
  src/diag.rs             # 诊断收集、格式化（text/json）、源码片段与脱字符
  src/source.rs           # SourceId、行列表、Span、行列换算
  src/lexer/{mod.rs,token.rs,literal.rs,template.rs}
  src/parser/{mod.rs,decl.rs,stmt.rs,expr.rs,ty.rs,recover.rs}
  src/ast.rs              # §5 的节点定义
  src/sema/{mod.rs,scope.rs,resolve.rs,check_expr.rs,check_stmt.rs,check_decl.rs,exhaust.rs,narrow.rs}
  src/types.rs            # TypeRef、等价、可赋值、转换
  src/infer.rs            # var / 字面量 / record / 泛型实参推断
  src/consteval.rs
  src/own/{mod.rs,state.rs,borrow.rs,defer.rs,arena.rs}
  src/hir/{mod.rs,lower.rs,mono.rs}
  src/layout.rs           # 大小/对齐/字段偏移/vtable
  src/codegen_c/{mod.rs,types.rs,expr.rs,stmt.rs,decl.rs,mangle.rs}
  rt/axrt.h  rt/axrt.c
  stdlib/{io.ax,json.ax,math.ax}     # 能写成 ax 的部分
  tests/{lex,parse,sema,own,run}/    # §16 用例，一个用例一个文件 + 期望输出
  tests/mod/<ID>/                    # 多文件用例：main.ax + 模块文件 + <ID>.out
  samples/demo/<ID>/                 # 可运行示例工程（多模块 + 类），断言仍归 tests/
```

---

## 2. 全局约定

### 2.1 诊断格式（D-05）

文本模式（默认），每条诊断恰好占 **1 行头 + 1 行源码 + 1 行脱字符**：

```
goods.ax:12:9: error[E4001]: 类型不匹配：期望 decimal，得到 int
    decimal total = count
            ^^^^^
```

- 位置是 **1-based 行号、1-based 列号（按 Unicode 码点计，不是字节）**。
- 头行格式：`{path}:{line}:{col}: {severity}[{code}]: {message}`
- `severity` ∈ `error` | `warning` | `note`
- 一个源位置最多展示一次源码行；同一行多条诊断各自重复展示。
- **诊断必须稳定排序**：按 (文件路径字典序, 行, 列, code) 排序后输出。

JSON 模式：每行一个 JSON 对象（JSONL），字段固定：

```json
{"path":"goods.ax","line":12,"col":9,"end_line":12,"end_col":16,
 "severity":"error","code":"E4001",
 "message":"类型不匹配：期望 decimal，得到 int",
 "rule":"T-14","notes":[{"message":"此处推导为 int","line":12,"col":21}]}
```

`suggestions` 可选，含 `{"message":"...","replacement":"..."}`；仅用于"能机械修复"的场景（如 `'a'` → `"a"`）。

**多语言文案（D-05a）**：中文模板是唯一事实源，写在各报错调用点（占位符 `{0} {1}` 与实参分离）；
英文翻译集中在 `rust/src/diag_msg.rs` 的 `CATALOG`（按 `(诊断码, 中文模板)` 对应），查不到回退中文。
文档侧：`docs/ERRORS.md`（多语言错误清单）与 `docs/STDLIB.md`（内置库说明）由 `tools/gen_errdoc.py` /
`tools/gen_stdlib_doc.py` 从源码生成——**改文案/加内建方法后必须重跑生成器**，禁止手改生成文件。

### 2.2 退出码（D-06）

| 码 | 含义 |
|---|---|
| 0 | 成功（`check` 无 error；`build` 产物成功；`run` 时被运行程序返回 0） |
| 1 | 编译失败（有 error 级诊断） |
| 2 | **运行时崩溃**（§10 的七类 trap 之一） |
| 其他 | `axc run`：透传被运行程序的 `main` 返回值（3~255） |

> `main` 里 `try` 失败 → 打印 `e.msg` 到 stderr、退出码 **1**（文稿 10.1：这是正常退出，不是崩溃）。与编译失败同码是刻意的：对脚本调用方而言都是"没成功"。

### 2.3 编译模型（影响所有推断边界）

- **一个 `.ax` 文件 = 一个模块**，模块名 = 相对搜索根的路径去掉 `.ax`、`/` 换成 `.`。`net/http/server.ax` 的模块名是 `net.http.server`。
- **跨模块只读签名，不读函数体**（文稿 1.1）。因此：函数返回类型、参数类型、字段类型、类型定义必须完整存在于源码；`var` 推断结果不得跨模块传播。
- 模块编译顺序：DFS 后序（依赖先编），循环导入直接 `E3026`。
- 每个模块独立做名称解析与类型检查；**泛型单态化在全程序范围内进行**（需要所有调用点）——这是唯一需要全程序视图的步骤。
- 入口：全工程有且仅有一个 `int main()` 或 `int main(string[] args)`。

### 2.4 命名与 mangle（D-07，必须逐字符可复现）

C 侧符号一律 ASCII，规则固定：

| 对象 | mangle 结果 | 例 |
|---|---|---|
| 模块内函数 | `{module}__{name}`，`.` → `_`，非 `[A-Za-z0-9_]` → `_` | `net.http.server` 的 `serve` → `net_http_server__serve` |
| 类方法 | `{module}__{Class}__{name}` | `User.rename` → `goods__User__rename` |
| 静态方法 | 同上（无 `this` 参数） | `User.from_email` → `goods__User__from_email` |
| 嵌套类型 | `{Outer}_{Inner}` | `Order.Line` → `Order_Line` |
| record 类型 | `__rec_{n}`，`n` 为该编译单元内按首次出现顺序的序号（从 0 起） | `__rec_0` |
| 元组类型 | `__tup_{n}_{m}`（n=元素数，m=首次出现序号） | `__tup_2_0` |
| 泛型实例 | `{base}__g_{hash8}`，`hash8` = 规范化类型实参串的 FNV-1a 32 位低 8 位十六进制 | `Box__g_3f2a91c4` |
| 局部变量 | `v_{name}`（同函数内重名不可能，见 `N-11` 禁遮蔽） | `v_total` |
| 字段 | `f_{name}` | `f_price` |
| 装箱 | `__box_{tag}` 由运行时提供，编译器只生成 `uint32` 类型标签 | — |
| 常量 | `k_{module}__{name}` | `k_goods__MAX` |

规范化类型实参串：按书写顺序，用 `,` 连接每个实参的**规范名**（`int`→`int`、`T[]`→`arr_int`、`Dog`→`Dog`、`map<string,int>`→`map_string_int`），再取 FNV-1a。

### 2.5 类型标签（D-08）

`object` 盒子里的运行时标签是 `uint32`，取值规则（必须全项目唯一，`match` 与解箱都依赖它）：

| 标签 | 类型 | 标签 | 类型 |
|---|---|---|---|
| 1 | `bool` | 9 | `int32` |
| 2 | `int` | 10 | `uint` |
| 3 | `int8` | 11 | `uint8` |
| 4 | `int16` | 12 | `uint16` |
| 5 | — 保留 | 13 | `uint32` |
| 6 | `float` | 14 | `string` |
| 7 | `decimal` | 1000+ | class / 数组 / map / enum（按 §12.5 的类型 id 分配，编译期固定） |
| 8 | `void`（不使用） | — | — |

**精确匹配**：`int32` 装进盒子后标签是 9，解箱到 `int`（标签 2）必须失败（文稿 6.7）。数组与 map 标签含元素类型（`int[]` 与 `object[]` 不同），由 §12.5 的结构化类型 id 决定。

---

## 3. 词法规范

### 3.1 源文件与位置（L-1…L-4）

| 编号 | 规则 |
|---|---|
| `L-1` | 输入按 UTF-8 解码；非法 UTF-8 序列 → `E1006`。允许并跳过开头的 BOM |
| `L-2` | `\r\n` 与 `\r` 一律规范化为 `\n`（在解码后、词法前）。行号按规范化后的 `\n` 计数 |
| `L-3` | 源文件中的 `\0`（U+0000）是 `E1006` |
| `L-4` | 每个 token 记录 `Span{start_line,start_col,end_line,end_col,start_byte,end_byte}`；列按**码点**计 |

### 3.2 Token 全集（L-5）

| 类别 | Token | 备注 |
|---|---|---|
| 字面量 | `INT_LIT` `FLOAT_LIT` `DECIMAL_LIT` `STRING_LIT` `TEMPLATE_LIT` `TRUE` `FALSE` `NULL` | 数值字面量已带目标类型标记（见 §3.3） |
| 标识符 | `IDENT` | `[A-Za-z_][A-Za-z0-9_]*`；单独的 `_` 是 `UNDERSCORE`，不是 IDENT（文稿 2.3） |
| 关键字 | 36 个，见 §3.4 | 全部小写，大小写敏感 |
| 运算符/标点 | `+ - * / % & \| ^ ~ << >> && \|\| ! == != < <= > >= = += -= *= /= %= ?. ?? ? : .. ..= . , ; ( ) [ ] { } => @ ++ --` | 注意：**没有** `->`（成员访问只有 `.`）、没有泛型否定 `!T`、`as` 是关键字不是运算符 |
| 结构 | `NEWLINE` `EOF` | `NEWLINE` 只在语句边界有意义（§4.2） |

**最大匹配**（maximal munch），且多字符运算符列表必须按长度降序尝试：`..=` → `..` → `.`；`?.` → `?`；`??` → `?`；`<<` `<=`；`>>` `>=`；`=>` `==` `=`。

> `&` 既是位与又是借用标记，`?` 既是三目又是可空类型又是 `?.`/`??` 的一部分——这三处**不在词法层区分**，由语法位置决定（§4.4）。

### 3.3 数值字面量（L-6…L-11）

| 编号 | 规则 |
|---|---|
| `L-6` | 十进制整数：`0` \| `[1-9][0-9_]*`；下划线只能出现在两个数字之间，首尾或连续下划线 → `E1005` |
| `L-7` | `0x` + 至少一个 `[0-9a-fA-F]`（允许 `_`）、`0b` + `[01]`、`0o` + `[0-7]`。前缀后无数字 → `E1005` |
| `L-8` | **禁止前导零十进制**：`[0-9]+` 以 `0` 开头且长度 > 1 且不是 `0x/0b/0o` → `E1001`（含修复建议"八进制写 `0o17`"） |
| `L-9` | 浮点：`digits '.' digits` 之后可跟指数（`e` 或 `E`，可带 `+`/`-`），或 `digits` 直接跟指数。**必须**要么有小数点要么有指数，才成 `FLOAT_LIT` |
| `L-10` | `decimal`：浮点或整数形式后紧跟 `d` / `D` 后缀 → `DECIMAL_LIT`（`19.99d`、`19d`、`1e3d` 均合法）。`d` 与数字之间不得有空格 |
| `L-11` | 词法阶段**只做形状校验，不做范围检查**；范围检查在常量求值（§8）按目标类型进行。字面量的默认类型：`INT_LIT`→`int`、`FLOAT_LIT`→`float`、`DECIMAL_LIT`→`decimal` |

**关键消歧**：`1..10` 必须切成 `INT_LIT(1)` `..` `INT_LIT(10)`，不能把 `1.` 当浮点。规则：扫描到 `.` 时，**只有下一个字符是数字**才并入浮点；若下一个也是 `.`，立即结束整数。`1.e3` 不合法（`.` 后无数字）→ `E1005`。

### 3.4 关键字（L-12，36 个，不得增删）

```
var         const       class       struct      interface   extend
implement   override    parent      this        enum        type
if          else        for         in          match       return
try         catch       finally     throw       defer       import
public      protected   private     static      new         as          break
continue    constructor true        false       null
```

另有一组**保留类型名**（同样是 IDENT 的词法形态，但语义上是根模块内建类型，用户在模块内声明同名类型 → `E3018`）：

```
bool int int8 int16 int32 uint uint8 uint16 uint32 float decimal
string object void result error func map Self range Rounding Arena ArenaRef
```

`true` / `false` / `null` 是关键字，不是标识符——不能作变量名。

### 3.5 字符串字面量（L-13…L-18）

三种形式，**双引号是唯一正常形式**：

| 形式 | 语法 | 插值 | 转义 |
|---|---|---|---|
| 普通 | `"..."` | 是 | 6 个 + `\u{...}` |
| 原始 | `r"..."` | **否** | 无（`\` 是字面反斜杠） |
| 多行 | `"""..."""` | 是 | 6 个 + `\u{...}` |

| 编号 | 规则 |
|---|---|
| `L-13` | 转义仅 `\n \t \r \\ \" \0` 与 `\u{HEX}`（1~6 位十六进制，值 ≤ 0x10FFFF 且不在 0xD800–0xDFFF）。其他转义 → `E1004`；`\u{}` 非法 → `E1007` |
| `L-14` | **单引号不是定界符**：遇到 `'` → `E1002`，消息固定为「单引号不是字符串定界符；一个字符就是长度为 1 的 string，请写 `"a"`」，并给出 `suggestion` |
| `L-15` | 未闭合字符串 / 未闭合 `/*` → `E1003`（报在起始位置） |
| `L-16` | 多行字符串 `"""..."""`：开头 `"""` 后紧跟的**第一个换行**与结尾 `"""` 前的**最后一个换行**被去除，其余原样保留（含缩进） |
| `L-17` | 原始字符串 `r"..."` 内 `{` **不触发插值**（`GAP-3`），`"` 不能转义（即原始字符串内不能出现 `"`） |
| `L-18` | 普通/多行字符串的词法结果是一个 `TEMPLATE_LIT`，内部是片段列表：`Text(chunk)` 与 `Interp(token_slice, format_opt)`。`{{` → 文本 `{`，`}}` → 文本 `}`（`GAP-4`）；`{` 后为空或不是合法表达式起点 → `E1008` |

`Interp` 的 `format_opt` 语法：`:` `.` `[0-9]+`，即 `{expr:.2}`；其余形式 → `E1008`。

> 为什么字符串在**词法**层就要拆成片段：插值表达式里可以嵌套字符串与花括号（`"{m["k"]}"`），交给语法层处理会与 map 字面量混淆。词法层用"括号配平 + 字符串内嵌套递归"扫描，得到确定边界后再交回语法分析器解析片段内的 token 序列。

### 3.6 注释（L-19…L-21）

```ax
// 单行
/* 多行 /* 可嵌套 */ */
```

| 编号 | 规则 |
|---|---|
| `L-19` | 两种注释均为**空白**，不产生 token；注释内的换行**仍产生 `NEWLINE`**（否则 `a /*x*/ \n b` 的语句边界会变） |
| `L-20` | 块注释支持嵌套，深度计数；EOF 时深度 > 0 → `E1003` |
| `L-21` | 文档注释就是 `//`，无专门语法；工具链按"紧邻声明上方、中间无空行"提取 |

### 3.7 标识符与属性（L-22…L-24）

| 编号 | 规则 |
|---|---|
| `L-22` | 标识符 `[A-Za-z_][A-Za-z0-9_]*`；单独 `_` → `UNDERSCORE` token（只用于 `match` 通配） |
| `L-23` | `@` 后必须是 IDENT，构成 `ATTR` token（`@os` / `@arch`）；其他名字 → `E3029` |
| `L-24` | 命名风格不参与词法与语法：**不产生任何诊断**。用户侧命名只作文档推荐（PascalCase），编译器不检查；唯一硬约束是标准库/内建方法的拼写本身必须 PascalCase，否则该名字不存在 |

---

## 4. 语法规范

### 4.1 完整 EBNF（G-1）

记号：`{ x }` = 0 次或多次；`[ x ]` = 可选；`|` = 择一；`/* */` = 注释；**`nl`** = 可选的 `NEWLINE`（见 §4.2）。

```ebnf
/* ============ 顶层 ============ */
module        = { module_item } EOF ;
module_item   = import_stmt
              | { attribute } top_decl ;

attribute     = '@' ( 'os' | 'arch' ) '(' ident { ',' ident } ')' ;

import_stmt   = 'import' module_path [ 'as' ident
                                    | '.' '{' ident { ',' ident } '}' ] ;
module_path   = ident { '.' ident } ;

top_decl      = const_decl | func_decl | class_decl | struct_decl
              | interface_decl | enum_decl | type_alias ;

/* ============ 声明 ============ */
const_decl    = 'const' [ type ] ident '=' expr ;
var_decl      = ( type | 'var' ) ident '=' expr ;
var_tuple     = 'var' '(' ident { ',' ident } ')' '=' expr ;

func_decl     = [ 'public' ] type ident [ type_params ] '(' params ')' block ;
params        = [ param { ',' param } [ ',' ] ] ;
param         = [ '&' [ 'const' ] ] type ident ;        /* 借用形参 */
type_params   = '<' type_param { ',' type_param } [ ',' ] '>' ;
type_param    = ident [ ':' constraint { '+' constraint } ] ;
constraint    = type_name ;                             /* 必须是 interface 名 */

type_alias    = [ 'public' ] 'type' ident [ type_params ] '=' type ;

/* ============ class ============ */
class_decl    = [ 'public' ] 'class' ident [ type_params ]
                [ 'extend' type ]
                [ 'implement' type { ',' type } ]
                '{' { class_member } '}' ;

class_member  = [ visibility ] [ 'static' | 'override' ] ( field_decl | method_decl )
              | 'constructor' '(' params ')' block
              | nested_decl ;
visibility    = 'public' | 'protected' ;
field_decl    = type ident ;                            /* 字段禁止初值 */
method_decl   = type ident '(' params ')' block ;
nested_decl   = class_decl | struct_decl | enum_decl | interface_decl | type_alias ;

/* ============ struct ============ */
struct_decl   = [ 'public' ] 'struct' ident [ type_params ]
                '{' { struct_member } '}' ;
struct_member = field_decl | method_decl
              | 'constructor' '(' params ')' block
              | nested_decl ;                            /* struct 成员无可见性修饰词 */

/* ============ interface ============ */
interface_decl= [ 'public' ] 'interface' ident
                [ 'extend' type { ',' type } ]
                '{' { iface_member } '}' ;
iface_member  = type ident '(' params ')' [ block ] ;    /* 无 block = 必须实现 */

/* ============ enum ============ */
enum_decl     = [ 'public' ] 'enum' ident '{' ident { ',' ident } [ ',' ] '}' ;

/* ============ 类型 ============ */
type          = prim_type { type_suffix } ;
type_suffix   = '?'                                     /* 可空 */
              | '[' [ expr ] ']' ;                       /* [] = T[]，[N] = T[N] */
prim_type     = 'func' '(' [ type { ',' type } [ ',' ] ')' type
              | '(' type ',' type { ',' type } ')'       /* 元组类型，(T,U,...) */
              | type_name [ type_args ] ;
type_name     = ident { '.' ident } ;
type_args     = '<' type { ',' type } [ ',' ] '>' ;

/* ============ 语句 ============ */
block         = '{' { nl_ok stmt } '}' ;
stmt          = labeled_stmt | var_decl | var_tuple | const_decl
              | if_expr | for_stmt | match_expr
              | return_stmt | throw_stmt | try_stmt | defer_stmt
              | break_stmt | continue_stmt
              | block | simple_stmt ;

labeled_stmt  = ident ':' for_stmt ;
for_stmt      = 'for' [ for_header ] block ;
for_header    = '(' ident ',' ident ')' 'in' expr
              | ident 'in' expr ;

return_stmt   = 'return' [ expr ] ;
throw_stmt    = 'throw' error_ctor ;
try_stmt      = 'try' block { catch_clause } [ 'finally' block ] ;
catch_clause  = 'catch' '(' 'error' [ '.' ident ] ident ')' block ;
defer_stmt    = 'defer' call_expr ;
break_stmt    = 'break' [ ident ] ;
continue_stmt = 'continue' [ ident ] ;

simple_stmt   = expr [ assign_op expr ]
              | expr ( '++' | '--' ) ;
assign_op     = '=' | '+=' | '-=' | '*=' | '/=' | '%=' ;

/* ============ if / match ============ */
if_expr       = 'if' expr block [ 'else' ( if_expr | block ) ] ;

match_expr    = 'match' expr '{' match_arm { ',' match_arm } [ ',' ] '}' ;
match_arm     = pattern '=>' expr ;

pattern       = literal
              | range_pat
              | type_name '.' ident                       /* 枚举成员：Status.Ok */
              | ( 'ok' | 'fail' ) '(' ident ')'           /* result 解构 */
              | type ident                                /* 类型模式：Dog x */
              | '_'                                       /* 通配 */
              | ident ;                                   /* 绑定 */
range_pat     = literal ( '..' | '..=' ) literal ;

/* ============ 表达式（15 级优先级，从低到高书写） ============ */
expr          = range_expr ;
range_expr    = ternary [ ( '..' | '..=' ) ternary ] ;
ternary       = coalesce [ '?' expr ':' expr ] ;          /* 右结合 */
coalesce      = logic_or [ '??' coalesce ] ;              /* 右结合 */
logic_or      = logic_and { '||' logic_and } ;
logic_and     = equality { '&&' equality } ;
equality      = relational { ( '==' | '!=' ) relational } ;
relational    = bitor [ ( '<' | '<=' | '>' | '>=' ) bitor ] ;   /* 只允许一次：不可连写 */
bitor         = bitxor { '|' bitxor } ;
bitxor        = bitand { '^' bitand } ;
bitand        = shift { '&' shift } ;
shift         = additive { ( '<<' | '>>' ) additive } ;
additive      = multiplicative { ( '+' | '-' ) multiplicative } ;
multiplicative= unary { ( '*' | '/' | '%' ) unary } ;
unary         = ( '!' | '-' | '~' ) unary | postfix ;

postfix       = primary { postfix_op } ;
postfix_op    = '.' ident
              | '?.' ident
              | '[' expr ']'
              | '(' [ arg { ',' arg } [ ',' ] ] ')'
              | 'try' ;                                   /* 之后不得再接 postfix_op */
arg           = expr | '&' expr ;                          /* 借用实参 */

primary       = literal
              | 'new' type_name '(' [ arg { ',' arg } [ ',' ] ] ')'
              | '(' expr ')'                                /* 括号 */
              | '(' expr ',' expr { ',' expr } [ ',' ] ')'   /* 元组值 */
              | array_lit
              | brace_lit
              | if_expr
              | match_expr
              | anon_func
              | 'this'
              | 'parent'                                    /* 仅构造首行 / parent.m() */
              | type_name                                   /* 含类型转换 T(x) 的 callee */
              | error_ctor ;
error_ctor    = 'error' '.' ident '(' expr ')' ;           /* error.NotFound("...") */

array_lit     = '[' [ expr { ',' expr } [ ',' ] ] ']' ;
brace_lit     = '{' [ pair { ',' pair } [ ',' ] ] '}' ;
pair          = STRING_LIT ':' expr ;                      /* 键必须是字符串字面量，GAP-1 */

anon_func     = '(' [ param { ',' param } [ ',' ] ] ')' block ;
call_expr     = postfix ;                                  /* 供 defer 使用，语义上必须是调用 */

literal       = INT_LIT | FLOAT_LIT | DECIMAL_LIT | TEMPLATE_LIT
              | 'true' | 'false' | 'null' ;
```

### 4.2 语句边界：换行、分号与续行（G-2…G-6）

规则来自文稿 2.1：**语句已完整就结束**，`;` 与换行等价，续行只依据封闭的运算符清单。

| 编号 | 规则 |
|---|---|
| `G-2` | 语句终结符 = `NEWLINE` \| `;` \| 前瞻到 `}` \| `EOF`。解析器在语句末尾调用 `expect_terminator()`：吞掉一个 `NEWLINE` 或一个 `;`（或两者任意组合），出现其他 token → `E2002` |
| `G-3` | **续行无需语法层特判**：只要语句在语法上还没完整，解析器自然继续消费下一个 token；`NEWLINE` 在"期望操作数/期望 `)`/期望 `,`"等位置被 `nl_ok()` 跳过 |
| `G-4` | `nl_ok()` 的插入点（穷举，不多不少）：块内语句之间、`(` `[` `{`（字面量）之后、`,` 之后、二元运算符之后、`=>` 之后、`:` 之后（三目与字面量键值）、`=` 与组合赋值之后、`return`/`throw`/`defer` 之后、`else` 之后、类型中 `\|` 与 `&` 之后的位置 |
| `G-5` | 因此"运算符必须留在行末"是**自然结果**：`price * count⏎+ shipping` 中第一行已完整，`+ shipping` 成为新语句，`+` 无对应一元语义 → `E2003`（消息：「上一行已是完整语句；续行请把运算符留在行末」） |
| `G-6` | `{` 与所属头部同行或另起一行都合法，无需特判：`if cond`、`int f(int a)`、`class C` 这些头部本身不是完整语句，解析器处于"必然期待 `{`"的状态，`NEWLINE` 被 `nl_ok()` 跳过 |

> 实现提示：`nl_ok()` 是**纯跳过**，不改变语法结构；`expect_terminator()` 是**必须有一个边界**。两者用错位置的表现是：前者该用没用 → 合法多行代码报错；后者该用没用 → 两条语句被粘成一条（典型症状是"`E4001 类型不匹配`"出现在完全无关的行）。写解析器时先标出所有块内语句边界，其余位置一律 `nl_ok()`。

### 4.3 关键字与标识符的角色（G-7…G-9）

| 编号 | 规则 |
|---|---|
| `G-7` | 关键字不能作标识符、类型名、字段名、方法名、成员名 → `E2001`（expected identifier, found keyword） |
| `G-8` | `this` 只在 class/struct 方法体内合法；`parent` 只在子类 constructor 首行（`parent(...)`）或方法体（`parent.m(...)`）内合法。其余位置 → `E3034` |
| `G-9` | `new` 后必须是 `type_name`（不接 `[]`、`?`、`func`），`new Arena()` / `new Dog(...)`；`new` 只用于 class 与 `Arena`，用于 struct/enum/interface/内建 → `E2012` |

### 4.4 六处歧义与消解算法（G-10…G-15，必须逐条实现）

| 编号 | 歧义 | 消解 |
|---|---|---|
| `G-10` | `(int a, int b) { ... }` 匿名函数 vs `(expr)` 括号 vs `(a, b)` 元组 | **投机解析**：在 `(` 处保存位置 → 试解析 `param_list ')'`，若成功且下一个有效 token 是 `{` → 匿名函数；否则回滚，按括号/元组解析。回滚必须无副作用（AST 节点在试解析期间挂到临时 arena，成功才提交） |
| `G-11` | `Dog x =>` 类型模式 vs `x =>` 绑定 | 若模式起点是"类型语法"（IDENT / 内建类型名 / `func` / `map` / 元组类型 / 带 `[]`/`?` 后缀）**且其后紧跟 IDENT** → 类型模式；否则 IDENT 单独出现 → 绑定模式；类型语法后不是 IDENT → `E6005`（类型模式必须带绑定名） |
| `G-12` | `outer: for ...` 标签 vs 以 IDENT 开头的表达式语句 | 语句起始处前瞻两个 token：`IDENT` `:` 且第三个是 `for` → 标签；否则按表达式语句。**注意**：`:` 后不是 `for` 时不是标签，按 `E2003` 报（不可能存在"表达式后跟冒号"的合法语句） |
| `G-13` | `T(x)` 是转换/构造还是函数调用 | **语法层不判定**，一律解析为 `Call{callee: Path(T)}`；语义层按 `N-20` 决议（类型优先）。唯一例外：`error.Variant(msg)` 在解析期直接归约为 `ErrorCtor`（`error` 是内建专用构造，不参与 `Call` 决议） |
| `G-14` | `{...}` 是 map 还是 record | **语法层不判定**，解析为 `BraceLit`；语义层按 `T-31` 推 `map<string,V>` 或 record |
| `G-15` | `&` 是位与还是借用 | 语法位置唯一决定：出现在**实参位置**（`arg = '&' expr`）或**形参声明**（`param = '&' [const] type ident`）是借用；其他二元位置是位与。**没有一元 `&` 表达式**，所以 `a & &b` 是语法错误 |

### 4.5 解析错误恢复（G-16）

| 编号 | 规则 |
|---|---|
| `G-16` | 每个声明/语句解析失败时，**跳到下一个同步点**继续：`;`、`NEWLINE`（若括号深度为 0）、`}`、`EOF`、或下一个 `public`/`class`/`struct`/`interface`/`enum`/`type`/`const` 关键字。一次编译至少报告前 20 条语法错误后停止（避免噪声淹没） |

> 恢复只影响"报告多少条"，不影响语义：**只要有任何 `E2xxx`，就不得进入类型检查**（AST 可能残缺，继续检查只会产生假错误）。

**顶层语句的判定与收集**（在进入"解析函数声明"之前，`top_is_stmt()` 按形态判定）。入口文件允许顶层语句（文稿 9.4），所以这里不报错，而是**按顺序收集到 `top_stmts`，文件解析完包成一个 `implicit: true` 的 `int main()`**；是否允许由 sema 判（被 `import` 的文件里报 `E3024`）。判定靠"跳类型前缀再看后一个 token"：`类型 名 (` 是函数声明，其余是语句。

| 形态 | 处理 |
|---|---|
| 以 `if`/`for`/`match`/`return`/`break`/`continue`/`defer`/`throw`/`try`/`var` 开头 | 语句，收集 |
| `Name(` / `Name.x` / `x = 1` / `x++` / `x += 1` | 语句，收集 |
| `类型 名 =`（含 `int[] xs =`、`T? x =`、`map<K, V> m =`、`a.b.C v =`） | 局部变量声明，收集 |
| `类型 名 (` | 函数声明，走 `parse_func` |
| 带可见性修饰词的语句（`public int x = 0`） | `E3024`（想声明模块级变量；语句不可能有可见性） |

包出来的隐式 `main`：`ret = int`、无参、`tok` 取第一条顶层语句的 token（诊断指回用户写的行），追加在 `items` 末尾——**sema 与后端不感知"隐式"之外的任何差别**，它就是普通函数。

---

## 5. AST 规范

### 5.1 节点清单（N 层结构，`--dump=ast` 的序列化顺序即此顺序）

**类型节点 `TypeAst`**

| 节点 | 字段 | 例 |
|---|---|---|
| `Named` | `path: Vec<Ident>`, `args: Vec<TypeAst>` | `map<string, object>`、`Order.Line` |
| `Array` | `elem: Box<TypeAst>`, `len: Option<ExprId>` | `int[]`、`int[4]` |
| `Nullable` | `inner: Box<TypeAst>` | `User?` |
| `Func` | `params: Vec<TypeAst>`, `ret: Box<TypeAst>` | `func(int, int) int` |
| `Tuple` | `elems: Vec<TypeAst>`（≥2） | `(int, string)` |
| `Borrow` | `mutable: bool`, `inner: Box<TypeAst>` | `&int[]`、`&const int[]` |

**声明节点**

| 节点 | 字段 |
|---|---|
| `Module` | `items: Vec<ItemId>` |
| `Import` | `path: Vec<Ident>`, `alias: Option<Ident>`, `names: Option<Vec<Ident>>` |
| `ConstDecl` | `ty: Option<TypeAst>`, `name`, `init: ExprId`, `vis: Vis` |
| `FuncDecl` | `vis`, `ret: TypeAst`, `name`, `generics: Vec<TypeParamAst>`, `params: Vec<ParamAst>`, `body: BlockId`, `attrs: Vec<Attr>` |
| `ClassDecl` | `vis`, `name`, `generics`, `parent: Option<TypeAst>`, `impls: Vec<TypeAst>`, `members: Vec<MemberId>` |
| `StructDecl` | `vis`, `name`, `generics`, `members`, `nest_in: Option<TypeId>` |
| `InterfaceDecl` | `vis`, `name`, `extends: Vec<TypeAst>`, `methods: Vec<MethodSig>` |
| `EnumDecl` | `vis`, `name`, `variants: Vec<Ident>` |
| `TypeAlias` | `vis`, `name`, `generics`, `target: TypeAst` |
| `Ctor` | `params`, `body: BlockId` |
| `Field` | `vis: Option<Vis>`, `ty: TypeAst`, `name` |
| `Method` | `vis: Option<Vis>`, `is_static: bool`, `is_override: bool`, `ret: TypeAst`, `name`, `params`, `body` |
| `MethodSig` | `ret: TypeAst`, `name`, `params`, `default_body: Option<BlockId>` |
| `ParamAst` | `borrow: Option<BorrowKind>`, `ty: TypeAst`, `name` |

**语句节点**

| 节点 | 字段 |
|---|---|
| `Block` | `stmts: Vec<StmtId>` |
| `VarDecl` | `explicit_ty: Option<TypeAst>`, `is_var: bool`, `name`, `init: ExprId` |
| `VarTuple` | `names: Vec<Ident>`, `init: ExprId` |
| `Simple` | `lhs: ExprId`, `op: Option<AssignOp>`, `rhs: Option<ExprId>`, `incdec: Option<IncDec>` |
| `IfStmt` | 复用 `IfExpr` |
| `ForStmt` | `label: Option<Ident>`, `kind: ForKind`, `body: BlockId` |
| `ForKind` | `Range{var, lo, hi, inclusive}` \| `Iter{var, iterable}` \| `KeyValue{k, v, iterable}` \| `Cond`（`for { }`） |
| `MatchStmt` | 复用 `MatchExpr` |
| `Return` | `value: Option<ExprId>` |
| `Throw` | `variant: Ident`, `msg: ExprId` |
| `Defer` | `call: ExprId`（必须是 `Call` 或 `MethodCall`） |
| `Break` / `Continue` | `label: Option<Ident>` |

**表达式节点**

| 节点 | 字段 | 备注 |
|---|---|---|
| `IntLit` | `value: u128`, `raw: String` | 保留 raw 供范围诊断与 `-min` 合成 |
| `FloatLit` | `value: f64`, `raw` | |
| `DecimalLit` | `value: Decimal`, `raw` | 十进制系数 + 标度，不用 f64 |
| `BoolLit` / `NullLit` | — | |
| `StringLit` | `parts: Vec<TemplatePart>` | `Text(String)` \| `Interp{expr, format: Option<u8>}` |
| `Path` | `segs: Vec<Ident>`, `args: Vec<TypeAst>` | `io.ReadLine`、`Status.Ok`、`T` |
| `This` / `Parent` | — | |
| `TupleLit` | `elems: Vec<ExprId>` | |
| `ArrayLit` | `elems: Vec<ExprId>` | `[]` 合法（需目标类型） |
| `BraceLit` | `pairs: Vec<(String, ExprId)>` | 键已在词法确定 |
| `Call` | `callee: ExprId`, `args: Vec<ArgAst>` | 转换/构造/普通调用共用 |
| `MethodCall` | `recv: ExprId`, `name`, `args` | |
| `FieldAccess` | `recv`, `name` | |
| `SafeAccess` | `recv`, `name` | `?.` |
| `Index` | `recv`, `index` | `[]` |
| `Unary` | `op`, `operand` | `! - ~` |
| `Binary` | `op`, `lhs`, `rhs` | |
| `Ternary` | `cond`, `then`, `els` | |
| `Coalesce` | `lhs`, `rhs` | `??` |
| `Range` | `lo`, `hi`, `inclusive` | 只允许出现在 `for` 头 / `.Step()` 实参 |
| `Try` | `inner` | 后缀 `try` |
| `New` | `ty: Vec<Ident>`, `args` | |
| `IfExpr` | `cond`, `then: BlockId`, `els: Option<Either<IfExpr, BlockId>>` | |
| `MatchExpr` | `scrutinee`, `arms: Vec<ArmAst>` | `ArmAst{pat: PatAst, body: ExprId}` |
| `AnonFunc` | `params`, `body: BlockId` | |
| `ErrorCtor` | `variant: Ident`, `msg: ExprId` | `error.NotFound("...")` |
| `ArgAst` | `kind: Value(ExprId) \| Borrow(ExprId) \| BorrowConst(ExprId)` | |

**模式节点 `PatAst`**

| 节点 | 字段 |
|---|---|
| `Lit` | `expr: ExprId`（必须是字面量） |
| `RangePat` | `lo`, `hi`, `inclusive` |
| `EnumMember` | `ty: Vec<Ident>`, `variant` |
| `ResultPat` | `is_ok: bool`, `bind: Ident` |
| `TypePat` | `ty: TypeAst`, `bind: Ident` |
| `Wild` | — |
| `Bind` | `name: Ident` |

### 5.2 每个节点必须携带的语义槽（sema 填写，codegen 只读）

| 槽 | 挂在 | 内容 |
|---|---|---|
| `ty` | 所有表达式 | 静态类型（`TypeRef`） |
| `value_kind` | 所有表达式 | `RValue` \| `Place`（可写位置） \| `Func`（函数值） \| `Type`（类型名） |
| `const_val` | 常量表达式 | `Option<ConstVal>` |
| `resolved` | `Path` | `Res::Local(idx)` \| `Res::Param(idx)` \| `Res::Func(sym)` \| `Res::Type(def)` \| `Res::EnumVariant` \| `Res::Module` \| `Res::Intrinsic(IntrinsicId)` |
| `conv` | `Call` / 赋值 / 实参 / 返回 | `ConvKind`: `None` \| `NumericCast` \| `DownCast` \| `Box` \| `Unbox` \| `RecordToStruct` \| `NullWrap` |
| `place` | `Place` 表达式 | `PlaceKind`: `Local` \| `Param` \| `Field(path)` \| `Index` \| `DerefBox` |
| `move` | 赋值/实参/返回 | `bool`：此处置为 move（引用类型）还是 copy |
| `drop_at_scope_end` | 局部变量 / 临时值 | `bool`：是否需要生成释放调用 |

### 5.3 dump 格式（M1/M2 验收用）

```
MODULE goods
  IMPORT db
  CLASS Goods vis=public
    FIELD id vis=public ty=int
    CTOR params=[(int,id),(string,name),(decimal,price),(int,stock)]
      THIS_FIELD id = PARAM id
      ...
    METHOD load is_static vis=public ret=result<Goods?>
      VAR row ty=<infer> init=TRY(CALL(db.query) args=[STR,d])
```

要求：字段顺序稳定、类型打印用规范名（`result<Goods?>`、`int[]`、`map<string,object>`）、缩进 2 空格。

---

## 6. 名称解析与作用域

### 6.1 作用域链（N-1…N-4）

| 编号 | 规则 |
|---|---|
| `N-1` | 作用域种类（由内到外）：`Block` → `Func`（参数 + 函数体顶层） → `Type`（成员） → `Module` → `Root`（内建类型与 intrinsic） |
| `N-2` | 声明在该声明语句**之后**生效；`for` 的循环变量只在循环体内生效（文稿 5.2）；`match` 的绑定名只在该 arm 内生效 |
| `N-3` | **禁止遮蔽**（文稿 3.3）：新声明名与作用域链上任一**变量名/参数名/循环变量名/match 绑定名**重名 → `E3001`。字段不受此限（必须 `this.x` 访问） |
| `N-4` | 同作用域重复声明 → `E3004` |

允许的重复：不同类型体内同名成员（不冲突）、不同模块同名符号（靠 import 别名区分）、`class` 成员与局部变量同名（因为是 `this.x` vs `x`，不构成遮蔽）。最后一条是 `GAP-6` 的显式收口。

### 6.2 声明与使用顺序（N-5…N-7）

| 编号 | 规则 |
|---|---|
| `N-5` | 模块内符号**先定义后使用，或同模块内任意顺序**（编译器先收集全部顶层声明建立符号表，再检查函数体）——文稿 1.2 的"先定义后使用"约束的是**跨模块**：`import` 才能用到别的模块 |
| `N-6` | 类/struct 成员互相引用不要求顺序（字段类型可指向本类之后声明的嵌套类型） |
| `N-7` | 顶层声明之间允许前向引用（函数 A 调用下方函数 B），因为签名已完整收集；**类型必须完整**：字段类型引用未定义类型 → `E3003` |

### 6.3 可见性（N-8…N-13）

| 编号 | 规则 |
|---|---|
| `N-8` | 模块级声明（函数、`const`、`class`/`struct`/`interface`/`enum`/`type`）：不写 = 仅本模块；`public` = 跨模块可见 |
| `N-9` | 类成员：不写 = 仅本类内；`protected` = 本类 + 所有子类（含跨模块子类）；`public` = 所有人 |
| `N-10` | `struct` 成员**不得写任何可见性修饰词**（写 `public`/`protected` 都是 `E3011`），一律 public |
| `N-11` | `constructor` **不得写 `public`**（`E3011`），默认 public；且不能写 `protected`/`static`/`override` |
| `N-12` | `protected` 只能出现在类成员上；模块级声明、struct 成员、interface 成员上 → `E3014` |
| `N-13` | 访问检查在**使用点**进行：`a.energy`（protected）在类外 → `E3013`；`u.from_email(...)`（经实例调静态）→ `E3013` 且消息指明"静态方法只能用 `类名.方法()`" |

### 6.4 命名空间与重名（N-14…N-16）

| 编号 | 规则 |
|---|---|
| `N-14` | **类型名与函数名共用一个命名空间**（文稿 6.5）：同模块内 `class Point` 与 `int Point(...)` 并存 → `E3018`。同理 `type Foo = int` 与 `class Foo` |
| `N-15` | 用户类型名不得与保留类型名（`int`、`object`、`result`…）重名 → `E3018` |
| `N-16` | 局部变量/参数与同模块**类型名**同名：允许，但若该标识符出现在 `Call` 位置被解析为类型时，局部变量遮蔽会让 `T(x)` 语义二义 → 一律 `E3018`（消息：「变量与类型同名，`T(x)` 无法判定是转换还是调用」）。这是 `GAP-7`（比文稿更严格，只影响可读性不影响表达力） |

### 6.5 内建命名空间（N-17…N-19）

根模块（`Root`）预置以下符号，**无需 import、不可被 import 覆盖、不可被模块内声明覆盖**：

| 类别 | 符号 |
|---|---|
| 类型 | 见 §3.4 的保留类型名表（含 `result`/`error`/`func`/`map`/`Self`/`range`/`Rounding`/`Arena`/`ArenaRef`） |
| 值 | `Print` / `Println`（`void Print(string)`）、`true`/`false`/`null`（关键字） |
| 类型静态命名空间 | `int.Parse` `float.Parse` `decimal.Parse` `int32.Truncate`（及 `int8/int16` 的 `.Truncate`）`string.FromCodepoint` |
| 枚举成员 | `Rounding.HalfUp` `Rounding.HalfEven` `Rounding.Down` |
| 错误变体 | `error.NotFound` `error.Invalid` `error.Money` `error.IO` |

| 编号 | 规则 |
|---|---|
| `N-17` | 内建符号与 intrinsic 绑定见 §15；模块内声明同名符号 → `E3018` |
| `N-18` | **枚举成员必须带类型名限定**（文稿 5.3）：裸写 `Ok`/`HalfUp` → `E6003`。即使只有一个 enum 有该成员也不放宽 |
| `N-19` | `Self` 只在 class/struct/interface 体内有效；出现在函数签名之外的模块级位置 → `E4039` |

### 6.6 `T(x)` 决议算法（N-20，必须逐条按序执行）

```
resolve_call(Path(p), args):
  1. if p 是单个标识符且解析为 Res::Type(D):
       a. D 是内建标量类型 → NumericCast / 见 C-1..C-20
       b. D 是 class       → 若是 new 语境 → 构造；否则 → 向下转型（C-14）
       c. D 是 struct      → 按 constructor 实参匹配（唯一签名）→ 构造
                             否则若 args.Len()==1 且实参是 record 且字段全匹配 → RecordToStruct（C-15）
                             否则 E4031
       d. D 是 enum/interface/type alias → 见 C 表；不允许则 E5001
  2. else if p 解析为 Res::Func / Res::Intrinsic → 普通调用
  3. else if p 解析为 Res::Module → 模块访问（必须后接 `.`，否则 E3013）
  4. else 未定义 → E3003
```

**二义判定（`E4032`）**：`Point(1, 2)` 同时满足"constructor 匹配"和"单实参 record 全匹配"时——只有 `args.Len()==1` 才可能二者都成立；此时若实参类型是 record 且其字段与该 struct 全匹配，则报 `E4032`，消息要求"先把 record 落到中间变量，再显式写意图"。**不得自行选一个**。

---

## 7. 类型系统

### 7.1 类型表示（`TypeRef`）

```rust
enum TypeRef {
  Bool,
  Int { bits: u8, signed: bool },     // int=64/true, uint=64/false, int8..uint32
  Float,                              // 恒 64 位
  Decimal,
  String,
  Void,
  Object,
  Array { elem: Rc<TypeRef>, len: Option<u64> },   // None = T[]
  Map { key: Rc<TypeRef>, val: Rc<TypeRef> },
  Tuple(Vec<TypeRef>),
  Func { params: Vec<TypeRef>, ret: Rc<TypeRef> },
  Nullable(Rc<TypeRef>),
  Record(Rc<RecordTy>),               // 有序字段 [(name, TypeRef)]
  Named { def: TypeDefId, args: Vec<TypeRef> },   // class / struct / interface / enum / alias(已展开)
  Enum(TypeDefId),
  Result(Rc<TypeRef>),
  Error,
  Range,
  Arena,
  ArenaRef(Rc<TypeRef>),
  Param(u32),                         // 泛型形参，仅单态化前存在
  Never,                              // 仅用于 throw / 必离开表达式
}
```

`type` 别名在解析期展开（`N-21`）：`type Json = map<string, object>` 之后 `Json` 与 `map<string, object>` 是**同一个 `TypeRef`**，不产生新类型、不参与重载。

### 7.2 类型等价（T-1…T-4）

| 编号 | 规则 |
|---|---|
| `T-1` | 结构等价用于：`Array`（元素 + 长度）、`Map`、`Tuple`、`Func`、`Nullable`、`Record`（字段名序列 + 每个类型） |
| `T-2` | 名义等价用于：`Named`（同一 `TypeDefId` + 相同实参列表）、`Enum` |
| `T-3` | `Record` **不与任何 `Named(struct)` 等价**（哪怕字段全同）：只能通过 `C-15` 显式转换 |
| `T-4` | 泛型实参必须**精确相等**（不变型，无协变/逆变）；`Box<Dog>` 不是 `Box<Animal>` |

### 7.3 可赋值性（T-5，唯一入口）

```
assignable(src, dst, ctx) -> Yes | NeedsConv(ConvKind) | No(reason):
  1. src == dst                                   → Yes
  2. dst = Nullable(D):
       src = Never                                → Yes
       src = Nullable(S) && assignable(S, D)      → Yes
       src = null literal                         → Yes
       assignable(src, D)                         → Yes   (隐式包裹)
  3. src = Nullable(S), dst 非可空:
       assignable(S, dst)                         → No(E4xxx 需判空)
  4. dst = Object:
       boxable(src)                               → NeedsConv(Box)
  5. src = Object, dst 具体类型 D:
       ctx 允许自动解箱 (P1..P8) 且 unboxable(D)   → NeedsConv(Unbox)
       否则                                        → No(E5002)
  6. dst = Named(interface I), src = Named(class C) 且 C 实现 I → Yes
  7. dst = Named(class P), src = Named(class C) 且 C 的祖先链含 P → Yes
  8. dst = Named(class C), src = Named(class P 或 interface) → NeedsConv(DownCast)（仅 `T(x)` 语境）
  9. dst = Record, src = Record 且字段全同 → Yes
 10. 其他 → No
```

| 编号 | 规则 |
|---|---|
| `T-5` | 任何赋值/实参/返回/字段初始化/元素写入/`??` 右侧，必须走 `assignable`，不得有第二条路径 |
| `T-6` | **装箱集合**（`boxable`）：`bool`、`int`/`uint` 家族、`float`、`decimal`、`string`、`T[]`、`map<K,V>`、`class` 实例。**不可装箱**：`struct`、`record`、`tuple`、`func`、`enum`、`range`、`ArenaRef`、`void` → `E4029` |
| `T-7` | `unboxable(D)`：`bool`、数值类型、`string`、`T[]`、`map`、class 实例、`D?`（含 `null` 情形）。`struct`/`record`/`func`/`enum` 不是合法解箱目标 → `E5002` |
| `T-8` | 无任何隐式数值转换（文稿 3.1）：整数家族之间、整数↔浮点、整数↔`decimal`、`float`↔`decimal` 都必须 `T(x)` → `E4003` |
| `T-9` | 字面量按目标类型落地：`IntLit` 赋给任意整数类型时，若值在范围内 → 该类型（不产生运行期转换）；越界 → `E4003`（消息含范围）。`FloatLit` 只可赋给 `float`；`DecimalLit` 只可赋给 `decimal`。负号落到无符号目标 → `E4003`。**目标为 `T?` 时先剥一层可空再判**（`decimal? d = 12.50d` 合法）——可不可空与字面量能否落地是两件事 |

### 7.4 自动解箱的八个位置（T-10，穷举）

只有以下位置允许 `object` 直接当目标类型用（文稿 6.7）：

| 位置 | 说明 |
|---|---|
| `P1` | 变量声明/赋值的左侧（`int n = o`） |
| `P2` | 函数实参（对应形参类型已知） |
| `P3` | `return`（函数返回类型已知） |
| `P4` | `if` / `for`-条件（目标类型 `bool`） |
| `P5` | 数组元素写入（`nums[i] = o`，元素类型 `T`） |
| `P6` | map 值写入（`m[k] = o`，值类型 `V`） |
| `P7` | struct/record 字段初始化（`Point(o, 2)` 的 `o` 对应 `int`） |
| `P8` | 复合字面量元素 / 元组元素（目标类型由外层声明给出） |

不在上表的位置**一律 `E5002`**：`o + 1`、`o.Len()`、`o == 1`、`f<T>(o)`（无法确定 `T`）都报错，消息建议"先把 `o` 落到具体类型的变量，或写 `int(o)`"。

### 7.5 类型推断（T-11…T-18）

| 编号 | 规则 |
|---|---|
| `T-11` | `var x = e`：`x` 的类型 = `e` 的类型。`e` 是 `NullLit` → `E4007`；`e` 是 `[]` → `E4007`；`e` 是 `{}` → `E4007`；`e` 类型为 `Void`/`Range` → `E4007` |
| `T-12` | `const N = e`（省略类型）：同 `T-11`，但 `e` 必须是常量表达式（§8） |
| `T-13` | `var` 只允许用于局部变量（文稿 3.3）：字段、参数、返回、`const` 模块级 → `E3006` |
| `T-14` | `BraceLit` 推断（`T-31`）：全部值类型 **等价** → `map<string, V>`；否则 → `Record{有序字段}`。空 `{}` 依赖目标类型，无目标 → `E4007` |
| `T-15` | `ArrayLit` 推断：`[a, b, c]` 所有元素必须可赋给公共类型 `T`（首元素类型，其余必须 `assignable(_, T)`，否则 `E4001`）；空 `[]` 必须由目标类型给出元素类型 |
| `T-16` | 泛型实参推断：对每个 `Param(i)` 收集约束（实参类型、返回目标类型），求唯一解；有多个候选或零候选 → `E4037`；推断结果必须满足约束（`E4036`） |
| `T-17` | `??` 结果类型 = `lhs` 解包后的类型（`T?`→`T`，`object?`→`object`）；`rhs` 必须 `assignable(rhs, 结果类型)`，否则 `E4015` |
| `T-18` | `match` / 三元 / `if` 表达式：所有分支类型必须**等价**（不做合流），否则 `E4013`/`E4014`；`Never` 可与任意类型共存（`throw` 分支） |

### 7.6 转换矩阵（C-1…C-20，唯一真相）

| 编号 | 源 → 目标 | 行为 | 运行期 |
|---|---|---|---|
| `C-1` | `int`/`uint` 家族互转 | 允许 | 值超目标范围 → **R107**（不截断）；要截断写 `int32.Truncate(x)` |
| `C-2` | 整数 → `float` | 允许 | 常量且 > 2^53 → `W1008` |
| `C-3` | 整数 → `decimal` | 允许 | 无损 |
| `C-4` | `decimal` → 整数 | 允许 | 丢小数（向零截断） |
| `C-5` | `decimal` → `float` | 允许 | `W1009` |
| `C-6` | `float` → `decimal` | **禁止**（`E5001`） | 必须 `decimal.Parse(string)` |
| `C-7` | `float` → 整数 | 允许 | 向零截断 |
| `C-8` | 任何 → `string` | **禁止**（`E5001`） | 用 `x.ToString()`；码点用 `string.FromCodepoint` |
| `C-9` | `string` → 数值 | **禁止**（`E5001`） | 用 `int.Parse`/`float.Parse`/`decimal.Parse`（返回 `result<T>`） |
| `C-10` | 任何 ↔ `bool` | **禁止**（`E5001`） | 只有 `bool` 自身可用 |
| `C-11` | 派生类 → 父类 / interface | 隐式（`Yes`） | 无 |
| `C-12` | 可装箱类型 → `object` | 隐式（`Box`） | 堆分配盒子 |
| `C-13` | `object` → 具体类型 | 解箱（`Unbox`） | 标签不等 → **R107**；`T?` 目标遇 `null` → `null` |
| `C-14` | 父类/interface → 派生类 | 断言式（`DownCast`） | 不是该类型 → **R107** |
| `C-15` | `record` → 命名 `struct` | 允许 | **编译期**校验字段名与类型全匹配，否则 `E4034` |
| `C-16` | `T[N]` → `T[]` | **禁止**（`E4001`） | `GAP-5`：需显式逐元素复制 |
| `C-17` | `enum` ↔ 整数 | **禁止**（`E4019`） | 序列化用 `x.ToString()`/`match` |
| `C-18` | `struct` → `object` | **禁止**（`E4029`） | 先改成 class |
| `C-19` | 元组 / `func` / `range` → 任何 | **禁止**（`E5001`） | — |
| `C-20` | 数值 → 同宽度（幂等） | 允许（恒等） | 无 |

**`T(x)` 的实参个数**：`C-1..C-14`、`C-17` 都是**一元**；`C-15` 也是一元（实参是 record）。`Point(1,2)` 是构造不是转换（`N-20`）。

### 7.7 可空与流敏感收窄（T-19…T-24）

| 编号 | 规则 |
|---|---|
| `T-19` | `null` 只属于 `T?`；`null` 字面量在无目标类型时是 `E4007` |
| `T-20` | 非可空类型上使用 `?.` → `E4022`；可空类型上使用 `.` → `E4022`（消息："`u` 可能为空，先判空或用 `?.`"） |
| `T-21` | **收窄只对"局部变量/参数"生效**：`this.x`、`a.f`、`m["k"]`、`nums[i]` 的收窄不成立（外部可改），必须重新判空。`GAP-8` |
| `T-22` | 收窄状态表：每变量 `NonNull` / `MaybeNull`。赋值、`try` 展开、传 `&` 可变借用后**重置为声明时状态** |
| `T-23` | 收窄触发形态（穷举）：`if x != null { A }` → A 内 NonNull；`if x == null { B } else { C }` → C 内 NonNull；`if x == null { B }` 且 `B` **必定离开**（见 `T-24`）→ 之后 NonNull；`x != null && ...` 的右侧、`x == null \|\| ...` 的右侧 → NonNull。其他形态（`!(x == null)` 以外）不推断 |
| `T-24` | `must_exit(block)`：块的**所有路径**以 `return`/`throw`/`break`/`continue` 结束。判定：块内最后一条语句是上述之一 → true；最后一条是 `if` 且 `then` 与 `else` 都 `must_exit` → true；最后一条是 `match` 且每个 arm 都 `must_exit`（arm 体为 `throw`/`return` 调用）→ true；否则 false。**不做循环分析**（`for` 永不 `must_exit`） |

### 7.8 record 与 struct（T-25…T-30）

| 编号 | 规则 |
|---|---|
| `T-25` | record 是匿名 struct：值语义、赋值即深/浅复制（按 §8.3 规则）、字段键固定 |
| `T-26` | record 的键名在类型里，读写用 `["键"]`；键不存在、给 record 加键 → 编译错误 `E4008`（消息区分"键拼错"与"record 键固定，要加键请用 map"） |
| `T-27` | record 不可作函数参数/返回类型（用户写不出名字）→ 若试图传出，`E4034` 并建议写命名 `struct` |
| `T-28` | record 不能装箱到 `object`（`E4029`），不能 `new`，不能继承 |
| `T-29` | `struct` 字段类型白名单：数值、`bool`、`decimal`、`string`、`enum`、其他 `struct`、`T[N]`、`T[]`、`map`。**禁止 `class` 实例引用**（含 `Dog[]`、`map<string, Dog>` 的元素）→ `E3023` |
| `T-30` | `struct` 不可递归包含自身（直接或经其他 struct）→ `E3022`；经 `T[]`/`map` 绕过的递归是允许的（但实现需先检查：`struct A { A[] xs }` 合法） |

### 7.9 泛型（T-31…T-36）

| 编号 | 规则 |
|---|---|
| `T-31` | 约束只能是 interface 名（`Comparable`/`Hashable`/`Printable`/用户 interface）；写内建非 interface 类型 → `E4036` |
| `T-32` | 约束内的运算符检查：`a > b` 要求 `T: Comparable`；`{x}` 插值要求 `T: Printable`；`m[t]` 作键要求 `T: Hashable + Comparable` |
| `T-33` | `float` 不实现 `Comparable`/`Hashable`（文稿 7.1）：`max<float>` → `E4036` |
| `T-34` | 单态化：每个**去重后的实参组合**生成一份代码；递归单态化（`f<T>` 内 `f<T[]>`）深度 > 64 → `E4038`（防止无限膨胀） |
| `T-35` | 泛型类的 static 方法不得引用类型参数 → `E4036` |
| `T-36` | 类型参数不得遮蔽外层类型参数名 → `E3001` |

### 7.10 元组（T-37…T-39）

| 编号 | 规则 |
|---|---|
| `T-37` | 元组只用于"同时返回多个正常值"；不参与遍历、不可下标、不可 `==`（除逐元素比较需手写）→ `E4020` |
| `T-38` | 解构唯一形式 `var (a, b) = t`；元素个数必须相等 → `E4027`；`t` 必须是元组 |
| `T-39` | 元组不是 `copy` 类型清单的成员但有值语义（按元素复制）——与 `struct` 同档处理 |

---

## 8. 常量求值

### 8.1 必须在编译期求值的场合（S-1）

| 场合 | 要求 |
|---|---|
| `const` 初始化器 | 必须常量（§8.2） |
| 定长数组长度 `T[N]` | 必须是 `int` 常量，`N > 0` 且 `N <= 2^31-1`；否则 `E4004` |
| 数组常量下标 `a[k]` | 必须常量，且 `0 <= k < N`；否则 `E4005`（`T[]` 的常量下标**不**做此检查，走运行期） |
| `match` 的区间/字面量模式 | 必须常量（且同 arm 内 low ≤ high） |
| 字面量范围检查 | 按目标类型在编译期核对（`T-9`） |
| `@os` / `@arch` 选择 | 按目标平台常量选择（§17 `GAP-9`） |
| 移位量 | 若为常量：`0 <= n < 位宽`，否则 `E4012` |

### 8.2 常量表达式集合（S-2，穷举）

允许：字面量、`const` 引用、`true`/`false`、`null`、字符串字面量（无插值）、**不带插值**的模板字符串、上述值的算术/位/比较/逻辑/三元/`??` 运算、`T(x)` 数值转换、枚举成员、由常量构成的数组字面量与 record 字面量、`s.Len()` / `s.ByteLen()`（常量字符串上）、`x.ToString()`（常量数值上）。

不允许（→ `E3xxx 非常量表达式`）：函数调用（除 `S-3` 的纯函数例外）、`try`、`new`、`match`、`if` 表达式、含插值的字符串、`At()`/`Get()`。

### 8.3 常量求值的溢出行为（S-3）

| 编号 | 规则 |
|---|---|
| `S-3` | 常量求值遇到整数溢出、除零、`decimal` 除不尽、移位越界、范围越界 → **编译错误**（`E4003`/`E4009`/`E4012`），而不是延迟到运行期。理由：运行期崩溃的前提是"值在运行期才知道" |
| `S-4` | 常量求值必须用**任意精度**中间值（`i128`/`u128`/十进制大数），最后按目标类型做范围检查——否则 `const BIG = 2^63` 会在中间步骤错误溢出 |
| `S-5` | `-9223372036854775808` 的解析：一元负号紧贴整数字面量时整体合成带符号字面量，范围检查针对合成值（文稿 3.1）。实现要求：`Unary{Neg, IntLit}` 在**常量求值阶段**先尝试 `-(value)` 于 `i128`，再检查目标类型范围 |
| `S-6` | `decimal` 常量必须用十进制表示（系数 + 标度），不得经 `f64` 中转——`19.99d` 的系数是 1999、标度 2 |

### 8.4 `ConstVal`

```rust
enum ConstVal {
  Bool(bool), Int { v: i128, bits: u8, signed: bool },
  Float(f64), Dec { coeff: i128, scale: u32 },
  Str(Rc<str>), Enum { ty: TypeDefId, idx: u32 },
  Array(Vec<ConstVal>), Record(Vec<(Rc<str>, ConstVal)>), Null,
}
```

`--dump=types` 必须能打印每个 `const` 的 `ConstVal`，用于 M3 验收。

---

## 9. 语义检查：诊断码全集

> 每个码必须能指回唯一规则编号；测试用例（§16）按码断言。**新增码必须同时在此表登记**，否则视为实现缺陷。

### 9.1 词法 `E1xxx`

| 码 | 触发 | 规则 |
|---|---|---|
| `E1001` | 前导零十进制（`010`） | `L-8` |
| `E1002` | 单引号字面量 | `L-14` |
| `E1003` | 未闭合字符串 / 未闭合块注释 | `L-15` `L-20` |
| `E1004` | 非法转义序列 | `L-13` |
| `E1005` | 数值字面量形状错误 | `L-6` `L-7` `L-9` |
| `E1006` | 非法字符 / 非法 UTF-8 / `\0` | `L-1` `L-3` |
| `E1007` | `\u{}` 非法码点 | `L-13` |
| `E1008` | 插值表达式为空 / 非法格式说明符 | `L-18` |

### 9.2 语法 `E2xxx`

| 码 | 触发 | 规则 |
|---|---|---|
| `E2001` | 期望某 token 未出现（含关键字当标识符） | `G-1` `G-7` |
| `E2002` | 语句未以换行/分号结束（同一行两条语句缺 `;`） | `G-2` |
| `E2003` | 行首运算符（上一行已完整） | `G-5` |
| `E2004` | 分支体未使用 `{}` | `G-1` |
| `E2005` | `if` 表达式缺 `else` | `G-1` |
| `E2006` | 非 `void` 函数存在路径无 `return`（仅 `main` 豁免） | 文稿 9.4 |
| `E2007` | `match` arm 缺 `=>` / 缺逗号 / 体不是表达式 | `G-1` |
| `E2008` | 匿名函数与括号表达式无法消解（试解析后仍失败） | `G-10` |
| `E2009` | `try` 后接 `.` 或 `[` | 文稿 10.1 |
| `E2010` | `++`/`--` 出现在表达式内，或使用前缀形式 `++i`/`--i` | 文稿 11.1 |
| `E2011` | 函数声明处出现非预期 token（如 `Name(` 之类的残片）；顶层语句本身合法，不再用此码 | 文稿 9.1 |
| `E2012` | `new` 后不是 class 类型 / `Arena` | `G-9` |
| `E2013` | `defer` 后不是一次调用 | 文稿 5.4 |
| `E2014` | `break`/`continue` 的标签不存在或不是循环标签 | 文稿 5.2 |
| `E2015` | 语法错误恢复：单次编译超过 20 条语法错误 | `G-16` |

### 9.3 声明 / 作用域 / 可见性 / 模块 `E3xxx`

| 码 | 触发 | 规则 |
|---|---|---|
| `E3001` | 变量遮蔽 | `N-3` |
| `E3002` | 变量声明无初值（字段除外） | 文稿 3.3 |
| `E3003` | 未定义标识符 / 未定义类型 | `N-5` `N-7` |
| `E3004` | 同作用域重复声明 | `N-4` |
| `E3005` | `const` 重赋值 | 文稿 3.3 |
| `E3006` | `var` 出现在字段/参数/返回/模块级 | `T-13` |
| `E3007` | 字段带初值 | 文稿 3.3 `G-1` |
| `E3008` | 有字段但无 `constructor` | 文稿 6.1 |
| `E3009` | `constructor` 未给某字段赋值 | 文稿 6.1 |
| `E3010` | 多个 `constructor` | 文稿 6.1 |
| `E3011` | `constructor` 写 `public`；struct 成员写可见性；interface 成员写可见性 | `N-10` `N-11` |
| `E3012` | 修饰符顺序错误 | 文稿 6.1 |
| `E3013` | 可见性不足（含经实例调静态方法） | `N-13` |
| `E3014` | `protected` 用在模块级/struct/interface | `N-12` |
| `E3015` | 覆写收窄可见性 | 文稿 6.2 |
| `E3016` | `override` 无对应父类或 interface 方法 | 文稿 6.2 |
| `E3017` | 子类同名方法未标 `override`（意外遮蔽） | 文稿 6.2 |
| `E3018` | 类型名与函数名重名 / 与内建类型重名 / 变量与类型同名 | `N-14`…`N-16` |
| `E3019` | 未实现 interface 的必须方法 | 文稿 6.3 |
| `E3020` | `extend` 多个父类 | 文稿 6.2 |
| `E3021` | `struct` 使用 `extend` / `implement` | 文稿 6.5 |
| `E3022` | `struct` 递归内联包含自身 | `T-30` |
| `E3023` | `struct` 字段含 class 引用（含容器元素） | `T-29` |
| `E3024` | 被 `import` 的文件出现顶层语句；或顶层语句带可见性修饰词（`public int x = 0`） | 文稿 9.4 |
| `E3025` | 无 `main` / 多个 `main` / `main` 签名不符 | 文稿 9.4 |
| `E3026` | 模块循环导入 | 文稿 9.2 |
| `E3027` | 导入的符号不存在或未导出 | 文稿 9.2 |
| `E3028` | `@os`/`@arch` 同组命中数 ≠ 1 | 文稿 9.3 |
| `E3029` | 未知属性 / 属性参数非法 | `L-23` |
| `E3030` | `as` 用于 import 以外的地方 | 文稿 9.2 |
| `E3031` | enum 成员带初值/载荷 | 文稿 6.4 |
| `E3032` | interface 含字段或 `constructor` | 文稿 6.3 |
| `E3034` | `this`/`parent` 用在非法位置 | `G-8` |
| `E3035` | `parent(...)` 不在 constructor 首条语句 | 文稿 6.2 |
| `E3036` | `static` 或 `override` 用于字段；`static` 与 `override` 同时出现 | 文稿 6.1 |
| `E3037` | 类成员（字段 / 方法 / 嵌套类型）缺少可见性修饰符 | 文稿 6.1 |
| `E3038` | 静态方法体内使用 `this` | 文稿 6.1 |
| `E3039` | 用户模块名与内建模块重名（`db`/`file`/`io`/`json`/`math`/`string`/`time`），同名文件不会被加载 | 文稿 9.2 |
| `E3040` | 同一文件里两条 `import` 抢同一个名字：末段短名（`net.http` 与 `web.http` 都是 `http`）或显式别名重复。判定在视图表插入处，报在第二条 `import` 上 | 文稿 9.2 |
| `E3041` | 入口文件同时有显式 `main` 与顶层语句（顶层语句会被包成隐式 `main`），两处入口冲突 | 文稿 9.4 |
| `E3042` | 裸类型名歧义：两个已导入模块都导出同名 `public` 类型（`import a` + `import b`，都有 `Product`）。同一行（类型标注 + `new`）只报一次 | 文稿 9.2 |

### 9.4 类型与表达式 `E4xxx`

| 码 | 触发 | 规则 |
|---|---|---|
| `E4001` | 类型不匹配（赋值/实参/返回/元素/字段） | `T-5` |
| `E4002` | 条件不是 `bool` | 文稿 5.1 |
| `E4003` | 需要显式转换 / 字面量超范围 | `T-8` `T-9` |
| `E4004` | 数组长度非常量 / ≤0 / 超上限 | `S-1` |
| `E4005` | 定长数组常量下标越界 | `S-1` |
| `E4006` | 字符串下标 `s[i]` | 文稿 11.2 |
| `E4007` | 无法推断（`var` 遇 `null`/空字面量/void/range） | `T-11` `T-19` |
| `E4008` | record 键不存在 / 给 record 加键 | `T-26` |
| `E4009` | 算术操作数类型不允许 / 常量除零 | 文稿 11.1 `S-3` |
| `E4010` | `+` 两边不都是 `string` | 文稿 11.2 |
| `E4011` | `float` 使用 `==` / `!=` | 文稿 3.7 |
| `E4012` | 移位量为负 / 常量越界 / 非常量非整数 | 文稿 3.7 `S-1` |
| `E4013` | 三元两分支类型不一致 | 文稿 5.1 |
| `E4014` | `match` 分支类型不一致 | 文稿 5.3 |
| `E4015` | `??` 右侧不可赋给左侧解包类型 | `T-17` |
| `E4016` | 比较连写（`a < b < c`） | 文稿 11.1 |
| `E4017` | 比较类型不实现 `Comparable` | 文稿 7.1 |
| `E4018` | map 键类型不满足 `Hashable + Comparable`（含 `object`、`float`） | 文稿 3.3 `7.1` |
| `E4019` | enum 与整数互转 | 文稿 6.4 |
| `E4020` | 元组下标/字段访问/遍历/`==` | `T-37` |
| `E4021` | `range` 被当作值（赋值/传参/比较/返回） | 文稿 5.2 |
| `E4022` | `?`/`?.` 用在非可空值，或 `.` 用在可空值 | `T-20` |
| `E4023` | `void` 值被使用 | 文稿 4 |
| `E4024` | 函数值使用 `==` / `!=` | 文稿 4.1 |
| `E4025` | 遍历对象不可遍历（非数组/map/string，或 range 以外的值） | 文稿 5.2 |
| `E4026` | 遍历期间修改长度（`Push`/`Pop`/`Remove`/`Clear`） | 文稿 5.2 |
| `E4027` | 元组解构元素个数不符 | `T-38` |
| `E4028` | `object` 访问成员（`o.x`、`o[k]`、`o.Len()`） | 文稿 6.7 |
| `E4029` | 不可装箱类型 → `object` | `T-6` |
| `E4030` | 格式说明符 `.N` 用于非数值类型 | 文稿 11.2 |
| `E4031` | 构造/转换实参个数或类型不匹配 | `N-20` |
| `E4042` | class 无 constructor 却被 `new`（消息两行指路：改用类名调静态，或补 constructor） | `N-20` |
| `E4032` | `T(x)` 同时匹配构造与 record 转换 | `N-20` |
| `E4033` | 字面量键不是字符串字面量 | `GAP-1` |
| `E4034` | record → struct 字段不匹配；record 作参数/返回 | `C-15` `T-27` |
| `E4035` | 类型实参个数不符 | `G-1` |
| `E4036` | 泛型约束未满足 / 约束不是 interface / static 引用 `T` | `T-31`…`T-35` |
| `E4037` | 泛型实参无法推断 | `T-16` |
| `E4038` | 单态化深度超限 | `T-34` |
| `E4039` | `Self` 用在类型体外 | `N-19` |
| `E4040` | 函数值可空标注位置错误（`func(...) T?` vs `(func(...) T)?`） | 文稿 4.1 |
| `E4041` | 定长数组过大（元素数 × 元素大小 > 8 MiB，`GAP-10`） | 实现限制 |

### 9.5 转换与装箱 `E5xxx`

| 码 | 触发 | 规则 |
|---|---|---|
| `E5001` | 禁止的转换（`C-6`/`C-8`/`C-9`/`C-10`/`C-19`） | §7.6 |
| `E5002` | 解箱出现在 `P1..P8` 之外 / 解箱目标不是 class、内建值、数组、map、`T?` | `T-10` `T-7` |
| `E5003` | 向下转型的目标既不是 class 也不是 interface | `C-14` |
| `E5004` | 转换目标不是类型（函数调用被误当转换） | `N-20` |
| `E5005` | `T(x)` 实参不是一元 | `C-1` |
| `E5006` | 字符串插值里用了暂不支持默认显示的类型（数组 / `map` / 元组 / `func` / class / struct） | 文稿 11.2 |

### 9.6 控制流与穷尽 `E6xxx`

| 码 | 触发 | 规则 |
|---|---|---|
| `E6001` | `match` 非穷尽（enum 缺成员 / `result` 缺 `ok`/`fail`） | 文稿 5.3 |
| `E6002` | 对非 enum/result/object 的 `match` 缺 `_` | 文稿 5.3 |
| `E6003` | 枚举成员未带类型名限定 | `N-18` |
| `E6004` | `ok(...)`/`fail(...)` 模式用于非 `result` | 文稿 10.1 |
| `E6005` | 类型模式缺绑定名 | `G-11` |
| `E6006` | 分支不可达（重复字面量 / 与前一区间重叠 / `_` 之后还有分支） | 文稿 5.3 |
| `E6007` | `match` 作表达式但某分支为 `void` | 文稿 5.3 |
| `E6008` | 循环标签重复 | 文稿 5.2 |
| `E6009` | `break`/`continue` 在循环外 | 文稿 5.2 |
| `E6010` | `try`/`throw` 出现在返回非 `result<...>` 的函数内，且不在 `try` 块内（`main` 除外） | 文稿 10.1 |
| `E6011` | `try` 作用于非 `result<T>` 值 | 文稿 10.1 |
| `E6012` | `catch` 顺序错误（兜底 `catch (error e)` 之后还有 catch）或同一变体重复捕获 | 文稿 10.2 |
| `E6013` | `break` / `continue` 写在 `try` / `catch` / `finally` 块内（会跳过 finally） | 文稿 10.2 |

### 9.7 所有权与借用 `E7xxx`（详见 §11）

| 码 | 触发 |
|---|---|
| `E7001` | 使用已 move 的变量（use after move） |
| `E7002` | 借用冲突：同一时刻可变借用与其他借用重叠 |
| `E7003` | 借用作为返回值 / 赋给变量 / 存进容器（借用逃逸） |
| `E7004` | 所有者在其借用期内被 move 或重赋值 |
| `E7005` | `&` 出现在非实参、非形参位置 |
| `E7006` | `ArenaRef` 逃出 `arena` 生命周期 |
| `E7007` | `&` 作用于不可借用的值（字面量、临时右值、`const` 变量的可变借用） |
| `E7008` | `defer` 引用了在其后声明的变量 |
| `E7009` | `defer` 使用已 move 的值 |
| `E7010` | 返回局部变量的借用 / 返回指向局部数组的引用 |

### 9.8 后端限制 `E9xxx`

| 码 | 触发 |
|---|---|
| `E9001` | 代码生成不支持（应为零；出现即为实现缺陷，必须修而非绕过） |

### 9.9 警告 `W1xxx`（默认不阻断，`-Werror` 时阻断）

| 码 | 触发 | 规则 |
|---|---|---|
| `W1001` | `var` 推出 `float` 且右值是无后缀浮点字面量（提示金额用 `decimal`） | 文稿 3.3 |
| `W1002` | ~~命名风格不符~~ **已废止（v0.32）**，编号不复用：命名风格不再产生诊断，只作文档推荐（2.3） | — |
| `W1003` | 多余借用（对 copy 类型用 `&`） | 文稿 8.2 |
| `W1004` | 连续三层 `try` 未落地处理 | 文稿 10.2 |
| `W1005` | 对 enum/result 使用 `_` 放弃穷尽检查 | 文稿 5.3 |
| `W1006` | 变量名与类型名同名（若触发 `E3018` 则不再报此警告） | `GAP-7` |
| `W1007` | 未使用的变量 / 未使用的 import | 实现约定 |
| `W1008` | 整数常量 → `float` 且 > 2^53 | `C-2` |
| `W1009` | `decimal` → `float` | `C-5` |
| `W1010` | 空块 / 空 `else` | 实现约定 |

### 9.10 运行时崩溃 `R1xx`（七类，唯一崩溃路径）

| 码 | 名称 | 触发 | 文稿 |
|---|---|---|---|
| `R101` | `index-out-of-range` | `T[]` 下标越界；读 `map` 缺失键 | 3.3、10.2 |
| `R102` | `divide-by-zero` | 整数/模除零 | 3.7 |
| `R103` | `inexact-division` | `decimal` 除不尽 | 3.2 |
| `R104` | `overflow` | 整数溢出；`decimal` 溢出/精度超限 | 3.7、3.2 |
| `R105` | `shift-out-of-range` | 运行期才知的移位量越界 | 3.7 |
| `R106` | `float-not-finite` | `float` 产生 `inf`/`nan` | 3.7 |
| `R107` | `bad-conversion` | 收窄越界；向下转型失败；解箱标签不符 | 3.1、6.7 |

**不允许**存在第八类崩溃。任何"可能失败"的常规失败（解析外部字符串、文件不存在）必须走 `result<T>`，不得 trap。

---

## 10. 运行时崩溃点（唯一崩溃路径）

### 10.1 插入位置表（Rt-1，穷举）

| 操作 | 检查 | 码 | 说明 |
|---|---|---|---|
| `arr[i]` 读（`T[]`） | `0 <= i < len` | `R101` | 报 `i`、`Len`、数组表达式源码位置 |
| `arr[i] = v` 写（`T[]`） | 同上 | `R101` | 写前检查，不写半截 |
| `map[k]` 读 | 键存在 | `R101` | 报键的字符串形式（`string` 键）或"key" |
| `map[k] = v` 写 | 无检查（键不存在则插入） | — | 分配失败另见 `10.4` |
| 整数 `/` `%` | 除数 ≠ 0 | `R102` | `int`/`uint` 家族 |
| `decimal` `/` | 整除（无余数） | `R103` | 消息含被除数、除数、建议 `Divide(...)` |
| 整数 `+ - *` | 结果在类型范围内 | `R104` | 用 `__builtin_add_overflow` 或宽中间值 |
| `decimal + - *` | 系数不超 128 位/精度上限 | `R104` | |
| `<<` `>>` | `0 <= n < bits` | `R105` | 常量情形已编译期拒绝 |
| `float + - * /` | 结果为有限值 | `R106` | `isfinite` 检查；除数为 0 亦落此类 |
| `T(x)` 数值收窄 | 值在目标范围 | `R107` | 消息含源值、目标类型、范围 |
| `T(o)` / 自动解箱 | 标签精确相等（或 `null`→`T?`） | `R107` | 消息含实际标签名、期望类型 |
| `Derived(base)` | 运行时类型是 `Derived` 或其子类 | `R107` | |
| `int32.Truncate(x)` | **无检查**（这是显式截断入口） | — | |
| `arr.At(i)` / `map.Get(k)` | **无检查**（返回 `T?`） | — | |

### 10.2 崩溃输出与退出码（Rt-2）

```
axlang: runtime trap [R101] index-out-of-range
  at goods.ax:24:19
  detail: index 9, len 4
```

- 输出到 **stderr**，格式固定为上面四行（便于测试断言）。
- **不打印调用栈**（无栈展开机制），但必须打印源码位置与 detail。
- 退出码 **2**。
- 崩溃前必须执行**已注册的 `defer` 吗？**——**不执行**。崩溃是"进程终止"，不是"正常离开作用域"；文稿 10.2 的崩溃路径优先于 5.4 的清理约定。`GAP-11`（明确写入，避免实现分歧）。

### 10.3 `main` 的特殊处理（Rt-3）

| 情形 | 行为 |
|---|---|
| `main` 内 `expr try` 得到 `fail(e)` | 打印 `e.msg` 到 stderr（仅 msg，不加前缀）、退出码 **1**，**不 trap** |
| `main` 内 `throw error.X(msg)` | 同上 |
| `main` 正常结束无 `return` | 退出码 0（文稿 9.4 唯一豁免） |

### 10.4 分配失败（Rt-4）

堆分配失败（`new`、`Push` 扩容、装箱、`arena.Alloc`）→ `R104`？**不是**。`GAP-12` 决策：分配失败走 `ax_trap` 的**保留码 `R100`（out-of-memory）**，但**不列入七类崩溃清单**，因为它不是程序 bug 而是环境限制。实现上仍是同一函数、同一退出码 2。若设计者要求严格"只有七类"，改为打印 `fatal: out of memory` 并退出 2，不占 `R` 编号。

---

## 11. 所有权与借用检查

### 11.1 哪些类型有所有权（O-1）

| 类型 | 语义 | 赋值/传参/返回 |
|---|---|---|
| `class` 实例 | 唯一所有者 | **move**（源失效） |
| `T[]`、`map` | 唯一所有者 | **move** |
| `object` 盒子 | 唯一所有者 | **move**；解箱时若盒内是引用，引用被移出 |
| `ArenaRef<T>` | 区域内别名 | **copy**（可随意复制、共享；区域同生共死） |
| `struct` / `record` | 值 | copy（含容器时**深复制**） |
| 内建值、`enum`、元组 | 值 | copy |
| `string` | 不可变、只读共享 | copy（引用计数语义，用户不可见） |
| `func` | 函数指针 | copy |

### 11.2 变量状态机（O-2…O-5）

每个局部变量/参数有状态：

```
Uninit --(声明完成)--> Live
Live --(move 出)--> Moved
Moved --(重新赋值)--> Live
Live --(可变借用开始)--> MutBorrowed
MutBorrowed --(借用结束)--> Live
```

| 编号 | 规则 |
|---|---|
| `O-2` | 在 `Moved` 状态下**读、写、借用、解箱、方法调用**任何使用 → `E7001`（消息："`buf` 已被移走（第 N 行），此处不可再用"） |
| `O-3` | 重新赋值可复活：`buf = new Buffer(8)` 之后 `Live` |
| `O-4` | 分支汇合：`if/else` 两个分支都 `Moved` → 之后 `Moved`；只有一个分支 move → 之后视为 `Moved`（保守：可能已移走）→ 使用即 `E7001`。**不做**"分支敏感的重赋值追踪"（`GAP-13`：保守，宁可要求用户重写） |
| `O-5` | 循环体：循环结束时若变量在体内被 move，则下一轮迭代时视为 `Moved` → `E7001`。实现方式：循环体入口保存状态快照，体尾把状态与该快照求"最保守合并" |

### 11.3 move 发生的位置（O-6，穷举）

| 位置 | 说明 |
|---|---|
| `var x = e` / `T x = e` | `e` 是引用类型 → move |
| `x = e` | 同上；`x` 原值先 drop |
| 实参传递（形参无 `&`） | move |
| `return e` | move |
| 字段初始化 `this.f = e` | move |
| 数组/`map` 元素写入 `a[i] = e`、`m[k] = e` | move |
| 装箱 `object o = e`（`e` 为引用类型） | move（盒内持有） |
| 解箱 `Dog d = o` | 从盒中 move 出引用（`o` 之后不可再用其内容） |
| `??` 右侧求值后赋给左侧 | 按赋值规则 |
| `Push(e)` / `insert` 等容器方法 | move（方法签名标注为 move 语义） |

**不 move 的位置**：`&` 借用实参、`.Len()`/`.At()`/`.Get()` 等只读方法、`==` 比较、`Print`/`Println`/插值（内部按只读借用处理）。

> 判据一句话：**"值被存进某个地方"就 move，"只是看一眼"就借用**。实现里给每个 intrinsic 与标准库方法标注 `transfer`（move）/`borrow`/`copy` 三种属性，见 §15。

### 11.4 借用（O-7…O-12）

借用**不是一等值**（文稿 8.2）：语法上只能出现在实参与形参，因此不需要生命周期标注，只需要**单条语句内的冲突检测**。

| 编号 | 规则 |
|---|---|
| `O-7` | 同一条语句内，同一 place 上出现 ≥2 个借用，且其中至少一个可变 → `E7002`（`f(&x, &x)`、`f(&x, g(&x))`、`f(&x, x = 1)` 全部报错） |
| `O-8` | 借用目标必须是 **place**（局部变量、参数、字段、元素）；字面量、临时右值、函数调用结果 → `E7007` |
| `O-9` | `&`（可变）作用于 `const` 变量 → `E7009`；`&const` 允许 |
| `O-10` | `&` 作用于 copy 类型合法但无收益 → `W1003`（不报错，文稿 8.2） |
| `O-11` | 借用不可逃逸：语法上已限定（`&` 只能在实参），因此**只需检查一条**——函数体内不得把 `&x` 的结果赋给变量/返回/存入容器（本轮实现中这条由语法层天然保证，`E7003` 仅用于兜底断言） |
| `O-12` | 借用期间所有者被 move：`f(&xs, g(xs))` 中 `g(xs)` 会 move `xs`，与 `&xs` 冲突 → `E7004`（由 `O-7` 的同一遍扫描覆盖：把 move 视作"独占使用"） |

**算法（单语句一遍扫描）**：

```
check_statement(stmt):
  uses = collect_uses(stmt)      // 按源码顺序，每个 use = (place, kind, span)
  // kind ∈ { Read, Write, BorrowConst, BorrowMut, Move }
  active = {}                    // place -> (kind, span)
  for u in uses (左到右):
     if u.place in active:
        prev = active[u.place]
        if prev.kind == BorrowConst && u.kind == BorrowConst: 允许（多只读共存）
        else if u.kind == Read && prev.kind == BorrowConst: 允许
        else if u.kind == BorrowConst && prev.kind == Read: 允许
        else → E7002/E7004（按哪个是借用选码）
     active[u.place] = u
  // 语句结束，所有借用失效（借用不跨界）
```

这条"借用不跨语句"的简化来自语言设计本身（借用不可存储），**不是**近似：文稿 8.2 已经排除了跨语句借用的可能性。

### 11.5 `defer` 检查（O-13…O-16）

| 编号 | 规则 |
|---|---|
| `O-13` | `defer` 在**注册时**求值接收者与实参？——**不**。`defer f.Close()` 在离开作用域时才求值（文稿 5.4 "离开当前作用域前必然执行"）。因此 `defer` 后的调用表达式里引用的变量，必须在其执行时仍然 `Live` → 检查"声明顺序"（`E7008`）与"是否被 move"（`E7009`） |
| `O-14` | `defer` 的执行点穷举：块正常结束、`return`、`throw`、`try` 传播、`break`/`continue` 离开该块、函数体结束。**崩溃（trap）不执行**（`Rt-2`） |
| `O-15` | 同作用域多条 `defer` 逆序执行（LIFO） |
| `O-16` | `defer` 不得出现在模块级、`interface` 默认方法之外的非块位置（语法已保证） |

### 11.6 `arena` 与 `ArenaRef`（O-17…O-20）

| 编号 | 规则 |
|---|---|
| `O-17` | `arena.Alloc(x)` 把 `x` move 进区域，返回 `ArenaRef<T>`（copy 语义） |
| `O-18` | `ArenaRef` 不得逃出 `arena` 的作用域：赋给外层作用域变量、`return`、作为实参传给"会存起来"的位置、存进数组/map/class 字段 → `E7006`。实现判据：`ArenaRef` 的**所有使用点**必须位于 `arena` 声明所在的块（含嵌套块）内 |
| `O-19` | `ArenaRef` 可自由复制、可写（`b.next = a`），是唯一放弃"借用互斥"的引用（文稿 8.4）；因此它**不参与** `O-7` 的冲突检测，只做逃逸检测 |
| `O-20` | `arena` 变量本身按唯一所有者管理：离开作用域整体释放，区域内对象不单独 drop |

### 11.7 与 `try` / `throw` 的交互（O-21）

`try` 展开失败与 `throw` 都是"提前返回"，因此：**先按 LIFO 执行当前作用域的 `defer`，再把 `fail` 值返回**（文稿 5.4 + 10.1）。代码生成必须把这条落成统一的 `cleanup` 链（§13.6），不能各写一遍。

---

## 12. 中间表示与内存布局

### 12.1 阶段划分（B-1）

```
AST ──(sema)──> AST(带 ty/resolved/conv 槽) ──(lower)──> HIR ──(mono)──> HIR' ──(layout)──> LIR ──(codegen_c)──> C11
```

| 层 | 职责 | 不允许做的事 |
|---|---|---|
| AST | 语法结构 | 不含类型决策 |
| AST+sema | 类型、决议、转换插入、move 标记 | 不含布局 |
| HIR | 去语法糖：`for` → 循环+break、`?.` → 分支、`??` → 分支、插值 → 拼接序列、`match` → 比较链/跳转表、`defer` → 清理链、`try` → 早退 | 不含地址/偏移 |
| HIR' | 单态化、record 具名化、元组具名化 | 不含 C 细节 |
| LIR | 每个表达式落到临时变量、类型有确定大小与 drop 函数、vtable 已建 | 不含文本格式 |
| C11 | 文本 | — |

### 12.2 去糖规则（B-2，必须逐条等价）

| 语法 | 降为 |
|---|---|
| `for i in a..b { B }` | `i = a; __hi = b; while (i < __hi) { B; i = i + 1 }`（`..=` 用 `<=`；`__hi` 在循环前求值一次） |
| `for x in xs { B }` | 下标循环 + `x = xs[i]`（元素按 copy/move 规则：`T[]` 元素是值则 copy；若元素是 class 引用则**借用**读，不得 move 出容器） |
| `for (k,v) in m { B }` | 按 `entries[0..len]` 顺序遍历，`k`/`v` 为只读借用视图（`v` 若是引用类型则为只读借用，写回需 `m[k] = v`） |
| `for { B }` | `while (1) { B }` |
| `(0..n).Step(s)` | 循环 + `i += s`，`s` 必须 > 0（`S-7`：`Step` 实参须常量或运行期检查，非正 → `R101`？`GAP-14` 决策：非正 `Step` 编译期可判则 `E4009`，否则 trap `R101` 且 detail 为 `step <= 0`） |
| `x?.f` | `x == null ? null : x.f`（结果类型 `T?`） |
| `a ?? b` | `a != null ? a! : b`（`a!` 表示解包） |
| `s = "a{x:.2}b"` | `ax_str_concat(ax_str_concat("a", fmt(x, 2)), "b")` |
| `match`（enum） | 对 tag 的 `switch`（C 的 `switch` 可用，因无 fallthrough 需求时每臂 `break`） |
| `match`（`object` 类型模式） | tag 比较链 + 解箱 |
| `match`（区间/字面量） | 比较链（`if/else if`），按源码顺序，`_` 为最后 `else` |
| `defer e` | 在当前块维护清理栈；块出口（含早退）按 LIFO 调用 |
| `expr try` | `tmp = expr; if (tmp.tag == FAIL) { cleanup; return tmp; }` |
| `throw e` | `cleanup; return fail(e);` |
| `if` 表达式 | 临时变量 + `if/else` 赋值 |
| `obj.method(args)` | 虚调用：`((VT*)obj)->method(obj, args)`；静态调用：`Class__method(obj, args)` |
| record 字面量 | 生成 `__rec_n` 结构体 + 逐字段赋值 |
| 元组 | 生成 `__tup_n_m` 结构体 |
| `T[N]` | C 数组 `T name[N]`（内联） |
| `T[]` | `ax_arr_<mangled>` 结构体（len/cap/data） |
| `map<K,V>` | `ax_map_<mangled>` 结构体 |
| `object` 装箱 | `ax_box_new(tag, payload)` |
| 自动解箱 | `ax_unbox_<tag>(box)`（内含 tag 检查与 trap） |

### 12.3 大小与对齐（B-3，64 位平台；32 位平台把 8 改 4）

| 类型 | size | align | C 表示 |
|---|---|---|---|
| `bool` | 1 | 1 | `uint8_t` |
| `int8`/`uint8` | 1 | 1 | `int8_t`/`uint8_t` |
| `int16`/`uint16` | 2 | 2 | `int16_t`/`uint16_t` |
| `int32`/`uint32` | 4 | 4 | `int32_t`/`uint32_t` |
| `int`/`uint` | 8 | 8 | `int64_t`/`uint64_t` |
| `float` | 8 | 8 | `double` |
| `decimal` | 32 | 16 | `ax_dec{ __int128 coeff; int32_t scale; int32_t _pad; }`（**D-09**，见 `CONFLICT-5`） |
| `string` | 16 | 8 | `ax_str{ const uint8_t* ptr; int64_t len; }`（len 是字节数；码点数走缓冲头部缓存） |
| `T[N]` | `size(T)*N` | `align(T)` | `T x[N]` |
| `T[]` | 24 | 8 | `{int64_t len; int64_t cap; T* data;}` |
| `map<K,V>` | 48 | 8 | `{int64_t len, cap; ax_mentry* entries; int32_t* idx; uint64_t seed; const ax_mops* ops;}` |
| `object` | 8 | 8 | `ax_box*` |
| `class` 引用 | 8 | 8 | `T*` |
| interface 值 | 16 | 8 | `{ void* obj; void** itab; }` |
| `enum` | 4 | 4 | `uint32_t` |
| `range` | 24 | 8 | `{int64_t lo, hi; uint8_t incl;}`（仅内部） |
| `func(...)R` | 8 | 8 | 返回类型 + 参数类型的 C 函数指针 |
| `ArenaRef<T>` | 8 | 8 | `T*`（不参与 drop） |
| `void` | 0 | 1 | `void` |

**结构体布局**：字段按声明顺序，插入最小填充满足对齐；`struct`/`record`/元组的 size 向上取整到最大对齐（C 默认行为，无需 `packed`）。

### 12.4 class 与 vtable（B-4）

```c
/* class Dog extend Animal */
typedef struct Animal Animal;
typedef struct Animal_VT Animal_VT;
struct Animal_VT {               /* 父类方法在前，子类新增在后 */
  void (*drop)(Animal*);
  ax_str (*describe)(Animal*);
  ax_str (*speak)(Animal*);
};
struct Animal { Animal_VT* vt; ax_str f_name; };
struct Dog { Animal_VT* vt; ax_str f_name; ax_str f_breed; };  /* 父类字段前缀一致 */
```

- **父类字段必须是子类字段的前缀**（保证 `Animal* ← Dog*` 的零成本上转）。
- 每个类一份静态 `VT` 实例，覆写方法的位置填子类实现。
- `static` 方法不进 vtable，直接生成 `Class__name`。
- 接口调用的 itab：`static void* const Dog__itab_Loggable[] = { (void*)&Dog__log_impl, ... }`；接口值 `{obj, itab}`，调用 `((void(*)(void*))itab[0])(obj)`。
- 实现 interface 的方法必须与 interface 签名**参数/返回类型完全一致**（`E3019`）。

### 12.5 类型 id（B-5）

`object` 标签（§2.5）中 ≥1000 的部分按以下规则分配，必须**全程序稳定**（同一类型在所有编译单元得到同一 id，否则跨模块 `match` 会错）：

```
id(T) = 1000 + fnv1a32(canonical_name(T)) % 100000
冲突处理：若两类型得到同 id，后者顺延 +1（并记录确定性映射表）
canonical_name: int → "int"; int[] → "int[]"; Dog → "mod.Dog";
                map<string,int> → "map<string,int>"; Box<int> → "mod.Box<int>"
```

要求：同一次编译内 `id` 只由 `canonical_name` 决定，不依赖遍历顺序（冲突顺延必须按 canonical_name 字典序处理，保证确定性）。

### 12.6 drop 函数（B-6）

每个需要清理的类型生成 `void __drop_X(X* v)`：

| 类型 | drop 行为 |
|---|---|
| class | `v->vt->drop(v)`（用户无析构，编译器生成：drop 各引用字段 → `ax_free(v)`） |
| `T[]` | 逐元素 drop（元素需要 drop 时）+ `ax_free(data)` |
| `map` | 逐 entry drop key/value + `ax_free` |
| `string` | `ax_str_release(v->ptr)` |
| `object` | 按 tag 分派到具体 drop |
| `ArenaRef` | **无操作**（区域统一释放） |
| struct/record/元组 | 逐字段 drop（有容器字段时） |
| 内建值/enum/func | 无（生成空函数或完全省略） |

> 编译器必须做"是否需要 drop"的静态判定（`needs_drop(T)`），否则每个赋值都会生成一堆空函数调用，`-O1` 也救不回来。

---

## 13. 后端：C11 代码生成规范

### 13.1 生成的 C 文件骨架（B-7）

```c
/* generated by axc 0.0.3 from goods.ax -- do not edit */
#include "axrt.h"

/* 前置声明 */
typedef struct Goods Goods;  static void __drop_Goods(Goods*);
/* 类型定义、vtable、itab、record 结构 */
/* 函数定义（顺序按 HIR 拓扑序，递归用前置声明） */

int main(int argc, char** argv) { ... }   /* axlang main 包装：构造 args、调用、返回码 */
```

要求：
- 每个生成的 C 函数以 `static` 限定（除非被其他编译单元引用，编译单元 = 一个 `.ax` + 一份 `.c`；跨模块调用需 `extern`，由 driver 生成统一 `axc_program.h` 声明全部 `public` 符号）。
- **不得**使用 GNU 扩展（statement expression、`typeof`、嵌套函数）；只允许 C11 + `__int128`（`D-09` 允许，因 decimal 需要；若目标编译器不支持则退化为软件 128 位实现，见 §14.3）。
- 每个可执行语句都要能映射回源码行：在生成的 C 里用 `#line` 或在内联 `ax_trap` 调用里硬编码 `file/line/col`（**采用后者**，更可靠且不干扰调试器）。

### 13.2 表达式降级（B-8）

**核心约束：求值顺序必须严格从左到右**（文稿 11.1）。C 不保证实参求值顺序，因此：

> **规则：任何含 ≥2 个副作用的表达式，必须先落到临时变量。**

```
t0 = f();            /* 左实参先算 */
t1 = g();
r  = h(t0, t1);
```

临时变量命名 `t{n}`，在函数体顶部统一声明（C89 风格，避免作用域问题），`n` 单调递增。左值与右值都先落临时，再做运算——这样生成的 C 里每个表达式最多一个函数调用，求值顺序由语句顺序保证。

### 13.3 语句降级（B-9）

| axlang | C |
|---|---|
| `if c { A } else { B }` | `if (c) { A } else { B }` |
| `for i in a..b` | `for (int64_t i = a, __hi = b; i < __hi; i++)`（`__hi` 先求值，满足文稿 5.2） |
| `for { }` | `for (;;)` |
| `break outer` | `goto __brk_outer;`（标签由 label 名 mangle） |
| `continue outer` | `goto __cont_outer;` |
| `match` | `switch` 或 `if/else` 链（见 `B-2`） |
| 块 | `{ ... }` + 块尾 cleanup |
| `var x = e` | `T x = e;`（`e` 已是临时） |

### 13.4 所有者与 drop 的落点（B-10）

- 局部变量声明处：`T v_x = <init>;`，并在**其作用域的 cleanup 点**生成 `__drop_T(&v_x);`
- 一个块的所有 drop 调用集中到块尾一个 `cleanup` 序列，**逆序**（与声明顺序相反）。
- 早退（`return`/`break`/`continue`/`throw`/`try` 失败）跳到该块的 cleanup 标签，再跳转。
- move 之后：`v_x` 置为"空值"（指针 `NULL`；容器 `{0,NULL,NULL}`），drop 函数必须容忍空值（`if (!v->data) return;`）。**这消除了"move 后 drop 造成 double free"的整类问题。**

### 13.5 `match` 的生成（B-11）

- enum：`switch (tag) { case 0: ... break; ... }`，无 `default`（穷尽已保证）；若有 `_` 则生成 `default`。
- `result`：`switch (r.tag)`，`ok` 分支解出 `r.u.ok`，`fail` 分支 `r.u.err`。
- `object` 类型模式：按源码顺序 `if (box->tag == id(Dog)) { Dog* x = (Dog*)box->payload; ... } else if ...`。
- 字面量/区间：`if (v == 200) ... else if (v >= 400 && v <= 499) ...`（区间按半开/闭精确生成）。
- **每个 arm 体只求值一次**：arm 体是表达式时先算到临时再赋给结果变量（避免"分支里算两次"）。

### 13.6 `try` 与 `defer` 的统一清理链（B-12，最关键的一段生成逻辑）

每个块维护一个**清理栈**（编译期列表，不运行时）：

```
block:
  pushed = []
  for stmt in block:
     if stmt is defer: pushed.push(stmt.call)
     else: gen(stmt)
  emit cleanup: for f in pushed.reversed(): emit_call(f)
```

早退语句的生成：

```c
/* return e */
t = e;
<cleanup for all enclosing blocks, inner→outer, each LIFO>
return t;
```

`try` 的生成：

```c
t = expr;                       /* result<T> */
if (t.tag == AX_FAIL) {
   <cleanup>
   return t;                    /* 原样返回，不复制 error */
}
v = t.u.ok;                     /* 展开 */
```

**必须一张统一的 `emit_cleanup_until(scope)` 函数**，`return`/`throw`/`try`/`break`/`continue` 全部调它。各写一份是这一层最常见的 bug 来源（漏 drop → 泄漏；drop 两次 → 崩溃）。

### 13.7 字符串插值（B-13）

`"合计：{total:.2}，件数：{count}"` 生成：

```c
ax_str_builder b; ax_sb_init(&b);
ax_sb_push(&b, AX_S("合计："));
ax_sb_push_dec(&b, total, 2);        /* .2：decimal 按 HalfUp 保留 2 位 */
ax_sb_push(&b, AX_S("，件数："));
ax_sb_push_int(&b, count);           /* 无格式说明符：默认格式 */
ax_str s = ax_sb_finish(&b);         /* 交出所有权，builder 失效 */
```

- 无格式说明符时的默认格式：`int` 十进制、`float` 最短往返表示（`%.15g` 后修正）、`decimal` 去除尾随零、`bool` → `true`/`false`、`string` 原样、`enum` → 成员名、class → `ClassName`、struct → `Name{field: value, ...}`（字段声明顺序）、数组 → `[a, b, c]`、map → `{k: v, ...}`（插入顺序）。
- **可空值**按 `.has` 分支生成：有值 push 内部值的显示形式、无值 push `"null"`：

```c
/* "v={x}"，x: string? */
if (t1.has) { ax_sb_push(&b, t1.v); } else { ax_sb_push(&b, AX_S("null")); }
```

  插值只做显示、不产生可空值逃逸，因此**不触发 `E4001`**；赋值与传参处仍照常要求判空。
- **当前实现边界**：已落地 `int`/`uint`、`float`、`decimal`、`bool`、`string`、`enum` 及其可空形式；数组 / `map` / 元组 / `func` / class / struct 的默认显示**尚未实现**，插值它们报 `E5006`（`GAP-12`）。上一条里这几项是设计目标，不是现状——codegen 的兜底分支只留注释，不允许退化成静默输出 `?`。
- `.N` 只允许数值类型（`E4030`），舍入固定 `Rounding.HalfUp`；可空数值不参与 `:.N`（先判空取出）。
- `Print(s)` 直接 `ax_write_stdout(s)`；`Println(s)` 在同一句后追加 `ax_write_stdout(AX_S("\n"))`。两者都不经过 `printf`（格式串一律由插入式拼接生成，避免 `%` 转义问题）。

### 13.8 装箱与解箱（B-14）

```c
object o = 42;          /* ax_box_new(AX_TAG_INT, &(int64_t){42}) */
int n = o;              /* ax_unbox_int(o, file, line, col) */
```

- 装箱走 `ax_box_new(tag, &payload, size, drop_fn)`，payload 按值复制（`string` 增加引用计数、容器 move）。
- 解箱必须传 `file/line/col`，trap 消息才能定位。
- `T?` 目标遇 `null`：`ax_box_is_null` → 赋值 `null`，不 trap。

### 13.9 常量与全局（B-15）

- 模块级 `const`：能编译期求值的直接内联；不能的（`S-2` 不允许，所以不会发生）。字符串常量用 `static const` 数组 + 只读 `ax_str`。
- **没有可变全局变量**，因此生成的 C 里不允许出现非 `const` 的全局对象（除 vtable/itab 这类只读表）。

---

## 14. 运行时库 `axrt`（C11）

### 14.1 崩溃与内存（Rt-5）

```c
_Noreturn void ax_trap(int code, const char* file, int line, int col, const char* fmt, ...);
void* ax_alloc(int64_t size);          /* 失败 → ax_oom() */
void  ax_free(void* p);
void  ax_oom(void);                    /* 见 10.4 */
```

### 14.2 字符串（Rt-6）

```c
typedef struct { const uint8_t* ptr; int64_t len; } ax_str;   /* len = 字节数 */
#define AX_S(lit) ((ax_str){ (const uint8_t*)(lit), (int64_t)(sizeof(lit)-1) })

ax_str  ax_str_from_bytes(const uint8_t* p, int64_t n);  /* 复制 + 建缓冲头 */
ax_str  ax_str_from_cstr(const char* s);
void    ax_str_retain(ax_str s);
void    ax_str_release(const uint8_t* p);
int64_t ax_str_len_cp(ax_str s);          /* 码点数（缓冲头缓存） */
int64_t ax_str_byte_len(ax_str s);        /* = s.len */
ax_str  ax_str_concat(ax_str a, ax_str b);
int     ax_str_eq(ax_str a, ax_str b);
int     ax_str_cmp(ax_str a, ax_str b);
ax_str  ax_str_slice_cp(ax_str s, int64_t from, int64_t to);   /* 按码点切片，内部用 */
int     ax_str_at(ax_str s, int64_t i, ax_str* out);           /* 0=命中,1=越界(→null) */
int     ax_str_codepoint_at(ax_str s, int64_t i, int32_t* out);
ax_str* ax_str_chars(ax_str s, int64_t* n);                    /* 调用方负责释放返回数组 */
int     ax_str_from_codepoint(int32_t cp, ax_str* out);        /* 0=成功,1=非法码点 */
int     ax_str_contains(ax_str s, ax_str sub);
ax_str* ax_str_split(ax_str s, ax_str sep, int64_t* n);        /* 至少 1 个元素 */
ax_str  ax_str_trim(ax_str s);
```

**缓冲头布局**（实现自由，但语义固定）：

```
[ int64_t rc ][ int64_t byte_len ][ int64_t cp_len(-1=未算) ][ bytes... 以 \0 结尾 ]
              ^ ptr 指向这里
```

`rc` 只在运行时内部使用（文稿 8 章开头）；`cp_len` 缓存使 `Len()` 第一次 O(n)、之后 O(1)（字符串不可变，安全）。

### 14.3 `decimal`（Rt-7，D-09）

```c
typedef struct { __int128 coeff; int32_t scale; int32_t _pad; } ax_dec;  /* 值 = coeff / 10^scale */

ax_dec ax_dec_from_i64(int64_t v);
ax_dec ax_dec_from_parts(__int128 coeff, int32_t scale);
ax_dec ax_dec_parse(ax_str s, int* ok);        /* 失败 ok=0，走 result */
ax_str ax_dec_to_str(ax_dec v);                /* 去尾随零 */
ax_dec ax_dec_add(ax_dec a, ax_dec b);         /* 溢出 → R104 */
ax_dec ax_dec_sub(ax_dec a, ax_dec b);
ax_dec ax_dec_mul(ax_dec a, ax_dec b);
ax_dec ax_dec_div_exact(ax_dec a, ax_dec b);   /* 除不尽 → R103 */
ax_dec ax_dec_div_round(ax_dec a, ax_dec b, int32_t digits, int mode);  /* divide(...) */
ax_dec ax_dec_rem(ax_dec a, ax_dec b);         /* a - trunc(a/b)*b，不触发除不尽 */
ax_dec ax_dec_round(ax_dec a, int32_t digits, int mode);
int    ax_dec_cmp(ax_dec a, ax_dec b);         /* -1/0/1 */
int    ax_dec_to_i64(ax_dec a, int64_t* out);  /* 截断向零；越界 → R107 */
double ax_dec_to_f64(ax_dec v);
```

- `mode`：0=`HalfUp`、1=`HalfEven`、2=`Down`（对应 `Rounding` 三个成员，**编译期映射，不由用户传任意整数**）。
- **精度要求**：必须支持至少 28 位有效十进制数字；系数溢出或有效位数超限 → `R104`。`__int128` 上限约 38 位，留 10 位余量给中间运算。
- `ax_dec_mul` 必须做"先乘后归一化"，否则 `19.99 * 3` 的中间系数会虚高溢出。

### 14.4 数组与 map（Rt-8）

```c
/* 动态数组：每个元素类型生成一套（单态化），以下为示意 */
typedef struct { int64_t len, cap; T* data; } ax_arr_T;
void ax_arr_T_init(ax_arr_T* a);
void ax_arr_T_push(ax_arr_T* a, T v);            /* 扩容 1.5~2 倍 */
T    ax_arr_T_pop(ax_arr_T* a);                  /* 空 → R101 */
void ax_arr_T_remove(ax_arr_T* a, int64_t i);    /* 越界 → R101 */
void ax_arr_T_clear(ax_arr_T* a);
T    ax_arr_T_index(ax_arr_T* a, int64_t i);     /* 越界 → R101 */
int  ax_arr_T_at(ax_arr_T* a, int64_t i, T* out);/* 越界 → 0（对外表现为 null） */

/* map：插入顺序 + 哈希索引 */
typedef struct { uint64_t hash; void* k; void* v; } ax_mentry;
typedef struct ax_mops {
  uint64_t (*hash)(const void* k);
  int      (*eq)(const void* a, const void* b);
  void     (*drop_k)(void* k); void (*drop_v)(void* v);
} ax_mops;
typedef struct { int64_t len, cap; ax_mentry* entries; int32_t* idx;
                 uint64_t seed; const ax_mops* ops; } ax_map;
void  ax_map_init(ax_map* m, const ax_mops* ops);
void  ax_map_put(ax_map* m, const void* k, const void* v);   /* 存在则覆盖，位置不变 */
int   ax_map_get(ax_map* m, const void* k, void* out);       /* 0=命中 */
void* ax_map_get_ptr(ax_map* m, const void* k);              /* NULL=缺失 → 走 R101 */
```

**map 的硬性语义**（文稿 5.2、3.3）：
1. **迭代顺序 = 插入顺序**（`entries` 是追加数组，`idx` 只做查找，不参与迭代）。
2. 覆盖已有键**不改变**其迭代位置。
3. `Get` 缺失 → 调用方（生成的代码）走 `R101`；`At()` 版本返回 `null`。
4. 键类型必须实现 `Hashable + Comparable`（编译期检查 `E4018`）；`int`/`uint`/`string`/`decimal`/`enum`/`bool`/`class`（后者按引用身份 hash）各有预置 `ax_mops`。

### 14.5 装箱（Rt-9）

```c
typedef struct ax_box { uint32_t tag; void (*drop)(void*); void* payload; } ax_box;
ax_box* ax_box_new(uint32_t tag, const void* payload, int64_t size, void (*drop)(void*));
void    ax_box_free(ax_box* b);
/* 每种 tag 一个解箱函数，trap 内建 */
int64_t ax_unbox_int(ax_box* b, const char* f, int l, int c);
double  ax_unbox_float(...); ax_dec ax_unbox_dec(...); ax_str ax_unbox_str(...);
bool    ax_unbox_bool(...); void* ax_unbox_ref(uint32_t want_tag, ...);
```

### 14.6 输出与格式化（Rt-10）

```c
void ax_write_stdout(ax_str s);
void ax_write_stderr(ax_str s);
typedef struct { uint8_t* buf; int64_t len, cap; } ax_str_builder;
void ax_sb_init(ax_str_builder* b);
void ax_sb_push(ax_str_builder* b, ax_str s);
void ax_sb_push_int(ax_str_builder* b, int64_t v);
void ax_sb_push_uint(ax_str_builder* b, uint64_t v);
void ax_sb_push_float(ax_str_builder* b, double v);
void ax_sb_push_float_n(ax_str_builder* b, double v, int32_t digits);
void ax_sb_push_dec(ax_str_builder* b, ax_dec v);
void ax_sb_push_dec_n(ax_str_builder* b, ax_dec v, int32_t digits);
ax_str ax_sb_finish(ax_str_builder* b);
```

### 14.7 文件（Rt-14）

```c
typedef struct { FILE* fp; } ax_file;
typedef struct { uint8_t tag; union { ax_file* ok; ax_err err; } u; } ax_res_file;

ax_res_file ax_file_open_result(ax_str path, int mode);  /* 0=rb 1=wb 2=ab */
int     ax_file_read_all(ax_file* f, ax_str* out);   /* 0=ok 1=err */
int     ax_file_read_line(ax_file* f, ax_str* out);  /* 0=读到 1=EOF，剥掉行尾 '\r' */
int64_t ax_file_write(ax_file* f, ax_str s);         /* >=0 字节数，<0 失败 */
int64_t ax_file_write_line(ax_file* f, ax_str s);    /* 同上，补 '\n' */
void    ax_file_close(ax_file* f);                   /* 幂等 */
```

- **一律二进制模式**（`rb`/`wb`/`ab`）：文本模式会把 `\n` 改写成 CRLF，导致同一份代码在 Windows 与 Linux 上写出的字节不同；`ReadLine` 负责剥掉读入侧的 `\r`。
- 路径转换上限 1023 字节（与 `ax_read_file` 一致），超长按打开失败处理。
- 打开失败、读失败、写失败统一映射 `error.IO`（`ax_err_new(4, …)`），不新增崩溃码。

---

## 15. 根模块最小标准库（intrinsic，无 FFI）

> 文稿 15 章把 FFI/`unsafe` 列为待定，因此**所有标准库能力必须是编译器 intrinsic**：符号在根模块预声明，代码生成直接映射到 `axrt` 函数。不得为"实现标准库"而发明 `extern` 语法。

`transfer` 列的含义（决定 move 还是借用，见 `O-6`）：`copy` = 只读、`borrow` = 只读借用、`move` = 存入。

### 15.1 语言级 intrinsic

| 符号 | 签名 | transfer | 依据 |
|---|---|---|---|
| `Print` | `void Print(string)`（不换行） | borrow | 文稿 11.2 |
| `Println` | `void Println(string)`（末尾补 `\n`） | borrow | 文稿 11.2 |
| `int.Parse` | `result<int> parse(string)` | borrow | 文稿 3.1 |
| `float.Parse` | `result<float> parse(string)` | borrow | 文稿 3.1 |
| `decimal.Parse` | `result<decimal> parse(string)` | borrow | 文稿 3.1/3.2 |
| `int8.Truncate` `int16.Truncate` `int32.Truncate` | `T truncate(int)` | copy | 文稿 3.1 |
| `string.FromCodepoint` | `result<string> from_codepoint(int)` | copy | 文稿 11.2 |
| `x.ToString()` | `string to_string()`（数值/`bool`/`enum`/`string`） | copy | 文稿 11.2 |

### 15.2 `string` 方法

| 方法 | 返回 | transfer | 依据 |
|---|---|---|---|
| `Len()` | `int`（码点数） | borrow | 文稿 11.2 |
| `ByteLen()` | `int`（UTF-8 字节数） | borrow | 文稿 11.2 |
| `At(i)` | `string?`（第 i 个码点） | borrow | 文稿 11.2 |
| `Chars()` | `string[]` | borrow | 文稿 11.2 |
| `CodepointAt(i)` | `int?` | borrow | 文稿 11.2 |
| `Contains(sub)` | `bool` | borrow | 文稿 6.1 示例 |
| `Split(sep)` | `string[]`（≥1 个元素） | borrow | 文稿 6.1 示例 |

### 15.3 数组 / `map` 方法

| 方法 | 返回 | transfer | 依据 |
|---|---|---|---|
| `Len()` | `int` | borrow | 文稿 3.3 |
| `Push(v)` | `void` | move | 文稿 3.3 |
| `Pop()` | `T`（空 → `R101`） | move | 文稿 5.2（禁止遍历期调用） |
| `Remove(i)` | `void`（越界 → `R101`） | move | 同上 |
| `Clear()` | `void` | move | 同上 |
| `At(i)` | `T?` | borrow | 文稿 3.3 |
| `Sort(cmp)` | `void`，`cmp: func(T,T) int` | move | 文稿 4.1 |
| `map.Len()` | `int` | borrow | — |
| `map.Get(k)` | `V?` | borrow | 文稿 3.3 |

### 15.4 模块（`import` 后可用）

| 模块 | 符号 | 签名 | 依据 |
|---|---|---|---|
| `io` | `ReadLine` | `string?`（stdin，EOF → `null`，`GAP-15`） | 文稿 11.3 |
| `io` | `ReadFile` | `result<string>`（整份读入，省掉 open/close） | 文稿 3.3 示例 |
| `file` | `Open` | `result<File>` 只读 | 文稿 11.3 |
| `file` | `Create` | `result<File>` 新建或清空 | 文稿 11.3 |
| `file` | `Append` | `result<File>` 追加，不存在则新建 | 文稿 11.3 |
| `file` | `File.Read()` | `result<string>` 读到末尾 | 文稿 11.3 |
| `file` | `File.ReadLine()` | `string?` 一行，剥掉行尾 `\r`；EOF → `null` | 文稿 11.3 |
| `file` | `File.Write(s)` | `result<int>` 写入字节数 | 文稿 11.3 |
| `file` | `File.WriteLine(s)` | `result<int>` 写入字节数（含 `\n`） | 文稿 11.3 |
| `file` | `File.Close()` | `void`，幂等 | 文稿 11.3 |
| `math` | `Equals` | `bool equals(float, float, float eps)` | 文稿 3.7 |
| `math` | `Compare` | `int compare(float, float)` | 文稿 3.7 |
| `json` | `Parse` | `result<Json> parse(string)`，`Json = map<string, object>` | 文稿 6.7 |
| `db` | `Query` | `result<Row> query(string, ...)` | 文稿 12 |
| `db` | `Row.Empty()` / `GetString` / `GetInt` / `GetDecimal` | 见 §16 `RUN-1` 的 fixture | 文稿 12 |

> `db` 在 M8 阶段实现为**内存 fixture provider**（读环境变量 `AX_DB_FIXTURE` 指向的 JSON），用于通过 §16 `RUN-1`。真实数据库驱动不在本规格范围内（属 FFI 待定项）。

---

## 16. 一致性测试用例（验收基准）

> 每个用例一个文件；负例断言**恰好一个诊断码**（允许附带警告，但不得有额外 error）。正例必须同时通过 `check` 与 `run`（若有输出则逐字节比对）。
> **编号约定**：用例编号带前导零（`T-01`、`GX-03`），规则编号不带（`T-1`、`T-31`），二者不混淆。

### 16.1 词法（M1）

| ID | 源 | 期望 |
|---|---|---|
| `L-01` | `int a = 010` | `E1001` |
| `L-02` | `string c = 'a'` | `E1002`，`suggestion` 含 `"a"` |
| `L-03` | `int a = 0x` | `E1005` |
| `L-04` | `int a = 1_000_000` | 通过；`ConstVal.Int = 1000000` |
| `L-05` | `int a = 1__0` | `E1005` |
| `L-06` | `decimal p = 19.99d` | 通过；`Dec{coeff:1999, scale:2}` |
| `L-07` | `decimal p = 19d` | 通过（`GAP-2`） |
| `L-08` | `float f = 1.5e-3` | 通过 |
| `L-09` | `string s = "a\n"` | 通过 |
| `L-10` | `string s = "\q"` | `E1004` |
| `L-11` | `string s = "\u{4F60}"` | 通过，值为 `你` |
| `L-12` | `/* /* */` | `E1003` |
| `L-13` | `string s = r"C:\path\a"` | 通过，`Len()==9`，无插值 |
| `L-14` | `for i in 1..10 { }` | 通过（`1` 与 `..` 正确切分） |
| `L-15` | 含 `$` 的源 | `E1006` |

### 16.2 语法（M2）

| ID | 源 | 期望 |
|---|---|---|
| `G-01` | `int a = 1; int b = 2` | 通过（同行两语句） |
| `G-02` | `int a = 10 int b = 20` | `E2002` |
| `G-03` | `decimal bad = price * count` ⏎ `+ shipping` | `E2003` |
| `G-04` | `decimal ok = price * count +` ⏎ `shipping` | 通过 |
| `G-05` | `if x` ⏎ `{ }` | 通过（`{` 换行） |
| `G-06` | `if x Println("a")` | `E2004` |
| `G-07` | `int s = if x >= 0 { 1 }` | `E2005` |
| `G-08` | `int a = i++` | `E2010` |
| `G-09` | `++i` | `E2010` |
| `G-10` | `nums.Sort((int a, int b) { return a - b })` | 通过（匿名函数消解） |
| `G-11` | `var t = (1, "one")` | 通过（元组） |
| `G-12` | `f((a, b))` | 通过（一个元组实参，非两个实参） |
| `G-13` | `outer: for i in 0..3 { break outer }` | 通过（标签消解） |
| `G-14` | `throw error.NotFound("x")` 单独成句 | 通过 |
| `G-15` | `return throw error.IO("x")` | `E2001`（`throw` 已是语句） |
| `G-16` | `int f() { g() try + 1 }` | 解析为 `(g() try) + 1`（见 `--dump=ast`） |
| `G-17` | `g() try.Len()` | `E2009` |
| `G-18` | 文稿 §12 完整示例 | `--dump=ast` 无 error |
| `Top-1` | 显式 `int main()` + 顶层 `Println("top level")` | `E3041`（两处入口，只报一条） |
| `Top-3` | 纯顶层语句（`Println` / `int x = 5` / `var` / `if` / `for` / 调用后面定义的函数） | 通过（隐式 `main`，`Top-3.out`） |

### 16.3 名称 / 作用域 / 可见性（M3）

| ID | 源 | 期望 |
|---|---|---|
| `N-01` | `int total = 0` + `if total > 0 { int total = 5 }` | `E3001` |
| `N-02` | `int a` | `E3002` |
| `N-03` | 顶层 `public int x = 0`（语句带可见性修饰词） | `E3024` |
| `N-04` | `class A { protected int e }` + 类外 `a.e` | `E3013` |
| `N-05` | `class Point {}` + `int Point(int a) { return a }` | `E3018` |
| `N-06` | `struct P { public int x }` | `E3011` |
| `N-07` | `class A { public constructor() {} }` | `E3011` |
| `N-08` | `class A { public override void f() {} }`（无父类） | `E3016` |
| `N-09` | 子类同名方法未写 `override` | `E3017` |
| `N-10` | 模块级 `protected int x = 1` | `E3014` |
| `N-11` | `u.from_email("a@b")`（static 经实例） | `E3013` |
| `N-12` | `class A { public int x } ` 无 constructor + `new A()` | `E3008` |
| `N-13` | `constructor(int a) { }`（未给字段赋值） | `E3009` |
| `N-14` | 两个 `constructor` | `E3010` |
| `N-15` | `class B extend A extend C` | `E3020`（或 `E2001`，取首个） |
| `N-16` | `struct S extend A` | `E3021` |
| `Mod-1` | 多文件：`import utils` / `as` 别名 / 点号路径 `import net.http` / 跨模块 class | 通过（`Mod-1.out`） |
| `Mod-2` | 具名导入 `import mathx.{square, LIMIT}` | 通过（当前只做白名单，见 GAP #11） |
| `Mod-3` | 三级路径 `import net.http.server` → `net.http.server.ping()` / `.PORT` | 通过（`Mod-3.out`） |
| `Mod-neg-private` | 用别的模块的非 `public` 符号 | `E3030` |
| `Mod-neg-cycle` | 两个模块互相 `import` | `E3026` |
| `Mod-neg-builtin` | 目录里有 `io.ax` 且 `import io` | `E3039`（内建模块优先，文件不被加载） |
| `Mod-neg-top` | 被 `import` 的 `lib.ax` 顶层写 `Println("side effect")` | `E3024`（顶层代码只允许出现在入口文件） |
| `Mod-4` | `import model.{greet, PORT, Config}` 后裸名用函数/常量/类型 | 通过（`Mod-4.out`：`hello, named` / `port=80`） |
| `Mod-neg-named` | `import model.{greet}` 后写 `model.hidden(...)` | 恰好一条 `E3030`（不再连带 `E5002`） |
| `Mod-5` | `import net.http` 后写 `http.ping()` / `http.PORT` / `new http.Server(...)` | 通过（末段短名与全路径等价） |
| `Mod-neg-short` | `import net.http` + `import web.http` | `E3040`（短名冲突，需 `as` 区分） |
| `Mod-6` | 3 段模块路径的类型标注 / `new` / 静态工厂 / `struct` 位置构造 | 通过（`model.Product` 短名亦可） |
| `Mod-7` | `import model` 后**裸类型名**：`Product p = new Product(...)` / `Product.Sample()` / `Point(3,4)` / `Color.Red` | 通过（`Mod-7.out`） |
| `Mod-neg-ambig` | `import a` + `import b`，两者都导出 `Product` | 恰好一条 `E3042` |
| `Mod-8` | 跨模块 interface 多态：类 `implement model.Describable`、interface 变量再赋子类实例（派发到 override）、interface 作跨模块参数 | 通过（`Mod-8.out`） |
| `Upcast-1` | 上转赋值：`Animal a = new Dog(...)`（声明 / 传参 / return 三处），派发走子类 override | 通过；生成 C 显式强转，`-Werror` 零警告（`Upcast-1.out`） |
| `Iface-1` | interface 多态：interface 变量 / 参数 / 返回值 / 数组元素，两个实现类派发 | 通过（`Iface-1.out`） |
| `Iface-2` | `interface Pet extend Animal`：父 interface 方法经子 interface 变量调用；实现 `Pet` 隐式实现 `Animal`（直接上转） | 通过（`Iface-2.out`） |
| `demo/OrderDemo` | 示例工程（`samples/demo/`）：`catalog.product` / `catalog.inventory` / `checkout.bill`（具名导入）/ `checkout.wholesale`（跨模块继承）；入口用**裸类型名**构造，另混用全路径写法作对照，含数组字段 `Push`、可空返回判空、`:.2` 金额 | 通过（`main.out` 逐字节比对） |
| `Struct-1` | `Point(1, 2)` 位置构造、字段写、赋值即复制、嵌套 `Box(Point(...), ...)` | 通过（`b.x = 99` 不影响 `a`） |
| `Main-1` | `void main() { }` | `E3025`（`check` 就报，不必等 `run`） |
| `N-17` | 写旧名 `print("x")` | `E3003`，提示必须同时含 `Print` 与 `Println` |

### 16.4 类型（M3/M4）

| ID | 源 | 期望 |
|---|---|---|
| `T-01` | `uint8 b = 255` | 通过 |
| `T-02` | `uint8 c = 300` | `E4003` |
| `T-03` | `int n = 1; uint8 d = n` | `E4003` |
| `T-04` | `int m = -9223372036854775808` | 通过（`S-5`） |
| `T-05` | `uint8 x = -1` | `E4003` |
| `T-06` | `float f = 1.0; if f == 1.0 { }` | `E4011` |
| `T-07` | `float x = 5 / 2` | `E4001` |
| `T-08` | `int[4] a = [1,2,3,4]; a[4] = 9` | `E4005` |
| `T-09` | `int[] a=[0,1,2,3]; int bad = a[9]` | 通过编译，运行 `R101` |
| `T-10` | `int[] a=[0]; int? m = a.At(9)` | 通过，`m == null` |
| `T-11` | `var cfg = {"name":"ax","port":8080}` + `cfg["port"] + 1` | 通过，类型 `int` |
| `T-12` | `cfg["nope"]` | `E4008` |
| `T-13` | `cfg["extra"] = 1` | `E4008` |
| `T-14` | `var ages = {"tom":18}; ages["bob"] = 31` | 通过（`map<string,int>`） |
| `T-15` | `var e = {}` | `E4007` |
| `T-16` | `map<string,int> e = {}` | 通过 |
| `T-17` | `int x = 3.5` | `E4001` |
| `T-18` | `a < b < c` | `E4016` |
| `T-19` | `a & b == 0`（`a`、`b` 为 `int`） | 通过，等价 `(a & b) == 0` |
| `T-20` | `string s = "a" + 1` | `E4010` |
| `T-21` | `string s="abc"; string c = s[0]` | `E4006` |
| `T-22` | `map<float,int> m` | `E4018` |
| `T-23` | `object o = 42; int n = o` | 通过 |
| `T-24` | `object o = 42; int n = o + 1` | `E5002` |
| `T-25` | `Point p = Point(1,2); object o = p` | `E4029` |
| `T-26` | `object a = 42; int? k = a` | 通过，`k == 42` |
| `T-27` | `object a = int32(1); int n = a` | 通过编译，运行 `R107` |
| `T-28` | `var n = null` | `E4007` |
| `T-29` | `string s = "中文"; s.Len() == 2` | 通过（码点数 2） |
| `T-30` | `User? u = find(1); User v = u` | `E4001`（可空不可直接赋给非可空） |
| `T-31` | `User? u = find(1); u.name` | `E4022` |
| `T-32` | `if u == null { return 0 }` + `u.name` | 通过（`T-24` 早退收窄） |
| `T-33` | `var r = 0..10` | `E4021` |
| `T-34` | `func(int) int f; f == g` | `E4024` |
| `T-35` | `x = (1,2); x[0]` | `E4020` |

### 16.5 转换（M4）

| ID | 源 | 期望 |
|---|---|---|
| `C-01` | `decimal d = 1.5` | `E4003` |
| `C-02` | `decimal d = decimal(1.5)` | `E5001` |
| `C-03` | `string s = string(65)` | `E5001` |
| `C-04` | `int n = int("42")` | `E5001` |
| `C-05` | `int32(a)`（`a` 为 `int`，值超范围） | 编译通过，运行 `R107` |
| `C-06` | `int32.Truncate(a)` | 通过，静默截断 |
| `C-07` | `float(d)`（`d` 为 `decimal`） | 通过 + `W1009` |
| `C-08` | `Animal a = new Dog("w","b")` | 通过（隐式上转） |
| `C-09` | `Dog d = Dog(a)`（`a` 实际是 `Cat`） | 编译通过，运行 `R107` |
| `C-10` | `Config(cfg)` 字段全匹配 | 通过 |
| `C-11` | `Config(cfg)` 缺字段 | `E4034` |
| `C-12` | record 同时匹配构造与转换 | `E4032` |
| `C-13` | `int[3] a=[1,2,3]; int[] b = a` | `E4001`（`GAP-5`） |
| `C-14` | `int(Status.Ok)` | `E4019` |

### 16.6 错误处理与 match（M5）

| ID | 源 | 期望 |
|---|---|---|
| `E-01` | `match r { ok(p) => f(p) }`（缺 `fail`） | `E6001` |
| `E-02` | `match code { 200 => f() }`（缺 `_`） | `E6002` |
| `E-03` | `match s { Ok => f() }` | `E6003` |
| `E-04` | `match o { Dog => f() }` | `E6005` |
| `E-05` | `match o { Dog x => f(), _ => g() }` | 通过 |
| `E-06` | 两个字面量相同 arm | `E6006` |
| `E-07` | `int f() { g() try; return 0 }` | `E6010` |
| `E-08` | `int n = 1 try` | `E6011`（`try` 作用于非 `result`） |
| `E-09` | `throw error.IO("x")` 在 `void` 函数内 | `E6010` |
| `E-10` | `int x = match c { 1 => f(), _ => g() }`（`f` 为 `void`） | `E6007` |
| `E-11` | `result<decimal> f() { var x = g() try; return ok(x) }` | 通过 |
| `E-12` | `main` 内 `Goods.load(1) try` | 通过（`main` 例外） |
| `E-13` | `main` 内 try 得 `fail` | 运行：stderr 打印 `e.msg`，退出码 1 |
| `E-14` | `int f() { }` | `E2006` |
| `E-15` | `int main() { }` | 通过（`main` 豁免） |

### 16.7 泛型（M6）

| ID | 源 | 期望 |
|---|---|---|
| `GX-01` | `T max<T: Comparable>(T a, T b) { return if a > b { a } else { b } }` | 通过 |
| `GX-02` | `max(1, 2)` | 通过，实例化 `int` |
| `GX-03` | `max(1.0, 2.0)` | `E4036`（`float` 无 `Comparable`） |
| `GX-04` | `class Box<T> { public T value; constructor(T v) { this.value = v } }` | 通过 |
| `GX-05` | `type Table<T> = map<string, T>` | 通过 |
| `GX-06` | `T f<T: Comparable + Printable>(T x)` + 插值 `{x}` | 通过 |
| `GX-07` | 约束为 `int` | `E4036` |
| `GX-08` | `Box<Dog>` 赋给 `Box<Animal>` | `E4001`（`T-4` 不变型） |
| `GX-09` | 泛型递归单态化超 64 层 | `E4038` |

### 16.8 所有权 / 借用 / defer / arena（M7）

| ID | 源 | 期望 |
|---|---|---|
| `O-01` | `var b = new Buffer(8); var c = b; b.write("x")` | `E7001` |
| `O-02` | `f(&x, &x)`（`f` 收两个 `&`） | `E7002` |
| `O-03` | `f(&[1,2,3])` | `E7007` |
| `O-04` | `const int n = 1; g(&n)` | `E7009` |
| `O-05` | 两条 `defer Println("a")` / `defer Println("b")` | 运行输出 `b` 再 `a` |
| `O-06` | `defer Println("{x}")` 在 `int x = 1` 之前 | `E7008` |
| `O-07` | `ArenaRef` 赋给 arena 块外的变量 | `E7006` |
| `O-08` | `string s = "ab"; f(&s)` | 通过 + `W1003` |
| `O-09` | `var b = new Buffer(8); var c = b; b = new Buffer(4)` | 通过（重新赋值复活） |
| `O-10` | `if c { var b = new Buffer(1) } else { }` 之后用 `b`（`b` 只在分支内 move） | `E7001`（保守，`GAP-13`） |
| `O-11` | `for v in dogs { }` 内 `dogs.Push(...)` | `E4026` |
| `O-12` | `defer` 在 `try` 传播前执行 | 运行验证：资源先释放、再返回 |
| `O-13` | `struct S { S next }` | `E3022` |
| `O-14` | `struct S { Dog d }` | `E3023` |
| `O-15` | `struct S { Dog[] d }` | `E3023`（`CONFLICT-2`） |

### 16.9 运行时（M8）

| ID | 源 | 期望 |
|---|---|---|
| `Rt-01` | `int[] a=[1]; int z=2; int x = a[z]` | `R101`，退出码 2 |
| `Rt-02` | `int z = 0; int x = 1 / z` | `R102` |
| `Rt-03` | `decimal u = 100.00d / 3` | `R103` |
| `Rt-04` | `int m = 9223372036854775807; int x = m + 1` | `R104` |
| `Rt-05` | `int m = 9223372036854775807; int x = m.wrapping_add(1)` | 通过，`x == -9223372036854775808` |
| `Rt-06` | `int z = 64; int x = 1 << z` | `R105` |
| `Rt-07` | `float f = 1.0; float z = 0.0; float g = f / z` | `R106` |
| `Rt-08` | 常量 `1 / 0` | `E4009`（编译期，不是 `R102`） |
| `Rt-09` | 常量 `1 << 64` | `E4012` |
| `Rt-10` | `map<string,int> m={"a":1}; int x = m["b"]` | `R101` |
| `Rt-11` | `map` 插入 `c,a,b` 后遍历 | 输出顺序 `c,a,b`（插入序） |
| `Rt-12` | `-7 / 2`、`-7 % 2` | `-3`、`-1`（余数随被除数） |
| `Rt-13` | `decimal t = 19.99d * 3` | `59.97d`（精确） |
| `Rt-14` | `123.456d.round(2, Rounding.HalfUp)` | `123.46d` |
| `Rt-15` | `100.00d.divide(3, 2, Rounding.HalfUp)` | `33.33d` |
| `Rt-16` | `String s = "中文"; s.byte_len()` | `6` |

### 16.10 端到端（M8 总验收）

| ID | 内容 | 期望 |
|---|---|---|
| `RUN-1` | 文稿 §12 完整示例（`Goods` / `DiscountedGoods` / `main`），`db` fixture：`{id:1001,name:"柴犬粮",price:"19.99",stock:10}` | stdout 恰好两行：`折后小计 53.97`、`柴犬粮 ¥19.99（库存 10）`；退出码 0 |
| `RUN-2` | 同上，`stock:0`，调用 `final_price(3)` | 第二行改为 `下单失败：insufficient stock`（`match` 的 `fail` 分支） |
| `RUN-3` | 同上，`load(9999)` 返回空行 | stdout `商品不存在`，退出码 1 |
| `RUN-4` | 递归 `fib(30)` | 正确结果，无栈溢出（`axc -O1`） |
| `RUN-5` | 10 万次 `map` 插入 + 遍历 | 顺序与插入一致，耗时 < 2s |
| `RUN-6` | `IO-1`：`Print("a")` `Print("b")` `Println("c")` `Println("")` `Print("x")` `Println("y")` | stdout 逐字节为 `abc\n\nxy\n`（`Print` 不换行、`Println` 换行） |
| `RUN-7` | `File-1`：`Create` → 两次 `WriteLine` → `Read` / 三次 `ReadLine` → `Append` → 再 `Read` | 中文与行尾无损；第三次 `ReadLine` 得 `null`；追加后内容为三行 |

### 16.11 标准库 IO（M8）

| ID | 源 | 期望 |
|---|---|---|
| `IO-1` | `Print` / `Println` 交替 | 只有 `Println` 补 `\n` |
| `FILE-1` | `file.Create("…") try` + `WriteLine` + `Close`，再 `file.Open` + `Read` | 内容与写入一致；`Read` 返回 `result<string>` |
| `FILE-2` | 同一句柄连续 `ReadLine` 到末尾 | 行尾 `\r` 被剥掉，末尾返回 `null` |
| `FILE-3` | 写只读路径 / 不存在的读路径 | `error.IO`，`e.msg` 为 `cannot open file` |

---

## 17. 收口表：缺口（GAP）与冲突（CONFLICT）

> 表内每一项都是"文稿没写死、实现必须选一个"的地方。**决策只约束实现，不改语言表面**；若设计者另有裁决，替换该行并同步 §16 对应用例。
> `性质` 列：`GAP` = 文稿未覆盖；`CONFLICT` = 文稿两处表述不一致。

| 编号 | 性质 | 问题 | 决策 | 影响 |
|---|---|---|---|---|
| `GAP-1` | GAP | `{...}` 字面量的键能否是表达式 | **必须是字符串字面量**。record 类型需要编译期固定键，运行时键只能是 `map`（用 `m[k]=v` 逐个插） | `E4033` |
| `GAP-2` | GAP | `19d`（整数形式 + `d`）是否合法 | 合法，值为 `Dec{19,0}` | `L-10` |
| `GAP-3` | **CONFLICT** | 11.2 说"插值在任何字符串中生效"，3.5 说 `r"..."` 内容原样 | `r"..."` **不插值**（"原样"优先），普通与多行字符串插值 | `L-17` |
| `GAP-4` | GAP | 需要输出字面量 `{` / `}` 时怎么写 | `{{` → `{`，`}}` → `}`（唯一入口，不新增转义） | `L-18` |
| `GAP-5` | GAP | `T[N]` 能否直接赋给 `T[]` | **不能**（`C-16`）。两者元素存储方式不同，隐式转换会掩盖一次复制；需要时显式复制 | `C-16` |
| `GAP-6` | GAP | 类成员与局部变量同名是否算遮蔽 | **不算**（字段必须 `this.x`，与 `x` 不冲突）；仅变量/参数/循环变量/match 绑定之间算遮蔽 | `N-3` |
| `GAP-7` | GAP | 变量与类型同名 | **禁止**（`E3018`）：`T(x)` 的判定依赖"标识符是类型"，同名会让语义二义 | `N-16` |
| `GAP-8` | GAP | 收窄是否作用于字段/元素 | **只作用于局部变量与参数**。`this.x`、`a.f`、`m[k]`、`nums[i]` 不参与收窄（可能被外部改动） | `T-21` |
| `GAP-9` | GAP | `@os` 的"同一功能多个定义"如何分组 | 分组键 = (名称, 参数类型列表, 返回类型)；同组内按目标平台**恰好命中一个** | `E3028` |
| `GAP-10` | GAP | 定长数组上限 | 单个数组长度的元素总大小 ≤ 8 MiB，超出 `E4041`（避免栈爆） | `E4041` |
| `GAP-11` | GAP | 崩溃时是否执行 `defer` | **不执行**。trap 直接终止，清理链不跑（`Rt-2`） | 生成器不得在 trap 前插 cleanup |
| `GAP-12` | GAP | 分配失败算什么 | 环境限制，非语言崩溃：`R100 out-of-memory`，退出码 2（不列入七类） | `Rt-4` |
| `GAP-13` | GAP | 分支中 move 后汇合的状态 | 只要任一分支 move，汇合后视为 `Moved`（保守）；不做分支敏感合并 | `O-4` |
| `GAP-14` | GAP | `.Step(n)` 的 `n <= 0` | 常量可判 → `E4009`；否则 trap `R101`，detail `step <= 0` | `B-2` |
| `GAP-15` | GAP | `io.ReadLine()` 在 EOF 时返回什么 | `string?`，EOF → `null`。文稿 5.2 的示例需写成 `ReadLine() ?? ""` 或判空 | `§15.4` |
| `GAP-16` | GAP | 元组解构能否带显式类型 | **只能 `var (a, b) = t`**（文稿 5.3 只有这一种写法） | `G-1` |
| `GAP-17` | GAP | `import io.{a}` 与 `import net.http.server` 的解析 | 先解析 `ident ('.' ident)*`；若下一个 token 是 `.` + `{` → 具名导入；是 `as` → 别名；否则是模块路径 | `G-1` |
| `GAP-18` | **CONFLICT** | `for (k, v) in m` 的 `v` 若是 class 引用，move 会掏空 map | `k`/`v` 是**只读借用视图**；要取出引用就 `m[k]`（会 move，且此时禁止遍历期修改的规则不适用——但取出后 map 该位置为空，`GAP-18a`） | `B-2` |
| `GAP-18a` | GAP | 从 map/数组 move 出引用后容器状态 | 位置置为空（`v == NULL`），之后读取该位置 → `R101`；`Len()` 不变 | `§12.6` |
| `GAP-19` | 文稿 15.7 待定 | 尾递归优化 | **M8 不实现 TCO**。文稿已声明"无法优化时不保证不爆栈"，因此不构成违约；`RUN-4` 用 30 层递归验收 | `—` |
| `GAP-20` | GAP | 无字段 struct / class 的空构造 | **class 禁止**：无 constructor 不可 `new`（`E4042`，constructor 是实例化唯一通行证；有字段没构造另由 `E3008` 在声明处拦，一处误用恰好一条诊断）；struct 允许 `Point()`（struct 无 constructor 概念）；有字段时构造签名唯一，无参调用不匹配 → `E4031` | `§6.1` `§6.6` |
| `CONFLICT-1` | CONFLICT | 6.5 说 struct 字段"唯一禁止 class 引用"，8.3 说含容器字段深复制 | 深复制引用容器会**共享可变对象**（两份所有者），故 `T[]`/`map` 字段的元素类型也不得是 class → `E3023` | `T-29` |
| `CONFLICT-2` | CONFLICT | 文稿 15.3 把 record 深复制是否 COW 列为待定 | 实现按**深复制**（语义确定优先），不做 COW；性能问题留待后续，语法不变 | `§12.6` |
| `CONFLICT-3` | CONFLICT | 1.1 "跨模块只读签名" vs 15.7 待定项"参数类型由调用点反推" | 采**只读签名**：`var` 推断结果不跨模块；泛型实参推断只在调用点所在编译单元内完成，结果写入单态化实例名 | `§2.3` |
| `CONFLICT-4` | CONFLICT | 6.2 "方法默认可被覆写"（虚） vs 15.7 待定"是否改默认非虚" | 按文稿现状：**默认虚**，每个类生成完整 vtable；去虚化只作后续优化 | `B-4` |
| `CONFLICT-5` | CONFLICT | 3.1 类型表标 `decimal` 为 16 字节，而"128 位系数 + 标度"无法压进 16 字节 | 实现用 32 字节（`__int128` 对齐要求），**语义不变**（仍是 128 位十进制定点、28+ 位有效数字）；若严格要求 16 字节，改用 BID 编码，仅需替换 §14.3 的 `ax_dec` 与运算函数 | `§12.3` `§14.3` |

### 17.1 新缺口的登记格式（必须使用）

发现 §17 未覆盖的问题时，在报告里追加：

```
GAP-REPORT n
  位置: <章节/规则编号>
  问题: <一句话，具体到"哪种写法、期望什么、文稿哪里没说">
  临时决策: <你实际怎么做的>
  保守性: <为什么这个选择不会放过错误代码>
  需设计者裁决: 是/否
```

**禁止**在没有 `GAP-REPORT` 的情况下自行扩展语法。AI 最容易犯的错是"顺手补一个合理写法"——那正是语言设计失控的起点。

---

## 18. 给 AI 的操作规程

### 18.1 绝对禁止（触碰即视为实现失败）

| # | 禁止 | 原因 |
|---|---|---|
| 1 | 新增关键字、运算符、语句形式 | 文稿 1.2「一件事只有一种做法」 |
| 2 | 加隐式数值转换（哪怕"只加宽化"） | 文稿 1.4 零隐式转换；`T-8` |
| 3 | 加 `while` / `loop` / `switch` / `try-catch` / `goto` / `set` / `list` | 文稿 1.2、1.4 |
| 4 | 给 `float` 开 `==` 例外，或让 `float` 实现 `Comparable` | 文稿 3.7、7.1 |
| 5 | 给 `object` 加成员（`ToString`/`Equals`/`Get`） | 文稿 6.7「没有成员」 |
| 6 | 让 `[]` 越界返回 `null` 或默认值 | 文稿 3.3：`[]` 崩、`At()` 可空，两条路不许混 |
| 7 | 让 `string` 支持 `s[i]` | 文稿 11.2 |
| 8 | 加 `char` 类型或以单字符为特例 | 文稿 3.5、v0.21 |
| 9 | 引入 GC、引用计数（用户可见）、或析构函数 | 文稿 8；`string` 的计数是内部实现 |
| 10 | 让 `map` 迭代顺序依赖哈希 | 文稿 5.2：必须插入序 |
| 11 | 跳过穷尽检查、或对 `object` 的 `match` 允许省 `_` | 文稿 5.3 |
| 12 | 为通过测试而放宽语义检查 | 本语言的产品就是"错的编译不过" |
| 13 | 用 `unsafe`/`extern`/`#include` 实现标准库 | 文稿 15 待定；标准库必须是 intrinsic |
| 14 | 在 `T(x)` 上开第二条语法（`as`、`(T)x`） | 文稿 3.1、9.2 |
| 15 | 修改本文档 A 层规则去"适配"实现 | A 层是语言，只能改语言设计稿 |

### 18.2 每个阶段的标准流程

```
1. 读该阶段对应的规则编号清单（§0.3 + 各章表格）
2. 先写"负例"：让每个 E#### 至少有一个用例失败（红）
3. 再写实现，直到负例全部报出正确码、正例全部通过（绿）
4. 跑 --dump 检查中间产物（AST/types/HIR）是否符合 §5.3 格式
5. 输出自检报告：已实现规则编号 / 未实现 / GAP-REPORT / 已知缺陷
6. 不得进入下一阶段，除非本阶段用例 100% 通过
```

### 18.3 遇到歧义时的三步

1. **查**：§17 → 文稿 → 本文档其他章节。
2. **判**：若仍无答案，选"能静态报错"的一侧（而不是"能跑通"的一侧）。
3. **记**：写 `GAP-REPORT`，并在代码处留 `// GAP-REPORT n: ...` 注释，便于后续统一裁决。

### 18.4 代码质量硬要求

| 要求 | 判据 |
|---|---|
| 诊断可定位 | 每条 error 都有 `file:line:col` 且指向**最相关**的 token（不是行首） |
| 无 panic | 编译器自身不得因输入错误崩溃（`Result` 传播，不用 `unwrap`） |
| 确定性 | 同一输入两次编译产出的 `.c` **逐字节相同**（禁遍历 HashMap 决定顺序） |
| 单遍可读 | 每个 `check_*` 函数只做一件事；类型规则集中在 `types.rs`，不得散落在 codegen |
| 有测试 | 每条规则至少一个用例；§16 的表格是下限，不是上限 |

---

## 19. 阶段自检清单

> 每个阶段结束时逐条勾选，未勾选即未完成。

**M1 词法**
- [ ] 36 个关键字、全部运算符、多字符最大匹配（含 `..=`/`?.`/`??`）
- [ ] `1..10` 不被误切为浮点
- [ ] 三种字符串 + 6 个转义 + `\u{}` + 插值片段化（含嵌套 `"`）
- [ ] 嵌套块注释 + 注释内换行仍产生 `NEWLINE`
- [ ] 位置（行列，码点计）在 `L-01`…`L-15` 全部正确
- [ ] 非法 UTF-8 / `\0` / 单引号 各有专属诊断与修复建议

**M2 语法**
- [ ] 完整 EBNF 落地，`--dump=ast` 格式符合 §5.3
- [ ] `nl_ok` / `expect_terminator` 位置正确（`G-02`…`G-05` 全绿）
- [ ] 六处歧义消解全部实现（`G-10`…`G-15`）
- [ ] 错误恢复不产生"假错误风暴"（单次 ≤ 20 条）
- [ ] 文稿 §12 示例可完整解析

**M3 名称与标量类型**
- [ ] 作用域链、禁遮蔽、未定义、重复声明
- [ ] 可见性三档 + `E3011` 的"不许写修饰词"三条
- [ ] `N-20` 的 `T(x)` 决议顺序与 `E4032`
- [ ] 字面量按目标类型落地（`T-01`…`T-05`）
- [ ] 15 级优先级 + 比较不可连写 + `&` 高于 `==`

**M4 聚合类型**
- [ ] `T[N]` 常量下标静态核对；`T[]` 直读直写 + `At`
- [ ] record 推断（全同 → map，杂 → record）、键固定、`E4008`
- [ ] struct 字段白名单 + 深复制 + 禁 class 引用
- [ ] class vtable 布局、`override` 三条检查
- [ ] interface 显式实现 + 默认方法 + itab
- [ ] enum 四条规则

**M5 错误处理**
- [ ] `try` 绑定最紧 + 不可接 `.` + 只在 `result` 函数内（`main` 例外）
- [ ] `match` 七种模式 + 穷尽 + 三条配套规则
- [ ] 可空收窄两条形态 + `must_exit`
- [ ] `R101`…`R107` 插入点全部就位（含"常量走编译期"分流）

**M6 泛型**
- [ ] 约束检查（`Comparable`/`Hashable`/`Printable` + 用户 interface）
- [ ] 单态化去重 + mangle 稳定 + 深度上限
- [ ] `float` 无 `Comparable`（`GX-03` 报 `E4036`）

**M7 所有权**
- [ ] move 位置穷举（`O-6`）+ 状态机 + use-after-move
- [ ] 借用单语句冲突 + 不可逃逸
- [ ] `defer` LIFO + 早退触发 + 不引后声明变量
- [ ] `arena` 逃逸检测

**M8 代码生成**
- [ ] 求值顺序严格左到右（临时变量方案）
- [ ] 统一 `emit_cleanup_until`，五类早退共用
- [ ] `move` 后置空 + drop 容空 → 无 double free
- [ ] map 插入序、`decimal` 精确运算、字符串码点语义
- [ ] `RUN-1`…`RUN-5` 全绿；生成的 `.c` 可被 `clang -std=c11 -Wall -Werror` 编译通过

**M9 收尾**
- [ ] §16 全部用例通过（含 12 个 `E3xxx` 声明类负例）
- [ ] `-Werror` 下文稿示例零警告
- [ ] 两次编译产物逐字节一致
- [ ] 自检报告：规则覆盖率、GAP-REPORT 清单、已知缺陷

---

## 附：与文稿的章节对照

| 本文档 | 文稿 | 说明 |
|---|---|---|
| §3 词法 | 2.1–2.3、3.5 | 补：位置、token 全集、插值片段化算法 |
| §4 语法 | 全章示例 | 补：完整 EBNF、语句边界算法、六处歧义消解 |
| §6 名称解析 | 3.3 可见性、6.1–6.5、9.1–9.3 | 补：命名空间、`T(x)` 决议顺序 |
| §7 类型系统 | 3.1–3.7、6.7、7 | 补：可赋值算法、`P1..P8` 穷举、收窄算法、`C-1..C-20` |
| §9 诊断码 | 各章"编译错误" | 补：编号体系（本文档新增，便于测试断言） |
| §10 崩溃点 | 10.2 | 补：插入位置表、输出格式、`defer` 不执行 |
| §11 所有权 | 8.1–8.4 | 补：move 位置穷举、单语句借用扫描算法 |
| §12 布局 | 6.6、8.3、6.1 static | 补：字节级布局、vtable/itab、drop 函数 |
| §13 代码生成 | 6.1「实现层面」 | 全新（文稿只给了一句 mangle 示例） |
| §14 运行时 | 3.2、11.2 | 全新 |
| §16 测试 | — | 全新 |
| §17 收口 | 15（待定事项） | 全新：把"待定"变成"实现必须选一个" |

