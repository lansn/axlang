# CHANGELOG — axlang / axc 版本变更记录

格式遵循 Keep a Changelog；版本号语义化（见 `ARCHITECTURE.md` §8）。
本项目起步于 2026-11，同一月内完成原型、迁移与首轮特性，条目按里程碑而非精确日期组织。

> **版本策略**：雏形期从 **0.0.1** 起步，每达成一个里程碑 +0.0.1；
> **1.0 及以上永远保留给语法冻结**（语言设计不再做破坏性变更）之后。
> 阶段进位条件见 `ARCHITECTURE.md` §8。

## [0.0.23] — 2026-11

### 命名规则按调用形状分：库函数全小写，方法 PascalCase

破坏性改名（用户拍板）。此前 ㉞（v0.31）把**模块函数也按 PascalCase** 统一（`io.ReadLine`、`math.Equals`），
理由是"标准库方法一律 C# 风格"；后果是 `io.ReadLine()` 与 `s.Len()` 同形——读代码时分不清哪个是
"某个对象上的方法"、哪个是"库里的一支自由函数"。本轮按**调用形状**重新划线：

- **内建库函数一律全小写**（多词 snake_case）：`print` / `println`（原 `Print` / `Println`）、
  `io.read_line` / `io.read_file`、`file.open` / `file.create` / `file.append`、`json.parse`、
  `math.compare` / `math.equals`、`db.query`、`time.now_ms`。`名字(...)` 即函数。
- **方法一律 PascalCase**（C# 风格）——实例方法、静态方法、接口方法都算：`s.Len()`、`f.ReadLine()`、
  `decimal.Parse(s)`、`x.ToString()`；内建接口同步为 `Printable.ToString()` / `Comparable.Compare(other)` /
  `Hashable.Hash()`（原 `to_string` / `compare` / `hash`，此前是 PascalCase 规则的例外）。
- **旧名不静默兼容**：`Print` / `Println` 报 `E3003` 并指名改写成 `print` / `println`；
  `io.ReadLine` 这类旧写法报"模块没有该符号"。

顺带修两个此前无用例覆盖的生成 bug：`io.read_file` 按单实参调用 `ax_read_file(path)`（运行时签名是
`(path, &out)`，C 编译直接失败）、`io.read_line` 按无实参调用 `ax_read_line()`（同样是 out 参数形态）——
现在都按 `string?` / `result<string>` 的 out 参数形态生成。运行时 `axrt.h` 补 `#include <math.h>`
（`math.equals` 生成 `fabs`，此前隐式声明报警告）。

验收：**166/166** 通过、Rust 零警告（新增 `Math-1` 模块函数正例、`IO-2` `io.read_file` 回读、
`Name-neg-1` 旧写法负例；`Out-1` 改为断言 `Print` 的迁移提示）。

## [0.0.22] — 2026-11

### 所有权 / 借用 / arena 落地（M7，O-01…O-20 全通）

- **借用检查改为强度感知的单语句扫描**（O-7/O-12，规格书 11.4）：语句入口
  清空使用记录，`check_expr` 期间按 place 收集 `&const`（只读）/`&`（可变，
  强度由被调函数形参声明决定）/ move / 写四类使用，语句出口按 place 分组
  互查。多个 `&const` 共存合法；出现可变借用、借用期间 move/写即报
  `E7002`/`E7004`。删除旧的语法 walk（它不分强度，`f(&const x, &const x)`
  会误报，嵌套调用的冲突还会报两次）。
- **move 位置穷举**（O-6）：赋值/传参/返回/初始化/字段与元素写入统一经
  `take()` 记录，`Move` 视作独占使用参与扫描；move 后源变量置 Moved。
- **修 O-9 复活语义**：对已 move 变量重新赋值时，读左值本身触发 E7001，
  而 `b = new Buf(4)` 恰是复活动作——赋值前对直接变量左值放回 Live
  （字段/元素左值不豁免，对已 move 对象写字段仍是 use-after-move）。
- **分支汇合**（O-4）：if/else 两支各自从进入前状态快照独立检查（修
  else 分支被 then 的 move 误伤），结束后按最保守合并——任一可达路径
  move 即 Moved（GAP-13）。"必然离开函数"判定与 `stmt_must_exit` 分开：
  后者把任意调用语句当作退出（历史近似），`sink(b)` 之后代码照常执行，
  它的 move 必须参与合并。
- **循环保守合并**（O-5）：循环体两遍检查——第一遍 probe（抑制诊断）收集
  体内会 move 哪些外层变量，第二遍按合并后的 Moved 状态正式检查，下一轮
  迭代使用即 E7001；迭代变量每轮重新绑定，不参与合并；循环体内先重赋值
  再使用、每轮新建局部对象都合法。
- **W1003**：对 copy 类型写 `&` 等价传值，警告不报错（Diags 警告通道 +
  `Sema::warn`，不影响退出码）。
- **`arena` 与 `ArenaRef` 真实落地**（O-17…O-20）：`axrt` 增加
  `ax_arena_create/adopt/free`（收养指针、整块释放）；`new Arena()` 生成
  `ax_arena_create()`（原为 `NULL` 占位）；`arena.Alloc(x)` 把 x move 进
  区域、返回 `ArenaRef<T>`（copy 语义，可自由复制/共享/写，是唯一绕过
  借用互斥的引用）；`ArenaRef<T>` 的 C 类型就是 T 的指针形态（class 本就是
  指针，原 `T*` 映射会生成 `Node**` 导致 C 编译失败）。区域变量离开作用域
  整体释放，复用 defer 清理栈（return/throw/break 提前退出同样触发）。
- **E7006 逃逸检测**：`ArenaRef` 的使用点必须在 arena 声明块内
  （region 绑定 + 作用域链比对）；不得作为返回值、不得作为实参传出、
  不得存进普通对象字段/容器——存进"区域内对象"（ArenaRef 解引用后的
  字段）合法，这是图/双向链表的写法（文稿 8.4）。
- **E7005**：`arena.Alloc` 只收养 class 实例 / object 盒子（C 层是指针的
  类型）；ArenaRef 字段在 constructor 全量赋值（E3009）中豁免——构造期
  没有区域可借，calloc 清零即空指针；struct 字段含 ArenaRef 按 class 引用
  同等禁止（E3023）。
- 顺带修复：CHANGELOG 里 0.0.20 / 0.0.21 两个区块此前被重复提交，去重。
- 测试：新增 `own/O-02…O-12`、`own/ArenaRef-neg-1..3`、`run/Borrow-1`、
  `run/Branch-1`、`run/Arena-1`；全量 **163/163**，`-Werror` 零警告。

## [0.0.21] — 2026-11

### 泛型单态化落地（GAP 泛型 #1，GX-01…09 全通）

- **泛型函数**：`(函数, 类型实参)` 生成一份 FuncInfo 实例（签名替换、
  体共享 AST），实例体按具体类型重查，codegen 只生成实例（模板跳过）。
  推断修复：实参先按无期望类型预查再统一（原实现推断时实参还没查过，
  恒报 E4037）；匿名函数实参推断期跳过（E4043），主循环按替换后形参检查。
- **泛型类 / struct**：`(类, 类型实参)` 生成实例 typedef（骨架立即入表，
  构造期间的自引用/嵌套实参命中注册表，不再撞 id），字段/ctor/方法签名
  全部替换并 concretize；`new Box(...)` 从 ctor 实参推断、构造实参推断
  优先于期望类型（实参写错要报错，不被期望悄悄带过）；struct 构造按位
  对应字段推断。实例类实现 interface → 胖指针派发直接可用。
