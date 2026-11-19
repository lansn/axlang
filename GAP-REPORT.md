# GAP-REPORT — axc 0.0.3 实现缺口与自检报告

> 依据 `axlang编译器实现规格书.md` §0.1 铁律 3、§17.1 登记格式与 §18 操作规程产出。
> 记录对象：**文稿（A 层）没写死、规格书 §17 也未收口**的点，以及本次实现的覆盖率自检。

生成时间：随本次实现一并产出（M0–M8 首轮打通后）
宿主语言：Rust（`rust/`，见 `RUST-MIGRATION.md`；早期 Python 参照实现已归档 `legacy/python-impl-1.0.zip`）
后端：C11 + `axrt` 静态链接，调用系统 gcc（严格按 D-02，未改）

---

## 一、实现决策变更（B 层，允许改但须一致）

### `D-01'` 宿主语言由 Rust 改为 Python

- **位置**：§0.2 B 层 / D-01
- **问题**：本机无 Rust 工具链（`cargo` 不可用）。D-01 属于 B 层（实现决策），A 层语言表面不受影响。
- **临时决策**：编译器前端用 Python 3.13 实现；语言表面（词法、语法、类型、语义）**一行不改**；后端严格按 D-02 生成 C11 并链接 `axrt`。
- **保守性**：语言行为由规格书决定，不由宿主语言决定；`axc` 仍是单一可执行入口（`axc.exe`），诊断格式、退出码、目录结构均按 §1 实现。
- **后续**：Rust 迁移已全部完成（见 `RUST-MIGRATION.md`），Python 版仅作参照实现归档在 `legacy/python-impl-1.0.zip`。
- **需设计者裁决**：否（B 层，若后续要 Rust 实现，前端可整体重写，A 层用例可直接复用）

### `D-03'` E3025（无 `main`）的判定时机

- **位置**：§9.4 / `E3025`
- **问题**：§16 的负例几乎都是"模块片段"（只有 class / 函数，没有 `main`）。若 `check` 阶段强制要求 `main`，这些用例会额外多出一条 `E3025`，违反 §16"负例恰好一个诊断码"。
- **临时决策**：`axc check` 允许模块是库（不要求 `main`）；`axc build` / `axc run` 产出可执行文件时要求 `main`，缺失 / 重复 / 签名不符均报 `E3025`。
- **保守性**：真正要生成程序时一定报错，不会产出无法启动的可执行文件；只是把"是不是程序"这个判断放到链接阶段。
- **需设计者裁决**：是（若要求 `check` 也强制 `main`，把 `require_main` 默认改为 `True` 即可）

---

## 二、GAP-REPORT 条目

### GAP-REPORT 1 — `decimal` 与整数混算

- **位置**：文稿 §3.1 / §11.1；规格书 `T-8`、`E4009`；文稿 §12 示例 `this.price * count`
- **问题**：`T-8` 明确"整数↔`decimal` 必须 `T(x)`，无隐式转换"，但文稿 §12 的正式示例写了 `this.price * count`（`decimal * int`），`RUN-1` 又要求它编译通过并算出 `53.97`。两处冲突。
- **临时决策**：算术运算符（`+ - * /`）允许 `decimal` 与整数家族混算，整数在运行期精确提升为 `decimal`（依据 `C-3`「整数 → decimal 允许，无损」），结果类型 `decimal`。**变量之间**仍按 `T-8` 禁止隐式转换（`decimal d = intVar` 仍是 `E4003`）。
- **保守性**：只对"算术运算符的操作数"开口，且提升方向唯一（整数→decimal，无损）；不引入 `int` 与 `float` 的任何隐式转换，也不允许 `decimal → int` 的隐式下行。
- **需设计者裁决**：是（若裁定"必须显式"，则文稿 §12 需改成 `this.price * decimal(count)`，本实现回退该条）

### GAP-REPORT 2 — 整数字面量落到 `decimal`

