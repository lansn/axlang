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


### 2. math（对标 Go math / C math.h）


### 3. json（对标 Go encoding/json）


### 4. file（对标 Go os / C stdio/stdlib）


### 5. db（对标 axlang 特有，无直接 Go 对标）


### 6. string（对标 Go strings / C string.h / C ctype.h）

**静态方法：**

| ax.函数          | 签名                                 | 对标                                       | 函数功能/用途           | 状态 | 说明        |
| -------------- | ---------------------------------- | ---------------------------------------- | ----------------- | -- | --------- |
| `string.from_codepoint` | `(cp: int) -> result<string>` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.compare` | `(a: string, b: string) -> int` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.fields` | `(s: string) -> string[]` | — |  | ✅ 已落地 | 标准 C11 实现 |

**实例方法：**

| ax.函数                 | 签名                                     | 对标                                 | 函数功能/用途     | 状态 | 说明        |
| --------------------- | -------------------------------------- | ---------------------------------- | ----------- | -- | --------- |
| `string.to_upper` | `() -> string` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.to_lower` | `() -> string` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.contains` | `(sub: string) -> bool` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.contains_any` | `(chars: string) -> bool` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.contains_rune` | `(r: int) -> bool` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.count` | `(substr: string) -> int` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.index_of` | `(sub: string) -> int` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.index_any` | `(chars: string) -> int` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.last_index_of` | `(sub: string) -> int` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.last_index_any` | `(chars: string) -> int` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.split` | `(sep: string) -> string[]` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.trim` | `() -> string` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.trim_left` | `(cutset: string) -> string` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.trim_right` | `(cutset: string) -> string` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.trim_prefix` | `(prefix: string) -> string` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.trim_suffix` | `(suffix: string) -> string` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.split_n` | `(sep: string, n: int) -> string[]` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.split_after` | `(sep: string) -> string[]` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.title` | `() -> string` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.map` | `(mapping: func(int) int) -> string` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.replace` | `(old: string, new: string) -> string` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.repeat` | `(n: int) -> string` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.starts_with` | `(prefix: string) -> bool` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.ends_with` | `(suffix: string) -> bool` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.len` | `() -> int` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.is_digit` | `() -> bool` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.is_alpha` | `() -> bool` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.is_space` | `() -> bool` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.is_lower` | `() -> bool` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.is_upper` | `() -> bool` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.is_alnum` | `() -> bool` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.equal_fold` | `(t: string) -> bool` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.to_upper_rune` | `(r: int) -> int` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `string.to_lower_rune` | `(r: int) -> int` | — |  | ✅ 已落地 | 标准 C11 实现 |


### 7. time（对标 Go time / C time.h）

**实例方法：**

| ax.函数                 | 签名                                     | 对标                                 | 函数功能/用途     | 状态 | 说明        |
| --------------------- | -------------------------------------- | ---------------------------------- | ----------- | -- | --------- |
| `time.to_string` | `() -> string` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `time.format` | `(fmt: string) -> string` | — |  | ✅ 已落地 | 标准 C11 实现 |

| ax.函数 | 类型 | 签名 | 对标 | 函数功能/用途 | 状态 | 说明 |
| ------ | ------ | ------ | ------ | ------ | ------ | ------ |
| `time.now_ms` | 模块 | `() -> int` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `time.now` | 模块 | `() -> time` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `time.from_unix_ms` | 模块 | `(ms: int) -> time` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `time.sleep` | 模块 | `(ms: int) -> void` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `time.parse` | 模块 | `(s: string, fmt: string) -> result<time>` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `time.unix` | 模块 | `(sec: int, nsec: int) -> time` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `time.unix_milli` | 模块 | `(ms: int) -> time` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `time.unix_micro` | 模块 | `(us: int) -> time` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `time.unix_nano` | 模块 | `(nsec: int) -> time` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `time.since` | 模块 | `(t: time) -> int` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `time.date` | 模块 | `(year, month, day, hour, min, sec, nsec: int) -> time` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `time.until` | 模块 | `(t: time) -> int` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `time.after` | instance | `(u: time) -> bool` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `time.before` | instance | `(u: time) -> bool` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `time.equal` | instance | `(u: time) -> bool` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `time.compare` | instance | `(u: time) -> int` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `time.sub` | instance | `(u: time) -> int` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `time.add` | instance | `(d: int) -> time` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `time.round` | instance | `(d: int) -> time` | — |  | ✅ 已落地 | 标准 C11 实现 |
| `time.truncate` | instance | `(d: int) -> time` | — |  | ✅ 已落地 | 标准 C11 实现 |


