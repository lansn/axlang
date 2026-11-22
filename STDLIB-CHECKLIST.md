# axlang 标准库清单（对标 C / Go）

本文档列出 axlang 内建模块与 C 标准库（stdio/stdlib/string/math/time/ctype/stdint/errno/signal/stdarg/setjmp/file）及 Go 标准库（fmt/os/strings/strconv/encoding/json/math/time/io/bufio/bytes/sort/path/filepath/log/errors/slices/maps/sync/unique）的对标关系，作为标准库落地进度的检视参考。目标：尽可能丰富，像 C/Go 标准库一样，最大限度满足开发需要。

本清单基于本机环境调研：Go 1.25.1 (windows/amd64, `go list std`)、GCC 16.2.0 (MinGW-w64)。Go 标准库函数签名以 `go doc <pkg>` 为准；C 标准库函数以 ISO C11 为准。

> **跨项目铁律**：所有涉及及生成的 C 代码 / GCC 源码，均必须使用标准 C（C11），不得使用任何 GNU/Clang 扩展（如 `__int128`、`__builtin_*_overflow`、`__attribute__((unused))` 等），以便跨平台编译。

---

## 状态图例

| 标记 | 含义 |
|------|------|
| `✅ 已落地` | 已在 `axrt.h`/`axrt.c` 实现、`sema`/`codegen` 绑定、测试用例齐备、回归通过 |
| `🔄 部分落地` | 核心函数已有，部分扩展函数待补 |
| `⬜ 未落地` | 尚未开始 |

---

## 优先级说明

| 优先级 | 含义 |
|--------|------|
| **P0 核心基础** | 程序能跑、能做基本 I/O、字符串、数学、文件操作；axlang 最小可用标准库 |
| **P1 常用扩展** | 日常开发常用功能，与已落地模块形成闭环 |
| **P2 进阶功能** | 并发、排序、编解码、更多数学、更多类型工具 |
| **P3 持续丰富** | 补充完善、边缘场景、对标更多 C/Go 函数 |

---

## 一、已落地模块（按模块逐函数清点）

### 1. io（对标 C stdio / Go io）

| 函数 | 签名 | 对标 | 状态 | 说明 |
|------|------|------|------|------|
| `io.ReadLine` | `() -> string?` | C `fgets` / Go `bufio.Reader.ReadString` | ✅ | 读取一行，EOF 时返回 null |
| `io.ReadFile` | `(path: string) -> result<string>` | C `fopen`+`fread` / Go `os.ReadFile` | ✅ | 读取整个文件 |
| `io.Write` | `(s: string) -> void` | C `fputs` / Go `fmt.Print` | ✅ | 写 stdout |
| `io.WriteLine` | `(s: string) -> void` | C `puts` / Go `fmt.Println` | ✅ | 写 stdout 并换行 |

### 2. math（对标 C math.h / Go math）

| 函数 | 签名 | 对标 | 状态 | 说明 |
|------|------|------|------|------|
| `math.Abs` | `(x: float) -> float` | C `fabs` / Go `math.Abs` | ✅ | 绝对值 |
| `math.Max` | `(a: float, b: float) -> float` | C `fmax` / Go `math.Max` | ✅ | 最大值 |
| `math.Min` | `(a: float, b: float) -> float` | C `fmin` / Go `math.Min` | ✅ | 最小值 |
| `math.Sqrt` | `(x: float) -> float` | C `sqrt` / Go `math.Sqrt` | ✅ | 平方根 |
| `math.Pow` | `(a: float, b: float) -> float` | C `pow` / Go `math.Pow` | ✅ | 幂 |
| `math.Exp` | `(x: float) -> float` | C `exp` / Go `math.Exp` | ✅ | 指数 |
| `math.Log` | `(x: float) -> float` | C `log` / Go `math.Log` | ✅ | 自然对数 |
| `math.Sin` | `(x: float) -> float` | C `sin` / Go `math.Sin` | ✅ | 正弦 |
| `math.Cos` | `(x: float) -> float` | C `cos` / Go `math.Cos` | ✅ | 余弦 |
| `math.Tan` | `(x: float) -> float` | C `tan` / Go `math.Tan` | ✅ | 正切 |
| `math.Ceil` | `(x: float) -> float` | C `ceil` / Go `math.Ceil` | ✅ | 向上取整 |
| `math.Floor` | `(x: float) -> float` | C `floor` / Go `math.Floor` | ✅ | 向下取整 |
| `math.Round` | `(x: float) -> float` | C `round` / Go `math.Round` | ✅ | 四舍五入 |
| `math.Sign` | `(x: float) -> int` | Go `math.Sign` | ✅ | 符号：-1/0/1 |
| `math.Mod` | `(x: float, y: float) -> float` | C `fmod` / Go `math.Mod` | ✅ | 取模 |
| `math.Equals` | `(a: float, b: float, eps: float) -> bool` | — | ✅ | `fabs(a-b) <= eps` |
| `math.Compare` | `(a: float, b: float) -> int` | — | ✅ | -1/0/1 |

