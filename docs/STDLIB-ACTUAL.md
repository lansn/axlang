# axlang 标准库参考（基于实际 ax 源码）

> **生成依据**：`axc/std/*.ax` 实际源码（v0.0.28），与编译器源码 `rust/src/sema/` 同步核对。
> **修订日期**：2026-11-27
> **跨项目铁律**：所有涉及及生成的 C 代码 / GCC 源码，均必须使用标准 C（C11），不得使用任何 GNU/Clang 扩展。

---

## 一、模块概览

axlang 标准库共 **16 个模块**，全部位于 `axc/std/*.ax`，通过 `import std.xxx` 导入。成员名统一使用 camelCase（如 `std.io.readLine`）。

与编译器内建模块（`INTERNAL_BUILTINS`：db/fmt/log/maps/reflect/sort/unique）严格互斥，不得并存。

| 模块 | 源文件 | 函数数 | 说明 |
|------|--------|--------|------|
| `std.errors` | `axc/std/errors.ax` | 3 | 错误类型定义（new_error/error_is/error_as） |
| `std.file` | `axc/std/file.ax` | 19 | 文件读写（open/create/append/read/writeLine/close 等） |
| `std.io` | `axc/std/io.ax` | 4 | 标准输入输出（readLine/readFile/write/writeLine） |
| `std.json` | `axc/std/json.ax` | 3 | JSON 解析与序列化（stringify/marshalIndent/valid） |
| `std.math` | `axc/std/math.ax` | 36 | 数学函数（abs/sin/cos/sqrt/exp/log 等） |
| `std.os` | `axc/std/os.ax` | 19 | 操作系统接口（getenv/getpid/getwd/args/environ 等） |
| `std.rand` | `axc/std/rand.ax` | 3 | 随机数生成（int/float/seed） |
| `std.strconv` | `axc/std/strconv.ax` | 11 | 字符串转数值（itoa/formatUint/formatInt 等） |
| `std.string` | `axc/std/string.ax` | 25 | 字符串操作（compare/contains/indexOf/toUpper/toLower 等） |
| `std.sync` | `axc/std/sync.ax` | 10 | 同步原语（mutexLock/rwmutexLock/waitgroupAdd 等） |
| `std.time` | `axc/std/time.ax` | 14 | 时间处理（nowMs/now/sleep/unix/date/toString/round/truncate 等） |
| `std.bytes` | `axc/std/bytes.ax` | 8 | 字节数组操作（contains/equal/index/split 等） |
| `std.bufio` | `axc/std/bufio.ax` | 3 | 缓冲 I/O（newReader/newWriter/scanner） |
| `std.csv` | `axc/std/csv.ax` | 2 | CSV 读写（readAll/write） |
| `std.filepath` | `axc/std/filepath.ax` | 12 | 路径操作（rel/join/dir/base/ext/split/clean/abs 等） |
| `std.atomic` | `axc/std/atomic.ax` | 4 | 原子操作（addInt/loadInt/storeInt/compareAndSwapInt） |

**合计：16 个模块，约 149 个 public 函数**

---

## 二、逐模块函数详情

### 2.1 std.errors —— 错误类型

源文件：`axc/std/errors.ax`

| 函数名 | 签名 | 功能 | 输入参数 | 输出参数 |
|--------|------|------|----------|----------|
| `new` | `new(msg: string) -> error` | 创建一个新的 error，错误码固定为 1，消息为 msg | `msg` (string) - 错误消息文本 | `error` - 新创建的 error 实例 |
| `is` | `is(err: error, target: error) -> int` | 判断 err 是否等于 target | `err` (error) - 被比较的 error；`target` (error) - 目标 error | `int` - 1 表示相等，0 表示不相等 |
| `as` | `as(err: error, target: ref error) -> int` | 尝试将 err 断言为 target 类型 | `err` (error) - 被断言的 error；`target` (ref error) - 断言结果输出目标 | `int` - 1 表示断言成功，0 表示失败 |

---

### 2.2 std.file —— 文件读写

源文件：`axc/std/file.ax`