- **位置**：规格书 `T-9`（字面量按目标类型落地）；文稿 §3.2 示例 `100.00d.Divide(3, 2, Rounding.HalfUp)`
- **问题**：`divide(除数, 小数位, 舍入)` 的第一个形参是 `decimal`，而示例传的是整数字面量 `3`。按 `T-8` 字面也不该隐式转换，但 `Rt-15` 要求它编译通过并输出 `33.33`。
- **临时决策**：整数字面量在"目标类型为 `decimal` 的位置"直接落地为 `decimal`（视为 `T-9` 的推广，字面量阶段完成、无运行期转换）；`float` 字面量赋给 `decimal` 仍按 `C-6` 报 `E4003`。
- **保守性**：只作用于**字面量**，不作用于变量；`decimal d = 1.5`（float 字面量）仍报错，不会掩盖"二进制浮点混进金额"这一真实错误。
- **需设计者裁决**：是

### GAP-REPORT 3 — `override` 用于实现 interface 声明的方法

- **位置**：文稿 §6.2 / §6.3；规格书 `E3016`；文稿 §12 的 `public override string describe()`
- **问题**：`E3016` 的文案已写成"没有对应的**父类或 interface**方法"，但 §6.2 的正文只在讲父类覆写；`DiscountedGoods` 的 `describe` 父类没有、interface 有，需要明确"这是合法的 `override`"。
- **临时决策**：`override` 的合法来源 = 父类同名方法 ∪ 已 `implement` 的 interface 方法；父类命中时额外校验签名（`E3019`），interface 命中时按 §6.3 校验签名。
- **保守性**：仍是"必须真有对应声明才能写 `override`"，不会放过拼错方法名的情况（父类、interface 都没有 → `E3016`）。
- **需设计者裁决**：否（`E3016` 文案已隐含此结论）

### GAP-REPORT 4 — 词法错误后是否继续语法分析

- **位置**：规格书 §4.5（只规定"有 `E2xxx` 不得进入类型检查"）、§16（负例恰好一个诊断码）
- **问题**：词法错误（如 `$`）后 token 流已不可信，继续解析会追加同源的假语法错误（实测 `1 $ 2` → `E1006` + `E2002`）。
- **临时决策**：**出现任何词法错误即停止后续阶段**（不进语法、不进类型检查）。语法错误同样不进类型检查（按 §4.5 原文）。
- **保守性**：只减少噪声，不改变"是否报错"的结论；一处词法错误只报一条。
- **需设计者裁决**：否

### GAP-REPORT 5 — `db.Query` 查无记录是 `fail` 还是空行

- **位置**：规格书 §15/§16.10 `RUN-3`；`ax_db_query_result` 的运行时语义
- **问题**：文稿 §12 里 `db.Query(...) try` 之后紧接 `if row.Empty() { return ok(null) }`，说明"查无记录"是**空行**而不是错误；但 §15 的 `db` 签名只写"见 §16 fixture"，没有规定未命中的返回形态。`RUN-2`（`stock:0`）与 `RUN-3`（`id` 不存在）需要区分"查到但字段为 0"与"查不到"。
- **临时决策**：`db.Query` 返回 `result<Row>`；**查询成功但无记录 → `ok(空行)`**（`row.Empty() == true`）；**没有夹具 / 环境不可用 → `fail(error.IO)`**。夹具来自环境变量 `AX_DB_FIXTURE`，形如 `id=1001,name=柴犬粮,price=19.99,stock=10`。
- **保守性**：`try` 只在"环境/驱动层失败"时中断；"数据不存在"走业务分支（`row.Empty()`），与文稿 §12 的控制流一致，不会把正常业务分支变成崩溃路径。
- **需设计者裁决**：是（若定为 `fail`，则 `RUN-3` 的期望输出需改为 stderr 报错 + 退出码 1）

### GAP-REPORT 6 — 夹具文本的编码

- **位置**：§16 夹具定义；`axrt` 的字符串/输出约定
- **问题**：Windows 下窄字符 `getenv` 返回 ANSI 代码页字节（GBK），与源码中的 UTF-8 字面量混排会输出乱码，`RUN-1` 的逐字节比对必然失败。
- **临时决策**：运行时统一经 `ax_env_utf8()` 读环境变量（Windows 走 `GetEnvironmentVariableW` + `WideCharToMultiByte(CP_UTF8)`），非 Windows 直接 `getenv`。
- **保守性**：只影响外部输入的解码，不改变语言语义；`ax_str` 仍是"UTF-8 字节 + 字节长度"。
- **需设计者裁决**：否

### GAP-REPORT 7 — 类型错误后的"级联抑制"

