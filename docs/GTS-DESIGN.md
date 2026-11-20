# axlang 泛型类型擦除共享（GTS）设计文档

## 1. 问题：泛型单态化导致代码膨胀

当前 axlang 对泛型类/函数做**完全单态化（full monomorphization）**：
- 每个泛型类的每个类型实参组合都会生成独立的 C struct 定义、方法实现、vtable
- 例如 `Box<int>`, `Box<string>`, `Box<Box<int>>` 各自生成一份完整代码
- 二进制体积随泛型实例化数量线性增长（code bloat）

## 2. 方案：泛型类型擦除共享（Generic Type Erasure Sharing）

### 核心思想
在 C 层，所有 class 实例都是指针，前两个字段固定为 `vt*` 和 `__tag`。如果泛型类的字段全是**引用类型**（可擦除为 `void*`），则所有实例化的运行时布局相同，可共享一份 C 代码。

### 类型类划分（Type Class）

| 类别 | C 层表示 | 示例 | 共享策略 |
|------|----------|------|----------|
| REF | `void*` | class 实例、`string`、`object`、`interface`、`queue<T>`、`thread`、`T?` | 全部 REF 类型共享一套代码 |
| VAL64 | `int64_t` | `int64`, `float`, `decimal` | 按大小分组共享 |
| VAL32 | `int32_t` | `int32` | 按大小分组共享 |
| VAL16 | `int16_t` | `int16` | 按大小分组共享 |
| VAL8 | `int8_t` | `int8` | 按大小分组共享 |

### 共享代码命名规则

泛型类 `Box<T>`：
- T 是 REF 类型 → 共享名 `Box_ref`
- T 是 VAL64 → 共享名 `Box_i64`
- T 是 VAL32 → 共享名 `Box_i32`
- 以此类推

非泛型类不受影响，保持原名。

### 调用点适配

通过 C `typedef` 将实例名映射到共享名：
```c
typedef Box_ref Box__int;   // Box<int> 别名为 Box_ref
```

调用点代码生成使用共享名（通过修改 `class_name` / `fn_name_idx` 实现），无需在每个调用点手动插入强制转换。

### 编译期决策规则

在 `need_typedef` 阶段，对每个 class typedef：
1. 若类名包含 `$`（泛型实例化标记）且**所有字段均可擦除为引用类型** → 进行类型擦除，生成 `typedef 共享名 实例名;`，不生成完整 struct
2. 若有任何字段是值类型（`int`, `float`, `struct`, `array` 等）→ 不擦除，按原逻辑生成独立 struct
3. 非泛型类（类名不含 `$`）→ 按原逻辑处理

方法和 vtable 仅为共享名生成一次，所有擦除的实例共用。

### 局限性

- 值类型字段的泛型类（如 `Pair<int>` 中两个 `int` 字段）无法擦除，因为大小不同
- 嵌套泛型（如 `Box<Box<int>>`）：外层 `Box` 的字段是 `Box<int>`（REF 类型，指针），可擦除；内层 `Box<int>` 的字段是 `int`（VAL 类型），不可擦除
- 需要 C 编译器支持 `typedef` 别名（C11 标准特性，符合 axlang 铁律）

## 3. 原型实现计划

### 文件修改
- `rust/src/backend/mod.rs`：添加类型擦除检测、共享名计算、`need_typedef`、`fn_name_idx`、`class_name`、`build_vtables`、方法生成循环的修改

### 验收标准
- 现有泛型测试（GX-1~4）继续通过
- 新增代码膨胀对比测试：同一泛型类多个引用类型实例化，生成的 C 代码中重复 struct 定义减少
- `cargo build --release` 通过，生成的 C 代码可被 C11 编译器编译

## 4. 三条铁律合规性

| 铁律 | 合规说明 |
|------|----------|
| 编译期拦截 | 类型擦除决策全部在编译期完成，运行时无类型检查开销 |
| 零成本抽象 | 共享代码通过 `typedef` 实现，无运行时间接调用或类型 dispatch 开销 |
| 不新增关键词 | 仅在编译器内部实现，对 axlang 源码语法无任何新增关键词 |