| 函数名 | 签名 | 功能 | 输入参数 | 输出参数 |
|--------|------|------|----------|----------|
| `open` | `open(path: string) -> File` | 打开指定路径的文件 | `path` (string) - 文件路径 | `File` - 文件句柄 |
| `create` | `create(path: string) -> File` | 创建并打开指定路径的文件 | `path` (string) - 文件路径 | `File` - 文件句柄 |
| `append` | `append(path: string) -> File` | 以追加模式打开指定路径的文件 | `path` (string) - 文件路径 | `File` - 文件句柄 |
| `remove` | `remove(path: string) -> int` | 删除指定路径的文件 | `path` (string) - 文件路径 | `int` - 0 表示成功，非 0 表示失败 |
| `rename` | `rename(old_path: string, new_path: string) -> int` | 重命名文件 | `old_path` (string) - 原文件路径；`new_path` (string) - 新文件路径 | `int` - 0 表示成功，非 0 表示失败 |
| `exists` | `exists(path: string) -> int` | 判断路径是否存在 | `path` (string) - 待判断的路径 | `int` - 1 表示存在，0 表示不存在 |
| `mkdir` | `mkdir(path: string) -> int` | 创建目录 | `path` (string) - 目录路径 | `int` - 0 表示成功，非 0 表示失败 |
| `isDir` | `isDir(path: string) -> int` | 判断路径是否为目录 | `path` (string) - 待判断的路径 | `int` - 1 表示是目录，0 表示不是 |
| `isFile` | `isFile(path: string) -> int` | 判断路径是否为普通文件 | `path` (string) - 待判断的路径 | `int` - 1 表示是普通文件，0 表示不是 |
| `size` | `size(path: string) -> int` | 返回文件大小（字节） | `path` (string) - 文件路径 | `int` - 文件大小（字节），失败返回 -1 |
| `modTime` | `modTime(path: string) -> int` | 返回文件修改时间（Unix 时间戳，秒） | `path` (string) - 文件路径 | `int` - 修改时间（秒），失败返回 -1 |
| `writeAll` | `writeAll(path: string, s: string) -> int` | 将字符串 s 写入指定路径的文件 | `path` (string) - 文件路径；`s` (string) - 要写入的字符串 | `int` - 0 表示成功，非 0 表示失败 |
| `chmod` | `chmod(path: string, mode: int) -> int` | 修改文件权限模式 | `path` (string) - 文件路径；`mode` (int) - 权限模式 | `int` - 0 表示成功，非 0 表示失败 |
| `chtimes` | `chtimes(path: string, atime: int, mtime: int) -> int` | 修改文件访问时间和修改时间 | `path` (string) - 文件路径；`atime` (int) - 访问时间；`mtime` (int) - 修改时间 | `int` - 0 表示成功，非 0 表示失败 |
| `tempFile` | `tempFile(dir: string, prefix: string) -> File` | 在指定目录创建带前缀的临时文件 | `dir` (string) - 目录路径；`prefix` (string) - 文件名前缀 | `File` - 临时文件句柄 |
| `readAll` | `readAll(f: File) -> string` | 读取文件 f 的全部内容 | `f` (File) - 文件句柄 | `string` - 文件全部内容 |
| `write` | `write(f: ref File, s: string) -> int` | 将字符串 s 写入文件 f | `f` (ref File) - 文件句柄引用；`s` (string) - 要写入的字符串 | `int` - 写入的字节数或状态码 |
| `writeLine` | `writeLine(f: ref File, s: string) -> int` | 将字符串 s 换行写入文件 f | `f` (ref File) - 文件句柄引用；`s` (string) - 要写入的字符串 | `int` - 写入的字节数或状态码 |
| `close` | `close(f: ref File) -> void` | 关闭文件 f | `f` (ref File) - 文件句柄引用 | 无 |

---

### 2.4 std.io —— 标准输入输出

源文件：`axc/std/io.ax`

| 函数名 | 签名 | 功能 | 输入参数 | 输出参数 |
|--------|------|------|----------|----------|
| `readLine` | `readLine(out: ref string) -> int` | 从标准输入读取一行，存入 out | `out` (ref string) - 存储读取内容的输出参数 | `int` - 读取的字节数，EOF 返回 -1 |
| `readFile` | `readFile(path: string, out: ref string) -> int` | 读取指定路径文件的全部内容，存入 out | `path` (string) - 文件路径；`out` (ref string) - 存储文件内容的输出参数 | `int` - 0 表示成功，非 0 表示失败 |
| `write` | `write(s: string) -> void` | 将字符串 s 输出到标准输出（不换行） | `s` (string) - 要输出的字符串 | 无 |
| `writeLine` | `writeLine(s: string) -> void` | 将字符串 s 输出到标准输出并换行 | `s` (string) - 要输出的字符串 | 无 |

---

### 2.5 std.json —— JSON 解析与序列化

源文件：`axc/std/json.ax`

| 函数名 | 签名 | 功能 | 输入参数 | 输出参数 |
|--------|------|------|----------|----------|
| `stringify` | `stringify(v: object) -> string` | 将对象 v 序列化为 JSON 字符串 | `v` (object) - 要序列化的对象 | `string` - JSON 格式的字符串 |
| `marshalIndent` | `marshalIndent(v: object, prefix: string, indent: string) -> string` | 将对象 v 序列化为带缩进的 JSON 字符串 | `v` (object) - 要序列化的对象；`prefix` (string) - 每行前缀；`indent` (string) - 缩进字符串 | `string` - 带缩进的 JSON 格式字符串 |
| `valid` | `valid(s: string) -> int` | 判断字符串 s 是否为合法的 JSON | `s` (string) - 待判断的字符串 | `int` - 1 表示合法，0 表示非法 |

#### ⚠️ json 模块与 STDLIB-STATUS.json 的不一致说明

经核对 `axc/std/json.ax` 实际源码与 `STDLIB-STATUS.json` 中的 json 模块条目，存在以下不一致：

| 项目 | `axc/std/json.ax` 实际 | `STDLIB-STATUS.json` 记录 | 说明 |
|------|------------------------|--------------------------|------|
| `parse` | **不存在** | 列为 `(s: string) -> result<map<string, object>>`，状态 `implemented` | `json.ax` 第 10-11 行注释说明：`json.parse` 返回 `result<map<string, object>>`，涉及运行时结果类型，当前 ax 源码层先不独立包装，可继续沿用内建模块或后续补齐。实际源码中没有 `parse` 函数。 |
| `stringify` | `(v: object) -> string` | `(v: object) -> string` | 一致 |
| `marshalIndent` | `(v: object, prefix: string, indent: string) -> string` | `(v: object, prefix: string, indent: string) -> string` | 一致 |
| `valid` | `(s: string) -> int`（返回 1/0） | `(s: string) -> bool` | 返回类型不一致：实际 ax 源码返回 `int`（1 合法，0 非法），STDLIB-STATUS.json 记录为 `bool` |

**建议**：以 `axc/std/json.ax` 实际源码为准。若需要 `parse` 功能，可暂时沿用编译器内建模块，或后续在 ax 层补齐。

---

### 2.6 std.math —— 数学函数

源文件：`axc/std/math.ax`