### 3. json（对标 Go encoding/json）

| 函数 | 签名 | 对标 | 状态 | 说明 |
|------|------|------|------|------|
| `json.Parse` | `(s: string) -> result<map<string, object>>` | Go `json.Unmarshal` | ✅ | 解析 JSON |
| `json.Stringify` | `(v: object) -> string` | Go `json.Marshal` | ✅ | 序列化 JSON |

### 4. file（对标 C stdio/stdlib / Go os）

| 函数 | 签名 | 对标 | 状态 | 说明 |
|------|------|------|------|------|
| `file.Open` | `(path: string) -> result<File>` | C `fopen` / Go `os.Open` | ✅ | 模式 0=只读 |
| `file.Create` | `(path: string) -> result<File>` | C `fopen("wb")` / Go `os.Create` | ✅ | 模式 1=写/截断 |
| `file.Append` | `(path: string) -> result<File>` | C `fopen("ab")` / Go `os.OpenFile` 追加 | ✅ | 模式 2=追加 |
| `file.Remove` | `(path: string) -> int` | C `remove` / Go `os.Remove` | ✅ | 0=成功 1=失败 |
| `file.Rename` | `(old: string, new: string) -> int` | C `rename` / Go `os.Rename` | ✅ | 0=成功 1=失败 |
| `file.Exists` | `(path: string) -> int` | C `fopen` 试探 / Go `os.Stat` | ✅ | 1=存在 0=不存在 |
| `file.Mkdir` | `(path: string) -> int` | C `mkdir` / Go `os.Mkdir` | ✅ | 0=成功 1=失败 |
| `file.ReadAll` | `(path: string) -> result<string>` | Go `os.ReadFile` | ✅ | 便捷全文件读 |
| `file.WriteAll` | `(path: string, s: string) -> int` | — | ✅ | 便捷全文件写，0=成功 1=失败 |
| `file.IsDir` | `(path: string) -> int` | Go `os.Stat`+`IsDir` | ✅ | 1=目录 0=否或不存在 |
| `file.IsFile` | `(path: string) -> int` | Go `os.Stat`+`Mode().IsRegular()` | ✅ | 1=常规文件 0=否或不存在 |

### 5. db（axlang 特有，无直接 C/Go 对标）

| 函数 | 签名 | 状态 | 说明 |
|------|------|------|------|
| `db.Query` | `(sql: string) -> result<Row>` | ✅ | 数据库查询 |

### 6. string（对标 C string.h / Go strings）

**静态方法：**

| 函数 | 签名 | 对标 | 状态 | 说明 |
|------|------|------|------|------|
| `string.FromCodepoint` | `(cp: int) -> result<string>` | — | ✅ | 码点转字符串 |
| `string.Compare` | `(a: string, b: string) -> int` | Go `strings.Compare` | ✅ | 字典序比较 |
| `string.Fields` | `(s: string) -> string[]` | Go `strings.Fields` | ✅ | 按空白分割 |

**实例方法（共 20 个）：**

| 函数 | 签名 | 对标 | 状态 | 说明 |
|------|------|------|------|------|
| `string.ToUpper` | `() -> string` | Go `strings.ToUpper` / C `toupper` | ✅ | 大写转换 |
| `string.ToLower` | `() -> string` | Go `strings.ToLower` / C `tolower` | ✅ | 小写转换 |
| `string.Contains` | `(sub: string) -> bool` | Go `strings.Contains` | ✅ | 包含子串 |
| `string.IndexOf` | `(sub: string) -> int` | Go `strings.Index` | ✅ | 首次出现索引 |
| `string.LastIndexOf` | `(sub: string) -> int` | Go `strings.LastIndex` | ✅ | 最后出现索引 |
| `string.Split` | `(sep: string) -> string[]` | Go `strings.Split` | ✅ | 分割 |
| `string.Trim` | `() -> string` | Go `strings.TrimSpace` | ✅ | 去首尾空白 |
| `string.Replace` | `(old: string, new: string) -> string` | Go `strings.ReplaceAll` | ✅ | 替换所有 |
| `string.Repeat` | `(n: int) -> string` | Go `strings.Repeat` | ✅ | 重复 n 次 |
| `string.StartsWith` | `(prefix: string) -> bool` | Go `strings.HasPrefix` | ✅ | 前缀判断 |
| `string.EndsWith` | `(suffix: string) -> bool` | Go `strings.HasSuffix` | ✅ | 后缀判断 |
| `string.Len` | `() -> int` | Go `len(s)` / C `strlen` | ✅ | 字符串长度 |

### 7. time（对标 C time.h / Go time）