- **位置**：§16"负例恰好一个诊断码"
- **问题**：一处错误（如 record 未定义键、可空类型直接取成员）会让表达式失去类型，后续检查再报一条"类型不允许"，形成假错误风暴。
- **临时决策**：引入内部占位类型 `T_UNKNOWN`（`("unknown",)`）：被报错位置返回它，`assignable` 视它与一切类型相容；`E4022`（可空成员访问）在报错后返回字段真实类型，避免后续二次报错。
- **保守性**：`T_UNKNOWN` **只在已经报过 error 之后**出现，不会让错误代码静默通过（有 error 就退出码 1）。
- **需设计者裁决**：否（属实现内部机制，不进入语言表面）

---

## 三、自检报告（§0.4 / §18）

### 3.1 已实现（按里程碑）

| 里程碑 | 内容 | 状态 |
|---|---|---|
| M1 词法 | 36 关键字、长运算符优先、三类字符串、`{{`/`}}`、插值片段化、`r"..."`、嵌套块注释、`0x/0b/0o`、`1_000`、`d` 后缀、码点列号换算、`L-01…L-15` | 完成 |
| M2 语法 | 全部语句/表达式、`nl()` 与 `expect_term()` 分工、`G-10` 投机解析、`G-11` 类型模式、`G-12` 标签、`G-13`/`G-14` 语法层不判定、`G-16` 恢复（≤20 条）、`E2003`/`E2004`/`E2009` | 完成 |
| M3 名称 | 作用域、遮蔽 `E3001`、可见性、`constructor` 全量赋值 `E3009`、`override` `E3016`/`E3017`/`E3019`、struct 规则 `E3021`/`E3022`/`E3023`、`E3025` | 完成 |
| M4 类型 | 全部类型构造、`assignable` 唯一入口、`T-8` 零隐式、字面量落地、可空收窄 `T-21`–`T-24`、`object` 装箱/解箱 `P1…P8`、记录键固定 `E4008` | 完成 |
| M5 错误/`match` | `result<T>`、`try`、`throw`、七种模式、穷尽检查 `E6001`/`E6002`/`E6006`/`E6007` | 完成 |
| M6 泛型 | 解析、约束检查 `E4036`、单态化（函数+类+struct+别名）、深度上限 `E4038` | **已完成**（v0.0.21）：泛型函数/类/struct 按实例单态化，别名引用期展开，不变型天然成立（实例 id 不同） |
| M7 所有权 | `E7001`（move 后使用）、`E7002`/`E7004`（借用单语句冲突/借用期 move）、`E7005`（Alloc 类型）、`E7006`（`ArenaRef` 逃逸）、`E7007`/`E7009`（借用目标/const）、`W1003`（多余借用）、分支汇合 `O-4`、循环两遍 `O-5`、`defer` 逆序、`arena`/`ArenaRef` 后端与整块释放 | **已完成**（v0.0.22）：`O-01…O-20` 全通（`E7008` 恒不可达，行为契约） |
| M8 后端+运行时 | C11 生成（继承拉平、vtable、构造/析构、`defer` 展开、decimal/整数/移位/浮点 trap）、`axrt`（字符串、decimal、数组、map 插入序、Row/db 夹具）、`RUN-1…RUN-4`、`Rt-01…Rt-16` | 完成 |
| CLI | `check`/`build`/`run`、`--emit=c`、`--dump=ast\|types\|c`、`--diag=text\|json`、`-Werror`、`--cc`、退出码 0/1/2 | 完成 |
| 分发 | `cargo build --release` 产出单文件 `dist/axc.exe`；`samples/` 10 个可直接运行的程序 + `run_all.py`；运行时 `ax_console_utf8()` 在 Windows 控制台自动切 UTF-8 并在退出时恢复 | 完成 |

### 3.2 未实现（已知缺陷，按影响排序）