| 函数名 | 签名 | 功能 | 输入参数 | 输出参数 |
|--------|------|------|----------|----------|
| `abs` | `abs(x: float) -> float` | 返回 x 的绝对值 | `x` (float) - 输入浮点数 | `float` - x 的绝对值 |
| `max` | `max(a: float, b: float) -> float` | 返回 a 和 b 中的较大值 | `a` (float) - 第一个浮点数；`b` (float) - 第二个浮点数 | `float` - 较大值 |
| `min` | `min(a: float, b: float) -> float` | 返回 a 和 b 中的较小值 | `a` (float) - 第一个浮点数；`b` (float) - 第二个浮点数 | `float` - 较小值 |
| `sqrt` | `sqrt(x: float) -> float` | 返回 x 的平方根 | `x` (float) - 输入浮点数 | `float` - x 的平方根 |
| `pow` | `pow(a: float, b: float) -> float` | 返回 a 的 b 次幂 | `a` (float) - 底数；`b` (float) - 指数 | `float` - a 的 b 次幂 |
| `exp` | `exp(x: float) -> float` | 返回 e 的 x 次幂 | `x` (float) - 指数 | `float` - e 的 x 次幂 |
| `log` | `log(x: float) -> float` | 返回 x 的自然对数 | `x` (float) - 输入浮点数 | `float` - x 的自然对数 |
| `sin` | `sin(x: float) -> float` | 返回 x 的正弦值（弧度） | `x` (float) - 角度（弧度） | `float` - 正弦值 |
| `cos` | `cos(x: float) -> float` | 返回 x 的余弦值（弧度） | `x` (float) - 角度（弧度） | `float` - 余弦值 |
| `tan` | `tan(x: float) -> float` | 返回 x 的正切值（弧度） | `x` (float) - 角度（弧度） | `float` - 正切值 |
| `ceil` | `ceil(x: float) -> float` | 返回不小于 x 的最小整数（向上取整） | `x` (float) - 输入浮点数 | `float` - 向上取整后的值 |
| `floor` | `floor(x: float) -> float` | 返回不大于 x 的最大整数（向下取整） | `x` (float) - 输入浮点数 | `float` - 向下取整后的值 |
| `round` | `round(x: float) -> float` | 返回 x 四舍五入后的整数 | `x` (float) - 输入浮点数 | `float` - 四舍五入后的值 |
| `sign` | `sign(x: float) -> int` | 返回 x 的符号，正数返回 1，负数返回 -1，零返回 0 | `x` (float) - 输入浮点数 | `int` - 符号值（1/-1/0） |
| `mod` | `mod(x: float, y: float) -> float` | 返回 x 除以 y 的余数（浮点取模） | `x` (float) - 被除数；`y` (float) - 除数 | `float` - 余数 |
| `equals` | `equals(a: float, b: float, eps: float) -> int` | 判断 a 和 b 是否在 eps 容差范围内相等 | `a` (float) - 第一个浮点数；`b` (float) - 第二个浮点数；`eps` (float) - 容差 | `int` - 1 表示相等，0 表示不相等 |
| `compare` | `compare(a: float, b: float) -> int` | 比较 a 和 b | `a` (float) - 第一个浮点数；`b` (float) - 第二个浮点数 | `int` - -1 表示 a<b，0 表示 a=b，1 表示 a>b |
| `exp2` | `exp2(x: float) -> float` | 返回 2 的 x 次幂 | `x` (float) - 指数 | `float` - 2 的 x 次幂 |
| `log10` | `log10(x: float) -> float` | 返回 x 的以 10 为底的对数 | `x` (float) - 输入浮点数 | `float` - x 的以 10 为底的对数 |
| `log2` | `log2(x: float) -> float` | 返回 x 的以 2 为底的对数 | `x` (float) - 输入浮点数 | `float` - x 的以 2 为底的对数 |
| `log1p` | `log1p(x: float) -> float` | 返回 ln(1 + x) | `x` (float) - 输入浮点数 | `float` - ln(1 + x) 的值 |
| `isNaN` | `isNaN(x: float) -> int` | 判断 x 是否为 NaN | `x` (float) - 输入浮点数 | `int` - 1 表示是 NaN，0 表示不是 |
| `isInf` | `isInf(x: float, sign: int) -> int` | 判断 x 是否为无穷大 | `x` (float) - 输入浮点数；`sign` (int) - 符号标志（>0 检查正无穷，<0 检查负无穷，=0 检查任意无穷） | `int` - 1 表示是无穷大，0 表示不是 |
| `signbit` | `signbit(x: float) -> int` | 判断 x 的符号位是否为负 | `x` (float) - 输入浮点数 | `int` - 1 表示符号位为负，0 表示不是 |
| `copysign` | `copysign(x: float, y: float) -> float` | 返回带 y 符号的 x 的绝对值 | `x` (float) - 输入浮点数；`y` (float) - 符号来源浮点数 | `float` - 带 y 符号的 \|x\| |
| `dim` | `dim(x: float, y: float) -> float` | 返回 max(x - y, 0) | `x` (float) - 第一个浮点数；`y` (float) - 第二个浮点数 | `float` - max(x - y, 0) 的值 |
| `ldexp` | `ldexp(x: float, exp: int) -> float` | 返回 x × 2^exp | `x` (float) - 底数；`exp` (int) - 指数 | `float` - x × 2^exp 的值 |
| `sinh` | `sinh(x: float) -> float` | 返回 x 的双曲正弦值 | `x` (float) - 输入浮点数 | `float` - 双曲正弦值 |
| `cosh` | `cosh(x: float) -> float` | 返回 x 的双曲余弦值 | `x` (float) - 输入浮点数 | `float` - 双曲余弦值 |
| `tanh` | `tanh(x: float) -> float` | 返回 x 的双曲正切值 | `x` (float) - 输入浮点数 | `float` - 双曲正切值 |
| `asin` | `asin(x: float) -> float` | 返回 x 的反正弦值（弧度） | `x` (float) - 输入浮点数 | `float` - 反正弦值（弧度） |
| `acos` | `acos(x: float) -> float` | 返回 x 的反余弦值（弧度） | `x` (float) - 输入浮点数 | `float` - 反余弦值（弧度） |
| `atan` | `atan(x: float) -> float` | 返回 x 的反正切值（弧度） | `x` (float) - 输入浮点数 | `float` - 反正切值（弧度） |
| `atan2` | `atan2(y: float, x: float) -> float` | 返回 y/x 的反正切值（考虑象限，弧度） | `y` (float) - 分子；`x` (float) - 分母 | `float` - 反正切值（弧度） |
| `hypot` | `hypot(p: float, q: float) -> float` | 返回 sqrt(p² + q²) | `p` (float) - 第一个浮点数；`q` (float) - 第二个浮点数 | `float` - sqrt(p² + q²) 的值 |
| `cbrt` | `cbrt(x: float) -> float` | 返回 x 的立方根 | `x` (float) - 输入浮点数 | `float` - x 的立方根 |
| `trunc` | `trunc(x: float) -> float` | 返回 x 向零取整的整数部分 | `x` (float) - 输入浮点数 | `float` - 向零取整后的值 |
| `frexp` | `frexp(f: float) -> (float, int)` | 将 f 分解为尾数和 2 的幂指数 | `f` (float) - 输入浮点数 | `(float, int)` - 尾数和指数 |

