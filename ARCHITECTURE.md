# axc 架构与开发指南（接手必读）

本文是给"接手的人"看的：读完应该知道代码怎么分层、为什么这么设计、
改一处要动哪些文件、怎么验证没改坏。语言语义的**真相来源**是
`axlang语法设计文稿.md`，实现规格是 `axlang编译器实现规格书.md`，
本文只讲实现侧的"为什么"与"怎么做"。

## 1. 项目一览

| 组成 | 位置 | 说明 |
|------|------|------|
| 编译器 axc | `rust/` | Rust 实现，唯一在维护的实现；产物单 exe 约 1.1 MB |
| C 运行时 axrt | `axc/rt/axrt.{h,c}` | 生成代码 `#include "axrt.h"` 并链接；与实现语言无关 |
| 测试 | `tests/` | §16 用例驱动器 + 七类用例目录（含 `mod` 多文件与 `demo` 示例工程） |
| 文档生成 | `tools/` | `gen_errdoc.py`（docs/ERRORS.md 错误清单）与 `gen_stdlib_doc.py`（docs/STDLIB.md 内置库说明），改诊断文案/内建方法后重跑 |
| 示例 | `samples/` | 12 个可直接运行的 `.ax` 程序 + `demo/` 多模块工程 |
| 分发产物 | `dist/axc.exe` | release 构建拷贝，测试脚本自动探测它 |
| 历史归档 | `legacy/` | Python 原型 zip（已退役，仅考古用） |

文档地图：

- `axlang语法设计文稿.md` —— 语言设计真相（语法、语义、标准库）
- `axlang编译器实现规格书.md` —— 实现规格（文法编号、诊断码、运行时契约）
- `GAP-REPORT.md` —— 规格与实现之间的已知缺口
- `RUST-MIGRATION.md` —— Python→Rust 迁移史与验收记录
- `CHANGELOG.md` —— 版本变更（含 cx→ax 更名史）

## 2. 编译管线与数据流

```text
 .ax 源码 (UTF-8)
    │  frontend::lexer    字符流 → Vec<Tok>（码点列号、插值片段化）
    ▼
 Vec<Tok>
    │  frontend::parser   Tok 流 → ast::Module（错误恢复、歧义消解）
    ▼
 ast::Module ──────────────► sema：检查结论全部写入 Sema::slots（键=节点 id）
    │
    ▼
 AST + slots
    │  backend (mod / codegen_fn / codegen_expr)
    ▼
 单个 C 翻译单元（纯 ASCII）+ axrt.h
    │  cli：调系统 cc（gcc，可 --cc 指定）
    ▼
 可执行文件
```

分层规则（依赖单向，违反即架构腐化）：

- `frontend` 不依赖 `sema` / `backend`；`ast` 只依赖 `lexer`（用 `Tok`）。
- `sema` 依赖 `frontend`（AST/Tok）与 `types`；产出只有 `Diags` + `slots`。
- `backend` **只读** AST 与 `slots`，不做任何重新检查；语义有疑问去查 sema。
- 跨阶段公共类型：`Tok`（lexer）、`Type/Ty`（types）、`Diags`（diag）。

## 3. 关键设计决策（为什么）

1. **类型用 `enum TyS` + `Rc`，结构相等即可作 HashMap key**。
   全项目的类型判定（canonical、assignable）只走 `types.rs`——
   改可赋值性规则只改这一个文件，sema/backend 不允许私写判定。

2. **语义槽 `Sema::slots: HashMap<u32, Slot>`**。
   每个节点有全局唯一 `id`，sema 把结论（类型、解析结果、转换方式、
   常量折叠值……）写进槽；backend 按节点 id 读取。这是 Rust 版
   与"把属性挂节点上"的动态语言实现的最本质差异，找"某表达式
   的类型在哪"一律先查 `Slot`。

3. **`tag_of` 是 ABI 约定**：复合类型标签 = 1000 + fnv1a32(无 namer
   的 canonical) % 100000。刻意不带 namer，否则跨编译单元对不上。
   改它=破坏所有已编译产物兼容性。

4. **生成的 C 是纯 ASCII**：非 ASCII 字符串字面量经 `cstr()` 转义成
   `\xHH` 字节序列，运行时按 UTF-8 fwrite。Windows 控制台代码页由
   `ax_console_utf8()` 在**交互式终端**（`_isatty`）才切换为 UTF-8，
   退出时恢复——重定向/管道不受影响，这是逐字节比对测试能工作的前提。

5. **vtable 约定**：类结构首两字段固定 `vt`、`__tag`；实例方法一律
   经 vtable 调用，条目首参 `void*` 规避函数指针类型不兼容。

6. **try/catch/finally 的展开**（backend/codegen_fn.rs）：
   - `FnGen::try_stack` 每层一个 `TryCtx`（错误变量、跳转标签、
     defer 基准深度、finally 体）；
   - `target` 指向"下一层"：try 体内 → catch 链入口，catch 体内 →
     finally 入口（catch 里再抛不能被自己重新捕获）；
   - `return` 先算值落临时，再按内→外补跑 finally 链；
   - **只写 finally 也生成 rethrow 段**——否则错误被吞（TRY-3 钉死）；
   - 未匹配错误：外层有 try 就转发，否则才走 return fail / main 退出。

7. **E6013（break/continue 跨 try 边界）判据用 `loop_depth`**，
   不能用 `loop_labels.len()`——后者只对带标签的循环入栈，
   无标签循环不占位，会把合法的 `try { for { break } }` 误判。