| 函数 | 类型 | 签名 | 对标 | 状态 | 说明 |
|------|------|------|------|------|------|
| `time.NowMs` | 模块 | `() -> int` | — | ✅ | 当前 Unix 毫秒 |
| `time.Now` | 模块 | `() -> time` | Go `time.Now` | ✅ | 当前时间 |
| `time.FromUnixMs` | 模块 | `(ms: int) -> time` | — | ✅ | 毫秒转 time |
| `time.Sleep` | 模块 | `(ms: int) -> void` | Go `time.Sleep` | ✅ | 毫秒睡眠 |
| `time.Parse` | 模块 | `(s: string, fmt: string) -> result<time>` | Go `time.Parse` | ✅ | 解析时间字符串 |
| `time.ToString` | 实例 | `() -> string` | — | ✅ | 格式化 `YYYY-MM-DD HH:MM:SS` |
| `time.Format` | 实例 | `(fmt: string) -> string` | C `strftime` | ✅ | strftime 风格格式化 |
| `time.Unix` | 模块 | `(sec: int, nsec: int) -> time` | Go `time.Unix` | ✅ | 秒+纳秒转 time |
| `time.UnixMilli` | 模块 | `(ms: int) -> time` | Go `time.UnixMilli` | ✅ | 毫秒转 time |
| `time.Since` | 模块 | `(t: time) -> int` | Go `time.Since` | ✅ | 与给定时间的差值毫秒 |

### 8. decimal（axlang 特有，高精度十进制）

| 函数 | 类型 | 签名 | 状态 | 说明 |
|------|------|------|------|------|
| `decimal.Parse` | 静态 | `(s: string) -> result<decimal>` | ✅ | 解析十进制 |
| `int8.Truncate` | 实例 | `(v: int) -> int8` | ✅ | 截断（溢出触发 R107） |
| `int16.Truncate` | 实例 | `(v: int) -> int16` | ✅ | |
| `int32.Truncate` | 实例 | `(v: int) -> int32` | ✅ | |

### 9. Int/Float/Bool/Enum（基础类型方法）

| 函数 | 类型 | 签名 | 状态 | 说明 |
|------|------|------|------|------|
| `ToString` | 各类型 | `() -> string` | ✅ | 转字符串 |
| `WrappingAdd` | 整型 | `(other: T) -> T` | ✅ | 绕环加法 |
| `Round` | 浮点 | `(digits: int) -> float` | ✅ | 四舍五入 |
| `Divide` | 数值型 | `(other: T) -> T` | ✅ | 除法 |

### 10. Array（数组）

| 函数 | 签名 | 状态 | 说明 |
|------|------|------|------|
| `Push` | `(elem: T) -> void` | ✅ | 尾部追加 |
| `Pop` | `() -> T?` | ✅ | 弹出末尾，空数组触发 R101 |
| `Remove` | `(index: int) -> void` | ✅ | 删除指定索引 |
| `Clear` | `() -> void` | ✅ | 清空 |
| `ToString` | `() -> string` | ✅ | 转字符串 |
| `==/!=` | `(other: T[]) -> bool` | ✅ | 逐元素比较 |
| 切片 | `[lo..hi]` / `[lo..=hi]` | ✅ | 动态浅拷贝 / 定长深拷贝 |

### 11. Map（映射）

| 函数 | 签名 | 状态 | 说明 |
|------|------|------|------|
| `Clear` | `() -> void` | ✅ | 清空 |
| `Remove` | `(key: K) -> void` | ✅ | 删除键 |
| `==/!=` | `(other: Map) -> bool` | ✅ | 比较 |

### 12. Range（范围迭代）

| 函数 | 签名 | 状态 | 说明 |
|------|------|------|------|
| `Step` | `(n: int) -> Range` | ✅ | 步进循环（防御性兜底） |

### 13. thread / queue（并发，对标 Go channel）

| 函数 | 类型 | 签名 | 状态 | 说明 |
|------|------|------|------|------|
| `thread.Push` | 队列 | `(v: T) -> void` | ✅ | 入队 |
| `thread.Pop` | 队列 | `() -> T?` | ✅ | 阻塞出队 |
| `thread.Close` | 队列 | `() -> void` | ✅ | 关闭队列 |
| `thread.Join` | 线程 | `() -> void` | ✅ | 等待线程结束 |
| `queue.TryPop` | 队列 | `() -> T?` | ✅ | 非阻塞出队，空队列返回 null |

### 14. os（对标 C stdlib / Go os）

| 函数 | 签名 | 对标 | 状态 | 说明 |
|------|------|------|------|------|
| `os.Args` | `() -> string[]` | Go `os.Args` | ✅ | 命令行参数（argv[1..]） |
| os.Chdir | `(path: string) -> int` | Go `os.Chdir` / C `chdir` | ✅ | 改变当前工作目录 |
| os.Environ | `() -> string[]` | Go `os.Environ` | ✅ | 环境变量列表 `"KEY=VALUE"` |
| os.Exit | `(code: int) -> void` | Go `os.Exit` / C `exit` | ✅ | 终止进程 |
| os.Getenv | `(name: string) -> string` | Go `os.Getenv` / C `getenv` | ✅ | 读环境变量，未设置返回空串 |
| os.Getpid | `() -> int` | Go `os.Getpid` / C `getpid` | ✅ | 当前进程 ID |
| os.Getppid | `() -> int` | Go `os.Getppid` / C `getppid` | ✅ | 父进程 ID（Windows 下返回 0） |
| os.Getwd | `() -> string` | Go `os.Getwd` / C `getcwd` | ✅ | 当前工作目录 |
| os.Hostname | `() -> string` | Go `os.Hostname` / C `gethostname` | ✅ | 主机名，失败返回空串 |
| os.Setenv | `(name: string, value: string) -> int` | Go `os.Setenv` / C `setenv` | ✅ | 设置环境变量，0=成功 1=失败 |
| os.Unsetenv | `(name: string) -> int` | Go `os.Unsetenv` / C `unsetenv` | ✅ | 删除环境变量，0=成功 1=失败 |