---

### 2.7 std.os —— 操作系统接口

源文件：`axc/std/os.ax`

| 函数名 | 签名 | 功能 | 输入参数 | 输出参数 |
|--------|------|------|----------|----------|
| `getenv` | `getenv(name: string) -> string` | 获取环境变量 name 的值 | `name` (string) - 环境变量名 | `string` - 环境变量值，未设置返回空字符串 |
| `userHomeDir` | `userHomeDir() -> string` | 返回当前用户的主目录路径 | 无 | `string` - 用户主目录路径 |
| `exit` | `exit(code: int) -> void` | 以退出码 code 终止当前进程 | `code` (int) - 退出码 | 无 |
| `getwd` | `getwd() -> string` | 返回当前工作目录路径 | 无 | `string` - 当前工作目录路径 |
| `chdir` | `chdir(path: string) -> int` | 将当前工作目录切换为 path | `path` (string) - 目标目录路径 | `int` - 0 表示成功，非 0 表示失败 |
| `getpid` | `getpid() -> int` | 返回当前进程 ID | 无 | `int` - 当前进程 ID |
| `getppid` | `getppid() -> int` | 返回当前进程的父进程 ID | 无 | `int` - 父进程 ID |
| `hostname` | `hostname() -> string` | 返回主机名 | 无 | `string` - 主机名 |
| `setenv` | `setenv(name: string, value: string) -> int` | 设置环境变量 name 为 value | `name` (string) - 环境变量名；`value` (string) - 环境变量值 | `int` - 0 表示成功，非 0 表示失败 |
| `unsetenv` | `unsetenv(name: string) -> int` | 删除环境变量 name | `name` (string) - 环境变量名 | `int` - 0 表示成功，非 0 表示失败 |
| `userCacheDir` | `userCacheDir() -> string` | 返回当前用户的缓存目录路径 | 无 | `string` - 用户缓存目录路径 |
| `getpagesize` | `getpagesize() -> int` | 返回系统页面大小（字节） | 无 | `int` - 系统页面大小（字节） |
| `getuid` | `getuid() -> int` | 返回当前用户 ID | 无 | `int` - 当前用户 ID |
| `getgid` | `getgid() -> int` | 返回当前组 ID | 无 | `int` - 当前组 ID |
| `geteuid` | `geteuid() -> int` | 返回当前有效用户 ID | 无 | `int` - 当前有效用户 ID |
| `getegid` | `getegid() -> int` | 返回当前有效组 ID | 无 | `int` - 当前有效组 ID |
| `getgroups` | `getgroups() -> int[]` | 返回当前用户所属的组 ID 列表 | 无 | `int[]` - 组 ID 列表 |
| `executable` | `executable() -> string` | 返回当前可执行文件的路径 | 无 | `string` - 可执行文件路径 |
| `args` | `args() -> string[]` | 返回命令行位置参数列表（不含程序名本身，对标 Go os.Args[1:]） | 无 | `string[]` - 命令行位置参数数组 |
| `environ` | `environ() -> string[]` | 返回所有环境变量 "KEY=VALUE" 数组（对标 Go os.Environ） | 无 | `string[]` - 环境变量数组，每项为 "KEY=VALUE" |

---

### 2.8 std.rand —— 随机数生成

源文件：`axc/std/rand.ax`

| 函数名 | 签名 | 功能 | 输入参数 | 输出参数 |
|--------|------|------|----------|----------|
| `int` | `int(n: int) -> int` | 返回 [0, n) 范围内的随机整数 | `n` (int) - 上限（不包含） | `int` - [0, n) 范围内的随机整数 |
| `float` | `float() -> float` | 返回 [0.0, 1.0) 范围内的随机浮点数 | 无 | `float` - [0.0, 1.0) 范围内的随机浮点数 |
| `seed` | `seed(seed: int) -> void` | 设置随机数生成器的种子 | `seed` (int) - 随机种子 | 无 |

---

### 2.9 std.strconv —— 字符串转数值

源文件：`axc/std/strconv.ax`

