# axlang 标准库清单（对标 Go 标准库 pkg.go.dev/std）

本文档列出 axlang 内建模块与 Go 标准库（基于 <https://pkg.go.dev/std，Go> 1.25.1）及 C 标准库的对标关系，作为标准库落地进度的检视参考。目标：像 Go/C 标准库一样丰富，最大限度满足开发需要。

> **跨项目铁律**：所有涉及及生成的 C 代码 / GCC 源码，均必须使用标准 C（C11），不得使用任何 GNU/Clang 扩展（如 `__int128`、`__builtin_*_overflow`、`__attribute__((unused))` 等），以便跨平台编译。

> **落地规范（每项必须完成）**：
> 1. **axrt.h**：新增 C 函数声明（标准 C11，无 GNU/Clang 扩展）
> 1. **axrt.c**：新增 C 函数实现
> 1. **rust/src/sema/mod.rs**：`BUILTIN_MODULES` 新增模块名（若新模块）
> 1. **rust/src/sema/expr.rs**：`module_sig` 新增签名（模块级函数）；或 `rust/src/sema/member.rs` `intrinsic()` 表新增签名（类型级/实例方法）
> 1. **rust/src/backend/codegen_expr.rs**：`module_call` / `method_call` 新增代码生成分支
> 1. **rust/src/backend/mod.rs**：如需新增 result 类型 C typedef
> 1. **tests/run/*.ax + .out**：新增测试用例，覆盖正例和负例
> 1. **回归验证**：`python tests/runtests.py` 零新增回归
> 1. **提交推送**：Git commit + **GitHub 推 `main` 分支**、**Gitee 推 `master` 分支**（两者不可混淆；分别执行 `git push github main` 和 `git push origin main:master`）
> 1. **更新文档**：本清单状态标记，`MEMORY.md` 同步更新，追加工作日志

> **生成说明**：本文档由 `generate_checklist.py` 从 `STDLIB-STATUS.json` 自动生成，请勿手动编辑。函数名均为小写 C 风格（snake_case）。

---

## 状态图例

| 标记 | 含义 |                                             |
| --------- | -------------------------------------------------- |
| `✅ 已落地`   | axrt.h/axrt.c 实现 + sema/codegen 绑定 + 测试用例齐备 + 回归通过 |
| `🔄 部分落地` | 核心函数已有，部分扩展函数待补                                    |
| `⬜ 未落地`   | 尚未开始                                               |

---

## 优先级说明

| 优先级 | 含义 |                                      |
| ----------- | ---------------------------------------- |
| **P0 核心基础** | 程序能跑、能做基本 I/O、字符串、数学、文件操作；axlang 最小可用标准库 |
| **P1 常用扩展** | 日常开发常用功能，与已落地模块形成闭环                      |
| **P2 进阶功能** | 并发、排序、编解码、更多数学、更多类型工具                    |
| **P3 持续丰富** | 补充完善、边缘场景、对标更多 Go/C 函数                   |

---

## 一、已落地模块（按模块逐函数清点）

### 1. io（对标 io / C stdio）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `io.read_line` | `() -> string?` | — | — | 标准 C11 实现 | — | ✅ 已落地 |
| `io.read_file` | `(path: string) -> result<string>` | — | — | 标准 C11 实现 | — | ✅ 已落地 |
| `io.write` | `(s: string) -> void` | — | — | 标准 C11 实现 | — | ✅ 已落地 |
| `io.write_line` | `(s: string) -> void` | — | — | 标准 C11 实现 | — | ✅ 已落地 |


### 2. math（对标 Go math / C math.h）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `math.abs` | `(x: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.max` | `(a: float, b: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.min` | `(a: float, b: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.sqrt` | `(x: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.pow` | `(a: float, b: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.exp` | `(x: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.log` | `(x: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.sin` | `(x: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.cos` | `(x: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.tan` | `(x: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.ceil` | `(x: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.floor` | `(x: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.round` | `(x: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.sign` | `(x: float) -> int` | — | — | — | — | ✅ 已落地 |
| `math.mod` | `(x: float, y: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.equals` | `(a: float, b: float, eps: float) -> bool` | — | — | — | — | ✅ 已落地 |
| `math.compare` | `(a: float, b: float) -> int` | — | — | — | — | ✅ 已落地 |
| `math.exp2` | `(x: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.log10` | `(x: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.log2` | `(x: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.log1p` | `(x: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.sinh` | `(x: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.cosh` | `(x: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.tanh` | `(x: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.asin` | `(x: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.acos` | `(x: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.atan` | `(x: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.atan2` | `(y: float, x: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.hypot` | `(p: float, q: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.cbrt` | `(x: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.trunc` | `(x: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.is_na_n` | `(f: float) -> bool` | — | — | — | — | ✅ 已落地 |
| `math.is_inf` | `(f: float, sign: int) -> bool` | — | — | — | — | ✅ 已落地 |
| `math.frexp` | `(f: float) -> (frac: float, exp: int)` | — | — | — | — | ⬜ 计划中 |
| `math.ldexp` | `(frac: float, exp: int) -> float` | — | — | — | — | ✅ 已落地 |
| `math.signbit` | `(f: float) -> bool` | — | — | — | — | ✅ 已落地 |
| `math.copysign` | `(f: float, sign: float) -> float` | — | — | — | — | ✅ 已落地 |
| `math.dim` | `(x: float, y: float) -> float` | — | — | — | — | ✅ 已落地 |


### 3. json（对标 Go encoding/json）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `json.parse` | `(s: string) -> result<map<string, object>>` | — | — | — | — | ✅ 已落地 |
| `json.stringify` | `(v: object) -> string` | — | — | — | — | ✅ 已落地 |
| `json.marshal_indent` | `(v: object, prefix: string, indent: string) -> string` | — | — | — | — | ✅ 已落地 |
| `json.valid` | `(s: string) -> bool` | — | — | — | — | ✅ 已落地 |


### 4. file（对标 Go os / C stdio/stdlib）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `file.open` | `(path: string) -> result<File>` | — | — | — | — | ✅ 已落地 |
| `file.create` | `(path: string) -> result<File>` | — | — | — | — | ✅ 已落地 |
| `file.append` | `(path: string) -> result<File>` | — | — | — | — | ✅ 已落地 |
| `file.remove` | `(path: string) -> int` | — | — | — | — | ✅ 已落地 |
| `file.rename` | `(old: string, new: string) -> int` | — | — | — | — | ✅ 已落地 |
| `file.exists` | `(path: string) -> int` | — | — | — | — | ✅ 已落地 |
| `file.mkdir` | `(path: string) -> int` | — | — | — | — | ✅ 已落地 |
| `file.read_all` | `(path: string) -> result<string>` | — | — | — | — | ✅ 已落地 |
| `file.write_all` | `(path: string, s: string) -> int` | — | — | — | — | ✅ 已落地 |
| `file.is_dir` | `(path: string) -> int` | — | — | — | — | ✅ 已落地 |
| `file.is_file` | `(path: string) -> int` | — | — | — | — | ✅ 已落地 |
| `file.size` | `(path: string) -> int` | — | — | — | — | ✅ 已落地 |
| `file.mod_time` | `(path: string) -> int` | — | — | — | — | ✅ 已落地 |
| `file.chmod` | `(path: string, mode: int) -> int` | — | — | — | — | ✅ 已落地 |
| `file.chtimes` | `(path: string, atime: int, mtime: int) -> int` | — | — | — | — | ✅ 已落地 |
| `file.temp_file` | `(dir: string, prefix: string) -> result<File>` | — | — | — | — | ✅ 已落地 |


### 5. db（对标 axlang 特有，无直接 Go 对标）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `db.query` | `(sql: string) -> result<Row>` | — | — | — | — | ✅ 已落地 |


### 6. string（对标 Go strings / C string.h / C ctype.h）

**静态方法：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `string.from_codepoint` | `(cp: int) -> result<string>` | — | — | — | — | ✅ 已落地 |
| `string.compare` | `(a: string, b: string) -> int` | — | — | — | — | ✅ 已落地 |
| `string.fields` | `(s: string) -> string[]` | — | — | — | — | ✅ 已落地 |

**实例方法：**

| ax.函数                 | 签名                                     | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| --------------------- | -------------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `string.to_upper` | `() -> string` | — | — | — | — | ✅ 已落地 |
| `string.to_lower` | `() -> string` | — | — | — | — | ✅ 已落地 |
| `string.contains` | `(sub: string) -> bool` | — | — | — | — | ✅ 已落地 |
| `string.contains_any` | `(chars: string) -> bool` | — | — | — | — | ✅ 已落地 |
| `string.contains_rune` | `(r: int) -> bool` | — | — | — | — | ✅ 已落地 |
| `string.count` | `(substr: string) -> int` | — | — | — | — | ✅ 已落地 |
| `string.index_of` | `(sub: string) -> int` | — | — | — | — | ✅ 已落地 |
| `string.index_any` | `(chars: string) -> int` | — | — | — | — | ✅ 已落地 |
| `string.last_index_of` | `(sub: string) -> int` | — | — | — | — | ✅ 已落地 |
| `string.last_index_any` | `(chars: string) -> int` | — | — | — | — | ✅ 已落地 |
| `string.split` | `(sep: string) -> string[]` | — | — | — | — | ✅ 已落地 |
| `string.trim` | `() -> string` | — | — | — | — | ✅ 已落地 |
| `string.trim_left` | `(cutset: string) -> string` | — | — | — | — | ✅ 已落地 |
| `string.trim_right` | `(cutset: string) -> string` | — | — | — | — | ✅ 已落地 |
| `string.trim_prefix` | `(prefix: string) -> string` | — | — | — | — | ✅ 已落地 |
| `string.trim_suffix` | `(suffix: string) -> string` | — | — | — | — | ✅ 已落地 |
| `string.split_n` | `(sep: string, n: int) -> string[]` | — | — | — | — | ✅ 已落地 |
| `string.split_after` | `(sep: string) -> string[]` | — | — | — | — | ✅ 已落地 |
| `string.title` | `() -> string` | — | — | — | — | ✅ 已落地 |
| `string.map` | `(mapping: func(int) int) -> string` | — | — | — | — | ✅ 已落地 |
| `string.replace` | `(old: string, new: string) -> string` | — | — | — | — | ✅ 已落地 |
| `string.repeat` | `(n: int) -> string` | — | — | — | — | ✅ 已落地 |
| `string.starts_with` | `(prefix: string) -> bool` | — | — | — | — | ✅ 已落地 |
| `string.ends_with` | `(suffix: string) -> bool` | — | — | — | — | ✅ 已落地 |
| `string.len` | `() -> int` | — | — | — | — | ✅ 已落地 |
| `string.is_digit` | `() -> bool` | — | — | — | — | ✅ 已落地 |
| `string.is_alpha` | `() -> bool` | — | — | — | — | ✅ 已落地 |
| `string.is_space` | `() -> bool` | — | — | — | — | ✅ 已落地 |
| `string.is_lower` | `() -> bool` | — | — | — | — | ✅ 已落地 |
| `string.is_upper` | `() -> bool` | — | — | — | — | ✅ 已落地 |
| `string.is_alnum` | `() -> bool` | — | — | — | — | ✅ 已落地 |
| `string.equal_fold` | `(t: string) -> bool` | — | — | — | — | ✅ 已落地 |
| `string.to_upper_rune` | `(r: int) -> int` | — | — | — | — | ✅ 已落地 |
| `string.to_lower_rune` | `(r: int) -> int` | — | — | — | — | ✅ 已落地 |


### 7. time（对标 Go time / C time.h）

**实例方法：**

| ax.函数                 | 签名                                     | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| --------------------- | -------------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `time.to_string` | `() -> string` | — | — | — | — | ✅ 已落地 |
| `time.format` | `(fmt: string) -> string` | — | — | — | — | ✅ 已落地 |

| ax.函数 | 类型 | 签名 | 对标 C | 对标 Go | 函数功能 | 说明 | 状态 |
| ------ | ------ | ------ | ------ | ------ | ------ | ------ | ----- |
| `time.now_ms` | 模块 | `() -> int` | — | — | — | — | ✅ 已落地 |
| `time.now` | 模块 | `() -> time` | — | — | — | — | ✅ 已落地 |
| `time.from_unix_ms` | 模块 | `(ms: int) -> time` | — | — | — | — | ✅ 已落地 |
| `time.sleep` | 模块 | `(ms: int) -> void` | — | — | — | — | ✅ 已落地 |
| `time.parse` | 模块 | `(s: string, fmt: string) -> result<time>` | — | — | — | — | ✅ 已落地 |
| `time.unix` | 模块 | `(sec: int, nsec: int) -> time` | — | — | — | — | ✅ 已落地 |
| `time.unix_milli` | 模块 | `(ms: int) -> time` | — | — | — | — | ✅ 已落地 |
| `time.unix_micro` | 模块 | `(us: int) -> time` | — | — | — | — | ✅ 已落地 |
| `time.unix_nano` | 模块 | `(nsec: int) -> time` | — | — | — | — | ✅ 已落地 |
| `time.since` | 模块 | `(t: time) -> int` | — | — | — | — | ✅ 已落地 |
| `time.date` | 模块 | `(year, month, day, hour, min, sec, nsec: int) -> time` | — | — | — | — | ✅ 已落地 |
| `time.until` | 模块 | `(t: time) -> int` | — | — | — | — | ✅ 已落地 |
| `time.after` | 模块 | `(u: time) -> bool` | — | — | — | — | ✅ 已落地 |
| `time.before` | 模块 | `(u: time) -> bool` | — | — | — | — | ✅ 已落地 |
| `time.equal` | 模块 | `(u: time) -> bool` | — | — | — | — | ✅ 已落地 |
| `time.compare` | 模块 | `(u: time) -> int` | — | — | — | — | ✅ 已落地 |
| `time.sub` | 模块 | `(u: time) -> int` | — | — | — | — | ✅ 已落地 |
| `time.add` | 模块 | `(d: int) -> time` | — | — | — | — | ✅ 已落地 |
| `time.round` | 模块 | `(d: int) -> time` | — | — | — | — | ✅ 已落地 |
| `time.truncate` | 模块 | `(d: int) -> time` | — | — | — | — | ✅ 已落地 |


### 8. decimal（对标 axlang 特有，高精度十进制）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `decimal.parse` | `(s: string) -> result<decimal>` | — | — | — | — | ✅ 已落地 |
| `decimal.truncate` | `(v: int) -> int8` | — | — | — | — | ✅ 已落地 |
| `decimal.truncate` | `(v: int) -> int16` | — | — | — | — | ✅ 已落地 |
| `decimal.truncate` | `(v: int) -> int32` | — | — | — | — | ✅ 已落地 |


### 9. int/float/bool/enum（对标 基础类型方法）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `int/float/bool/enum.to_string` | `() -> string` | — | — | — | — | ✅ 已落地 |
| `int/float/bool/enum.wrapping_add` | `(other: T) -> T` | — | — | — | — | ✅ 已落地 |
| `int/float/bool/enum.round` | `(digits: int) -> float` | — | — | — | — | ✅ 已落地 |
| `int/float/bool/enum.divide` | `(other: T) -> T` | — | — | — | — | ✅ 已落地 |


### 10. array（对标 Go slice / C 数组）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `array.push` | `(elem: T) -> void` | — | — | — | — | ✅ 已落地 |
| `array.pop` | `() -> T?` | — | — | — | — | ✅ 已落地 |
| `array.remove` | `(index: int) -> void` | — | — | — | — | ✅ 已落地 |
| `array.clear` | `() -> void` | — | — | — | — | ✅ 已落地 |
| `array.to_string` | `() -> string` | — | — | — | — | ✅ 已落地 |
| `array.==` | `(other: T[]) -> bool` | — | — | — | — | ✅ 已落地 |
| `array.!=` | `(other: T[]) -> bool` | — | — | — | — | ✅ 已落地 |
| `array.切片` | `[lo..hi] / [lo..=hi]` | — | — | — | — | ✅ 已落地 |


### 11. map（对标 Go map）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `map.clear` | `() -> void` | — | — | — | — | ✅ 已落地 |
| `map.remove` | `(key: K) -> void` | — | — | — | — | ✅ 已落地 |
| `map.==` | `(other: Map) -> bool` | — | — | — | — | ✅ 已落地 |
| `map.!=` | `(other: Map) -> bool` | — | — | — | — | ✅ 已落地 |


### 12. range（对标 Go range）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `range.step` | `(n: int) -> Range` | — | — | — | — | ✅ 已落地 |


### 13. thread（对标 Go channel / goroutine）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `thread.push` | `(v: T) -> void` | — | — | — | — | ✅ 已落地 |
| `thread.pop` | `() -> T?` | — | — | — | — | ✅ 已落地 |
| `thread.close` | `() -> void` | — | — | — | — | ✅ 已落地 |
| `thread.join` | `() -> void` | — | — | — | — | ✅ 已落地 |


### 14. queue（对标 Go channel）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `queue.try_pop` | `() -> T?` | — | — | — | — | ✅ 已落地 |


### 15. os（对标 Go os / C stdlib）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `os.args` | `() -> string[]` | — | — | — | — | ✅ 已落地 |
| `os.chdir` | `(path: string) -> int` | — | — | — | — | ✅ 已落地 |
| `os.environ` | `() -> string[]` | — | — | — | — | ✅ 已落地 |
| `os.exit` | `(code: int) -> void` | — | — | — | — | ✅ 已落地 |
| `os.getenv` | `(name: string) -> string` | — | — | — | — | ✅ 已落地 |
| `os.getpid` | `() -> int` | — | — | — | — | ✅ 已落地 |
| `os.getppid` | `() -> int` | — | — | — | — | ✅ 已落地 |
| `os.getwd` | `() -> string` | — | — | — | — | ✅ 已落地 |
| `os.hostname` | `() -> string` | — | — | — | — | ✅ 已落地 |
| `os.setenv` | `(name: string, value: string) -> int` | — | — | — | — | ✅ 已落地 |
| `os.unsetenv` | `(name: string) -> int` | — | — | — | — | ✅ 已落地 |
| `os.user_home_dir` | `() -> string` | — | — | — | — | ✅ 已落地 |
| `os.user_cache_dir` | `() -> string` | — | — | — | — | ✅ 已落地 |
| `os.getpagesize` | `() -> int` | — | — | — | — | ✅ 已落地 |
| `os.getuid` | `() -> int` | — | — | — | — | ✅ 已落地 |
| `os.getgid` | `() -> int` | — | — | — | — | ✅ 已落地 |
| `os.geteuid` | `() -> int` | — | — | — | — | ✅ 已落地 |
| `os.getegid` | `() -> int` | — | — | — | — | ✅ 已落地 |
| `os.getgroups` | `() -> int[]` | — | — | — | — | ✅ 已落地 |
| `os.executable` | `() -> string` | — | — | — | — | ✅ 已落地 |


### 16. strconv（对标 Go strconv）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `strconv.atoi` | `(s: string) -> int` | — | — | — | — | ✅ 已落地 |
| `strconv.itoa` | `(v: int) -> string` | — | — | — | — | ✅ 已落地 |
| `strconv.parse_bool` | `(s: string) -> bool` | — | — | — | — | ✅ 已落地 |
| `strconv.format_bool` | `(v: bool) -> string` | — | — | — | — | ✅ 已落地 |
| `strconv.parse_float` | `(s: string) -> float` | — | — | — | — | ✅ 已落地 |
| `strconv.format_float` | `(v: float) -> string` | — | — | — | — | ✅ 已落地 |
| `strconv.parse_int` | `(s: string, base: int) -> int` | — | — | — | — | ✅ 已落地 |
| `strconv.format_int` | `(v: int, base: int) -> string` | — | — | — | — | ✅ 已落地 |
| `strconv.parse_uint` | `(s: string, base: int) -> int` | — | — | — | — | ✅ 已落地 |
| `strconv.format_uint` | `(v: int, base: int) -> string` | — | — | — | — | ✅ 已落地 |
| `strconv.can_backquote` | `(s: string) -> bool` | — | — | — | — | ✅ 已落地 |
| `strconv.quote` | `(s: string) -> string` | — | — | — | — | ✅ 已落地 |
| `strconv.unquote` | `(s: string) -> string` | — | — | — | — | ✅ 已落地 |


### 17. filepath（对标 Go path/filepath）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `filepath.join` | `(parts: string[]) -> string` | — | — | — | — | ✅ 已落地 |
| `filepath.dir` | `(path: string) -> string` | — | — | — | — | ✅ 已落地 |
| `filepath.base` | `(path: string) -> string` | — | — | — | — | ✅ 已落地 |
| `filepath.ext` | `(path: string) -> string` | — | — | — | — | ✅ 已落地 |
| `filepath.split` | `(path: string) -> string[2]` | — | — | — | — | ✅ 已落地 |
| `filepath.clean` | `(path: string) -> string` | — | — | — | — | ✅ 已落地 |
| `filepath.abs` | `(path: string) -> string` | — | — | — | — | ✅ 已落地 |
| `filepath.is_abs` | `(path: string) -> bool` | — | — | — | — | ✅ 已落地 |
| `filepath.eval_symlinks` | `(path: string) -> string` | — | — | — | — | ⬜ 计划中 |
| `filepath.glob` | `(pattern: string) -> string[]` | — | — | — | — | ⬜ 计划中 |
| `filepath.rel` | `(basepath, targpath: string) -> string` | — | — | — | — | ✅ 已落地 |
| `filepath.walk` | `(root: string, fn: func) -> int` | — | — | — | — | ⬜ 计划中 |


### 18. rand（对标 Go math/rand）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `rand.int` | `(n: int) -> int` | — | — | — | — | ✅ 已落地 |
| `rand.float` | `() -> float` | — | — | — | — | ✅ 已落地 |
| `rand.seed` | `(seed: int) -> void` | — | — | — | — | ✅ 已落地 |


### 19. fmt（对标 Go fmt / C stdio 格式化）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `fmt.print` | `(args: any...) -> void` | — | — | 通过 print/println 部分实现 | — | 🔄 部分落地 |
| `fmt.println` | `(args: any...) -> void` | — | — | 同上 | — | 🔄 部分落地 |
| `fmt.printf` | `(format: string, args: any...) -> void` | — | — | 编译期核对占位符 | — | 🔄 部分落地 |
| `fmt.sprintf` | `(format: string, args: any...) -> string` | — | — | 标准 C11 实现 | — | ✅ 已落地 |
| `fmt.sprint` | `(args: any...) -> string` | — | — | 标准 C11 实现 | — | ✅ 已落地 |
| `fmt.fprintf` | `(w: File, format: string, args: any...) -> int` | — | — | 标准 C11 实现 | — | ✅ 已落地 |
| `fmt.errorf` | `(format: string, args: any...) -> error` | — | — | 标准 C11 实现 | — | ✅ 已落地 |
| `fmt.scanf` | `(format: string, args: any...) -> int` | — | — | 标准 C11 实现 | — | ✅ 已落地 |


### 20. sort（对标 Go sort）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `sort.ints` | `(a: int[]) -> void` | — | — | — | — | ✅ 已落地 |
| `sort.strings` | `(a: string[]) -> void` | — | — | — | — | ✅ 已落地 |
| `sort.float64s` | `(a: float[]) -> void` | — | — | — | — | ✅ 已落地 |
| `sort.ints_are_sorted` | `(a: int[]) -> bool` | — | — | — | — | ✅ 已落地 |
| `sort.strings_are_sorted` | `(a: string[]) -> bool` | — | — | — | — | ✅ 已落地 |
| `sort.search_ints` | `(a: int[], x: int) -> int` | — | — | — | — | ✅ 已落地 |
| `sort.search_strings` | `(a: string[], x: string) -> int` | — | — | — | — | ✅ 已落地 |
| `sort.slice` | `(a: any[], less: func(i, j int) bool) -> void` | — | — | axlang 不支持闭包，暂不落地 | — | ⬜ 未落地 |
| `sort.slice_stable` | `(a: any[], less: func(i, j int) bool) -> void` | — | — | axlang 不支持闭包，暂不落地 | — | ⬜ 未落地 |


### 21. slices（对标 Go slices (Go 1.21+)）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `slices.contains` | `(x: T) -> bool` | — | — | — | — | ✅ 已落地 |
| `slices.index` | `(x: T) -> int` | — | — | — | — | ✅ 已落地 |
| `slices.sort` | `() -> void` | — | — | — | — | ✅ 已落地 |
| `slices.sort` | `(cmp) -> void` | — | — | 带比较函数，GAP #10 | — | ✅ 已落地 |
| `slices.reverse` | `() -> void` | — | — | — | — | ✅ 已落地 |
| `slices.clone` | `() -> T[]` | — | — | — | — | ✅ 已落地 |
| `slices.insert` | `(i, x) -> void` | — | — | — | — | ✅ 已落地 |
| `slices.fill` | `(x) -> void` | — | — | — | — | ✅ 已落地 |
| `slices.resize` | `(n) -> void` | — | — | — | — | ✅ 已落地 |
| `slices.merge` | `(other) -> void` | — | — | — | — | ✅ 已落地 |
| `slices.min` | `() -> T` | — | — | — | — | ✅ 已落地 |
| `slices.max` | `() -> T` | — | — | — | — | ✅ 已落地 |
| `slices.sum` | `() -> T` | — | — | — | — | ✅ 已落地 |
| `slices.delete` | `(i, j) -> T[]` | — | — | 与 arr.Remove 语义重复，暂不落地 | — | ⬜ 未落地 |


### 22. maps（对标 Go maps (Go 1.21+)）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `maps.keys` | `(m: Map) -> K[]` | — | — | — | — | ✅ 已落地 |
| `maps.values` | `(m: Map) -> V[]` | — | — | — | — | ✅ 已落地 |
| `maps.clone` | `(m: Map) -> Map` | — | — | — | — | ✅ 已落地 |
| `maps.copy` | `(dst: Map, src: Map) -> void` | — | — | — | — | ✅ 已落地 |
| `maps.delete` | `(m: Map, key: K) -> void` | — | — | — | — | ✅ 已落地 |
| `maps.delete_func` | `(m: Map, f: func(K) bool) -> void` | — | — | axlang 不支持闭包，暂不落地 | — | ⬜ 未落地 |
| `maps.equal` | `(m1: Map, m2: Map) -> bool` | — | — | — | — | ✅ 已落地 |


### 23. unique（对标 Go unique (Go 1.23+)）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `unique.make` | `(v: T) -> Handle[T]` | — | — | — | — | ✅ 已落地 |
| `unique.value` | `(h: Handle[T]) -> T` | — | — | — | — | ✅ 已落地 |


### 24. bytes（对标 Go bytes）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `bytes.contains` | `(s: bytes, sub: bytes) -> bool` | — | — | — | — | ✅ 已落地 |
| `bytes.equal` | `(a: bytes, b: bytes) -> bool` | — | — | — | — | ✅ 已落地 |
| `bytes.index` | `(s: bytes, sub: bytes) -> int` | — | — | — | — | ✅ 已落地 |
| `bytes.split` | `(s: bytes, sep: bytes) -> bytes[]` | — | — | — | — | ✅ 已落地 |
| `bytes.trim_space` | `(s: bytes) -> bytes` | — | — | — | — | ✅ 已落地 |
| `bytes.to_upper` | `(s: bytes) -> bytes` | — | — | — | — | ✅ 已落地 |
| `bytes.to_lower` | `(s: bytes) -> bytes` | — | — | — | — | ✅ 已落地 |
| `bytes.repeat` | `(s: bytes, count: int) -> bytes` | — | — | — | — | ✅ 已落地 |


### 25. bufio（对标 Go bufio）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `bufio.new_reader` | `(r: File) -> File` | — | — | — | — | ✅ 已落地 |
| `bufio.new_writer` | `(w: File) -> File` | — | — | — | — | ✅ 已落地 |
| `bufio.scanner` | `(r: File) -> File` | — | — | — | — | ✅ 已落地 |


### 26. csv（对标 Go encoding/csv）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `csv.read_all` | `(r: Reader) -> string[][]` | — | — | — | — | ✅ 已落地 |
| `csv.write` | `(w: Writer, record: string[]) -> int` | — | — | — | — | ✅ 已落地 |


### 27. errors（对标 Go errors）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `errors.new` | `(msg: string) -> error` | — | — | — | — | ✅ 已落地 |
| `errors.is` | `(err: error, target: error) -> bool` | — | — | — | — | ✅ 已落地 |
| `errors.as` | `(err: error, target: error) -> bool` | — | — | axlang 无运行时类型反射，target 限定为 error 类型 | — | ✅ 已落地 |


### 28. log（对标 Go log）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `log.print` | `(v: any...) -> void` | — | — | — | — | ✅ 已落地 |
| `log.printf` | `(format: string, v: any...) -> void` | — | — | — | — | ✅ 已落地 |
| `log.fatal` | `(v: any...) -> void` | — | — | — | — | ✅ 已落地 |


### 29. sync（对标 Go sync）

**mutex（struct）方法：**

- `sync.mutex.Lock` — ✅ 已落地
- `sync.mutex.Unlock` — ✅ 已落地

**rwmutex（struct）方法：**

- `sync.rwmutex.Lock` — ✅ 已落地
- `sync.rwmutex.Unlock` — ✅ 已落地
- `sync.rwmutex.RLock` — ✅ 已落地
- `sync.rwmutex.Runlock` — ✅ 已落地

**wait_group（struct）方法：**

- `sync.wait_group.Add` — ✅ 已落地
- `sync.wait_group.Done` — ✅ 已落地
- `sync.wait_group.Wait` — ✅ 已落地

**once（struct）方法：**

- `sync.once.Do` — ✅ 已落地


### 30. atomic（对标 Go sync/atomic）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `atomic.add_int` | `(addr: *int, delta: int) -> int` | — | — | — | — | ✅ 已落地 |
| `atomic.load_int` | `(addr: *int) -> int` | — | — | — | — | ✅ 已落地 |
| `atomic.store_int` | `(addr: *int, val: int) -> void` | — | — | — | — | ✅ 已落地 |
| `atomic.compare_and_swap_int` | `(addr: *int, old: int, new: int) -> bool` | — | — | — | — | ✅ 已落地 |


### 31. reflect（对标 Go reflect）

**模块级函数：**

| ax.函数          | 签名                                 | 对标 C                    | 对标 Go                    | 函数功能           | 说明        | 状态  |
| -------------- | ---------------------------------- | ------------------------- | -------------------------- | ------------------ | ----------- | ----- |
| `reflect.type_of` | `(v: any) -> string` | — | — | axlang 无运行时类型反射，需设计可落地子集 | — | ⬜ 未落地 |
| `reflect.value_of` | `(v: any) -> any` | — | — | 同上 | — | ⬜ 未落地 |
| `reflect.num_field` | `(v: any) -> int` | — | — | 同上 | — | ⬜ 未落地 |


---

## 二、待落地清单（按优先级 P0 → P3）

> 本部分由 `generate_checklist.py` 从 `STDLIB-STATUS.json` 中 `not_implemented` / `planned` 状态的函数自动汇总。

### P0（核心基础，立即开始）

#### math 扩展

| ax.函数 | 签名 | 对标 C | 对标 Go | 函数功能 | 说明 | 状态 |
| ------ | ------ | ------ | ------ | ------ | ------ | ----- |
| `math.frexp` | `(f: float) -> (frac: float, exp: int)` | — | — | — | — | ⬜ 计划中 |

#### filepath 扩展

| ax.函数 | 签名 | 对标 C | 对标 Go | 函数功能 | 说明 | 状态 |
| ------ | ------ | ------ | ------ | ------ | ------ | ----- |
| `filepath.eval_symlinks` | `(path: string) -> string` | — | — | — | — | ⬜ 计划中 |
| `filepath.glob` | `(pattern: string) -> string[]` | — | — | — | — | ⬜ 计划中 |
| `filepath.walk` | `(root: string, fn: func) -> int` | — | — | — | — | ⬜ 计划中 |

#### sort 扩展

| ax.函数 | 签名 | 对标 C | 对标 Go | 函数功能 | 说明 | 状态 |
| ------ | ------ | ------ | ------ | ------ | ------ | ----- |
| `sort.slice` | `(a: any[], less: func(i, j int) bool) -> void` | — | — | axlang 不支持闭包，暂不落地 | — | ⬜ 未落地 |
| `sort.slice_stable` | `(a: any[], less: func(i, j int) bool) -> void` | — | — | axlang 不支持闭包，暂不落地 | — | ⬜ 未落地 |

#### slices 扩展

| ax.函数 | 签名 | 对标 C | 对标 Go | 函数功能 | 说明 | 状态 |
| ------ | ------ | ------ | ------ | ------ | ------ | ----- |
| `slices.delete` | `(i, j) -> T[]` | — | — | 与 arr.Remove 语义重复，暂不落地 | — | ⬜ 未落地 |

#### maps 扩展

| ax.函数 | 签名 | 对标 C | 对标 Go | 函数功能 | 说明 | 状态 |
| ------ | ------ | ------ | ------ | ------ | ------ | ----- |
| `maps.delete_func` | `(m: Map, f: func(K) bool) -> void` | — | — | axlang 不支持闭包，暂不落地 | — | ⬜ 未落地 |

#### reflect 扩展

| ax.函数 | 签名 | 对标 C | 对标 Go | 函数功能 | 说明 | 状态 |
| ------ | ------ | ------ | ------ | ------ | ------ | ----- |
| `reflect.type_of` | `(v: any) -> string` | — | — | axlang 无运行时类型反射，需设计可落地子集 | — | ⬜ 未落地 |
| `reflect.value_of` | `(v: any) -> any` | — | — | 同上 | — | ⬜ 未落地 |
| `reflect.num_field` | `(v: any) -> int` | — | — | 同上 | — | ⬜ 未落地 |

---

## 三、当前进度汇总

| 阶段 | 已完成 | 待完成 |
| --- | --- | --- |
| 已落地模块 | io, json, file, db, decimal, int/float/bool/enum, array, map, range, thread, queue, strconv, rand, slices, maps, unique, bytes, bufio, csv, errors, log, sync, atomic | 共 23 个模块 |
| 待落地模块 | math, string, time, os, filepath, fmt, sort, reflect | 共 8 个模块 |
| 已落地函数 | 258 / 272 | — |


*更新于 2026-11-25，基于 STDLIB-STATUS.json 自动生成*