| # | 缺口 | 影响用例 | 说明 |
|---|---|---|---|
| ~~1~~ | 泛型单态化 | `GX-01…GX-09` | **已修**（v0.0.21）：`(函数, 类型实参)` 生成 FuncInfo 实例、`(类, 类型实参)` 生成实例 typedef，体按实例重查、codegen 只生成实例；推断（实参统一 + 期望兜底）、约束检查、mangle 去重、E4038 深度上限 |
| ~~2~~ | 完整所有权分析 | `O-03`、`O-05…O-08`、`O-10…O-12` | **已修**（v0.0.22）：借用改为强度感知的单语句扫描（多 `&const` 合法、`&`+`&const`/move/写报 E7002/E7004）；分支汇合按各分支快照独立检查后保守合并（O-4）；循环体两遍检查（probe 收集 → 合并状态正式查，O-5）；`W1003` 多余借用警告；修 O-9 复活（对已 move 变量赋值的左值读取不再误报 E7001）。**设计上不存在跨语句借用**（借用不可存储，文稿 8.2），故无需生命周期分析；`E7008`（defer 引后声明变量）恒不可达——注册时检查已由名字解析天然拦截，行为契约保留不触发 |
| ~~3~~ | `arena` 语句与 `ArenaRef` | `O-07`、`O-17…O-20` | **已修**（v0.0.22）：`axrt` 增加 `ax_arena_create/adopt/free`；`new Arena()` 真实生成（原 `NULL` 占位）；`arena.Alloc(x)` move 进区域、返回 `ArenaRef<T>`（copy 语义）；`ArenaRef<T>` C 类型 = T 的指针形态；区域变量离开作用域整块释放（复用 defer 清理栈）；`E7006` 逃逸检测（作用域绑定 + return/实参/普通对象字段/容器四处拦截）；`E7005` 只收养 class 实例/object 盒子 |
| 4 | 函数值（文稿 4.1）的边角 | §4.1 | **主体已实现**（v0.0.3）：`func(...) T` 类型语法（此前解析器把 `func` 当普通类型名，整章语法不可用）、匿名函数字面量（合成顶层函数、不捕获环境）、顶层函数/静态方法引用（`twice`、`Num.neg`）、函数值经参数传递与间接调用。**注**：匿名函数不捕获环境是文稿 4.1 的明文设计（583 行：函数值就是 8 字节函数指针），不是缺口。**剩余**：泛型函数值实例化 |
| 5 | ~~`json` / `time` 模块~~ **已修（0.0.18）** | §15 全表 | `json.Parse` / `json.Stringify` / `time.NowMs` 已从桩补为真实实现：手写递归下降 JSON 解析（object/array/string/int/float/bool/null、`\uXXXX` 代理对、错误定位消息）、stringify 往返（嵌套 map/数组 + 转义）、`now_ms` 用 `GetSystemTimeAsFileTime`。`json` 与 `time` 仍需显式 `import`。历史描述：只有签名与桩实现。**`file` 已在 v0.0.5 实化**：`Open`/`Create`/`Append` + `Read`/`ReadLine`/`Write`/`WriteLine`/`Close` 全部经 `ax_file` 句柄走真实 `FILE*`（二进制模式），`ReadLine` 剥 `\r`；此前 `file.Open` 生成的 `ax_file` 类型根本未定义（一用就 C 编译失败）、`ReadAll` 恒返回 `fail("file not supported")`。`json` 只有 `Parse`/`stringify`，`time` 只有 `now_ms` |
| 6 | `string` 方法子集 | 部分 | `len`/`byte_len`/`at`/`contains`/`split`/`trim` 已通，其余按桩 |
| 7 | `--dump=hir` | §1.2 | 未产出 HIR 层 dump（`ast`/`types`/`c` 已有） |
| 8 | `R101` 消息带实际键值 | §10.1 | map 越界目前 `keydesc` 固定为 `"key"`，未拼出 string 键的真实值 |
| 9 | 用户模块 `import` 的边角 | §9 | **主体已实现**（v0.0.3）：多文件编译、`import mod` / `mod.{item}` 具名导入 / `as` 别名、点号路径（`import net.http` → `net/http.ax`）、循环导入 E3026、可见性（默认私有 / `public` 导出，E3030）。**剩余边角**：模块类型的嵌套类型（`mod.Outer.Inner`）、跨模块继承/override、跨模块泛型约束（`mod.Iface` 作约束）未支持。模块路径相对**入口文件**目录解析（已写进文稿 9.2）；3 段以上模块路径（`net.http.server.ping()`）的下钻**已在 v0.0.7 修**；`-I <dir>` 追加搜索根**已在 v0.0.7 实现**（此前 spec 有、实现忽略）；内建模块名重名现在报 `E3039` 而不是静默用内建实现 |
| 10 | ~~数组 `Sort(cmp)` 未生成代码~~ **v0.0.17 已修** | 任何排序场景 | sema 有签名（`func(T,T) int`）、规格书 15.3 有定义，但后端 `intrinsic` 无 `Sort` 分支——`xs.Sort(cmp)` 走默认分支**静默返回 `"0"`**，数组原样不动且无任何诊断。**修法**：每个调用点派生文件级 qsort 适配器——比较器值暂存进 `static axfn_* axsort_ctx_N`，`static int axsort_cmp_N(const void*, const void*)` 用 memcpy 取元素后调比较器（值/指针元素统一、无 const 纠纷），声明进 `fn_decls`（type_defs 之后、调用点之前）、定义进 `extra_fns`（fns 之后）。匿名函数 / 命名函数 / 函数值均可作比较器；int/string/class/struct 元素皆可。回归用例 `tests/run/Sort-1.ax`，`-Werror` 零警告 |
| 11 | 具名导入只做白名单、未绑定名字 | `import m.{a}` 后的裸 `a(...)` | **v0.0.9 已修主体**：裸名解析在「本地名 → 当前模块限定名」之后加了一站——查具名导入清单指向的模块，函数 / 常量 / 类型三种导出统一生效（`greet("x")`、`{PORT}`、`new Config(PORT)`），白名单语义不变（`m.c` 仍 `E3030`）。**剩余**：两个模块引入同名符号时按视图名字典序取第一个，**不报歧义**——要么补 `E3xxx` 歧义诊断，要么在文稿里把这条写死为规则 |
| 12 | 复合类型的默认显示未实现 | 插值数组 / class / struct / map | 规格书 §13.7 与文稿 6.2 原写"所有类型都能插值"（数组 `[a, b]`、class `ClassName`、struct `Name{f: v}`），实际 codegen 只有标量/`string`/`enum` 分支，其余落到兜底分支**静默打印一个 `?`**——既不报错也不给可读输出，是可静默骗过测试的那种缺陷。`v0.0.8`：①可空值（`T?`）**已实现**；②其余复合类型改报 `E5006` 明确拒绝。`v0.0.17`：③**`Printable` 真正接入**——class / struct 实现 `ToString()` 后 `{p}` 直接插值，`Printable` 接口变量经虚表派发，可空形式有值显示 / `null`；未实现的 class 插值仍 `E5006`（回归 `Printable-1` / `Printable-neg-2`）。**剩余**：数组 / `map` / 元组的默认显示格式仍未定（继续 `E5006`，等显示格式定稿后补 codegen） |
| 13 | ~~interface 类型的变量 / 参数 / 字段不可用~~ **v0.0.16 已修（核心）** | `Shape s = c`（`c: Circle`，`Circle implement Shape`） | sema 侧照常接受（隐式上转，文稿 6.3），但 codegen **没有为 interface 生成任何 C 类型**：`ct()` 拼出 `Shape`（跨模块时 `model__Shape`）却没定义 → `unknown type name 'Shape'`，C 编译失败。**修法（胖指针 + shim 虚表）**：①interface 落成 `Iface_X { void* obj; Iface_X_VT* vt; }`，VT 条目按 `all_iface_methods`（自身在前、extends 追加在后）拉平；②每个 (类, interface) 组合生成 shim `Class__Iface__VT_`（实现沿继承链找 override，找不到用 interface 默认方法体；父 interface 传递展开——实现 `Pet` 即有 `Dog__Animal__VT_`）；③上转（声明/赋值/传参/return）在 `coerce_c` 组装胖指针；④interface 接收者按**接收者的静态 interface** 取 VT 布局经 `tv.vt` 动态派发（方法来自 extends 链时 owner 与接收者不是同一 interface，布局不能混用）；⑤类接收者调 interface 独有方法走该类 shim 地址；⑥`find_method` 补 interface extends 链、方法归属改 `funcs[m].owner`。**已覆盖**：interface 变量/参数/返回值/字段/数组元素、可空 interface、interface extend、跨模块 interface（`Mod-8`）、interface 默认方法体、内建 `Comparable`/`Hashable`/`Printable` + `Self` 语义（v0.0.17：预置根模块、实现签名 Self 放宽、派发适配器、`Printable` 接入插值）。**仍欠**：match 类型模式仍拒绝 interface（`E4001`，穷尽性未定） |
| 14 | ~~上转赋值的 C 指针类型不兼容（仅警告）~~ **v0.0.15 已修** | `Animal a = new Dog("旺财")`（`Dog extend Animal`） | 子类结构体把父类字段排在最前，**运行时布局是兼容的**（字段读写、`override` 派发都正确），但 codegen 生成的 C 是 `Animal* v_a = same__Dog__new(...);`——两个不同的 struct 指针类型，gcc 报 `-Wincompatible-pointer-types`。每个父类指针接收子类实例的地方都会刷一条警告。同模块与跨模块表现一致，与导入无关。**修法**：`coerce_c`（src→want 的统一转换出口，声明/赋值/传参/return 全经过）新增 `upcast_target`——src 沿 `TypeDef::parent` 链向上找到 dst 且两者都是 class 时，生成显式强转 `(Animal*)`；可空目标（axopt 结构体）与 class→interface（GAP #13 胖指针）不在此路径。回归用例 `tests/run/Upcast-1.ax`（声明/传参/return 三处上转 + 同类型赋值不多余强转），`-Werror` 构建零警告 |
| 15 | 裸类型名的具名导入歧义仍按字典序 | `import a.{Product}` + `import b.{Product}` | 裸**类型名**歧义已在 v0.0.14 报 `E3042`（`import a` + `import b` 都有 `Product`）；但**具名导入**把同名符号引入当前作用域时仍按视图名字典序取第一个、不报错（`named_import_canon`）。两者口径不一致：同一个名字，一种写法报错、另一种默默选中一个。**要修**：`named_import_canon` 也走歧义诊断（需 `&mut self`，或在 collect 阶段就把冲突定死） |