| 函数名 | 签名 | 功能 | 输入参数 | 输出参数 |
|--------|------|------|----------|----------|
| `itoa` | `itoa(v: int) -> string` | 把 int 转成十进制字符串（integer to ASCII，如 42 → "42"） | `v` (int) - 要转换的整数 | `string` - 十进制字符串表示 |
| `formatUint` | `formatUint(v: int, base: int) -> string` | 将整数 v 转换为指定进制的字符串表示（无符号） | `v` (int) - 要转换的整数；`base` (int) - 进制基数 | `string` - 指定进制的字符串表示 |
| `formatInt` | `formatInt(v: int, base: int) -> string` | 将整数 v 转换为指定进制的字符串表示（有符号） | `v` (int) - 要转换的整数；`base` (int) - 进制基数 | `string` - 指定进制的字符串表示 |
| `formatBool` | `formatBool(v: bool) -> string` | 将布尔值 v 转换为字符串 "true" 或 "false" | `v` (bool) - 要转换的布尔值 | `string` - "true" 或 "false" |
| `formatFloat` | `formatFloat(v: float) -> string` | 将浮点数 v 转换为字符串表示 | `v` (float) - 要转换的浮点数 | `string` - 浮点数的字符串表示 |
| `parseBool` | `parseBool(s: string) -> int` | 将字符串 s 解析为布尔值 | `s` (string) - 待解析的字符串 | `int` - 1 表示 true，0 表示 false，-1 表示非法 |
| `parseInt` | `parseInt(s: string, base: int) -> int` | 将字符串 s 按指定进制解析为整数 | `s` (string) - 待解析的字符串；`base` (int) - 进制基数 | `int` - 解析后的整数值 |
| `parseUint` | `parseUint(s: string, base: int) -> int` | 将字符串 s 按指定进制解析为无符号整数 | `s` (string) - 待解析的字符串；`base` (int) - 进制基数 | `int` - 解析后的无符号整数值 |
| `canBackquote` | `canBackquote(s: string) -> int` | 判断字符串 s 是否可以用反引号直接表示（不含特殊字符） | `s` (string) - 待判断的字符串 | `int` - 1 表示可以，0 表示不可以 |
| `quote` | `quote(s: string) -> string` | 将字符串 s 加上 Go 风格的双引号和转义 | `s` (string) - 要加引号的字符串 | `string` - 加引号后的字符串 |
| `unquote` | `unquote(s: string) -> string` | 去除字符串 s 的 Go 风格双引号和转义 | `s` (string) - 要去引号的字符串 | `string` - 去引号后的字符串 |

---

### 2.10 std.string —— 字符串操作

源文件：`axc/std/string.ax`

> 注：`string` 既是 axlang 内建类型名，也是 `std.string` 模块名。因名称冲突，调用 `std.string` 模块函数时建议使用全路径 `std.string.compare(...)`，而非短名 `string.compare(...)`（短名会被解析为内建类型 `string` 的静态方法）。

| 函数名 | 签名 | 功能 | 输入参数 | 输出参数 |
|--------|------|------|----------|----------|
| `compare` | `compare(a: string, b: string) -> int` | 字典序比较 a 和 b | `a` (string) - 第一个字符串；`b` (string) - 第二个字符串 | `int` - -1 表示 a<b，0 表示 a=b，1 表示 a>b |
| `contains` | `contains(s: string, sub: string) -> int` | 判断 s 是否包含子串 sub | `s` (string) - 主字符串；`sub` (string) - 子串 | `int` - 1 表示包含，0 表示不包含 |
| `indexOf` | `indexOf(s: string, sub: string) -> int` | 返回子串 sub 在 s 中首次出现的索引 | `s` (string) - 主字符串；`sub` (string) - 子串 | `int` - 首次出现的索引，未找到返回 -1 |
| `lastIndexOf` | `lastIndexOf(s: string, sub: string) -> int` | 返回子串 sub 在 s 中最后出现的索引 | `s` (string) - 主字符串；`sub` (string) - 子串 | `int` - 最后出现的索引，未找到返回 -1 |
| `count` | `count(s: string, substr: string) -> int` | 返回子串 substr 在 s 中非重叠出现的次数 | `s` (string) - 主字符串；`substr` (string) - 子串 | `int` - 出现次数 |
| `trim` | `trim(s: string) -> string` | 去除字符串 s 两端的空白字符 | `s` (string) - 待处理的字符串 | `string` - 去除空白后的字符串 |
| `toUpper` | `toUpper(s: string) -> string` | 将字符串 s 转换为大写 | `s` (string) - 待转换的字符串 | `string` - 大写后的字符串 |
| `toLower` | `toLower(s: string) -> string` | 将字符串 s 转换为小写 | `s` (string) - 待转换的字符串 | `string` - 小写后的字符串 |
| `repeat` | `repeat(s: string, n: int) -> string` | 将字符串 s 重复 n 次 | `s` (string) - 字符串；`n` (int) - 重复次数 | `string` - 重复后的字符串 |
| `replace` | `replace(s: string, old: string, replacement: string) -> string` | 将字符串 s 中所有 old 替换为 replacement | `s` (string) - 原字符串；`old` (string) - 被替换的子串；`replacement` (string) - 替换后的子串 | `string` - 替换后的字符串 |
| `startsWith` | `startsWith(s: string, prefix: string) -> int` | 判断 s 是否以 prefix 开头 | `s` (string) - 主字符串；`prefix` (string) - 前缀 | `int` - 1 表示是，0 表示否 |
| `endsWith` | `endsWith(s: string, suffix: string) -> int` | 判断 s 是否以 suffix 结尾 | `s` (string) - 主字符串；`suffix` (string) - 后缀 | `int` - 1 表示是，0 表示否 |
| `isDigit` | `isDigit(s: string) -> int` | 判断 s 是否全为数字字符 | `s` (string) - 待判断的字符串 | `int` - 1 表示全为数字，0 表示不是 |
| `isAlpha` | `isAlpha(s: string) -> int` | 判断 s 是否全为字母字符 | `s` (string) - 待判断的字符串 | `int` - 1 表示全为字母，0 表示不是 |
| `isSpace` | `isSpace(s: string) -> int` | 判断 s 是否全为空白字符 | `s` (string) - 待判断的字符串 | `int` - 1 表示全为空白，0 表示不是 |
| `isLower` | `isLower(s: string) -> int` | 判断 s 是否全为小写字母 | `s` (string) - 待判断的字符串 | `int` - 1 表示全为小写字母，0 表示不是 |
| `isUpper` | `isUpper(s: string) -> int` | 判断 s 是否全为大写字母 | `s` (string) - 待判断的字符串 | `int` - 1 表示全为大写字母，0 表示不是 |
| `isAlnum` | `isAlnum(s: string) -> int` | 判断 s 是否全为字母或数字 | `s` (string) - 待判断的字符串 | `int` - 1 表示全为字母或数字，0 表示不是 |
| `containsRune` | `containsRune(s: string, cp: int) -> int` | 判断 s 是否包含指定的 UTF-8 码点 cp | `s` (string) - 主字符串；`cp` (int) - 待查找的 Unicode 码点 | `int` - 1 表示包含，0 表示不包含 |
| `containsAny` | `containsAny(s: string, chars: string) -> int` | 判断 s 是否包含 chars 中的任意字符 | `s` (string) - 主字符串；`chars` (string) - 字符集 | `int` - 1 表示包含任意字符，0 表示不包含 |
| `indexAny` | `indexAny(s: string, chars: string) -> int` | 返回 chars 中任意字符在 s 中首次出现的码点索引 | `s` (string) - 主字符串；`chars` (string) - 字符集 | `int` - 首次出现的索引，未找到返回 -1 |
| `lastIndexAny` | `lastIndexAny(s: string, chars: string) -> int` | 返回 chars 中任意字符在 s 中最后出现的码点索引 | `s` (string) - 主字符串；`chars` (string) - 字符集 | `int` - 最后出现的索引，未找到返回 -1 |
| `title` | `title(s: string) -> string` | 将字符串 s 转换为标题大写（每个单词首字母大写，仅处理 ASCII） | `s` (string) - 待转换的字符串 | `string` - 标题大写后的字符串 |
| `equalFold` | `equalFold(a: string, b: string) -> int` | 忽略大小写比较两个字符串是否相等（仅处理 ASCII 大小写） | `a` (string) - 第一个字符串；`b` (string) - 第二个字符串 | `int` - 1 表示相等，0 表示不相等 |
| `toUpperRune` | `toUpperRune(c: int) -> int` | 将单个字符 c 转换为大写（仅处理 ASCII a-z<->A-Z，非 ASCII 原样返回） | `c` (int) - 待转换的字符（Unicode 码点） | `int` - 转换后的字符码点 |
| `toLowerRune` | `toLowerRune(c: int) -> int` | 将单个字符 c 转换为小写（仅处理 ASCII A-Z<->a-z，非 ASCII 原样返回） | `c` (int) - 待转换的字符（Unicode 码点） | `int` - 转换后的字符码点 |