### 15. strconv（对标 Go strconv）

| 函数 | 签名 | 对标 | 状态 | 说明 |
|------|------|------|------|------|
| `strconv.Atoi` | `(s: string) -> int` | Go `strconv.Atoi` | ✅ | 字符串转 int（失败返回 0） |
| `strconv.Itoa` | `(v: int) -> string` | Go `strconv.Itoa` | ✅ | int 转字符串 |
| `strconv.ParseBool` | `(s: string) -> bool` | Go `strconv.ParseBool` | ✅ | 字符串转 bool（失败返回 false） |
| `strconv.FormatBool` | `(v: bool) -> string` | Go `strconv.FormatBool` | ✅ | bool 转字符串 |
| `strconv.ParseFloat` | `(s: string) -> float` | Go `strconv.ParseFloat` | ✅ | 字符串转 float（失败返回 0.0） |
| `strconv.FormatFloat` | `(v: float) -> string` | Go `strconv.FormatFloat` | ✅ | float 转字符串 |
| `strconv.ParseInt` | `(s: string, base: int) -> int` | Go `strconv.ParseInt` | ✅ | 字符串转 int（指定进制，失败返回 0） |
| `strconv.FormatInt` | `(v: int, base: int) -> string` | Go `strconv.FormatInt` | ✅ | int 转指定进制字符串 |
| `strconv.ParseUint` | `(s: string, base: int) -> int` | Go `strconv.ParseUint` | ✅ | 字符串转无符号 int（指定进制，失败返回 0） |
| `strconv.FormatUint` | `(v: int, base: int) -> string` | Go `strconv.FormatUint` | ✅ | 无符号 int 转指定进制字符串 |

### 16. filepath（对标 Go path/filepath）

| 函数 | 签名 | 对标 | 状态 | 说明 |
|------|------|------|------|------|
| `filepath.Join` | `(parts: string[]) -> string` | Go `filepath.Join` | ✅ | 用平台分隔符连接 |
| `filepath.Dir` | `(path: string) -> string` | Go `filepath.Dir` | ✅ | 返回目录部分 |
| `filepath.Base` | `(path: string) -> string` | Go `filepath.Base` | ✅ | 返回最后一段文件名 |
| `filepath.Ext` | `(path: string) -> string` | Go `filepath.Ext` | ✅ | 返回扩展名 |
| `filepath.Split` | `(path: string) -> string[2]` | Go `filepath.Split` | ✅ | 拆分为 (dir, file) |
| `filepath.Clean` | `(path: string) -> string` | Go `filepath.Clean` | ✅ | 路径规范化 |
| `filepath.Abs` | `(path: string) -> string` | Go `filepath.Abs` | ✅ | 转为绝对路径 |
| `filepath.IsAbs` | `(path: string) -> bool` | Go `filepath.IsAbs` | ✅ | 判断是否为绝对路径 |

### 17. rand（对标 Go math/rand）

| 函数 | 签名 | 对标 | 状态 | 说明 |
|------|------|------|------|------|
| `rand.Int` | `(n: int) -> int` | Go `rand.Intn` | ✅ | [0, n) 范围随机整数 |
| `rand.Float` | `() -> float` | Go `rand.Float64` | ✅ | [0, 1) 范围随机浮点数 |
| `rand.Seed` | `(seed: int) -> void` | Go `rand.Seed` | ✅ | 设置随机种子 |

### 18. sort（对标 Go sort）

| 函数 | 签名 | 对标 | 状态 | 说明 |
|------|------|------|------|------|
| `sort.Ints` | `(a: int[]) -> void` | Go `sort.Ints` | ✅ | 整数切片排序 |
| `sort.Strings` | `(a: string[]) -> void` | Go `sort.Strings` | ✅ | 字符串切片排序 |

---

## 二、待落地清单（P0 → P3）

本节按优先级列出尚未落地或部分落地的函数，每项均需按“落地规范”逐项完成。

### P0（核心基础，立即开始）

#### strconv 扩展

| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `strconv.CanBackquote` | `(s: string) -> bool` | Go `strconv.CanBackquote` | ✅ 已落地（2026-11-22） |
| `strconv.Quote` | `(s: string) -> string` | Go `strconv.Quote` | ✅ 已落地（2026-11-22） |
| `strconv.Unquote` | `(s: string) -> string` | Go `strconv.Unquote` | ✅ 已落地（2026-11-22） |