### 3.3 用例与结果

```
python tests/runtests.py          # 109/109 通过
python tests/runtests.py run -v   # 端到端逐条
python samples/run_all.py         # 11 个示例程序跑一遍看输出
```

- 目录：`tests/lex`（15）、`tests/parse`（12+）、`tests/sema`（28+，含 Vis-1..3、Ctor-1..2）、`tests/own`（1）、`tests/run`（23+，含 `RUN-1…RUN-4`、`Rt-01…Rt-16`、`0numeric`、`Loop-1`、`Rec-1`、`Fix-1`、`TRY-1…TRY-4`、`Ctor-1`）
- 负例断言"恰好一条诊断且 code 相符"；正例跑 `check` + 生成 + C 编译 + 执行 + 逐字节比对 stdout/退出码。
- §16.10 总验收：
  - `RUN-1` stdout 恰好两行 `折后小计 53.97`、`柴犬粮 ¥19.99（库存 10）`，退出码 0 —— **通过**
  - `RUN-2` `stock:0` → `下单失败：insufficient stock` 走 `match` 的 `fail` 分支 —— **通过**
  - `RUN-3` `load(9999)` → `商品不存在`，退出码 1 —— **通过**
  - `RUN-4` `fib(30)` = `832040` —— **通过**
  - `RUN-5`（10 万次 map 插入 + 遍历）未纳入，见 §3.2 第 5 条同级的性能验证缺口（功能上 `Rt-11` 已覆盖插入序）