---

### 2.11 std.sync —— 同步原语

源文件：`axc/std/sync.ax`

> 注：Mutex/RWMutex/WaitGroup/Once 为 axlang 内建句柄类型，零初始化后直接使用。

| 函数名 | 签名 | 功能 | 输入参数 | 输出参数 |
|--------|------|------|----------|----------|
| `mutexLock` | `mutexLock(m: ref Mutex) -> void` | 对互斥锁 m 加锁，阻塞直到获得锁 | `m` (ref Mutex) - 互斥锁引用 | 无 |
| `mutexUnlock` | `mutexUnlock(m: ref Mutex) -> void` | 对互斥锁 m 解锁 | `m` (ref Mutex) - 互斥锁引用 | 无 |
| `rwmutexLock` | `rwmutexLock(rw: ref RWMutex) -> void` | 对读写锁 rw 加写锁，阻塞直到获得锁 | `rw` (ref RWMutex) - 读写锁引用 | 无 |
| `rwmutexUnlock` | `rwmutexUnlock(rw: ref RWMutex) -> void` | 对读写锁 rw 解写锁 | `rw` (ref RWMutex) - 读写锁引用 | 无 |
| `rwmutexRLock` | `rwmutexRLock(rw: ref RWMutex) -> void` | 对读写锁 rw 加读锁，阻塞直到获得锁 | `rw` (ref RWMutex) - 读写锁引用 | 无 |
| `rwmutexRunlock` | `rwmutexRunlock(rw: ref RWMutex) -> void` | 对读写锁 rw 解读锁 | `rw` (ref RWMutex) - 读写锁引用 | 无 |
| `waitgroupAdd` | `waitgroupAdd(wg: ref WaitGroup, delta: int) -> void` | 为等待组 wg 添加 delta 个计数 | `wg` (ref WaitGroup) - 等待组引用；`delta` (int) - 增加的计数值 | 无 |
| `waitgroupDone` | `waitgroupDone(wg: ref WaitGroup) -> void` | 为等待组 wg 的计数减 1 | `wg` (ref WaitGroup) - 等待组引用 | 无 |
| `waitgroupWait` | `waitgroupWait(wg: ref WaitGroup) -> void` | 阻塞直到等待组 wg 的计数为 0 | `wg` (ref WaitGroup) - 等待组引用 | 无 |
| `onceDo` | `onceDo(o: ref Once) -> void` | 确保只执行一次，适用于单例初始化 | `o` (ref Once) - Once 控制引用 | 无 |

---

### 2.12 std.time —— 时间处理

源文件：`axc/std/time.ax`

> 注：`time` 是 axlang 的内建具名类型（C 层 int64_t），与 int 在 ax 类型系统中不做隐式转换，且 ax 无显式 cast 语法。