#### 字符分类与转换（对标 C ctype.h）

| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `string.IsDigit` | `() -> bool`（实例） | C `isdigit` | ✅ 已落地（2026-11-22） |
| `string.IsAlpha` | `() -> bool`（实例） | C `isalpha` | ✅ 已落地（2026-11-22） |
| `string.IsSpace` | `() -> bool`（实例） | C `isspace` | ✅ 已落地（2026-11-22） |
| `string.IsLower` | `() -> bool`（实例） | C `islower` | ✅ 已落地（2026-11-22） |
| `string.IsUpper` | `() -> bool`（实例） | C `isupper` | ✅ 已落地（2026-11-22） |
| `string.IsAlnum` | `() -> bool`（实例） | C `isalnum` | ✅ 已落地（2026-11-22） |
| `string.ToUpperRune` | `(r: int) -> int` | C `toupper` / Go `unicode.ToUpper` | ⬜ |
| `string.ToLowerRune` | `(r: int) -> int` | C `tolower` / Go `unicode.ToLower` | ⬜ |

### P1（常用扩展）

#### io 扩展（对标 C stdio / Go bufio）

| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `io.ReadAll` | `(path: string) -> result<string>` | Go `os.ReadFile` | 🔄（file.ReadAll 已有，io 层待补） |
| `io.WriteLine` 已落地 | — | — | ✅ |
| `io.ReadFile` 已落地 | — | — | ✅ |

#### file 扩展（对标 C stdio / Go os）

| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `file.Size` | `(path: string) -> int` | Go `os.Stat`+`Size()` | ⬜ |
| `file.ModTime` | `(path: string) -> int` | Go `os.Stat`+`ModTime()` | ⬜ |
| `file.Chmod` | `(path: string, mode: int) -> int` | Go `os.Chmod` / C `chmod` | ⬜ |
| `file.Chtimes` | `(path: string, atime: int, mtime: int) -> int` | Go `os.Chtimes` | ⬜ |
| `file.TempFile` | `(dir: string, prefix: string) -> result<File>` | Go `os.CreateTemp` | ⬜ |

#### os 扩展（对标 C stdlib / Go os）

| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `os.UserHomeDir` | `() -> string` | Go `os.UserHomeDir` | ⬜ |
| `os.UserCacheDir` | `() -> string` | Go `os.UserCacheDir` | ⬜ |
| `os.Getpagesize` | `() -> int` | Go `os.Getpagesize` | ⬜ |
| `os.Getuid` | `() -> int` | Go `os.Getuid` | ⬜ |
| `os.Getgid` | `() -> int` | Go `os.Getgid` | ⬜ |
| `os.Geteuid` | `() -> int` | Go `os.Geteuid` | ⬜ |
| `os.Getegid` | `() -> int` | Go `os.Getegid` | ⬜ |
| `os.Getgroups` | `() -> int[]` | Go `os.Getgroups` | ⬜ |
| `os.Executable` | `() -> string` | Go `os.Executable` | ⬜ |

#### string 扩展（对标 Go strings）

| 函数 | 类型 | 签名 | 对标 | 状态 |
|------|------|------|------|------|
| `string.ContainsAny` | 实例 | `(chars: string) -> bool` | Go `strings.ContainsAny` | ⬜ |
| `string.ContainsRune` | 实例 | `(r: int) -> bool` | Go `strings.ContainsRune` | ⬜ |
| `string.Count` | 实例 | `(substr: string) -> int` | Go `strings.Count` | ⬜ |
| `string.IndexAny` | 实例 | `(chars: string) -> int` | Go `strings.IndexAny` | ⬜ |
| `string.LastIndexAny` | 实例 | `(chars: string) -> int` | Go `strings.LastIndexAny` | ⬜ |
| `string.EqualFold` | 实例 | `(t: string) -> bool` | Go `strings.EqualFold` | ⬜ |
| `string.TrimLeft` | 实例 | `(cutset: string) -> string` | Go `strings.TrimLeft` | ⬜ |
| `string.TrimRight` | 实例 | `(cutset: string) -> string` | Go `strings.TrimRight` | ⬜ |
| `string.TrimPrefix` | 实例 | `(prefix: string) -> string` | Go `strings.TrimPrefix` | ⬜ |
| `string.TrimSuffix` | 实例 | `(suffix: string) -> string` | Go `strings.TrimSuffix` | ⬜ |
| `string.SplitN` | 实例 | `(sep: string, n: int) -> string[]` | Go `strings.SplitN` | ⬜ |
| `string.SplitAfter` | 实例 | `(sep: string) -> string[]` | Go `strings.SplitAfter` | ⬜ |
| `string.Title` | 实例 | `() -> string` | Go `strings.Title` | ⬜ |
| `string.Map` | 实例 | `(mapping: func(int) int) -> string` | Go `strings.Map` | ⬜ |

#### math 扩展（对标 C math.h / Go math）

| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `math.Exp2` | `(x: float) -> float` | Go `math.Exp2` | ⬜ |
| `math.Log10` | `(x: float) -> float` | C `log10` / Go `math.Log10` | ⬜ |
| `math.Log2` | `(x: float) -> float` | Go `math.Log2` | ⬜ |
| `math.Log1p` | `(x: float) -> float` | C `log1p` / Go `math.Log1p` | ⬜ |
| `math.Sin`/`math.Cos`/`math.Tan` 已落地 | — | — | ✅ |
| `math.Sinh` | `(x: float) -> float` | C `sinh` / Go `math.Sinh` | ⬜ |
| `math.Cosh` | `(x: float) -> float` | C `cosh` / Go `math.Cosh` | ⬜ |
| `math.Tanh` | `(x: float) -> float` | C `tanh` / Go `math.Tanh` | ⬜ |
| `math.Asin` | `(x: float) -> float` | C `asin` / Go `math.Asin` | ⬜ |
| `math.Acos` | `(x: float) -> float` | C `acos` / Go `math.Acos` | ⬜ |
| `math.Atan` | `(x: float) -> float` | C `atan` / Go `math.Atan` | ⬜ |
| `math.Atan2` | `(y: float, x: float) -> float` | C `atan2` / Go `math.Atan2` | ⬜ |
| `math.Hypot` | `(p: float, q: float) -> float` | C `hypot` / Go `math.Hypot` | ⬜ |
| `math.Cbrt` | `(x: float) -> float` | Go `math.Cbrt` | ⬜ |
| `math.Trunc` | `(x: float) -> float` | C `trunc` / Go `math.Trunc` | ⬜ |
| `math.Frexp` | `(f: float) -> (frac: float, exp: int)` | C `frexp` / Go `math.Frexp` | ⬜ |
| `math.Ldexp` | `(frac: float, exp: int) -> float` | C `ldexp` / Go `math.Ldexp` | ⬜ |
| `math.IsNaN` | `(f: float) -> bool` | C `isnan` / Go `math.IsNaN` | ⬜ |
| `math.IsInf` | `(f: float, sign: int) -> bool` | C `isinf` / Go `math.IsInf` | ⬜ |
| `math.Signbit` | `(f: float) -> bool` | Go `math.Signbit` | ⬜ |
| `math.Copysign` | `(f: float, sign: float) -> float` | C `copysign` / Go `math.Copysign` | ⬜ |
| `math.Dim` | `(x: float, y: float) -> float` | Go `math.Dim` | ⬜ |

#### time 扩展（对标 C time.h / Go time）

| 函数 | 类型 | 签名 | 对标 | 状态 |
|------|------|------|------|------|
| `time.Date` | 模块 | `(year: int, month: int, day: int, hour: int, min: int, sec: int, nsec: int) -> time` | Go `time.Date` | ⬜ |
| `time.UnixMicro` | 模块 | `(us: int) -> time` | Go `time.UnixMicro` | ⬜ |
| `time.UnixNano` | 模块 | `(nsec: int) -> time` | Go `time.UnixNano` | ⬜ |
| `time.Until` | 模块 | `(t: time) -> int` | Go `time.Until` | ⬜ |
| `time.After` | 实例 | `(u: time) -> bool` | Go `time.After` | ⬜ |
| `time.Before` | 实例 | `(u: time) -> bool` | Go `time.Before` | ⬜ |
| `time.Equal` | 实例 | `(u: time) -> bool` | Go `time.Equal` | ⬜ |
| `time.Compare` | 实例 | `(u: time) -> int` | Go `time.Compare` | ⬜ |
| `time.Sub` | 实例 | `(u: time) -> int` | Go `time.Sub` | ⬜ |
| `time.Add` | 实例 | `(d: int) -> time` | Go `time.Add` | ⬜ |
| `time.Round` | 实例 | `(d: int) -> time` | Go `time.Round` | ⬜ |
| `time.Truncate` | 实例 | `(d: int) -> time` | Go `time.Truncate` | ⬜ |

### P2（进阶功能）

#### fmt（对标 Go fmt / C stdio 格式化）

| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `fmt.Sprintf` | `(format: string, args: any[]) -> string` | Go `fmt.Sprintf` / C `snprintf` | 🔄（fmt 模块已有 Printf/Print/Println，Sprintf 待补） |
| `fmt.Sprintf` 已部分落地 | — | — | 🔄 |
| `fmt.Fprintf` | `(w: Writer, format: string, args: any[]) -> int` | Go `fmt.Fprintf` / C `fprintf` | ⬜ |
| `fmt.Errorf` | `(format: string, args: any[]) -> error` | Go `fmt.Errorf` | ⬜ |
| `fmt.Sprint` | `(args: any[]) -> string` | Go `fmt.Sprint` | ⬜ |
| `fmt.Scanf` | `(format: string, args: any[]) -> int` | Go `fmt.Scanf` / C `scanf` | ⬜ |

#### encoding/json 扩展（对标 Go encoding/json）

| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `json.MarshalIndent` | `(v: object, prefix: string, indent: string) -> string` | Go `json.MarshalIndent` | ⬜ |
| `json.Valid` | `(s: string) -> bool` | Go `json.Valid` | ⬜ |