### 8. decimal（对标 axlang 特有，高精度十进制）


### 9. int/float/bool/enum（对标 基础类型方法）


### 10. array（对标 Go slice / C 数组）


### 11. map（对标 Go map）


### 12. range（对标 Go range）


### 13. thread（对标 Go channel / goroutine）


### 14. queue（对标 Go channel）


### 15. os（对标 Go os / C stdlib）


### 16. strconv（对标 Go strconv）


### 17. filepath（对标 Go path/filepath）


### 18. rand（对标 Go math/rand）


### 19. fmt（对标 Go fmt / C stdio 格式化）


### 20. sort（对标 Go sort）


### 21. slices（对标 Go slices (Go 1.21+)）


### 22. maps（对标 Go maps (Go 1.21+)）


### 23. unique（对标 Go unique (Go 1.23+)）


### 24. bytes（对标 Go bytes）


### 25. bufio（对标 Go bufio）


### 26. csv（对标 Go encoding/csv）


### 27. errors（对标 Go errors）


### 28. log（对标 Go log）


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


### 31. reflect（对标 Go reflect）


---

## 二、待落地清单（按优先级 P0 → P3）

> 本部分由 `generate_checklist.py` 从 `STDLIB-STATUS.json` 中 `not_implemented` / `planned` 状态的函数自动汇总。

### P0（核心基础，立即开始）

#### math 扩展

| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 |
| ------ | ------ | ------ | ------ | ------ |
| `math.frexp` | `(f: float) -> (frac: float, exp: int)` | — |  | ⬜ 计划中 |

#### filepath 扩展

| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 |
| ------ | ------ | ------ | ------ | ------ |
| `filepath.eval_symlinks` | `(path: string) -> string` | — |  | ⬜ 计划中 |
| `filepath.glob` | `(pattern: string) -> string[]` | — |  | ⬜ 计划中 |
| `filepath.walk` | `(root: string, fn: func) -> int` | — |  | ⬜ 计划中 |

#### sort 扩展

| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 |
| ------ | ------ | ------ | ------ | ------ |
| `sort.slice` | `(a: any[], less: func(i, j int) bool) -> void` | — | axlang 不支持闭包，暂不落地 | ⬜ 未落地 |
| `sort.slice_stable` | `(a: any[], less: func(i, j int) bool) -> void` | — | axlang 不支持闭包，暂不落地 | ⬜ 未落地 |

#### slices 扩展

| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 |
| ------ | ------ | ------ | ------ | ------ |
| `slices.delete` | `(i, j) -> T[]` | — | 与 arr.Remove 语义重复，暂不落地 | ⬜ 未落地 |

#### maps 扩展

| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 |
| ------ | ------ | ------ | ------ | ------ |
| `maps.delete_func` | `(m: Map, f: func(K) bool) -> void` | — | axlang 不支持闭包，暂不落地 | ⬜ 未落地 |

#### reflect 扩展

| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 |
| ------ | ------ | ------ | ------ | ------ |
| `reflect.type_of` | `(v: any) -> string` | — | axlang 无运行时类型反射，需设计可落地子集 | ⬜ 未落地 |
| `reflect.value_of` | `(v: any) -> any` | — | 同上 | ⬜ 未落地 |
| `reflect.num_field` | `(v: any) -> int` | — | 同上 | ⬜ 未落地 |

---

## 三、当前进度汇总

| 阶段 | 已完成 | 待完成 |
| --- | --- | --- |
| 已落地模块 | io, json, file, db, decimal, int/float/bool/enum, array, map, range, thread, queue, strconv, rand, slices, maps, unique, bytes, bufio, csv, errors, log, sync, atomic | 共 23 个模块 |
| 待落地模块 | math, string, time, os, filepath, fmt, sort, reflect | 共 8 个模块 |
| 已落地函数 | 258 / 272 | — |


*更新于 2026-11-25，基于 STDLIB-STATUS.json 自动生成*