| 函数名 | 签名 | 功能 | 输入参数 | 输出参数 |
|--------|------|------|----------|----------|
| `nowMs` | `nowMs() -> int` | 返回当前时间的毫秒时间戳（Unix 时间戳，整数） | 无 | `int` - 当前时间的毫秒时间戳 |
| `now` | `now() -> time` | 返回当前的 time 值 | 无 | `time` - 当前时间值 |
| `sleep` | `sleep(ms: int) -> void` | 休眠指定的毫秒数 | `ms` (int) - 休眠毫秒数 | 无 |
| `unix` | `unix(sec: int, nsec: int) -> time` | 根据秒和纳秒构造 time 值 | `sec` (int) - 秒；`nsec` (int) - 纳秒 | `time` - 构造的时间值 |
| `unixMilli` | `unixMilli(ms: int) -> time` | 根据毫秒时间戳构造 time 值 | `ms` (int) - 毫秒时间戳 | `time` - 构造的时间值 |
| `fromUnixMs` | `fromUnixMs(ms: int) -> time` | 根据毫秒时间戳构造 time 值（与 unixMilli 相同） | `ms` (int) - 毫秒时间戳 | `time` - 构造的时间值 |
| `unixMicro` | `unixMicro(us: int) -> time` | 根据微秒时间戳构造 time 值 | `us` (int) - 微秒时间戳 | `time` - 构造的时间值 |
| `unixNano` | `unixNano(nsec: int) -> time` | 根据纳秒时间戳构造 time 值 | `nsec` (int) - 纳秒时间戳 | `time` - 构造的时间值 |
| `since` | `since(t: time) -> int` | 返回从 t 到现在经过的毫秒数 | `t` (time) - 起始时间值 | `int` - 经过的毫秒数 |
| `date` | `date(year: int, month: int, day: int, hour: int, min: int, sec: int, nsec: int) -> time` | 根据日期时间构造 time 值 | `year` (int) - 年；`month` (int) - 月；`day` (int) - 日；`hour` (int) - 时；`min` (int) - 分；`sec` (int) - 秒；`nsec` (int) - 纳秒 | `time` - 构造的时间值 |
| `until` | `until(t: time) -> int` | 返回从现在到 t 的剩余毫秒数 | `t` (time) - 目标时间值 | `int` - 剩余毫秒数 |
| `toString` | `toString(ms: int) -> string` | 按 "YYYY-MM-DD HH:MM:SS" 格式将毫秒时间戳格式化为字符串 | `ms` (int) - Unix 毫秒时间戳 | `string` - 格式化后的时间字符串 |
| `round` | `round(t: time, d: int) -> time` | 将时间 t 四舍五入到 d 毫秒间隔的最近整数倍（d <= 0 时返回 t 本身） | `t` (time) - 起始时间值；`d` (int) - 间隔毫秒数 | `time` - 舍入后的时间值 |
| `truncate` | `truncate(t: time, d: int) -> time` | 将时间 t 向零截断到 d 毫秒间隔的整数倍（d <= 0 时返回 t 本身） | `t` (time) - 起始时间值；`d` (int) - 间隔毫秒数 | `time` - 截断后的时间值 |

---

### 2.13 std.bytes —— 字节数组操作

源文件：`axc/std/bytes.ax`

| 函数名 | 签名 | 功能 | 输入参数 | 输出参数 |
|--------|------|------|----------|----------|
| `contains` | `contains(s: bytes, sub: bytes) -> bool` | 判断字节序列 s 是否包含子序列 sub | `s` (bytes) - 字节序列；`sub` (bytes) - 子序列 | `bool` - true 表示包含，false 表示不包含 |
| `equal` | `equal(a: bytes, b: bytes) -> bool` | 判断两个字节序列是否相等 | `a` (bytes) - 第一个字节序列；`b` (bytes) - 第二个字节序列 | `bool` - true 表示相等，false 表示不相等 |
| `index` | `index(s: bytes, sub: bytes) -> int` | 返回子序列 sub 在 s 中首次出现的索引 | `s` (bytes) - 字节序列；`sub` (bytes) - 子序列 | `int` - 首次出现的索引，未找到返回 -1 |
| `split` | `split(s: bytes, sep: bytes) -> bytes[]` | 按分隔符 sep 分割字节序列 s | `s` (bytes) - 字节序列；`sep` (bytes) - 分隔符 | `bytes[]` - 分割后的片段数组 |
| `trimSpace` | `trimSpace(s: bytes) -> bytes` | 去除字节序列 s 两端的空白字符 | `s` (bytes) - 待处理的字节序列 | `bytes` - 去除空白后的字节序列 |
| `toUpper` | `toUpper(s: bytes) -> bytes` | 将字节序列 s 转换为大写 | `s` (bytes) - 待转换的字节序列 | `bytes` - 大写后的字节序列 |
| `toLower` | `toLower(s: bytes) -> bytes` | 将字节序列 s 转换为小写 | `s` (bytes) - 待转换的字节序列 | `bytes` - 小写后的字节序列 |
| `repeat` | `repeat(s: bytes, count: int) -> bytes` | 将字节序列 s 重复 count 次 | `s` (bytes) - 字节序列；`count` (int) - 重复次数 | `bytes` - 重复后的字节序列 |

---

### 2.14 std.bufio —— 缓冲 I/O

源文件：`axc/std/bufio.ax`

| 函数名 | 签名 | 功能 | 输入参数 | 输出参数 |
|--------|------|------|----------|----------|
| `newReader` | `newReader(r: File) -> File` | 为文件 r 创建带缓冲的读取器 | `r` (File) - 要包装的文件句柄 | `File` - 带缓冲的读取器句柄 |
| `newWriter` | `newWriter(w: File) -> File` | 为文件 w 创建带缓冲的写入器 | `w` (File) - 要包装的文件句柄 | `File` - 带缓冲的写入器句柄 |
| `scanner` | `scanner(r: File) -> File` | 为文件 r 创建扫描器 | `r` (File) - 要扫描的文件句柄 | `File` - 扫描器句柄 |

---

### 2.15 std.csv —— CSV 读写