#### encoding/csv（对标 Go encoding/csv）

| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `csv.ReadAll` | `(r: Reader) -> string[][]` | Go `csv.NewReader`+`ReadAll` | ⬜ |
| `csv.Write` | `(w: Writer, record: string[]) -> int` | Go `csv.Writer.Write` | ⬜ |

#### sort 扩展（对标 Go sort）

| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `sort.Float64s` | `(a: float[]) -> void` | Go `sort.Float64s` | ⬜ |
| `sort.IntsAreSorted` | `(a: int[]) -> bool` | Go `sort.IntsAreSorted` | ⬜ |
| `sort.StringsAreSorted` | `(a: string[]) -> bool` | Go `sort.StringsAreSorted` | ⬜ |
| `sort.SearchInts` | `(a: int[], x: int) -> int` | Go `sort.SearchInts` | ⬜ |
| `sort.SearchStrings` | `(a: string[], x: string) -> int` | Go `sort.SearchStrings` | ⬜ |
| `sort.Slice` | `(a: any[], less: func(i, j int) bool) -> void` | Go `sort.Slice` | ⬜ |
| `sort.SliceStable` | `(a: any[], less: func(i, j int) bool) -> void` | Go `sort.SliceStable` | ⬜ |

#### slices（对标 Go slices，Go 1.21+）

| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `slices.Contains` | `(a: T[], x: T) -> bool` | Go `slices.Contains` | ⬜ |
| `slices.Index` | `(a: T[], x: T) -> int` | Go `slices.Index` | ⬜ |
| `slices.Sort` | `(a: T[]) -> void` | Go `slices.Sort` | ⬜ |
| `slices.SortStable` | `(a: T[]) -> void` | Go `slices.SortStable` | ⬜ |
| `slices.Reverse` | `(a: T[]) -> void` | Go `slices.Reverse` | ⬜ |
| `slices.Clone` | `(a: T[]) -> T[]` | Go `slices.Clone` | ⬜ |
| `slices.Concat` | `(a: T[]...) -> T[]` | Go `slices.Concat` | ⬜ |
| `slices.Insert` | `(a: T[], index: int, x: T...) -> T[]` | Go `slices.Insert` | ⬜ |
| `slices.Delete` | `(a: T[], i: int, j: int) -> T[]` | Go `slices.Delete` | ⬜ |

#### maps（对标 Go maps，Go 1.21+）

| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `maps.Keys` | `(m: Map) -> K[]` | Go `maps.Keys` | ⬜ |
| `maps.Values` | `(m: Map) -> V[]` | Go `maps.Values` | ⬜ |
| `maps.Clone` | `(m: Map) -> Map` | Go `maps.Clone` | ⬜ |
| `maps.Copy` | `(dst: Map, src: Map) -> void` | Go `maps.Copy` | ⬜ |
| `maps.DeleteFunc` | `(m: Map, f: func(K) bool) -> void` | Go `maps.DeleteFunc` | ⬜ |
| `maps.Equal` | `(m1: Map, m2: Map) -> bool` | Go `maps.Equal` | ⬜ |

#### unique（对标 Go unique，Go 1.23+）

| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `unique.Make` | `(v: T) -> Handle[T]` | Go `unique.Make` | ⬜ |
| `unique.Value` | `(h: Handle[T]) -> T` | Go `unique.Value` | ⬜ |

#### bytes / bufio（对标 Go bytes / bufio）

| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `bytes.Contains` | `(s: bytes, sub: bytes) -> bool` | Go `bytes.Contains` | ⬜ |
| `bytes.Equal` | `(a: bytes, b: bytes) -> bool` | Go `bytes.Equal` | ⬜ |
| `bytes.Index` | `(s: bytes, sub: bytes) -> int` | Go `bytes.Index` | ⬜ |
| `bytes.Split` | `(s: bytes, sep: bytes) -> bytes[]` | Go `bytes.Split` | ⬜ |
| `bytes.TrimSpace` | `(s: bytes) -> bytes` | Go `bytes.TrimSpace` | ⬜ |
| `bytes.ToUpper` | `(s: bytes) -> bytes` | Go `bytes.ToUpper` | ⬜ |
| `bytes.ToLower` | `(s: bytes) -> bytes` | Go `bytes.ToLower` | ⬜ |
| `bytes.Repeat` | `(s: bytes, count: int) -> bytes` | Go `bytes.Repeat` | ⬜ |
| `bufio.NewReader` | `(r: Reader) -> Reader` | Go `bufio.NewReader` | ⬜ |
| `bufio.NewWriter` | `(w: Writer) -> Writer` | Go `bufio.NewWriter` | ⬜ |
| `bufio.Scanner` | `(r: Reader) -> Scanner` | Go `bufio.NewScanner` | ⬜ |

#### log / errors（对标 Go log / errors）

| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `log.Print` | `(v: any...) -> void` | Go `log.Print` | ⬜ |
| `log.Printf` | `(format: string, v: any...) -> void` | Go `log.Printf` | ⬜ |
| `log.Fatal` | `(v: any...) -> void` | Go `log.Fatal` | ⬜ |
| `errors.New` | `(msg: string) -> error` | Go `errors.New` | ⬜ |
| `errors.Is` | `(err: error, target: error) -> bool` | Go `errors.Is` | ⬜ |
| `errors.As` | `(err: error, target: any) -> bool` | Go `errors.As` | ⬜ |

#### sync / sync/atomic（对标 Go sync，并发原语）

| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `sync.Mutex.Lock` | `(m: Mutex) -> void` | Go `sync.Mutex.Lock` | ⬜ |
| `sync.Mutex.Unlock` | `(m: Mutex) -> void` | Go `sync.Mutex.Unlock` | ⬜ |
| `sync.RWMutex` | 类型 | — | ⬜ |
| `sync.WaitGroup` | 类型 | — | ⬜ |
| `sync.Once` | 类型 | — | ⬜ |
| `atomic.AddInt` | `(addr: *int, delta: int) -> int` | Go `atomic.AddInt64` | ⬜ |
| `atomic.LoadInt` | `(addr: *int) -> int` | Go `atomic.LoadInt64` | ⬜ |
| `atomic.StoreInt` | `(addr: *int, val: int) -> void` | Go `atomic.StoreInt64` | ⬜ |
| `atomic.CompareAndSwapInt` | `(addr: *int, old: int, new: int) -> bool` | Go `atomic.CompareAndSwapInt64` | ⬜ |

#### reflect（对标 Go reflect）

| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `reflect.TypeOf` | `(v: any) -> string` | Go `reflect.TypeOf` | ⬜ |
| `reflect.ValueOf` | `(v: any) -> any` | Go `reflect.ValueOf` | ⬜ |
| `reflect.NumField` | `(v: any) -> int` | Go `reflect.NumField` | ⬜ |

---

## 三、落地规范（每项必须完成）

1. **axrt.h**：新增 C 函数声明（标准 C11，无 GNU/Clang 扩展）
2. **axrt.c**：新增 C 函数实现
3. **rust/src/sema/mod.rs**：`BUILTIN_MODULES` 新增模块名（若新模块）
4. **rust/src/sema/expr.rs**：`module_sig` 新增签名
5. **rust/src/backend/codegen_expr.rs**：`module_call` 新增代码生成分支
6. **rust/src/backend/mod.rs**：如需新增 result 类型 C typedef（如 `result<time>` 走 `ax_res_time`）
7. **tests/run/*.ax + .out**：新增测试用例，覆盖正例和负例
8. **回归验证**：`python tests/runtests.py` 零新增回归
9. **提交推送**：Git commit + GitHub/Gitee 推送
10. **更新文档**：本清单状态标记，`MEMORY.md` 同步更新，追加工作日志

---

## 四、当前进度汇总

| 阶段 | 状态 | 说明 |
|------|------|------|
| 已落地模块 | io, math, json, file, db, string, time, decimal, Int/Float/Bool/Enum, Array, Map, Range, thread/queue, os, strconv, filepath, rand, sort | 共 18 个模块，函数数量已列于上文 |
| P0 待落地 | strconv.CanBackquote/Quote/Unquote；string.IsDigit/IsAlpha/IsSpace/IsLower/IsUpper/IsAlnum/ToUpperRune/ToLowerRune | 等待落地 |
| P1 待落地 | file.Size/ModTime/Chmod/Chtimes/TempFile；os.UserHomeDir/…；string.ContainsAny/Count/IndexAny/…；math.Exp2/Log10/…；time.Date/… | 等待落地 |
| P2 待落地 | fmt.Sprintf/Fprintf/…；json.MarshalIndent/Valid；csv.ReadAll/Write；sort.Float64s/…；slices.*；maps.*；unique.*；bytes/bufio；log/errors；sync/atomic；reflect | 等待落地 |
| P3 待落地 | 持续丰富，补充 C/Go 标准库边缘函数 | 等待落地 |

*更新于 2026-11-22*

---

## 五、建议落地顺序（P0 首）

1. `string.IsDigit` / `IsAlpha` / `IsSpace` / `IsLower` / `IsUpper` / `IsAlnum`（对标 C ctype.h，字符串基础能力）
2. `strconv.CanBackquote` / `Quote` / `Unquote`（对标 Go strconv，字符串引用处理）
3. `file.Size` / `file.ModTime`（对标 Go os.Stat，文件元信息）
4. `math.Log10` / `math.Exp2` / `math.IsNaN` / `math.IsInf`（对标 C math.h / Go math，数学能力补齐）
5. `time.Date` / `time.Until` / `time.After` / `time.Before`（对标 Go time，时间能力补齐）
6. `slices.Contains` / `slices.Index` / `slices.Sort`（对标 Go slices，泛型切片工具）

每项完成后同步更新本清单、`MEMORY.md`、工作日志，并 commit/push 到 GitHub/Gitee。