- **泛型别名**：`type Table<T> = map<string, T>` 声明期带参数环境校验，
  引用期 expand_alias 展开（GX-05）。
- **槽表按实例隔离**：键从 `节点 id` 变为 `(实例号 << 32) | 节点 id`；
  实例号 0 是非泛型世界，现有代码零成本。实例体内合成的匿名函数继承
  实例号；codegen 侧 `CodeGen::slot()` 按当前实例取键。
- **约束**：声明期 + 调用点双检；约束失败不实例化（E4036 一条诊断）；
  `Comparable` 类实例的 `a > b` 按 `a.compare(b) > 0` 生成（顺带修正
  Param 分支永不可达的死代码）。
- **不变型**（GX-08）：实例 id 不同即不同类型，`Box<Dog>` → `Box<Animal>`
  天然报 E4001。
- **深度上限**（GX-09）：函数链与类型链共 64 层，触顶报一次 E4038；
  超限实例照常物化（签名具体、调用点类型正确），只不再查其函数体——
  致命错误之外零级联。
- **修 move 清零的参数名**：`this.f = p`（p 是引用类型参数）置空时误写
  `v_p`，应为 `p_p`——泛型类构造测试暴露。
- 修 `resolve_type` 泛型环境只认 Param 值的限制：实例环境是具体类型，
  实例体内 `sink<T[]>` 这类引用模板参数的写法因此可用。

### 测试

- 新增 `GX-1`（泛型函数六种场景）、`GX-2`（泛型类/struct/interface）、
  `GX-3`（泛型别名）、`GX-neg-1..4`（E4036×2 / E4001 / E4038）。
- **150/150** 通过，`-Werror` 零警告；规格书 v0.20（M6 勾完）、
  配套文稿 v0.44，GAP 泛型 #1 关闭。

## [0.0.20] — 2026-11

### `typeof(表达式)` 落地（文稿 11.2）

- **编译期常量**：返回实参静态类型的显示名（`int` / `int8` / `Dog` /
  `int[]` / `int?` / `map<string, object>`）。sema 在 `check_call` 求名后
  同时存 `bname`（codegen 拼字符串字面量）与 `const_value`（供常量求值）；
  codegen 直接 `AX_S("...")`，实参不出现在生成代码里，零运行时开销。
- 可用于 `Println(typeof(a))`、插值 `{typeof(x)}`、
  `const string K = typeof(s)`。
- 边界：裸写 `typeof` 报 `E3003` 并提示写成 `typeof(表达式)`；实参有错时
  只报实参的错；与 `match` 类型模式不重叠（`typeof` = 编译器认为是什么，
  `match` = 运行期实际是什么，`object` 装箱值 typeof 恒为 `object`）。

### 同轮修掉的真 bug

- **函数内局部 `const` 从未生成声明**：`Stmt::Const` 的 codegen 是空语句，
  sema 已 declare 成只读变量、使用处引用 `v_<name>`，于是
  `const string K = "x"; Println(K)` 炸 `v_K undeclared`（全局 const 走
  另一条路，所以从未暴露）。改为与 `var` 同一条 `var_decl` 路径。

### `sizeof(表达式)` 落地（文稿 11.2，用户裁定 class 给指针大小）

- **编译期大小**：返回实参类型在 C 侧的存储字节数（`int`），codegen 直接发
  `(int64_t)sizeof(<ct>)`——`int` = 8、`uint8` = 1、`decimal` = 32、
  `string` = 16、`int[]` = 24、class 按引用语义给**指针大小**（8），
  对象本身的大小不在此问（与赋值语义一致）。
- 值由 C 编译器计算，sema 不算 → 不能进 `const` 声明；`void` 实参拒绝；
  裸写 `sizeof` 报 `E3003` 带提示（与 `typeof` 同款）。
- 顺带修：定长数组 `int[3]` 调 `.Len()` 生成了 `.len` 成员访问，但
  `axfix_*` 结构没有 `len` 成员——改为发编译期常数。

### 测试

- 新增 `Typeof-1`（七种类型显示名 + 插值 + const）、`Typeof-neg-1`
  （裸 `typeof`）、`Lit-1`（`0x`/`0b`/`0o` 进制字面量专项）、
  `Str-1`（码点往返专项：`CodepointAt` 越界 `null` / `FromCodepoint` /
  逐码点迭代）、`Sizeof-1`（十一种类型的存储大小 + 表达式）、
  `Sizeof-neg-1`（裸 `sizeof`）。
- **143/143** 通过，`-Werror` 零警告；规格书 v0.19（§15.1 增 `typeof` /
  `sizeof`），配套文稿 v0.43。

## [0.0.19] — 2026-11

### `match` 类型模式补实（文稿 5.3）

- **codegen 从没写对过**：`check_match` 在 arm 结束处把**臂体返回类型**写回
  `arm.pat.id`，覆盖了 `check_pattern` 存下的目标类型，于是判 tag 时按 `void`
  的哈希比（`Dog` / `Cat` 撞成同一个 `51847u`），绑定变量还声明成 `void v_d`，
  单文件就 `void value not ignored`。删掉这行（`arm_tys` 早已收集臂值类型）。
- **两种来源的 tag 字段分开处理**：`object` 是 `ax_box`（成员 `tag`），类引用
  是对象头 `T*`（成员 `__tag`，构造时写入真实类型）。原实现一律发 `->__tag`，
  对 `object` 直接编译失败。现在 `object` 比 `tag` 后从 `payload` 取指针（class）
  或 `ax_unbox_*`（标量）；类引用比 `__tag` 后强转——`Animal a = new Cat(...)`
  用 `Cat c` 也能命中。
- **标量类型模式落地**：`bool` / `int` 系 / `float` / `decimal` / `string` 可作
  模式，异构 `object[]` 不必再靠断言式下转分流。数组 / `map` / 元组 / `func` /
  interface 仍拒绝（interface 是胖指针，复合类型运行期只剩装箱标签）。

### `Print` / `Println` 实参放宽（文稿 11.2）

- 签名仍是 `void Print(string)`，但实参是单个**可插值值**时按 `Print("{x}")`
  生成（`ax_sb` → `push_interp` → `ax_sb_finish`）：`Println(123)`、
  `Println(3.5)`、`Println(true)`、`Println(enum)`、`Println(实现 Printable 的类)`
  都能直接用。
- 不是第二套格式化：占位符仍不支持、多实参仍报 `E4031`、`int[]` / `map` /
  元组仍报 `E4001`——省掉的只是花括号。
- 修一处旧文案：`Println(args.At(0))` 不再要求判空，与 `Println("{args.At(0)}")`
  一样输出值或 `null`。

### 测试

- 新增 `Pat-1`（object 箱 / 类引用 / 标量模式四种来源）、`Pat-neg-1`、
  `Print-1`（隐式插值各类型）、`Print-neg-1`（复合类型拒绝）。
- **137/137** 通过，`-Werror` 零警告；规格书 §13 / §15.1 / §16 同步
  （v0.17，配套文稿 v0.41）。

## [0.0.18] — 2026-11

### json / time 模块实化（GAP #5）

- **`json.Parse`**：手写递归下降解析器（`ax_json_parse_result`），支持 object /
  array / string / int / float / true / false / null，`\uXXXX`（含 UTF-16 代理对）
  解码，语法错误返回带位置的 fail 消息。顶层必须是 object，否则 fail。
- **JSON null 的语义定案**：`map<string, object>` 里没有 null 的合法载体
  （`object` 不能为空），所以 **null 值的键不入表**——`Get` 返回 `has=0`，
  `== null` 与 `??` 都能安全生效，不会在解箱时 trap。数组内的 null 保留占位。