8. **对外契约，一经发布不改**：
   - 退出码：0 成功 / 1 有 error 级诊断 / 2 目标程序运行时崩溃；
   - 诊断码与文案（测试以期望输出逐字钉死）；
   - `--dump=ast` 的序列化格式。

9. **"历史行为"不"顺手修正"**。代码里标注 `历史行为（测试钉死）`
   的地方（如 SafeCall 恒生成 "0"、E7008 不可达），是自原型时代
   沿用、已被测试锁定的行为。觉得奇怪先查 CHANGELOG 与对应测试，
   真要改就升级版本号 + 改测试 + 记 CHANGELOG，三件套缺一不可。

## 4. 如何加一个语言特性（标准动作）

以"给语言加一个新的运算符/语句"为例，七步走：

1. **设计文稿**：先在 `axlang语法设计文稿.md` 定语义与文法（含反例），
   与既有特性的交互写清楚。文档先行，不写"边写边定"的代码。
2. **词法**（`frontend/lexer.rs`）：新关键字加进关键字表（现在是
   36 个），新 token 形态进 `TokVal`。
3. **语法**（`frontend/parser.rs`）：文法编号接入，注意 `nl()`
   与 `expect_term()` 的分工（G-2/G-4），歧义消解走 Mark 回溯。
4. **AST**（`frontend/ast.rs`）：新节点 + `dump` 序列化 + `id`。
5. **语义**（`sema/`）：检查逻辑按"语句→stmt.rs、表达式→expr.rs、
   走 `.`/`[`→member.rs"落位，结论写入 `slots`；错误给新诊断码
   （码段分配见规格书）。
6. **后端**（`backend/`）：按节点生成 C；若引入新的控制流/错误
   语义，检查 `try_stack` / `defers` 的交互（跳转不得越过 finally）。
7. **测试三件套**：`tests/` 加正反例（负例 `// EXPECT: E####`，
   正例 `run/` 放 `.out/.exit`）；`samples/` 加可运行示例（同步
   `samples/README.md`）；文档同步（语法文稿 + README + CHANGELOG）。

验收标准：`cargo build` 零警告、`python tests/runtests.py` 全绿、
`python samples/run_all.py` 全绿。三者任一红即不得提交。

## 5. 构建与测试

```bash
cd rust && cargo build --release     # 产物 rust/target/release/axc.exe
cp target/release/axc.exe ../dist/   # 更新分发产物
python tests/runtests.py             # 全量 99 项（自动探测 axc.exe）
python tests/runtests.py run -v      # 只跑端到端 22 项，逐条打印
python samples/run_all.py            # 11 个示例
```

Windows 下 `cargo` 找不到时用 `rust\build.cmd`（自带 PATH 回落）。
工具链备注（GNU target + rust-lld 的原因）见 `RUST-MIGRATION.md`。

## 6. 代码约定

- 每个源文件头部 `//!` 写：职责、输入输出、关键不变量、与邻接文件的分界。
  这是读代码的第一入口，改文件先改注释。
- 公共类型/方法用 `///` 说清语义与不变量；不写"见实现"式的废话注释。
- 诊断码一经发布不改字；新增诊断码先查规格书码段分配表。
- `#![allow(dead_code)]` 只为有意保留的预留接口存在（main.rs 里列了
  清单）；新增死代码要么删掉，要么补进清单说明为什么保留。
- 源码统一 UTF-8 无 BOM；注释说中文，标识符与诊断码用英文。

## 7. 已知缺口

- 未实现：泛型单态化、完整借用生命周期、arena、匿名函数后端、
  json/file/time 模块（详见 `GAP-REPORT.md` §3.2）。
- 选项缺口：`--dump=types` 未实现；`-Werror` 能解析但未生效
  （详见 `GAP-REPORT.md`）。

## 8. 版本策略

- 版本号**单一来源**是 `rust/Cargo.toml` 的 `version`，`-V/--version`
  与生成的 C 头部注释都从它取（`CARGO_PKG_VERSION`），改版本只改一处。
- **雏形期从 0.0.1 起步，每达成一个里程碑 +0.0.1**，进位要慢——
  语言远未稳定，版本号不追功能节奏，只记录里程碑。当前：
  0.0.1（原型）→ 0.0.2（Rust 迁移）→ 0.0.3（try/catch + 工程化）→ 0.0.4（标准库方法名 PascalCase）→ 0.0.5（`Print`/`Println` 拆分、`File` 实化）→ 0.0.6（顶层语句诊断明确化）→ 0.0.7（`-I` 生效、内建模块名保留 `E3039`）。
- **1.0 及以上永远保留给语法冻结**（语言设计不再做破坏性变更）之后，
  期间无论加多少特性都不进位到 1.x。
- 阶段进位的判定条件（满足才允许进位）：
  - 升 **0.1.0**：语法冻结——规格书全部条目实现或明确移出，
    GAP-REPORT 清零或仅剩非语言项，连续两个里程碑无破坏性变更；
  - 升 **1.0.0**：0.1.x 稳定运行一个时期、对外有真实使用者之后。
- 每次 `Cargo.toml` 改版本，**必须同步在 `CHANGELOG.md` 加对应条目**；
  历史版本号一经写入 CHANGELOG 不再重编号。
- CHANGELOG 按"语言 / 编译器 / 工程与文档"三栏记，重大决策写明理由。