### 3.4 已知与规格书的偏差

1. **`RUN-1` 的期望数值**：规格书 §16.10 写 `53.97`，而文稿 §12 的 `Goods.total` 是 `price * count`（`19.99 × 3 = 59.97`）。`53.97` 只有在走 `DiscountedGoods.final_price`（再乘 `0.9` 并 `round(2, HalfUp)`）时成立。本次以**文稿 §12 原文**为唯一真相建用例，得到的 `53.97` 与规格书期望一致；若有人把 `main` 简化成只调 `Goods.total`，正确输出应是 `59.97`。
2. **`￥` 与 `¥`**：文稿 §12 用 `¥`（U+00A5），早期手写草稿用过全角 `￥`（U+FFE5）。用例已按文稿改为 `¥`。
3. **`decimal` 内存布局**：按 `CONFLICT-5` 的收口，用标准 C11 struct `ax_dec`（`ax_i128 { int64_t hi; uint64_t lo }` 系数 + `int32_t scale`，24 字节实际/32 字节对齐），语义仍是 128 位十进制定点，**无任何 `__int128` 或 GCC/Clang 扩展**。
4. **`try/catch/finally`（超出原设计，用户裁定新增）**：原文稿 10.2 明写"没有 try-catch 块"，捕获只能写整段 `match`。按"就地捕获"的实际需求补上块语法，语义仍建立在 `result<T>` 上，没有引入异常类层级、没有把七类崩溃变成可捕获异常。同时新增 `E6012`（catch 顺序/重复）、`E6013`（`break`/`continue` 会跳过 finally），关键字由 33 个增至 36 个（`catch`、`finally`、`private`，后者随"类成员可见性必须显式"E3037 引入）。文稿 10.2、规格书 §3.4 与 §9.6 码表已同步。近期同族变更：类成员可见性必须显式（E3037）、无 constructor 不可 new（E4042）。