- **`json.stringify`**：标量 / map / 数组递归序列化，字符串按 JSON 规则转义
  （`"` `\` `\n` `\r` `\t` 与 `\u00XX`），嵌套结构与往返一致。
- **`time.now_ms`**：`GetSystemTimeAsFileTime` 换算 Unix 毫秒（此前恒返回 0）。
- `result<map<string, object>>` 在 C 侧走运行时同形结构 `ax_res_map`
  （与 `ax_res_row` / `ax_res_file` 同模式）。

### 同轮修掉的真 bug

- **match 臂体调用被静默丢弃**：臂是表达式，`ok(v) => userFunc(v)` 这类
  「纯 C 表达式形态」的调用生成后字符串被丢掉，整个调用不执行（`Println`
  这类走 emit 的内建看不出问题）。两处 match codegen 补 `(void)(expr);` 落句，
  与 `Stmt::Simple` 一致。
- **`??` 对 `object?` 的兜底值没装箱**：`t.has ? t.v : "x"` 里 `ax_box*` 与
  `ax_str` 类型不匹配（C 编译失败）；右侧改为先按可空内层类型转换。
- **内建模块实参不按签名转换**：`json.stringify(m)` 传 map 给 `object` 形参时
  没装箱；模块调用改为与用户函数同一套 coerce 规则。
- **字符串里的 JSON 文本被误当插值**：`"{\"k\":1}"` 的 `{` 触发插值解析。
  lexer 规则改为：`{}`、`{\"...`、`{"k":`（字符串字面量后跟 `:`）当字面 `{`，
  其余按插值——`{"中文".ByteLen()}` 这种「字符串字面量开头的插值」保持可用；
  负号插值 `{-7 / 2}` 不回归（Rt-12）。

### 测试与示例

- 新增 `tests/run/Json-1`：解析 + 各类型解箱 + null 键缺席 + 语法错误 fail +
  stringify 往返 + `now_ms` 可用性。
- **133/133** 通过（132 → 133），示例 13/13（`samples/test.ax` 为用户草稿，
  顶层 `Print(123)` 报 `E4001` 属设计行为，未改动）。

## [0.0.17] — 2026-11

### 内建 interface 落地（文稿 7.1，GAP #10/#12）
- **`Comparable` / `Hashable` / `Printable` 预置在根模块**：无需 import、不可重名声明
  （重名 → `E3018`）。方法签名：`int compare(Self other)` / `uint hash()` / `string to_string()`，
  声明为占位 FuncInfo（无默认方法体），实现类必须提供实现（缺失 → `E3019`）。
- **`Self` 语义**：签名参数声明为 interface（Self）时，实现方直接写实现类类型——
  implement 签名对照放宽匹配；派发处生成转换 thunk（胖指针 `.obj` → 实现类指针），
  `Comparable ca = m; ca.compare(other)` 经 shim 虚表动态派发。
- **`Printable` 接入插值（GAP #12 剩余）**：class / struct 实现 `to_string()` 后
  `{p}` 直接插值；`Printable` 接口变量本身经虚表派发；可空形式有值显示、`null` 显示 `null`。
  未实现的 class 插值仍报 `E5006`。
- **数组 `Sort(cmp)` codegen（GAP #10）**：每个调用点派生文件级 qsort 适配器
  （比较器暂存 + `(const void*, const void*)` 收敛），匿名函数 / 命名函数 / 函数值均可作比较器；
  元素为 class / struct / string / 数值皆可。此前 sema 有签名、后端静默返回 `"0"`。

### 修复
- **`recv_c` 优先级坑**：复杂接收者的字段访问裸接 `->field` 被 C 解析成
  `*(...->field)`（`ps[0].name` 编译失败）——复杂接收者先落临时变量。
- **`ax_narrow` 的 `uint64` 分支 UB**：`1 << 64` 在 x86 移位量取模得 1 → 上界算成 0，
  任何正数 `uint(x)` 都 trap「超出范围」——64 位特判 `UINT64_MAX`。
- **interface 缺实现漏报**：`find_method` 的兜底（0.0.16 为类接收者调 interface
  独有方法而加）会把 interface 签名本身返回回来，implement 校验被骗成 `Some`——
  校验处改为仅沿父类链查找实现。
- **下转 `T(x)` 语义写反**：合法的 `Dog(父类值)` 被报 `E5003`、无意义的
  `Animal(子类值)` 反而放行成运行时 trap——改为：源是目标子类 = 上转恒成立；
  源是目标父类或 interface = 合法下转（运行时 tag 检查）；互不为父子的两个
  class 编译期拒绝。interface 胖指针作下转源时 codegen 先取 `.obj`。
  回归用例 `tests/run/Down-1.ax`。
- **object 自动解箱（0.0.16 追加）**：`string t = a[3]`（`a: object[]`）自动解箱路径
  codegen 漏了，裸拷贝盒子指针 → C `invalid initializer`；`coerce_c` 统一出口补齐，
  运行时标签不符 `R107` 终止。

### 测试
- 新增 `Sort-1`（三种比较器）、`Printable-1`（插值四场景）、`Comp-1`（Self 派发 / 多
  implement / hash）、`Box-1`（object[] 装箱解箱）、`Printable-neg-1/2/3`
  （重名声明 `E3018` / 未实现插值 `E5006` / 缺实现 `E3019`）。
- 全部新用例 `-Werror` 构建零警告。

### 验收
`cargo build --release` 零警告 + **132/132** 通过（125 → 132），示例 **13/13**。

## [0.0.16] — 2026-11

### interface 多态落地（GAP #13 核心）
- **interface 类型可用**：`Shape s = new Circle(...)`、interface 参数 / 返回值 / 字段 /
  数组元素 / 可空 `Shape?` 全部可编译可运行——此前 codegen 不为 interface 生成任何 C 类型，
  一用就 `unknown type name` 编译失败。
- **表示层**：interface 落成胖指针 `Iface_X { void* obj; Iface_X_VT* vt; }`。VT 条目按
  `all_iface_methods`（自身方法在前、extends 链追加在后）拉平。
- **(类, interface) shim 虚表**：每个实现类生成 `Class__Iface__VT_`，把类的方法实现
  （沿继承链找 override，链上没有用 interface 默认方法体）按 interface 方法序填入；
  父 interface 传递展开——类 `implement Pet` 就同时生成 `Dog__Animal__VT_`。
- **上转**：子类实例 → interface 在统一转换出口 `coerce_c` 组装胖指针
  （`(Iface_Shape){ (void*)obj, &Circle__Shape__VT_ }`），声明 / 赋值 / 传参 / return 全覆盖。
- **派发**：interface 接收者按**接收者的静态 interface** 取 VT 布局、经 `tv.vt` 运行时派发
  （方法可能来自 extends 链，方法归属与接收者不是同一 interface 时布局不能混用）；
  类接收者调用 interface 独有方法（类虚表没有该条目）直接取该类 shim 地址。
- **sema**：`find_method` 补 interface extends 链（`Pet extend Animal` 后 `p.speak()` 成立）；
  实例方法归属改 `funcs[m].owner`——class 方法落类上、interface 方法落 interface 上，
  codegen 据此选虚表。interface 默认方法体生成 C 函数（无 this）。
- **interface extend 与跨模块**：`interface B extend A` 的方法经 B 变量可调；跨模块
  `implement model.Describable`、interface 作跨模块参数（`Mod-8`）。

### 上转赋值显式强转（GAP #14）
- `Animal a = new Dog("旺财")` 生成 `(Animal*)Dog__new(...)`——子类结构体父类字段在前，
  布局兼容但 C 是两个不同 struct 指针类型，此前每个父类指针接收子类实例处刷一条
  `-Wincompatible-pointer-types`。同修赋值 / 传参 / return 三处；同类型赋值不多余强转。
  可空目标（axopt 结构体）与 class→interface（胖指针）不在此裸转路径。

### 修复：object 自动解箱（文稿 6.7）
- `string t = a[3]`（`a: object[]`）此前 codegen 只在显式 `T(o)` 转换表达式走解箱，
  **自动解箱路径漏了**——直接裸拷贝盒子指针，C 报 `invalid initializer`。
  `coerce_c` 统一出口补「object → 具体类型」自动解箱（目标类型已知时进行，
  运行时核对标签，不符即 `R107 bad-conversion` 终止，与显式 `T(o)` 检查完全相同）。
- 覆盖：内建值类型 / string / 数组 / map / class / 可空 `T?`，object 变量中转后解箱同样成立。

### 测试与示例
- 新增 `Upcast-1`（三处上转 + 同类型赋值）、`Iface-1`（interface 变量/参数/返回值/数组，
  两实现类派发）、`Iface-2`（interface extend + 隐式实现父 interface）、
  `Mod-8`（跨模块 interface：implement / 变量再赋子类 / interface 参数）、
  `Box-1`（object[] 异构数组：装箱 / 自动解箱 / object 中转 / 空数组）。
- 全部 interface 用例 `-Werror` 构建零警告（shim 加 `__attribute__((unused))`）。
- 边角验证：interface 默认方法体（类未实现时用 interface 的）、可空 interface 判空。

### 验收
`cargo build --release` 零警告 + **125/125** 通过（120 → 125），示例 **13/13**。

## [0.0.15] — 2026-11

### 诊断多语言（默认英文）
- **错误文案支持英文与中文**：`--lang=en|zh` / 环境变量 `AXC_LANG` / 工作目录 `axc.toml`（顶层 `lang = "zh"`），
  优先级从高到低，默认 **英文**。中文模板仍是各报错调用点的唯一事实源（`{0} {1}` 占位符），
  英文翻译集中在 `rust/src/diag_msg.rs` 的 `CATALOG` 目录表，按 `(诊断码, 中文模板)` 对应；查不到就回退中文。
- 全部 363 个报错调用点改写为新签名 `(code, tok, rule, zh模板, &[实参])`——原来 121 个码 332 条文案
  散落在 `format!` 里无法翻译，现在实参与模板分离，文案可整体替换。
- CLI 汇总行（`axc: N 个错误`）与「找不到文件」提示一并双语；负例只按诊断码断言，120/120 不受影响。

### 文档自动生成
- `docs/ERRORS.md`：多语言错误清单（123 个码、323 条文案，含中文/英文释义与出处 rule），
  由 `tools/gen_errdoc.py` 生成；目录缺英文条目时脚本报错退出，防止文档与代码漂移。
- `docs/STDLIB.md`：内置库与标准库说明（全局函数、7 个内建模块、string/array/map/decimal/int/float/
  bool/enum 实例方法、静态方法、`Row`/`File`），由 `tools/gen_stdlib_doc.py` 从 sema 内建签名表
  自动抽取——给内建表新增方法后重跑脚本即自动出现，描述缺省标注「签名待补充」。

## [0.0.14] — 2026-11

### 语法
- **裸类型名**：`import catalog.product` 之后直接写 `Product`——`new Product(...)`、类型标注
  `Product p`、静态方法 `Product.Sample()`、`struct` 位置构造 `Point(3, 4)`、`enum` 变体
  `Color.Red` 全部成立。以前只有 `catalog.product.Product`（全路径）与 `product.Product`（末段短名）
  两种写法，类名出现几处就要抄几遍模块路径，而 `import` 本来就该做"把公开类型带进作用域"这件事。
  三种写法指向同一个类。
- **新增 `E3042` 裸类型名歧义**：`import a` + `import b` 且两者都导出 `Product` 时，裸名没有唯一
  答案，直接报错并列出是哪两个模块（提示写全路径或 `as` 改名）。同一行既做类型标注又 `new` 时只报
  一条——那两处解析的是同一个名字，报两遍纯属噪声。
- 实现：新增 `Sema::lookup_type_name`（裸名 → 当前模块限定名 → 具名导入 → 已导入模块的 `public`
  导出），替换了 9 处直接查 `types` 表的写法。

### 修复
- **`Product.Sample()` / `Color.Red` 在裸名下失败**：`TyS::Meta` 里存的可能是裸名，而 `types` 表的键
  是注册名 `模块__类型`，两处 `self.types.get(tname)` 直接落空——静态方法引用报"没有方法/没有静态
  成员"，`enum` 变体报"没有静态成员 `Red`"。改为走 `lookup_type_name`（注册名与裸名都能落定）。

### 测试与示例
- `samples/demo/OrderDemo/`：示例工程移入 `samples/`（与其它示例并列，`run_all.py` 一并跑），
  入口改用裸类名，另混用一处全路径作对照。
- 新用例 `tests/mod/Mod-7`（裸名：`new` / 类型标注 / 静态工厂 / `struct` 位置构造 / `enum` 变体 /
  旧写法并存）、`tests/mod/Mod-neg-ambig`（`E3042`）。
- 驱动器：`demo` 类别的用例目录改为 `samples/demo`（断言仍归 `tests/`）；`samples/run_all.py`
  现在会跑 `demo/<ID>/main.ax`，共 **13** 个程序。

### 文档
- 文稿 9.2 补「裸类型名」规则与示例；规格书 §1 目录树、§9.5 诊断码表（`E3042`）、§16.3 用例表同步。

### 验收
`cargo build --release` 零警告 + **120/120** 通过，示例 **13/13**。

## [0.0.13] — 2026-11

### 测试
- **新增 `tests/demo/OrderDemo/`**：一个四模块的可运行示例工程，专门演示「多模块 + 类的引入与调用」——
  `catalog/product.ax`（字段/构造/方法/静态工厂/模块常量）、`catalog/inventory.ax`（数组字段 +
  `Push` + 可空返回的 `Find`）、`checkout/bill.ax`（具名导入 `import catalog.product.{Product, TAX_RATE}`）、
  `checkout/wholesale.ax`（跨模块继承 `extend product.Product`，`parent(...)` 调父类构造）。入口 `main.ax`
  把全路径 `catalog.product.Product` 与末段短名 `product.Product` 两种写法摆在一起，逐字节比对 `main.out`。
- 驱动器增 **`demo`** 类别（与 `mod` 同构：`tests/demo/<ID>/main.ax` + 模块文件 + 期望输出）。
- **多文件用例的期望输出此前大半是死文件**：`run_positive` 按 `main.ax` 的 basename 找期望文件（即只认
  `main.out`），而 `Mod-1.out` / `Mod-2.out` / `Mod-5.out` / `Mod-6.out` 都叫目录名——那五个用例的 stdout
  **从未被断言**，改坏输出照样绿。现在 `<ID>.out` 与 `main.out` 都认（`<ID>.out` 优先），并给缺文件的
  `Mod-3` 补上 `Mod-3.out`。顺带把多文件用例的产物名从一律 `main.exe` 改成 `<ID>.exe`，不再互相覆盖。

### 修复
- **数组字段上的可变方法静默失效**：`inv.items.Push(p)` / `this.items.Push(p)` 编译通过、运行无事，
  数组却始终是空的。`ax_arr` 是**值类型**，而 codegen 只对 `Path` 接收者保留原名，字段接收者一律
  `declare` 成临时副本——`Push` 改的是副本，改完即弃。现在按「左值链」（变量 / `this` / 字段访问）
  判断，直接对原字段取址；函数调用的返回值不是左值，仍走副本（不会对临时量取地址）。
- **`decimal` 的 `:.N` 不补尾零**：文稿 11.2 写「保留 N 位小数」，实现是「按 N 位四舍五入后交给
  `ax_dec_to_str`」，而后者会去掉无意义的尾零——`{10.50d:.2}` 输出 `10.5`。`ax_sb_push_dec_n` 改为
  舍入后补齐到 N 位小数（`10.50`、`0.13` → `0.13`、整数 `19` → `19.00`）。
- **模块级 `decimal` 常量无法编译**：生成的 `static const ax_dec k_... = ax_dec_lit("13", 2);` 不是 C
  常量初始化式，GCC 报 `initializer element is not constant`——只要模块里写了 `public const decimal TAX_RATE = 0.13d`
  就连 `main` 都编译不过。decimal 常量不再落 C 变量（引用点本来就内联 `const_c` 的表达式）。

### 文档
- 规格书：§1 目录树补 `tests/mod/<ID>/`、`tests/demo/<ID>/`；§16.3 补 `demo/OrderDemo` 一行。

### 验收
`cargo build --release` 零警告 + **118/118** 通过，示例 **12/12**。

## [0.0.12] — 2026-11

### 语法
- **隐式 `main`：入口文件允许顶层语句**（FreeBASIC 式）。`Println("hello")` 单独一行就是完整程序，
  不必先套 `int main() { ... }`——单文件试写、示例、教学都省掉一层样板。实现上 parser 把顶层语句
  按书写顺序收集，文件解析完包成一个 `implicit: true` 的 `int main()`，**sema 与后端完全不感知**，
  所以作用域 / `defer` / `try` / 退出码语义与手写 `main` 一模一样。
  边界：①隐式 main 只有 `int main()` 一种形态（要 `args` 就显式写）；②顶层变量是隐式 main 的
  **局部变量**，函数引用不到（无全局变量原则不破）；③显式 `main` + 顶层语句 → 新增 `E3041`；
  ④被 `import` 的文件仍禁止顶层语句（`E3024`）——FreeBASIC 把它们编成"模块构造器"，那套机制依赖
  模块级可变状态，ax 没有，留一半语义没有意义。

### 修复
- **`main` 末尾省略 `return` 的豁免从未落地**：文稿写着"等价 `return 0`"，codegen 只在 `return`
  语句路径上处理，函数体尾部什么都不发——C 函数掉出末尾，返回值是寄存器残留。实测
  `int main() { int x = 3 }` 退出码 **1**、`Print("hello")` 也是 1，`Println("x")` 侥幸 0。
  现在函数体末尾统一补 `return 0;`（已以 `return` 结尾则不补），只对 `int main` 生效。
- **`public int x = 0` 曾被当成顶层语句悄悄接收**：`public` 已被吞掉、语句又不可能有可见性，
  于是"想声明模块级变量"的笔误变成了一个静默的局部变量。现在报 `E3024`。

### 测试
- 新增 `tests/run/Top-3`（隐式 main 正例）、`tests/mod/Mod-neg-top`（模块顶层语句负例）。
- `tests/parse/Top-1` 改为 `E3041` 情形，`Top-2` 删除（其断言已随规则反转），`tests/sema/N-03`
  改为"顶层语句带可见性修饰词"。
- `samples/run_all.py` 不再排除手写的 `samples/test.ax`（它现在合法），**12/12**。

### 文档
- 文稿 v0.40（㊸）：9.4 重写为「入口点」含隐式 main 代码块与四条边界；规格书 v0.16：
  §4.5 顶层判定改为"收集并包 main"、§9.5 加 `E3041`、`E2011`/`E3024` 用法改写、§16.3 用例表更新。

### 验收
`cargo build --release` 零警告 + **117/117** 通过，示例 **12/12**。

## [0.0.11] — 2026-11

### 类型与构造
- **模块路径的类型名不再限两段**：`resolve_type` 与 `check_new` 都硬编码 `path.len() == 2` 且视图
  `k == 1`，导致 `shop.model.Product p = new shop.model.Product(...)` 报「未定义的类型 `Product`」。
  模块路径支持三段（`import net.http.server`），类型名却不支持——同一个语言两套规矩。改为按最长视图
  匹配、末段才是类型名。
- **`struct` 位置构造真正生成代码**：文稿 3.3 写"`struct` 构造写 `Point(1, 2)`"，sema 接受，但 codegen
  一律按 class 发 `{mod}__{Type}__new(...)`，而 struct 没有 constructor、没有 `__new` 函数——**单文件**
  就报 `implicit declaration of function 'pt__Point__new'`。改为发 C 复合字面量
  `((Point){ .f_x = 3, .f_y = 4 })`，按声明顺序做 designated initializer（求值顺序与书写一致）。
- **`struct` 字段跨模块不再被当私有**：`check_field_vis` 只看字段上的修饰词，可 struct 成员按定义
  一律 public（写修饰词反而 `E3011`），于是别处 `pt.x` 报「`x` 是私有的」。对 `struct` 直接跳过可见性检查。

### 诊断
- `E3008` 的类名由内部注册名 `model__NoCtor` 改回 `NoCtor`——`模块__` 前缀是 C 侧唯一化用的，
  用户从未见过。

### 文档
- 文稿 v0.39（㊷）：9.2 补「跨模块实例化」代码块与规则（类型标注/`new` 段数不限、`class` 必须 `constructor`、
  `struct` 位置构造不写 `new`、`private` 字段 `E3013`）；规格书 v0.15：§16.3 补 `Mod-6` / `Struct-1`；
  `GAP-REPORT` #13 记下 `interface` 类型变量整体不可用（`Shape s = c` 的 C 类型从未定义，单文件同样）。

### 验收
`cargo build --release` 零警告 + **116/116** 通过（新增 `Struct-1`、`Mod-6`），示例 11/11。

## [0.0.10] — 2026-11

### 模块
- **导入默认绑定末段短名**：`import net.http` 之后 `http.ping()` / `http.PORT` / `new http.Server(...)`
  / `http.Server` 作为类型名，与 `net.http.ping()` 完全等价。此前只能写全路径，写短名报
  `E3003 未定义的标识符 http`——而 9.2 又写着"模块名就是文件名"（`net.http` 的名字本就是 `http`），
  两条规则互相矛盾。现在一条 `import` 同时登记**全路径**（区隔同名来源）与**末段短名**（模块自己的名字）。
- **`as` 独占**：`import net.http as h` 只留 `h`，不再自动生成 `http`。别名是用户对"我要叫什么"的
  显式回答，不该被自动短名越过。
- **新增 `E3040` 短名冲突**：`import net.http` + `import web.http` 会抢同一个 `http`。此处**不按字典序
  取第一个**（与具名导入同名歧义的处理不同）——那个名字是用户写的，这个短名是编译器生成的，
  自动生成的东西没有资格悄悄替换用户的意图，所以直接报错并提示用 `as`。
- 短名视图同样受具名导入白名单约束（`import net.http.{ping}` 之后 `http.other()` 仍 `E3030`）。

### 诊断
- **两处级联收敛**：成员访问 / 方法调用的接收者没解析成功时，不再追加
  「`object` 没有成员」（`E4028`）与「`object` 没有成员方法」（`E4028`），改为比较 `error_count`。
  `http.ping()`（旧版本）会报 `E3003` + `E4028` 两条，后一条把错因盖住。
- `E3030` 白名单提示改用 **import 原文**：短名视图下报 `` `other` 不在 `import net.http.{...}` 的具名导入清单里 ``
  而不是 `import http.{...}`。

### 文档
- 文稿 v0.38（㊶）：9.2 新增"默认短名"规则；规格书 v0.14：§9.5 码表加 `E3040`、§16.3 补 `Mod-5` / `Mod-neg-short`。

### 验收
`cargo build --release` 零警告 + **114/114** 通过（新增 `Mod-5` 短名正例、`Mod-neg-short` 冲突负例）。

## [0.0.9] — 2026-11

### 模块
- **具名导入真正绑定名字**（`GAP-REPORT` #11 主体修复）：`import m.{a}` 此前只把清单当白名单
  （`m.c` → `E3030`），**没把 `a` 引入作用域**——`import model.{greet}` 之后写 `greet("x")` 仍报
  `E3003 未定义的标识符`，于是「具名导入」只能限制、不能简化。现在裸名解析在
  「本地名 → 当前模块限定名」之后多查一站：**具名导入清单指向的模块**，函数 / 常量 / 类型
  三种导出统一生效（`greet("x")`、`{PORT}`、`new Config(PORT)`），白名单限制不变。
  多个模块引入同名符号时按视图名字典序取第一个（确定但不报错，歧义诊断待补）。

### 诊断
- **`axc check` 漏检 `main` 签名**：`require_main` 把「要不要有 main」和「main 签名对不对」一起
  短路掉了，于是 `void main()` / `string main()` / `int main(int x)` 在 `check` 下**零诊断**，
  到 `run` 才报 `E3025`。现在签名校验与存在性校验拆开：**只要有 `main` 就校验签名**（`check` 也算），
  只有「缺 main」这一条由 `require_main` 决定。
- **两处「同因级联」收敛为一条**：名字没解析成功（`E3003` / `E3030`）时不再追加
  「`object` 不是可调用对象」（`E5004`）与「插值里不能直接用 object」（`E5002`）。
  此前 `import util.{greet}` + `greet(...)` 会报两条，`{util.MAX_RETRY}` 越界访问也报两条，
  真正的错因反而被后一条盖住。判定方式从"看节点上有没有 `res`"改为**比较 `error_count`**。

### 文档
- 文稿 v0.37（㊵）：9.2 补同名歧义的当前处理方式；规格书 v0.13：§16.3 补 `Mod-4`、`Mod-neg-named`。

### 验收
`cargo build --release` 零警告 + **112/112** 通过（新增 `Mod-4` 具名导入正例、`Mod-neg-named` 白名单负例、`Main-1` 签名负例）。

## [0.0.8] — 2026-11

### 修复
- **插值里的可空值静默打印 `?`**（本次由 `"v={args.At(0)}"` 复现）：`string?` 进插值后既不报错
  也不显示值，输出一个 `?`。根因在 codegen——插值分派只有 `int`/`float`/`decimal`/`bool`/`string`/`enum`
  六支，其余一律落到 `_ =>` 兜底打 `"?"`，而 sema 只拦了 `object`（`E5002`），
  `T?` 与数组/class/struct 全部漏过。更糟的是**同一件事换个写法就换个结果**：
  `Println(args.At(0))` 正确报 `E4001`（可空不能当非空用），`Println("{args.At(0)}")` 却编译通过。
- **可空值插值现在有确定语义**：按 `.has` 分支生成——有值 push 内部值、无值 push `"null"`。
  插值只做"显示"，不产生可空值逃逸，因此不触发 `E4001`；赋值与传参处仍照常要求判空
  （`string s = args.At(0)` 照旧报错）。`"v={args.At(0)}"` 现在正确输出 `v=a`。
- **复合类型插值不再静默**：数组 / `map` / 元组 / `func` / class / struct 的默认显示本就未实现
  （规格书 §13.7 写了 `[a, b]`、`ClassName`、`Name{f: v}`，只是纸面承诺），以前走兜底打 `?`。
  现在报 `E5006`，文案给出可行做法（先取标量或自己拼接）；codegen 兜底降级为带类型注释的防御代码。
- **`decimal? d = 12.50d` 被误报 `E4003`**（查可空插值时顺手抓到）：字面量落地只比对目标类型的
  字面写法 `decimal`，没剥可空包装，于是"能不能落在 `decimal?`"被判成"不能落在 `decimal`"。
  `float` 字面量那一支同样受影响（`float? x = 1.5` 走 `E4001`）。新增 `types::strip_nullable`，
  两处统一按内层类型判定——目标写 `decimal?` 时落地规则就该看 `decimal`，可不可空是另一件事。
- **`{d:.2}`（可空数值）的报错文案**：原来只说"只能用于数值类型，得到 `decimal?`"，
  而 `decimal?` 的内层正是数值，读者会以为是编译器不认识可空。改为「可空值 `decimal?` 不能直接加
  格式说明符 `:.2`；先判空取出再格式化」——`:.N` 对 `null` 没有定义舍入行为，必须显式取出。

### 文档
- 文稿 6.2 删掉"所有类型都能进字符串插值"的空头承诺，改为如实描述已实现范围；11.2 新增
  "插值能显示什么"一条（含可空值示例与 `E4001` 的边界：只有花括号里的位置放行）。
- 规格书 §13.7 补可空值的生成片段与"当前实现边界"，§9.5 码表新增 `E5006`。
- `GAP-REPORT` 新增第 12 条：复合类型默认显示未实现（含 `Printable` 目前只是 interface 名字白名单、
  插值根本不查它这一事实）。

### 验收
`cargo build --release` 零警告 + **109/109** 通过（新增 `Null-1` 可空插值正例、`Interp-1` 复合类型负例）。

## [0.0.7] — 2026-11

### 模块（CLI）
- **`-I <dir>` 真正生效**：此前 `parse_args` 里没有这个分支，`-I` 被当作"未知选项"静默忽略，
  而规格书 §1 早已把它写成"模块搜索根"——文档有、实现无。现在搜索根 = 入口文件目录（优先）
  + 各 `-I` 目录（按给出顺序），`E3027` 的文案改为列出**所有查过的路径**，不再只报第一个。

### 修复
- **3 段以上的模块路径不通**：`import net.http.server` 之后写 `net.http.server.ping()` 会报
  「模块 `net` 没有公开成员 `http`」。原因是模块视图的下钻只看**精确**视图名，而 `net`、`net.http`
  只是已导入视图的**前缀**。改为按前缀判定后，任意深度（`a.b.c.d`）都能逐段下钻，
  `net.http.server.PORT` 这类常量访问同样可用。

### 诊断
- **`E3039`：用户模块名与内建模块重名**。`db`/`file`/`io`/`json`/`math`/`string`/`time`
  七个名字优先解析为内建模块；若工程里存在同名文件（如 `string.ax`），该文件**不会被加载**。
  此前这是静默的，报出来的是「模块 `io` 没有符号 `my_val`」，看不出真正原因。

### 文档
- 文稿 v0.35（㊳）：9.2 改为「导入与模块命名」，补清 **没有 `package` 声明**、
  **目录不是包**（对比 Go）、**路径按入口文件解析**、`-I`、内建名保留；9.4 补
  「`check` 不要求 `main`，`run`/`build` 必须，被导入模块禁止 `main`」。
- 规格书 v0.11：`E3039` 入码表，§16.3 补 5 条模块用例（含 `Mod-neg-builtin`）。
- GAP-REPORT：#9 更新（`-I` 已实现、路径解析规则已入文稿）、**新增 #11**
  （具名导入只做白名单、名字未绑定）。

验收：`cargo build --release` 零警告 + **107/107** 通过（新增 `Mod-3` 三级路径、`Mod-neg-builtin`）。

## [0.0.6] — 2026-11

### 诊断（仅报错质量，无语法变更）
- **顶层语句的报错从「费解 + 多报」改为「一条 + 给出改法」**：把语句直接写在文件顶层
  （`Println("hello")`）原先会被读成「返回类型 `Println` + 缺函数名」，报
  「期望函数名，得到 `(`」，并连带多报一条 `E2011`。现在解析器在解析函数声明**之前**
  先做三条前置判定：
  - 以 `if`/`for`/`match`/`return`/`break`/`continue`/`defer`/`throw`/`try` 开头 → `E2011`
  - `Name(` 形态（函数调用） → `E2011`，文案直接提示「需要写在函数体内，例如 `int main() { ... }`」
  - `类型 名 =`（含 `int[] xs =`、`T? x =`） → `E3024`，提示「只能 `const`」
  命中即 `sync()`，**一处误用恰好一条诊断**。

### 文档
- 文稿 v0.34（㊲）：9.1 补一句「顶层只允许声明与 `import`，不允许语句」——原规则只写了
  「模块级只有 `const`」，漏了语句这一半。
- 规格书 v0.10：`E2011` / `E3024` 描述补全，§4.5 增加「顶层语句的三条前置判定」表，
  §16.2 增加 `Top-1`、`Top-2` 两条用例。

验收：`cargo build --release` 零警告 + **105/105** 通过（新增 `Top-1`、`Top-2`）。

## [0.0.5] — 2026-11

### 语言
- **输出拆成 `Print`（不换行）/ `Println`（换行）**【BREAKING】：旧名 `print` 有两个毛病——
  名字说「打印」却**恒定换行**（想不换行只能自己拼字符串），且全小写与「标准库 PascalCase」
  的规则不一致。现在名字直接说明行为，与 C# 的 `Write`/`WriteLine` 同构。旧名保留专门诊断
  （`E3003`，提示同时含 `Print` 与 `Println`），不做静默兼容。
- **`File` 接口统一命名并真正实现**【BREAKING】：`ReadAll` → `Read`，新增 `ReadLine` /
  `Write` / `WriteLine`；打开补 `file.Create`（新建或清空）与 `file.Append`（追加），
  三个入口对应三种意图，不必记模式常量。`Write`/`WriteLine` 返回 `result<int>`（写入字节数）
  而不是 `void`——写文件是唯一会「部分成功」的操作，必须可捕获，而 `result<void>` 在语言
  里不存在。
- **`file` 从桩变为真实现**：此前 `file.Open` 生成未定义的 `ax_file` 类型（一用就 C 编译
  失败）、`File.ReadAll` 恒返回 `fail("file not supported")`、`Close` 只是一句注释。现在
  运行时 `ax_file` 持有 `FILE*`，一律二进制模式（`\n` 就是 `\n`，Windows 不改写 CRLF），
  `ReadLine` 主动剥掉行尾 `\r`，同一份代码在 Windows 与 Linux 写出相同字节。
- `io.Write` 删除（与 `Print` 重复，同一件事不留第二个入口）；`io.ReadLine`、`io.ReadFile`
  保留。格式化维持「只有插值」：`Println("{0}", x)` 报 `E1008`（插值表达式为空），三条
  理由写进文稿 11.2（位置对齐易错、占位符类型不可检查、花括号里本来就是表达式）。

### 修复
- **C 字符串转义 `\xHH` 会贪婪吞掉后续字节**：非 ASCII 文本写成 C 字面量时用 `\xHH`，
  而 C 的 `\x` 转义是贪婪的——`"行1"` 生成 `"\xe8\xa1\x8c1"`，`\x8c1` 被读成一个十六进制
  转义（0x8c1，越界），字符串静默损坏，gcc 报 `hex escape sequence out of range`。
  改为定长八进制 `\ooo`（固定 3 位，后随任何字符都安全）。
- **语句位置的 `expr try` 触发 `-Wunused-value`**：`f.WriteLine(s) try` 展开成 `tN.u.ok;`，
  gcc 报"语句无效果"。表达式语句统一加 `(void)` 前缀。

### 文档
- **用户侧命名从「必须 snake_case」改为「不做要求，只推荐 PascalCase」**（文稿 v0.32 ㉟、
  规格书 v0.8）：命名风格不影响正确性，编译器不检查、不报错、不警告；警告码 `W1002`
  （命名风格不符）**废止且编号不复用**。唯一保留的硬约束是标准库与内建方法的**拼写本身**
  必须 PascalCase——那是名字而非风格，`len` 根本不存在。存量示例与测试不改
  （snake_case 依旧合法），新代码按推荐风格走。

## [0.0.4] — 2026-11

### 语言
- **标准库与内建方法名改用 C# PascalCase**【BREAKING】：此前全语言方法名一律
  snake_case，`s.len()`、`nums.push(v)`、`row.get_string("k")` 与用户方法
  `check_email()` 同形，读代码时看不出哪个是语言给的能力。改为按 C# 官方约定，
  **只有标准库与内建方法用 PascalCase**：
  - `string`：`Len` `ByteLen` `At` `Chars` `CodepointAt` `Contains` `Split` `Trim` `ToString`
  - 数组：`Len` `Push` `Pop` `Remove` `Clear` `At` `Sort`
  - `map`：`Len` `Get` `Keys` `Has` `Remove`
  - `decimal`：`Round` `Divide` `ToString`；整数/浮点：`ToString` `WrappingAdd`
  - `Row` / `File`：`Empty` `Close` `ReadAll` `GetString` `GetInt` `GetDecimal`
  - 模块函数：`io.ReadLine` `io.Write` `io.ReadFile` `math.Equals` `math.Compare`
    `json.Parse` `db.Query` `file.Open`
  - 类型静态方法：`int.Parse` `float.Parse` `decimal.Parse` `int8.Truncate`
    `string.FromCodepoint`；另有 `range.Step`、`arena.Alloc`
  - **不动的三样**：用户方法保持 snake_case、常量保持 `MAX_RETRY`、内建类型名保持
    全小写。这条与「内建类型小写、用户类型 PascalCase」互为镜像——类型上内建小写、
    方法上内建 PascalCase，两个方向相反，正好让「谁写的」一眼可辨（文稿 2.3）。
    （**该分工已在 [未发布] 放宽**：用户侧命名改为只推荐 PascalCase，编译器零诊断。）
  - 破坏性变更按「仅向前生效」本不该回改旧示例，但语法改了旧示例就编译不过，
    因此全量同步：编译器 110 处（含 `"Row.GetString"` 复合键与 3 条错误提示文案）、
    示例与测试 49 处、文稿 118 处。语法文稿 v0.30 → v0.31（㉞），规格书 v0.6 → v0.7。
    验收：`cargo build --release` 零警告 + **99/99** 通过 + 11/11 示例通过。

### 修复
- **匿名函数实参导致外层局部变量「消失」**【既有 bug】：`check_func` 检查函数体时
  直接替换 `self.scope`，结束时只清空 `current_func` / `cur_class` / 泛型环境，
  既不恢复作用域也不还原本来的函数上下文。由于匿名函数合成是在**表达式检查中途**
  调用 `check_func`，其结果就是：`twice((int n) { ... }, base)` 里 `base` 报
  `E3003` 未定义，`xs.Sort(cmp)` 之后 `xs` 再也用不了——任何带匿名函数的调用都会
  让调用方的局部变量全部作废。改为进入时快照、退出时原样恢复（`scope` /
  `current_func` / `cur_class` / `generic_env` / `generic_cons`）。新增
  `tests/run/Scope-1`（`r=7` / `base=5`）。
- **数组可变方法静默失效**【既有 bug】：`intrinsic` 对方法接收者一律先 `declare`
  成临时变量，而 `ax_arr` 是按值赋的结构体，`Push`/`Pop`/`Remove`/`Clear` 改的是
  副本，原数组毫无变化——`xs.Push(4)` 之后 `xs.Len()` 仍是 3，且不报任何错。
  改为：接收者是变量路径、且方法属于「修改接收者」这一类时直接用其左值，不落
  临时副本；非路径接收者（如 `f().Push(1)`）仍走副本（本就无意义）。新增
  `tests/run/Mut-1`（push 4 / pop 剩 3 / remove 2 / clear 0）。基线 **99/99 → 101/101**。

## [0.0.3] — 2026-11

### 语言
- **函数类型与匿名函数落地**（文稿 4.1，此前整章语法不可用——词法把 `func`
  归为保留类型名 IDENT，解析器的 `func(...)` 分支是死代码）：`func(参数) 返回`
  类型可用于变量/参数/返回值/`type` 别名；匿名函数字面量 `(参数) { 函数体 }`
  需要目标类型（无上下文报 `E4043`），编译为合成顶层函数（**不捕获环境**，
  闭包留待后续）；顶层函数与静态方法引用（`twice`、`Num.neg`）即函数值；
  函数值可经参数传递并间接调用。新诊断码 `E4043`。
- **用户模块 `import` 落地**（文稿 9，此前只支持 7 个内建模块）：一个 `.ax`
  文件 = 一个模块，模块路径即目录结构（`import a.b` → 入口文件同目录的
  `a/b.ax`）；支持 `import mod` / 具名导入 `mod.{a, b}` / `as` 别名；文件内
  符号默认私有、`public` 导出（未导出被引用报 `E3030`）；循环导入编译错误
  （`E3026`）；非入口文件声明 `main` 报 `E3028`。可跨模块使用顶层函数、
  模块级 `const`、类型（含静态方法与构造）。测试驱动新增 `tests/mod/`
  多文件用例类别。
- **静态方法体内使用 `this` 改为编译错误**：新诊断码 `E3038`（消息含指路：
  改实例方法或经参数传实例）。此前 `static` 方法里写 `this.xxx` 能通过
  sema，属规格 6.1 的实现缺口。
- **constructor 是实例化的唯一通行证**【BREAKING】：class 没有声明 `constructor`
  就不能 `new`，报新诊断码 `E4042`（消息两行指路：改用类名调静态方法，或补
  constructor）——堵住"零字段纯工具类被静默 new 出无意义实例"的洞，"有
  constructor ⇔ 可 new"在定义处一眼可判。与 `E3008`（有字段必须有 constructor）
  分工：有字段没构造在声明处报、零字段没构造在 `new` 处报，一处误用恰好一条
  诊断；struct/enum 无 constructor 概念，不受约束。静态工厂模式不受影响。
  语法文稿 v0.29 → v0.30（㉝），规格书 v0.5 → v0.6，GAP-20 的 class 部分落地。
  新增 `tests/sema/Ctor-1..2`（负例）+ `tests/run/Ctor-1`（正例），基线 89/89 → **92/92**。
- **类成员可见性必须显式**【BREAKING】：class 的字段、方法、嵌套类型必须写明
  `public` / `private` / `protected`，缺省报新诊断码 `E3037`——"谁能调"要一眼可见，
  不设需要预先学习的默认值；`private` 升为关键字（35 → 36）。`constructor`
  维持唯一例外（默认 public 且禁止再写）；struct 成员（一律 public）与
  interface 方法（天然公开）维持原设计。语法文稿 v0.28 → v0.29（㉜），规格书 v0.4 → v0.5。
  新增负例 `tests/sema/Vis-1..3`，验收基线 86/86 → **89/89**。
- **新增 `try` / `catch` / `finally` 语句**（参考 PHP 的错误处理语法，取代原 10.2 节的隐式 `try` 表设计）：
  - `try` 块内 `result<T>` 在需要 `T` 的位置隐式展开（等价自动加 `try`）；
  - `catch` 按错误变体过滤（NotFound/Invalid/Money/IO），兜底分支必须最后；
  - `finally` 四条路径必执行；`return` 先算值再按内→外补跑 finally；
  - `defer` 先于 `catch` 执行，`finally` 在 `catch` 之后；
  - 新诊断码：E6012（catch 了未出现的变体）、E6013（break/continue 跨 try 边界，
    判据为 `loop_depth` 深于进入 try 时的基准）、E3027（兜底 catch 顺序）。
- 关键字 33 → 35（`catch`、`finally`）。

### 编译器
- Windows 控制台中文显示：新增运行时 `ax_console_utf8()`，交互式终端把代码页
  切到 UTF-8、退出时恢复；重定向/管道不动代码页（测试逐字节比对依赖此行为）。
- `main` 包装生成豁免 `-Wreturn-type`（sema 已检查，属语言层特例）。
- `--help` 补 `--keep` 说明。
- 新增 `-V` / `--version`：打印版本号后退出；生成的 C 头部注释中的版本号
  与 `Cargo.toml` 同源（`CARGO_PKG_VERSION`），不再硬编码。

### 工程与文档
- **项目更名 cxlang → axlang，编译器 cxc → axc**，源文件扩展名 `.cx` → `.ax`，
  运行时 cxrt → axrt、运行时符号前缀 `cx_*` → `ax_*`（cx 命名仅存于 legacy 归档内容）。
- **Rust 源码重组**为 `frontend/`（lexer/parser/ast）、`sema/`（mod/stmt/expr/member）、
  `backend/`（mod/codegen_fn/codegen_expr），根层保留 main/cli/diag/types；
  全部 `crate::` 路径与模块头注释同步重写。
- 文档体系：新增 `ARCHITECTURE.md`（接手指南）与本文件；`main.rs` 写入 crate 级
  导览；各模块 `//!` 头去迁移腔、关键类型补 `///` 文档；清理 `.build/` 打包残留。
- 移除 Python 实现的构建/测试链路（实现归档见 [0.0.1]），测试驱动改为
  自动探测 `axc.exe`（rust/target → dist）。
- **版本策略定案**：从 0.0.1 起步、每里程碑 +0.0.1，1.0 保留给语法冻结；
  历史里程碑编号为 0.0.1（原型）/ 0.0.2（迁移）/ 0.0.3（本版）。
- 验收基线：**86/86**（78 原有 + 8 try/catch 用例）+ 11 个示例全绿 + cargo 零警告。

## [0.0.2] — 2026-11

### 编译器
- **Python → Rust 迁移完成**：14 个源文件约 1.56 万行，行为与 Python 版全量差分
  88 文件 0 差异（诊断码、文案、生成 C、退出码逐字节一致）。
- `dist/axc.exe` 单文件分发（约 1.1 MB，零运行时依赖）。

### 工程与文档
- Python 原型打包归档 `legacy/python-impl-1.0.zip`（7935 行，CRC 校验通过），
  移出构建链路；`RUST-MIGRATION.md` 记录逐阶段验收与工具链备注
  （本机缺 `link.exe`，改用 GNU target + rust-lld）。
- 验收基线：78/78 + 10 示例（彼时）。

## [0.0.1] — 2026-11

### 语言
- 按规格书跑通语言全流程：词法（三类字符串 + 插值片段化）、语法（G-1…G-16）、
  语义（类型收窄、所有权 Move、错误处理 `result`/`trap`）、C11 后端
  （继承拉平、vtable、defer 展开、decimal 128 位定点）。

### 工程与文档
- Python 原型实现（8 模块 + 入口 + 打包脚本），此后作为 Rust 迁移的行为 oracle，
  退役后归档于 `legacy/`（其内部文件名仍是历史命名，属归档快照，不随项目更名改写）。
- 已知历史决策：E7008 自原型时代即不可达，为行为兼容保留（见 sema/mod.rs 行为契约）。