源文件：`axc/std/csv.ax`

| 函数名 | 签名 | 功能 | 输入参数 | 输出参数 |
|--------|------|------|----------|----------|
| `readAll` | `readAll(r: File) -> string[][]` | 读取 CSV 文件 r 的全部记录 | `r` (File) - CSV 文件句柄 | `string[][]` - 二维字符串数组，每行一个记录，每列一个字段 |
| `write` | `write(w: File, record: string[]) -> int` | 将一条记录写入 CSV 文件 w | `w` (File) - CSV 文件句柄；`record` (string[]) - 要写入的一条记录 | `int` - 0 表示成功，非 0 表示失败 |

---

### 2.16 std.filepath —— 路径操作

源文件：`axc/std/filepath.ax`

| 函数名 | 签名 | 功能 | 输入参数 | 输出参数 |
|--------|------|------|----------|----------|
| `rel` | `rel(basepath: string, targpath: string) -> string` | 返回从 basepath 到 targpath 的相对路径 | `basepath` (string) - 基准路径；`targpath` (string) - 目标路径 | `string` - 相对路径字符串 |
| `join` | `join(parts: string[]) -> string` | 将多个路径片段连接为单一路径 | `parts` (string[]) - 路径片段数组 | `string` - 连接后的路径字符串 |
| `dir` | `dir(path: string) -> string` | 返回路径的目录部分 | `path` (string) - 文件路径 | `string` - 目录部分字符串 |
| `base` | `base(path: string) -> string` | 返回路径的文件名部分 | `path` (string) - 文件路径 | `string` - 文件名部分字符串 |
| `ext` | `ext(path: string) -> string` | 返回路径的扩展名（含点） | `path` (string) - 文件路径 | `string` - 扩展名字符串（含点） |
| `split` | `split(path: string) -> string[]` | 将路径分割为目录和文件名两部分 | `path` (string) - 文件路径 | `string[]` - 分割后的字符串数组 |
| `clean` | `clean(path: string) -> string` | 规范化路径，删除冗余的斜杠和 .. 等 | `path` (string) - 待规范化的路径 | `string` - 规范化后的路径字符串 |
| `abs` | `abs(path: string) -> string` | 返回路径的绝对路径形式 | `path` (string) - 待转换的路径 | `string` - 绝对路径字符串 |
| `isAbs` | `isAbs(path: string) -> int` | 判断路径是否为绝对路径 | `path` (string) - 待判断的路径 | `int` - 1 表示绝对路径，0 表示相对路径 |
| `evalSymlinks` | `evalSymlinks(path: string) -> string` | 解析路径中的符号链接，返回实际路径 | `path` (string) - 待解析的路径 | `string` - 解析后的实际路径字符串 |
| `glob` | `glob(pattern: string) -> string[]` | 根据模式匹配文件 | `pattern` (string) - 文件匹配模式 | `string[]` - 匹配到的路径数组 |
| `walk` | `walk(root: string) -> string[]` | 递归遍历根目录 | `root` (string) - 根目录路径 | `string[]` - 所有文件路径数组 |

---

### 2.17 std.atomic —— 原子操作

源文件：`axc/std/atomic.ax`

> 注：对 int 变量取地址做原子操作，无需构造对象。

| 函数名 | 签名 | 功能 | 输入参数 | 输出参数 |
|--------|------|------|----------|----------|
| `addInt` | `addInt(addr: ref int, delta: int) -> int` | 原子地将 delta 加到 *addr，返回加法后的值 | `addr` (ref int) - 目标整数变量的引用；`delta` (int) - 增量 | `int` - 加法后的值 |
| `loadInt` | `loadInt(addr: ref int) -> int` | 原子地读取 *addr 的值 | `addr` (ref int) - 目标整数变量的引用 | `int` - *addr 的当前值 |
| `storeInt` | `storeInt(addr: ref int, val: int) -> void` | 原子地将 val 存入 *addr | `addr` (ref int) - 目标整数变量的引用；`val` (int) - 要存储的值 | 无 |
| `compareAndSwapInt` | `compareAndSwapInt(addr: ref int, old: int, replacement: int) -> bool` | 原子比较并交换，*addr 等于 old 时替换为 replacement | `addr` (ref int) - 目标整数变量的引用；`old` (int) - 期望的旧值；`replacement` (int) - 新值 | `bool` - true 表示交换成功，false 表示失败 |

---

## 三、与编译器内建模块的区分

| 类别 | 模块 | 实现方式 | 是否需 import | 示例 |
|------|------|----------|---------------|------|
| 编译器内建 | db/fmt/log/maps/reflect/sort/unique | Rust 编译器端硬编码，无 ax 源码 | 否（裸模块名直接使用） | `sort.ints(arr)`、`log.print(...)`、`fmt.printf(...)` |
| 标准库 | std.*（16 个模块） | ax 语法编写，位于 `axc/std/*.ax` | 是（`import std.xxx`） | `import std.io; io.writeLine(...)` |

**铁律**：编译器内建模块与标准库模块严格互斥，不得并存；BUILTIN_MODULES 已清空，不保留单段兼容别名。

---

## 四、已知问题与预存在错误

以下 ax 源码文件因编译器预存在问题，无法通过 `axc check` 独立编译（与本文档生成无关，不影响 `import std.xxx` 后的正常使用）：

| 文件 | 错误 | 说明 |
|------|------|------|
| `axc/std/errors.ax` | E3024 | 模块级函数被编译器误认为变量声明（预存在，非本次注释引入） |
| `axc/std/math.ax` | E3018 | `sqrt`/`pow`/`exp`/`log` 与 extern "C" 中的 C 库函数同名冲突（预存在） |
| `axc/std/rand.ax` | E3018 | `int`/`float` 是内建类型名，不能作为函数名（预存在） |

---