### 3.5 打包 exe 时暴露并修复的缺陷

这些是写 `samples/` 时（此前 `tests/` 没有覆盖到的写法）才暴露出来的**真实代码生成缺陷**，均已修，并各自补了回归用例：

| # | 现象 | 根因 | 回归用例 |
|---|---|---|---|
| 1 | `01_hello.ax` 编译出 `static int64_t 01_hello__main(void);` → `invalid suffix on integer constant` | `san()` 只替换非字母数字，没处理"C 标识符不能以数字开头"，而模块名取自文件名 | `0numeric` |
| 2 | 标签循环里的 `continue outer` 生成 `goto tNN_cont` → `label used but not defined` | 六类循环只 emit 了 `_brk`，没 emit `_cont` | `Loop-1` |
| 3 | 定长数组下标生成 `v_a..v[0]`（多一个点） | 模板 `"%s%s.v[%s]"` 与分隔符 `.`/`->` 叠加 | `Fix-1` |
| 4 | `for x in 定长数组` 取 `.len` → `axfix_... has no member named 'len'` | 定长数组长度在类型里，没有 `.len` 字段 | `Fix-1` |
| 5 | `var cfg = {...}` 生成 `(int64_t){ .f_name = ... }` | `record` 类型缺 `ct()`/`need()` 分支，退化成 `int64_t` | `Rec-1` |
| 6 | `bag["count"] = 3`（`map<string, object>`）直接赋 `int64_t` 给 `ax_box*` | `gen()` 的 `want` 参数只在部分分支生效，字面量/变量引用提前 return，绕过 P1–P8 的位置转换 | 由 `samples/06` 覆盖 |

第 6 条改成了统一出口：`gen()` 生成完值后统一补 `src→want` 的表示层转换，各分支只负责"按自己的类型生成值"；`gen_to()` 降级为纯转发，避免内外两层各转一次（装箱套两层盒子、可空包两层结构体）。

### 3.6 打包与示例

- `cargo build --release`：产出单文件 `dist/axc.exe`（约 1.1 MB，零运行时依赖，只需系统 `gcc` 即可编译目标程序）。
- `samples/`：11 个可直接运行的程序（`01_hello` … `11_try_catch`），覆盖类型、控制流、类与接口、`result`/`try`、容器、`decimal`、`defer`、七类崩溃、完整业务示例；`samples/run_all.py` 一键跑完。语言**没有** `while`/`do-while`/`goto`/`char`/`switch`/命名实参（文稿明确删除），示例注释里已标注，避免被误当成编译器缺口。

---

## 四、待设计者裁决清单
| # | 条目 | 若裁定为"另一种" | 改动量 |
|---|---|---|---|
| 1 | GAP-1 `decimal` ⊕ 整数是否合法 | 删 `check_binary` 的 decimal/int 分支 + 改文稿 §12 示例 | 2 处 + 1 个用例 |
| 2 | GAP-2 整数字面量可落到 `decimal` | 删 `check_int_lit` 的 decimal 分支 + `Rt-15` 改成 `decimal(3)` | 2 处 |
| 3 | D-03' `check` 是否强制 `main` | `require_main` 默认改 `True`，§16 负例补 `main` | 1 行 + 约 20 个用例 |
| 4 | GAP-5 `db.Query` 未命中形态 | 改 `ax_db_query_result` 返回 `tag=0`，`RUN-3` 期望改 stderr | 2 处 |
