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

### 1. io（对标 Go io / C stdio）

| ax.函数          | 签名                                 | 对标                                       | 函数功能/用途           | 状态 | 说明        |
| -------------- | ---------------------------------- | ---------------------------------------- | ----------------- | -- | --------- |
| `io.ReadLine`  | `() -> string?`                    | Go `bufio.Reader.ReadString` / C `fgets` | 读取一行，EOF 时返回 null | ✅  | 标准 C11 实现 |
| `io.ReadFile`  | `(path: string) -> result<string>` | Go `os.ReadFile` / C `fopen`+`fread`     | 读取整个文件            | ✅  | 标准 C11 实现 |
| `io.Write`     | `(s: string) -> void`              | Go `fmt.Print` / C `fputs`               | 写 stdout          | ✅  | 标准 C11 实现 |
| `io.WriteLine` | `(s: string) -> void`              | Go `fmt.Println` / C `puts`              | 写 stdout 并换行      | ✅  | 标准 C11 实现 |

### 2. fmt（对标 Go fmt / C stdio 格式化）

| ax.函数         | 签名                                                 | 对标                              | 函数功能/用途                                             | 状态 | 说明        |
| ------------- | -------------------------------------------------- | ------------------------------- | --------------------------------------------------- | -- | --------- |
| `fmt.Print`   | `(args: any...) -> void`                           | Go `fmt.Print`                  | 已通过 `print/println` 部分实现，fmt 模块函数待补                 | 🔄 | 部分落地      |
| `fmt.Println` | `(args: any...) -> void`                           | Go `fmt.Println`                | 同上                                                  | 🔄 | 部分落地      |
| `fmt.Printf`  | `(format: string, args: any...) -> void`           | Go `fmt.Printf` / C `printf`    | fmt 模块已有 Printf/Print/Println，编译期核对占位符              | 🔄 | 部分落地      |
| `fmt.Sprintf` | `(format: string, args: any...) -> string`         | Go `fmt.Sprintf` / C `snprintf` | 按格式字符串生成字符串，对标 Go fmt.Sprintf / C snprintf；编译期核对占位符 | ✅  | 标准 C11 实现 |
| `fmt.Sprint`  | `(args: any...) -> string`                         | Go `fmt.Sprint`                 | 将任意参数拼接为字符串，空格分隔，对标 Go fmt.Sprint                   | ✅  | 标准 C11 实现 |
| `fmt.Fprintf` | `(w: File, format: string, args: any...) -> int` | Go `fmt.Fprintf` / C `fprintf`  | 按格式字符串写入指定 File，对标 Go fmt.Fprintf / C fprintf；编译期核对占位符 | ✅  | 标准 C11 实现 |
| `fmt.Errorf`  | `(format: string, args: any...) -> error`          | Go `fmt.Errorf`                 | 按格式生成错误值，对标 Go fmt.Errorf；编译期核对占位符，返回 ax_err | ✅  | 标准 C11 实现 |
| `fmt.Scanf`   | `(format: string, args: any...) -> int`            | Go `fmt.Scanf` / C `scanf`      | 从 stdin 按格式读取，返回读取的项数；支持 %d/%f/%s，实参须为 int/float/string 变量 | ✅  | 标准 C11 实现 |

### 3. math（对标 Go math / C math.h）

| ax.函数          | 签名                                         | 对标                          | 函数功能/用途                                         | 状态 | 说明        |
| -------------- | ------------------------------------------ | --------------------------- | ----------------------------------------------- | -- | --------- |
| `math.Abs`     | `(x: float) -> float`                      | Go `math.Abs` / C `fabs`    | 绝对值                                             | ✅  | 标准 C11 实现 |
| `math.Max`     | `(a: float, b: float) -> float`            | Go `math.Max` / C `fmax`    | 最大值                                             | ✅  | 标准 C11 实现 |
| `math.Min`     | `(a: float, b: float) -> float`            | Go `math.Min` / C `fmin`    | 最小值                                             | ✅  | 标准 C11 实现 |
| `math.Sqrt`    | `(x: float) -> float`                      | Go `math.Sqrt` / C `sqrt`   | 平方根                                             | ✅  | 标准 C11 实现 |
| `math.Pow`     | `(a: float, b: float) -> float`            | Go `math.Pow` / C `pow`     | 幂                                               | ✅  | 标准 C11 实现 |
| `math.Exp`     | `(x: float) -> float`                      | Go `math.Exp` / C `exp`     | 指数                                              | ✅  | 标准 C11 实现 |
| `math.Log`     | `(x: float) -> float`                      | Go `math.Log` / C `log`     | 自然对数                                            | ✅  | 标准 C11 实现 |
| `math.Sin`     | `(x: float) -> float`                      | Go `math.Sin` / C `sin`     | 正弦                                              | ✅  | 标准 C11 实现 |
| `math.Cos`     | `(x: float) -> float`                      | Go `math.Cos` / C `cos`     | 余弦                                              | ✅  | 标准 C11 实现 |
| `math.Tan`     | `(x: float) -> float`                      | Go `math.Tan` / C `tan`     | 正切                                              | ✅  | 标准 C11 实现 |
| `math.Ceil`    | `(x: float) -> float`                      | Go `math.Ceil` / C `ceil`   | 向上取整                                            | ✅  | 标准 C11 实现 |
| `math.Floor`   | `(x: float) -> float`                      | Go `math.Floor` / C `floor` | 向下取整                                            | ✅  | 标准 C11 实现 |
| `math.Round`   | `(x: float) -> float`                      | Go `math.Round` / C `round` | 四舍五入                                            | ✅  | 标准 C11 实现 |
| `math.Sign`    | `(x: float) -> int`                        | Go `math.Sign`              | 符号：-1/0/1                                       | ✅  | 标准 C11 实现 |
| `math.Mod`     | `(x: float, y: float) -> float`            | Go `math.Mod` / C `fmod`    | 取模                                              | ✅  | 标准 C11 实现 |
| `math.Equals`  | `(a: float, b: float, eps: float) -> bool` | —                           | fabs(a-b) <= eps                                | ✅  | 标准 C11 实现 |
| `math.Compare` | `(a: float, b: float) -> int`              | —                           | -1/0/1                                          | ✅  | 标准 C11 实现 |
| `math.Exp2`    | `(x: float) -> float`                      | Go `math.Exp2` / C `exp2`   | C11 exp2，对标 Go math.Exp2                        | ✅  | 标准 C11 实现 |
| `math.Log10`   | `(x: float) -> float`                      | Go `math.Log10` / C `log10` | C11 log10，对标 Go math.Log10                      | ✅  | 标准 C11 实现 |
| `math.Log2`    | `(x: float) -> float`                      | Go `math.Log2` / C `log2`   | 二进制对数，对标 Go math.Log2 / C log2；标准 C11 直接映射      | ✅  | 标准 C11 实现 |
| `math.Log1p`   | `(x: float) -> float`                      | Go `math.Log1p` / C `log1p` | log(1+x)，对标 Go math.Log1p / C log1p；标准 C11 直接映射 | ✅  | 标准 C11 实现 |
| `math.Sinh`    | `(x: float) -> float`                      | Go `math.Sinh` / C `sinh`   | 双曲正弦，对标 Go math.Sinh / C sinh；标准 C11 直接映射 | ✅ | 标准 C11 实现 |
| `math.Cosh`    | `(x: float) -> float`                      | Go `math.Cosh` / C `cosh`   | 双曲余弦，对标 Go math.Cosh / C cosh；标准 C11 直接映射 | ✅ | 标准 C11 实现 |
| `math.Tanh`    | `(x: float) -> float`                      | Go `math.Tanh` / C `tanh`   | 双曲正切，对标 Go math.Tanh / C tanh；标准 C11 直接映射 | ✅ | 标准 C11 实现 |
| `math.Asin`     | `(x: float) -> float`                   | Go `math.Asin` / C `asin`         | 反正弦，对标 Go math.Asin / C asin；标准 C11 直接映射 | ✅ | 标准 C11 实现 |
| `math.Acos`     | `(x: float) -> float`                   | Go `math.Acos` / C `acos`         | 反余弦，对标 Go math.Acos / C acos；标准 C11 直接映射 | ✅ | 标准 C11 实现 |
| `math.Atan`     | `(x: float) -> float`                   | Go `math.Atan` / C `atan`         | 反正切，对标 Go math.Atan / C atan；标准 C11 直接映射 | ✅ | 标准 C11 实现 |
| `math.Atan2`    | `(y: float, x: float) -> float`         | Go `math.Atan2` / C `atan2`       | 反正切（带符号商），对标 Go math.Atan2 / C atan2；标准 C11 直接映射 | ✅ | 标准 C11 实现 |
| `math.Hypot`    | `(p: float, q: float) -> float`         | Go `math.Hypot` / C `hypot`       | 平方和开方，对标 Go math.Hypot / C hypot；标准 C11 直接映射 | ✅ | 标准 C11 实现 |
| `math.Cbrt`     | `(x: float) -> float`                   | Go `math.Cbrt`                    | 立方根，对标 Go math.Cbrt；标准 C11 直接映射            | ✅ | 标准 C11 实现 |
| `math.Trunc`    | `(x: float) -> float`                   | Go `math.Trunc` / C `trunc`       | 向零取整，对标 Go math.Trunc / C trunc；标准 C11 直接映射 | ✅ | 标准 C11 实现 |
| `math.Frexp`    | `(f: float) -> (frac: float, exp: int)` | Go `math.Frexp` / C `frexp`       | 未落地                        | ⬜  | 计划中       |
| `math.Ldexp`    | `(frac: float, exp: int) -> float`      | Go `math.Ldexp` / C `ldexp`       | 未落地                        | ⬜  | 计划中       |
| `math.IsNaN`    | `(f: float) -> bool`                    | Go `math.IsNaN` / C `isnan`       | C11 isnan，对标 Go math.IsNaN | ✅  | 标准 C11 实现 |
| `math.IsInf`    | `(f: float, sign: int) -> bool`         | Go `math.IsInf` / C `isinf`       | C11 isinf，对标 Go math.IsInf | ✅  | 标准 C11 实现 |
| `math.Signbit`  | `(f: float) -> bool`                    | Go `math.Signbit`                 | 未落地                        | ⬜  | 计划中       |
| `math.Copysign` | `(f: float, sign: float) -> float`      | Go `math.Copysign` / C `copysign` | 未落地                        | ⬜  | 计划中       |
| `math.Dim`      | `(x: float, y: float) -> float`         | Go `math.Dim`                     | 未落地                        | ⬜  | 计划中       |

### 4. json（对标 Go encoding/json）

| ax.函数                | 签名                                                      | 对标                      | 函数功能/用途  | 状态 | 说明        |
| -------------------- | ------------------------------------------------------- | ----------------------- | -------- | -- | --------- |
| `json.Parse`         | `(s: string) -> result<map<string, object>>`            | Go `json.Unmarshal`     | 解析 JSON  | ✅  | 标准 C11 实现 |
| `json.Stringify`     | `(v: object) -> string`                                 | Go `json.Marshal`       | 序列化 JSON | ✅  | 标准 C11 实现 |
| `json.MarshalIndent` | `(v: object, prefix: string, indent: string) -> string` | Go `json.MarshalIndent` | 带缩进序列化 JSON，对标 Go json.MarshalIndent；编译期核对实参类型 | ✅  | 标准 C11 实现 |
| `json.Valid`         | `(s: string) -> bool`                                   | Go `json.Valid`         | 判断 JSON 是否合法，对标 Go json.Valid；编译期核对实参类型 | ✅  | 标准 C11 实现 |

### 5. file（对标 Go os / C stdio/stdlib）

| ax.函数           | 签名                                              | 对标                                    | 函数功能/用途                | 状态 | 说明        |
| --------------- | ----------------------------------------------- | ------------------------------------- | ---------------------- | -- | --------- |
| `file.Open`     | `(path: string) -> result<File>`                | Go `os.Open` / C `fopen`              | 模式 0=只读                | ✅  | 标准 C11 实现 |
| `file.Create`   | `(path: string) -> result<File>`                | Go `os.Create` / C `fopen("wb")`      | 模式 1=写/截断              | ✅  | 标准 C11 实现 |
| `file.Append`   | `(path: string) -> result<File>`                | Go `os.OpenFile` 追加 / C `fopen("ab")` | 模式 2=追加                | ✅  | 标准 C11 实现 |
| `file.Remove`   | `(path: string) -> int`                         | Go `os.Remove` / C `remove`           | 0=成功 1=失败              | ✅  | 标准 C11 实现 |
| `file.Rename`   | `(old: string, new: string) -> int`             | Go `os.Rename` / C `rename`           | 0=成功 1=失败              | ✅  | 标准 C11 实现 |
| `file.Exists`   | `(path: string) -> int`                         | Go `os.Stat` / C `fopen` 试探           | 1=存在 0=不存在             | ✅  | 标准 C11 实现 |
| `file.Mkdir`    | `(path: string) -> int`                         | Go `os.Mkdir` / C `mkdir`             | 0=成功 1=失败              | ✅  | 标准 C11 实现 |
| `file.ReadAll`  | `(path: string) -> result<string>`              | Go `os.ReadFile`                      | 便捷全文件读                 | ✅  | 标准 C11 实现 |
| `file.WriteAll` | `(path: string, s: string) -> int`              | —                                     | 便捷全文件写，0=成功 1=失败       | ✅  | 标准 C11 实现 |
| `file.IsDir`    | `(path: string) -> int`                         | Go `os.Stat`+`IsDir`                  | 1=目录 0=否或不存在           | ✅  | 标准 C11 实现 |
| `file.IsFile`   | `(path: string) -> int`                         | Go `os.Stat`+`Mode().IsRegular()`     | 1=常规文件 0=否或不存在         | ✅  | 标准 C11 实现 |
| `file.Size`     | `(path: string) -> int`                         | Go `os.Stat`+`Size()`                 | 字节数，失败返回 -1            | ✅  | 标准 C11 实现 |
| `file.ModTime`  | `(path: string) -> int`                         | Go `os.Stat`+`ModTime()`              | Unix 毫秒，失败返回 -1        | ✅  | 标准 C11 实现 |
| `file.Chmod`    | `(path: string, mode: int) -> int`              | Go `os.Chmod` / C `chmod`             | 修改文件权限，0=成功 1=失败       | ✅  | 标准 C11 实现 |
| `file.Chtimes`  | `(path: string, atime: int, mtime: int) -> int` | Go `os.Chtimes`                       | 修改访问/修改时间，0=成功 1=失败    | ✅  | 标准 C11 实现 |
| `file.TempFile` | `(dir: string, prefix: string) -> result<File>` | Go `os.CreateTemp`                    | 创建临时文件，返回 result<File> | ✅  | 标准 C11 实现 |

### 6. db（axlang 特有，无直接 Go 对标）

| ax.函数      | 签名                             | 对标 | 函数功能/用途 | 状态 | 说明        |
| ---------- | ------------------------------ | -- | ------- | -- | --------- |
| `db.Query` | `(sql: string) -> result<Row>` | —  | 数据库查询   | ✅  | 标准 C11 实现 |

### 7. string（对标 Go strings / C string.h / C ctype.h）

**静态方法：**

| ax.函数                  | 签名                              | 对标                   | 函数功能/用途 | 状态 | 说明        |
| ---------------------- | ------------------------------- | -------------------- | ------- | -- | --------- |
| `string.FromCodepoint` | `(cp: int) -> result<string>`   | —                    | 码点转字符串  | ✅  | 标准 C11 实现 |
| `string.Compare`       | `(a: string, b: string) -> int` | Go `strings.Compare` | 字典序比较   | ✅  | 标准 C11 实现 |
| `string.Fields`        | `(s: string) -> string[]`       | Go `strings.Fields`  | 按空白分割   | ✅  | 标准 C11 实现 |

**实例方法（已落地 26 个）：**

| ax.函数                 | 签名                                     | 对标                                 | 函数功能/用途     | 状态 | 说明        |
| --------------------- | -------------------------------------- | ---------------------------------- | ----------- | -- | --------- |
| `string.ToUpper`      | `() -> string`                         | Go `strings.ToUpper` / C `toupper` | 大写转换        | ✅  | 标准 C11 实现 |
| `string.ToLower`      | `() -> string`                         | Go `strings.ToLower` / C `tolower` | 小写转换        | ✅  | 标准 C11 实现 |
| `string.Contains`     | `(sub: string) -> bool`                | Go `strings.Contains`              | 包含子串        | ✅  | 标准 C11 实现 |
| `string.IndexOf`      | `(sub: string) -> int`                 | Go `strings.Index`                 | 首次出现索引      | ✅  | 标准 C11 实现 |
| `string.LastIndexOf`  | `(sub: string) -> int`                 | Go `strings.LastIndex`             | 最后出现索引      | ✅  | 标准 C11 实现 |
| `string.Split`        | `(sep: string) -> string[]`            | Go `strings.Split`                 | 分割          | ✅  | 标准 C11 实现 |
| `string.Trim`         | `() -> string`                         | Go `strings.TrimSpace`             | 去首尾空白       | ✅  | 标准 C11 实现 |
| `string.Replace`      | `(old: string, new: string) -> string` | Go `strings.ReplaceAll`            | 替换所有        | ✅  | 标准 C11 实现 |
| `string.Repeat`       | `(n: int) -> string`                   | Go `strings.Repeat`                | 重复 n 次      | ✅  | 标准 C11 实现 |
| `string.StartsWith`   | `(prefix: string) -> bool`             | Go `strings.HasPrefix`             | 前缀判断        | ✅  | 标准 C11 实现 |
| `string.EndsWith`     | `(suffix: string) -> bool`             | Go `strings.HasSuffix`             | 后缀判断        | ✅  | 标准 C11 实现 |
| `string.Len`          | `() -> int`                            | Go `len(s)` / C `strlen`           | 字符串长度       | ✅  | 标准 C11 实现 |
| `string.IsDigit`      | `() -> bool`                           | C `isdigit`                        | 所有字符均为数字    | ✅  | 标准 C11 实现 |
| `string.IsAlpha`      | `() -> bool`                           | C `isalpha`                        | 所有字符均为字母    | ✅  | 标准 C11 实现 |
| `string.IsSpace`      | `() -> bool`                           | C `isspace`                        | 所有字符均为空白    | ✅  | 标准 C11 实现 |
| `string.IsLower`      | `() -> bool`                           | C `islower`                        | 所有字符均为小写    | ✅  | 标准 C11 实现 |
| `string.IsUpper`      | `() -> bool`                           | C `isupper`                        | 所有字符均为大写    | ✅  | 标准 C11 实现 |
| `string.IsAlnum`      | `() -> bool`                           | C `isalnum`                        | 所有字符均为字母或数字 | ✅  | 标准 C11 实现 |
| `string.ContainsAny`  | `(chars: string) -> bool`              | Go `strings.ContainsAny`           | 未落地         | ⬜  | 计划中       |
| `string.ContainsRune` | `(r: int) -> bool`                     | Go `strings.ContainsRune`          | 未落地         | ⬜  | 计划中       |
| `string.Count`        | `(substr: string) -> int`              | Go `strings.Count`                 | 未落地         | ⬜  | 计划中       |
| `string.IndexAny`     | `(chars: string) -> int`               | Go `strings.IndexAny`              | 未落地         | ⬜  | 计划中       |
| `string.LastIndexAny` | `(chars: string) -> int`               | Go `strings.LastIndexAny`          | 最后出现任意字符的码点索引，对标 Go strings.LastIndexAny；标准 C11 直接映射 | ✅ | 标准 C11 实现 |
| `string.EqualFold`    | `(t: string) -> bool`                  | Go `strings.EqualFold`             | 忽略大小写比较，对标 Go strings.EqualFold；标准 C11 直接映射 | ✅ | 标准 C11 实现 |
| `string.TrimLeft` | `(cutset: string) -> string` | Go `strings.TrimLeft` | 去除左侧字符集，对标 Go strings.TrimLeft；标准 C11 直接映射 | ✅ | 标准 C11 实现 |
| `string.TrimRight` | `(cutset: string) -> string` | Go `strings.TrimRight` | 去除右侧字符集，对标 Go strings.TrimRight；标准 C11 直接映射 | ✅ | 标准 C11 实现 |
| `string.TrimPrefix` | `(prefix: string) -> string` | Go `strings.TrimPrefix` | 未落地 | ⬜ | 计划中 |
| `string.TrimSuffix` | `(suffix: string) -> string` | Go `strings.TrimSuffix` | 未落地 | ⬜ | 计划中 |
| `string.SplitN` | `(sep: string, n: int) -> string[]` | Go `strings.SplitN` | 未落地 | ⬜ | 计划中 |
| `string.SplitAfter` | `(sep: string) -> string[]` | Go `strings.SplitAfter` | 未落地 | ⬜ | 计划中 |
| `string.Title` | `() -> string` | Go `strings.Title` | 未落地 | ⬜ | 计划中 |
| `string.Map` | `(mapping: func(int) int) -> string` | Go `strings.Map` | 未落地 | ⬜ | 计划中 |
| `string.ToUpperRune` | `(r: int) -> int` | C `toupper` / Go `unicode.ToUpper` | 未落地 | ⬜ | 计划中 |
| `string.ToLowerRune` | `(r: int) -> int` | C `tolower` / Go `unicode.ToLower` | 未落地 | ⬜ | 计划中 |


### 8. time（对标 Go time / C time.h）



| ax.函数 | 类型 | 签名 | 对标 | 函数功能/用途 | 状态 | 说明 |
| ------ | ------ | ------ | ------ | ------ | ------ | ------ |
| `time.NowMs` | 模块 | `() -> int` | — | 当前 Unix 毫秒 | ✅ | 标准 C11 实现 |
| `time.Now` | 模块 | `() -> time` | Go `time.Now` | 当前时间 | ✅ | 标准 C11 实现 |
| `time.FromUnixMs` | 模块 | `(ms: int) -> time` | — | 毫秒转 time | ✅ | 标准 C11 实现 |
| `time.Sleep` | 模块 | `(ms: int) -> void` | Go `time.Sleep` | 毫秒睡眠 | ✅ | 标准 C11 实现 |
| `time.Parse` | 模块 | `(s: string, fmt: string) -> result<time>` | Go `time.Parse` | 解析时间字符串 | ✅ | 标准 C11 实现 |
| `time.ToString` | 实例 | `() -> string` | — | 格式化 `YYYY-MM-DD HH:MM:SS` | ✅ | 标准 C11 实现 |
| `time.Format` | 实例 | `(fmt: string) -> string` | C `strftime` | strftime 风格格式化 | ✅ | 标准 C11 实现 |
| `time.Unix` | 模块 | `(sec: int, nsec: int) -> time` | Go `time.Unix` | 秒+纳秒转 time | ✅ | 标准 C11 实现 |
| `time.UnixMilli` | 模块 | `(ms: int) -> time` | Go `time.UnixMilli` | 毫秒转 time | ✅ | 标准 C11 实现 |
| `time.Since` | 模块 | `(t: time) -> int` | Go `time.Since` | 与给定时间的差值毫秒 | ✅ | 标准 C11 实现 |
| `time.Date` | 模块 | `(year, month, day, hour, min, sec, nsec: int) -> time` | Go `time.Date` / C 手工日历计算 | ax_time_date，UTC 组装，标准 C11 | ✅ | 标准 C11 实现 |
| `time.UnixMicro` | 模块 | `(us: int) -> time` | Go `time.UnixMicro` | 未落地 | ⬜ | 计划中 |
| `time.UnixNano` | 模块 | `(nsec: int) -> time` | Go `time.UnixNano` | 未落地 | ⬜ | 计划中 |
| `time.Until` | 模块 | `(t: time) -> int` | Go `time.Until` / C `t - now` | ax_time_until，t - 当前毫秒 | ✅ | 标准 C11 实现 |
| `time.After` | 实例 | `(u: time) -> bool` | Go `time.After` / C `t > u` | 实例方法，时间比较 | ✅ | 标准 C11 实现 |
| `time.Before` | 实例 | `(u: time) -> bool` | Go `time.Before` / C `t < u` | 实例方法，时间比较 | ✅ | 标准 C11 实现 |
| `time.Equal` | 实例 | `(u: time) -> bool` | Go `time.Equal` | 未落地 | ⬜ | 计划中 |
| `time.Compare` | 实例 | `(u: time) -> int` | Go `time.Compare` | 未落地 | ⬜ | 计划中 |
| `time.Sub` | 实例 | `(u: time) -> int` | Go `time.Sub` | 未落地 | ⬜ | 计划中 |
| `time.Add` | 实例 | `(d: int) -> time` | Go `time.Add` | 未落地 | ⬜ | 计划中 |
| `time.Round` | 实例 | `(d: int) -> time` | Go `time.Round` | 未落地 | ⬜ | 计划中 |
| `time.Truncate` | 实例 | `(d: int) -> time` | Go `time.Truncate` | 未落地 | ⬜ | 计划中 |


### 9. decimal（axlang 特有，高精度十进制）



| ax.函数 | 类型 | 签名 | 函数功能/用途 | 状态 | 说明 |
| ------ | ------ | ------ | ------ | ------ | ------ |
| `decimal.Parse` | 静态 | `(s: string) -> result<decimal>` | 解析十进制 | ✅ | 标准 C11 实现 |
| `int8.Truncate` | 实例 | `(v: int) -> int8` | 截断 | ✅ | 标准 C11 实现 |
| `int16.Truncate` | 实例 | `(v: int) -> int16` | 截断为 int16，对标 C 类型截断 | ✅ | 标准 C11 实现 |
| `int32.Truncate` | 实例 | `(v: int) -> int32` | 截断为 int32，对标 C 类型截断 | ✅ | 标准 C11 实现 |


### 10. Int/Float/Bool/Enum（基础类型方法）



| ax.函数 | 类型 | 签名 | 函数功能/用途 | 状态 | 说明 |
| ------ | ------ | ------ | ------ | ------ | ------ |
| `ToString` | 各类型 | `() -> string` | 转字符串 | ✅ | 标准 C11 实现 |
| `WrappingAdd` | 整型 | `(other: T) -> T` | 绕环加法 | ✅ | 标准 C11 实现 |
| `Round` | 浮点 | `(digits: int) -> float` | 四舍五入 | ✅ | 标准 C11 实现 |
| `Divide` | 数值型 | `(other: T) -> T` | 除法 | ✅ | 标准 C11 实现 |


### 11. Array（数组）



| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 | 说明 |
| ------ | ------ | ------ | ------ | ------ | ------ |
| `Push` | `(elem: T) -> void` | — | 尾部追加 | ✅ | 标准 C11 实现 |
| `Pop` | `() -> T?` | — | 弹出末尾，空数组触发 R101 | ✅ | 标准 C11 实现 |
| `Remove` | `(index: int) -> void` | — | 删除指定索引 | ✅ | 标准 C11 实现 |
| `Clear` | `() -> void` | — | 清空 | ✅ | 标准 C11 实现 |
| `ToString` | `() -> string` | — | 转字符串 | ✅ | 标准 C11 实现 |
| `==/!=` | `(other: T[]) -> bool` | — | 逐元素比较 | ✅ | 标准 C11 实现 |
| 切片 | `[lo..hi]` / `[lo..=hi]` | — | 切片 | ✅ | 动态浅拷贝 / 定长深拷贝 |



### 12. Map（映射）



| ax.函数 | 签名 | 函数功能/用途 | 状态 | 说明 |
| ------ | ------ | ------ | ------ | ------ |
| `Clear` | `() -> void` | 清空 | ✅ | 标准 C11 实现 |
| `Remove` | `(key: K) -> void` | 删除键 | ✅ | 标准 C11 实现 |
| `==/!=` | `(other: Map) -> bool` | 比较 | ✅ | 标准 C11 实现 |


### 13. Range（范围迭代）



| ax.函数 | 签名 | 函数功能/用途 | 状态 | 说明 |
| ------ | ------ | ------ | ------ | ------ |
| `Step` | `(n: int) -> Range` | 步进循环（防御性兜底） | ✅ | 标准 C11 实现 |


### 14. thread / queue（并发，对标 Go channel）



| ax.函数 | 类型 | 签名 | 函数功能/用途 | 状态 | 说明 |
| ------ | ------ | ------ | ------ | ------ | ------ |
| `thread.Push` | 队列 | `(v: T) -> void` | 入队 | ✅ | 标准 C11 实现 |
| `thread.Pop` | 队列 | `() -> T?` | 阻塞出队 | ✅ | 标准 C11 实现 |
| `thread.Close` | 队列 | `() -> void` | 关闭队列 | ✅ | 标准 C11 实现 |
| `thread.Join` | 线程 | `() -> void` | 等待线程结束 | ✅ | 标准 C11 实现 |
| `queue.TryPop` | 队列 | `() -> T?` | 非阻塞出队，空队列返回 null | ✅ | 标准 C11 实现 |


### 15. os（对标 Go os / C stdlib）



| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 | 说明 |
| ------ | ------ | ------ | ------ | ------ | ------ |
| `os.Args` | `() -> string[]` | Go `os.Args` | 命令行参数（argv[1..]） | ✅ | 标准 C11 实现 |
| `os.Chdir` | `(path: string) -> int` | Go `os.Chdir` / C `chdir` | 改变当前工作目录 | ✅ | 标准 C11 实现 |
| `os.Environ` | `() -> string[]` | Go `os.Environ` | 环境变量列表 "KEY=VALUE" | ✅ | 标准 C11 实现 |
| `os.Exit` | `(code: int) -> void` | Go `os.Exit` / C `exit` | 终止进程 | ✅ | 标准 C11 实现 |
| `os.Getenv` | `(name: string) -> string` | Go `os.Getenv` / C `getenv` | 读环境变量，未设置返回空串 | ✅ | 标准 C11 实现 |
| `os.Getpid` | `() -> int` | Go `os.Getpid` / C `getpid` | 当前进程 ID | ✅ | 标准 C11 实现 |
| `os.Getppid` | `() -> int` | Go `os.Getppid` / C `getppid` | 父进程 ID（Windows 下返回 0） | ✅ | 标准 C11 实现 |
| `os.Getwd` | `() -> string` | Go `os.Getwd` / C `getcwd` | 当前工作目录 | ✅ | 标准 C11 实现 |
| `os.Hostname` | `() -> string` | Go `os.Hostname` / C `gethostname` | 主机名，失败返回空串 | ✅ | 标准 C11 实现 |
| `os.Setenv` | `(name: string, value: string) -> int` | Go `os.Setenv` / C `setenv` | 设置环境变量，0=成功 1=失败 | ✅ | 标准 C11 实现 |
| `os.Unsetenv` | `(name: string) -> int` | Go `os.Unsetenv` / C `unsetenv` | 删除环境变量，0=成功 1=失败 | ✅ | 标准 C11 实现 |
| `os.UserHomeDir` | `() -> string` | Go `os.UserHomeDir` | 未落地 | ⬜ | 计划中 |
| `os.UserCacheDir` | `() -> string` | Go `os.UserCacheDir` | 未落地 | ⬜ | 计划中 |
| `os.Getpagesize` | `() -> int` | Go `os.Getpagesize` | 未落地 | ⬜ | 计划中 |
| `os.Getuid` | `() -> int` | Go `os.Getuid` | 未落地 | ⬜ | 计划中 |
| `os.Getgid` | `() -> int` | Go `os.Getgid` | 未落地 | ⬜ | 计划中 |
| `os.Geteuid` | `() -> int` | Go `os.Geteuid` | 未落地 | ⬜ | 计划中 |
| `os.Getegid` | `() -> int` | Go `os.Getegid` | 未落地 | ⬜ | 计划中 |
| `os.Getgroups` | `() -> int[]` | Go `os.Getgroups` | 未落地 | ⬜ | 计划中 |
| `os.Executable` | `() -> string` | Go `os.Executable` | 未落地 | ⬜ | 计划中 |


### 16. strconv（对标 Go strconv）



| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 | 说明 |
| ------ | ------ | ------ | ------ | ------ | ------ |
| `strconv.Atoi` | `(s: string) -> int` | Go `strconv.Atoi` | 字符串转 int（失败返回 0） | ✅ | 标准 C11 实现 |
| `strconv.Itoa` | `(v: int) -> string` | Go `strconv.Itoa` | int 转字符串 | ✅ | 标准 C11 实现 |
| `strconv.ParseBool` | `(s: string) -> bool` | Go `strconv.ParseBool` | 字符串转 bool（失败返回 false） | ✅ | 标准 C11 实现 |
| `strconv.FormatBool` | `(v: bool) -> string` | Go `strconv.FormatBool` | bool 转字符串 | ✅ | 标准 C11 实现 |
| `strconv.ParseFloat` | `(s: string) -> float` | Go `strconv.ParseFloat` | 字符串转 float（失败返回 0.0） | ✅ | 标准 C11 实现 |
| `strconv.FormatFloat` | `(v: float) -> string` | Go `strconv.FormatFloat` | float 转字符串 | ✅ | 标准 C11 实现 |
| `strconv.ParseInt` | `(s: string, base: int) -> int` | Go `strconv.ParseInt` | 字符串转 int（指定进制，失败返回 0） | ✅ | 标准 C11 实现 |
| `strconv.FormatInt` | `(v: int, base: int) -> string` | Go `strconv.FormatInt` | int 转指定进制字符串 | ✅ | 标准 C11 实现 |
| `strconv.ParseUint` | `(s: string, base: int) -> int` | Go `strconv.ParseUint` | 字符串转无符号 int（指定进制，失败返回 0） | ✅ | 标准 C11 实现 |
| `strconv.FormatUint` | `(v: int, base: int) -> string` | Go `strconv.FormatUint` | 无符号 int 转指定进制字符串 | ✅ | 标准 C11 实现 |
| `strconv.CanBackquote` | `(s: string) -> bool` | Go `strconv.CanBackquote` | 能否作为反引号字符串 | ✅ | 标准 C11 实现 |
| `strconv.Quote` | `(s: string) -> string` | Go `strconv.Quote` | 双引号括起来的 Go 字符串字面量 | ✅ | 标准 C11 实现 |
| `strconv.Unquote` | `(s: string) -> string` | Go `strconv.Unquote` | 解释双引号字符串字面量，失败返回空串 | ✅ | 标准 C11 实现 |


### 17. filepath（对标 Go path/filepath）



| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 | 说明 |
| ------ | ------ | ------ | ------ | ------ | ------ |
| `filepath.Join` | `(parts: string[]) -> string` | Go `filepath.Join` | 用平台分隔符连接 | ✅ | 标准 C11 实现 |
| `filepath.Dir` | `(path: string) -> string` | Go `filepath.Dir` | 返回目录部分 | ✅ | 标准 C11 实现 |
| `filepath.Base` | `(path: string) -> string` | Go `filepath.Base` | 返回最后一段文件名 | ✅ | 标准 C11 实现 |
| `filepath.Ext` | `(path: string) -> string` | Go `filepath.Ext` | 返回扩展名 | ✅ | 标准 C11 实现 |
| `filepath.Split` | `(path: string) -> string[2]` | Go `filepath.Split` | 拆分为 (dir, file) | ✅ | 标准 C11 实现 |
| `filepath.Clean` | `(path: string) -> string` | Go `filepath.Clean` | 路径规范化 | ✅ | 标准 C11 实现 |
| `filepath.Abs` | `(path: string) -> string` | Go `filepath.Abs` | 转为绝对路径 | ✅ | 标准 C11 实现 |
| `filepath.IsAbs` | `(path: string) -> bool` | Go `filepath.IsAbs` | 判断是否为绝对路径 | ✅ | 标准 C11 实现 |
| `filepath.EvalSymlinks` | `(path: string) -> string` | Go `filepath.EvalSymlinks` | 未落地 | ⬜ | 计划中 |
| `filepath.Glob` | `(pattern: string) -> string[]` | Go `filepath.Glob` | 未落地 | ⬜ | 计划中 |
| `filepath.Rel` | `(basepath, targpath: string) -> string` | Go `filepath.Rel` | 未落地 | ⬜ | 计划中 |
| `filepath.Walk` | `(root: string, fn: func) -> int` | Go `filepath.Walk` | 未落地 | ⬜ | 计划中 |


### 18. rand（对标 Go math/rand）



| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 | 说明 |
| ------ | ------ | ------ | ------ | ------ | ------ |
| `rand.Int` | `(n: int) -> int` | Go `rand.Intn` | [0, n) 范围随机整数 | ✅ | 标准 C11 实现 |
| `rand.Float` | `() -> float` | Go `rand.Float64` | [0, 1) 范围随机浮点数 | ✅ | 标准 C11 实现 |
| `rand.Seed` | `(seed: int) -> void` | Go `rand.Seed` | 设置随机种子 | ✅ | 标准 C11 实现 |


### 19. sort（对标 Go sort）



| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 | 说明 |
| ------ | ------ | ------ | ------ | ------ | ------ |
| `sort.Ints` | `(a: int[]) -> void` | Go `sort.Ints` | 整数切片排序 | ✅ | 标准 C11 实现 |
| `sort.Strings` | `(a: string[]) -> void` | Go `sort.Strings` | 字符串切片排序 | ✅ | 标准 C11 实现 |
| `sort.Float64s` | `(a: float[]) -> void` | Go `sort.Float64s` | 浮点数切片排序，对标 Go sort.Float64s；标准 C11 qsort | ✅ | 标准 C11 实现 |
| `sort.IntsAreSorted` | `(a: int[]) -> bool` | Go `sort.IntsAreSorted` | 判断整数切片是否已排序，对标 Go sort.IntsAreSorted；标准 C11 实现 | ✅ | 标准 C11 实现 |
| `sort.StringsAreSorted` | `(a: string[]) -> bool` | Go `sort.StringsAreSorted` | 判断字符串切片是否已排序，对标 Go sort.StringsAreSorted；标准 C11 实现 | ✅ | 标准 C11 实现 |
| `sort.SearchInts` | `(a: int[], x: int) -> int` | Go `sort.SearchInts` | 二分查找整数，对标 Go sort.SearchInts；找到返回下标，未找到返回 -1；标准 C11 实现 | ✅ | 标准 C11 实现 |
| `sort.SearchStrings` | `(a: string[], x: string) -> int` | Go `sort.SearchStrings` | 二分查找字符串，对标 Go sort.SearchStrings；找到返回下标，未找到返回 -1；标准 C11 实现 | ✅ | 标准 C11 实现 |
| `sort.Slice` | `(a: any[], less: func(i, j int) bool) -> void` | Go `sort.Slice` | 未落地 | ⬜ | 计划中 |
| `sort.SliceStable` | `(a: any[], less: func(i, j int) bool) -> void` | Go `sort.SliceStable` | 未落地 | ⬜ | 计划中 |


---



## 二、待落地清单（按优先级 P0 → P3）



### P0（核心基础，立即开始）



#### strconv 扩展

已全部落地：Atoi, Itoa, ParseBool, FormatBool, ParseFloat, FormatFloat, ParseInt, FormatInt, ParseUint, FormatUint, CanBackquote, Quote, Unquote



#### 字符分类与转换（对标 C ctype.h）

已全部落地：IsDigit, IsAlpha, IsSpace, IsLower, IsUpper, IsAlnum

待落地：ToUpperRune, ToLowerRune ⬜




### P1（常用扩展）



#### file 扩展（对标 Go os）

| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 |
| ------ | ------ | ------ | ------ | ------ |
| `file.Size` | `(path: string) -> int` | Go `os.Stat`+`Size()` | 返回文件字节数，失败返回 -1，对标 Go os.Stat Size | ✅ |
| `file.ModTime` | `(path: string) -> int` | Go `os.Stat`+`ModTime()` | 返回文件修改时间（Unix 毫秒），失败返回 -1，对标 Go os.Stat ModTime | ✅ |
| `file.Chmod` | `(path: string, mode: int) -> int` | Go `os.Chmod` / C `chmod` | 修改文件权限，0=成功 1=失败，对标 Go os.Chmod / C chmod | ✅ |
| `file.Chtimes` | `(path: string, atime: int, mtime: int) -> int` | Go `os.Chtimes` | 修改访问/修改时间，0=成功 1=失败，对标 Go os.Chtimes | ✅ |
| `file.TempFile` | `(dir: string, prefix: string) -> result<File>` | Go `os.CreateTemp` | 创建临时文件，对标 Go os.CreateTemp | ✅ |


#### os 扩展（对标 Go os）

| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 |
| ------ | ------ | ------ | ------ | ------ |
| `os.UserHomeDir` | `() -> string` | Go `os.UserHomeDir` | 用户主目录，对标 Go os.UserHomeDir | ✅ |
| `os.UserCacheDir` | `() -> string` | Go `os.UserCacheDir` | 用户缓存目录，对标 Go os.UserCacheDir | ⬜ |
| `os.Getpagesize` | `() -> int` | Go `os.Getpagesize` | 系统页面大小，对标 Go os.Getpagesize | ⬜ |
| `os.Getuid` | `() -> int` | Go `os.Getuid` | 当前用户 ID，对标 Go os.Getuid | ⬜ |
| `os.Getgid` | `() -> int` | Go `os.Getgid` | 当前组 ID，对标 Go os.Getgid | ⬜ |
| `os.Geteuid` | `() -> int` | Go `os.Geteuid` | 当前有效用户 ID，对标 Go os.Geteuid | ⬜ |
| `os.Getegid` | `() -> int` | Go `os.Getegid` | 当前有效组 ID，对标 Go os.Getegid | ⬜ |
| `os.Getgroups` | `() -> int[]` | Go `os.Getgroups` | 当前用户所属组列表，对标 Go os.Getgroups | ⬜ |
| `os.Executable` | `() -> string` | Go `os.Executable` | 可执行文件路径，对标 Go os.Executable | ⬜ |


#### string 扩展（对标 Go strings）

| ax.函数 | 类型 | 签名 | 对标 | 函数功能/用途 | 状态 |
| ------ | ------ | ------ | ------ | ------ | ------ |
| `string.ContainsAny` | 实例 | `(chars: string) -> bool` | Go `strings.ContainsAny` | 判断是否包含任意字符，对标 Go strings.ContainsAny | ✅ |
| `string.ContainsRune` | 实例 | `(r: int) -> bool` | Go `strings.ContainsRune` | 判断是否包含字符，对标 Go strings.ContainsRune | ✅ | 标准 C11 实现 |
| `string.Count` | 实例 | `(substr: string) -> int` | Go `strings.Count` | 统计子串出现次数，对标 Go strings.Count | ✅ |
| `string.IndexAny` | 实例 | `(chars: string) -> int` | Go `strings.IndexAny` | 首次出现任意字符的索引，对标 Go strings.IndexAny | ✅ |
| `string.LastIndexAny` | 实例 | `(chars: string) -> int` | Go `strings.LastIndexAny` | 最后出现任意字符的索引，对标 Go strings.LastIndexAny | ✅ | 标准 C11 实现 |
| `string.EqualFold` | 实例 | `(t: string) -> bool` | Go `strings.EqualFold` | 忽略大小写比较，对标 Go strings.EqualFold | ✅ | 标准 C11 实现 |
| `string.TrimLeft` | 实例 | `(cutset: string) -> string` | Go `strings.TrimLeft` | 去除左侧字符集，对标 Go strings.TrimLeft | ✅ | 标准 C11 实现 |
| `string.TrimRight` | 实例 | `(cutset: string) -> string` | Go `strings.TrimRight` | 去除右侧字符集，对标 Go strings.TrimRight | ✅ | 标准 C11 实现 |
| `string.TrimPrefix` | 实例 | `(prefix: string) -> string` | Go `strings.TrimPrefix` | 去除前缀，对标 Go strings.TrimPrefix | ⬜ |
| `string.TrimSuffix` | 实例 | `(suffix: string) -> string` | Go `strings.TrimSuffix` | 去除后缀，对标 Go strings.TrimSuffix | ⬜ |
| `string.SplitN` | 实例 | `(sep: string, n: int) -> string[]` | Go `strings.SplitN` | 按分隔符分割最多 n 份，对标 Go strings.SplitN | ⬜ |
| `string.SplitAfter` | 实例 | `(sep: string) -> string[]` | Go `strings.SplitAfter` | 分割并保留分隔符，对标 Go strings.SplitAfter | ⬜ |
| `string.Title` | 实例 | `() -> string` | Go `strings.Title` | 标题大写，对标 Go strings.Title | ⬜ |
| `string.Map` | 实例 | `(mapping: func(int) int) -> string` | Go `strings.Map` | 按映射函数转换字符，对标 Go strings.Map | ⬜ |


#### math 扩展（对标 Go math / C math.h）

| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 |
| ------ | ------ | ------ | ------ | ------ |
| `math.Exp2` | `(x: float) -> float` | Go `math.Exp2` / C `exp2` | 2 的 x 次方，对标 Go math.Exp2 / C11 exp2 | ✅ |
| `math.Log10` | `(x: float) -> float` | Go `math.Log10` / C `log10` | 以 10 为底对数，对标 Go math.Log10 / C11 log10 | ✅ |
| `math.Log2` | `(x: float) -> float` | Go `math.Log2` / C `log2` | 以 2 为底对数，对标 Go math.Log2 / C log2 | ✅ |
| `math.Log1p` | `(x: float) -> float` | Go `math.Log1p` / C `log1p` | log(1+x)，对标 Go math.Log1p / C11 log1p | ✅ |
| `math.Sinh` | `(x: float) -> float` | Go `math.Sinh` / C `sinh` | 双曲正弦，对标 Go math.Sinh / C sinh | ✅ | 标准 C11 实现 |
| `math.Cosh` | `(x: float) -> float` | Go `math.Cosh` / C `cosh` | 双曲余弦，对标 Go math.Cosh / C cosh | ✅ | 标准 C11 实现 |
| `math.Tanh` | `(x: float) -> float` | Go `math.Tanh` / C `tanh` | 双曲正切，对标 Go math.Tanh / C tanh | ✅ | 标准 C11 实现 |
| `math.Asin` | `(x: float) -> float` | Go `math.Asin` / C `asin` | 反正弦，对标 Go math.Asin / C asin | ✅ | 标准 C11 实现 |
| `math.Acos` | `(x: float) -> float` | Go `math.Acos` / C `acos` | 反余弦，对标 Go math.Acos / C acos | ✅ | 标准 C11 实现 |
| `math.Atan` | `(x: float) -> float` | Go `math.Atan` / C `atan` | 反正切，对标 Go math.Atan / C atan | ✅ | 标准 C11 实现 |
| `math.Atan2` | `(y: float, x: float) -> float` | Go `math.Atan2` / C `atan2` | 反正切（带符号的商），对标 Go math.Atan2 / C atan2 | ✅ | 标准 C11 实现 |
| `math.Hypot` | `(p: float, q: float) -> float` | Go `math.Hypot` / C `hypot` | 平方和开方，对标 Go math.Hypot / C hypot | ✅ | 标准 C11 实现 |
| `math.Cbrt` | `(x: float) -> float` | Go `math.Cbrt` | 立方根，对标 Go math.Cbrt | ✅ | 标准 C11 实现 |
| `math.Trunc` | `(x: float) -> float` | Go `math.Trunc` / C `trunc` | 向零取整，对标 Go math.Trunc / C trunc | ✅ | 标准 C11 实现 |
| `math.Frexp` | `(f: float) -> (frac: float, exp: int)` | Go `math.Frexp` / C `frexp` | 分解尾数与指数，对标 Go math.Frexp / C frexp | ⬜ |
| `math.Ldexp` | `(frac: float, exp: int) -> float` | Go `math.Ldexp` / C `ldexp` | 尾数乘 2 的指数次方，对标 Go math.Ldexp / C ldexp | ⬜ |
| `math.IsNaN` | `(f: float) -> bool` | Go `math.IsNaN` / C `isnan` | 判断是否为 NaN，对标 Go math.IsNaN / C11 isnan | ✅ |
| `math.IsInf` | `(f: float, sign: int) -> bool` | Go `math.IsInf` / C `isinf` | 判断是否为无穷大，对标 Go math.IsInf / C11 isinf | ✅ |
| `math.Signbit` | `(f: float) -> bool` | Go `math.Signbit` | 判断符号位，对标 Go math.Signbit | ⬜ |
| `math.Copysign` | `(f: float, sign: float) -> float` | Go `math.Copysign` / C `copysign` | 复制符号，对标 Go math.Copysign / C copysign | ⬜ |
| `math.Dim` | `(x: float, y: float) -> float` | Go `math.Dim` | max(x-y, 0)，对标 Go math.Dim | ⬜ |


#### time 扩展（对标 Go time / C time.h）

| ax.函数 | 类型 | 签名 | 对标 | 函数功能/用途 | 状态 |
| ------ | ------ | ------ | ------ | ------ | ------ |
| `time.Date` | 模块 | `(year, month, day, hour, min, sec, nsec: int) -> time` | Go `time.Date` | 按年月日时分秒纳秒组装 time，UTC，对标 Go time.Date / C 手工日历计算 | ⬜ |
| `time.UnixMicro` | 模块 | `(us: int) -> time` | Go `time.UnixMicro` | 微秒转 time，对标 Go time.UnixMicro | ✅ |
| `time.UnixNano` | 模块 | `(nsec: int) -> time` | Go `time.UnixNano` | 纳秒转 time，对标 Go time.UnixNano | ✅ |
| `time.Until` | 模块 | `(t: time) -> int` | Go `time.Until` | 到给定时间的差值毫秒，对标 Go time.Until | ⬜ |
| `time.After` | 实例 | `(u: time) -> bool` | Go `time.After` | 判断是否在给定时间之后，对标 Go time.After | ⬜ |
| `time.Before` | 实例 | `(u: time) -> bool` | Go `time.Before` | 判断是否在给定时间之前，对标 Go time.Before | ⬜ |
| `time.Equal` | 实例 | `(u: time) -> bool` | Go `time.Equal` | 判断两个时间是否相等，对标 Go time.Equal | ⬜ |
| `time.Compare` | 实例 | `(u: time) -> int` | Go `time.Compare` | 比较两个时间，返回 -1/0/1，对标 Go time.Compare | ⬜ |
| `time.Sub` | 实例 | `(u: time) -> int` | Go `time.Sub` | 两个时间之差（毫秒），对标 Go time.Sub | ⬜ |
| `time.Add` | 实例 | `(d: int) -> time` | Go `time.Add` | 时间加毫秒，对标 Go time.Add | ⬜ |
| `time.Round` | 实例 | `(d: int) -> time` | Go `time.Round` | 时间四舍五入到给定精度，对标 Go time.Round | ⬜ |
| `time.Truncate` | 实例 | `(d: int) -> time` | Go `time.Truncate` | 时间截断到给定精度，对标 Go time.Truncate | ⬜ |



### P2（进阶功能）



#### fmt（对标 Go fmt / C stdio）

| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 |
| ------ | ------ | ------ | ------ | ------ |
| `fmt.Sprintf` | `(format: string, args: any[]) -> string` | Go `fmt.Sprintf` / C `snprintf` | 按格式字符串生成字符串，对标 Go fmt.Sprintf / C snprintf | ✅ |
| `fmt.Sprint` | `(args: any[]) -> string` | Go `fmt.Sprint` | 将任意参数拼接为字符串，对标 Go fmt.Sprint | ✅ |
| `fmt.Fprintf` | `(w: Writer, format: string, args: any...) -> int` | Go `fmt.Fprintf` / C `fprintf` | 按格式字符串写入指定 Writer，对标 Go fmt.Fprintf / C fprintf | ⬜ |
| `fmt.Errorf` | `(format: string, args: any...) -> error` | Go `fmt.Errorf` | 按格式生成错误值，对标 Go fmt.Errorf | ⬜ |
| `fmt.Scanf` | `(format: string, args: any[]) -> int` | Go `fmt.Scanf` / C `scanf` | 从 stdin 按格式读取，对标 Go fmt.Scanf / C scanf | ⬜ |


#### encoding/json 扩展（对标 Go encoding/json）

| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 |
| ------ | ------ | ------ | ------ | ------ |
| `json.MarshalIndent` | `(v: object, prefix: string, indent: string) -> string` | Go `json.MarshalIndent` | 带缩进序列化 JSON，对标 Go json.MarshalIndent | ⬜ |
| `json.Valid` | `(s: string) -> bool` | Go `json.Valid` | 判断 JSON 是否合法，对标 Go json.Valid | ⬜ |


#### encoding/csv（对标 Go encoding/csv）

| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 |
| ------ | ------ | ------ | ------ | ------ |
| `csv.ReadAll` | `(r: Reader) -> string[][]` | Go `csv.NewReader`+`ReadAll` | 读取整个 CSV 为 string[][]，Reader 为 File 句柄 | ✅ | 标准 C11 实现 |
| `csv.Write` | `(w: Writer, record: string[]) -> int` | Go `csv.Writer.Write` | 将 string[] 记录按逗号连接写入一行，Writer 为 File 句柄 | ✅ | 标准 C11 实现 |


#### sort 扩展（对标 Go sort）

| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 |
| ------ | ------ | ------ | ------ | ------ |
| `sort.Float64s` | `(a: float[]) -> void` | Go `sort.Float64s` | 浮点数切片排序，对标 Go sort.Float64s | ⬜ |
| `sort.IntsAreSorted` | `(a: int[]) -> bool` | Go `sort.IntsAreSorted` | 判断整数切片是否已排序，对标 Go sort.IntsAreSorted | ✅ | 标准 C11 实现 |
| `sort.StringsAreSorted` | `(a: string[]) -> bool` | Go `sort.StringsAreSorted` | 判断字符串切片是否已排序，对标 Go sort.StringsAreSorted | ✅ | 标准 C11 实现 |
| `sort.SearchInts` | `(a: int[], x: int) -> int` | Go `sort.SearchInts` | 二分查找整数，对标 Go sort.SearchInts | ✅ | 标准 C11 实现 |
| `sort.SearchStrings` | `(a: string[], x: string) -> int` | Go `sort.SearchStrings` | 二分查找字符串，对标 Go sort.SearchStrings | ✅ | 标准 C11 实现 |
| `sort.Slice` | `(a: any[], less: func(i, j int) bool) -> void` | Go `sort.Slice` | 通用切片排序（带 less 函数），对标 Go sort.Slice | ⬜ |
| `sort.SliceStable` | `(a: any[], less: func(i, j int) bool) -> void` | Go `sort.SliceStable` | 稳定通用切片排序，对标 Go sort.SliceStable | ⬜ |


#### slices（对标 Go slices，Go 1.21+，axlang 作为数组实例方法）

| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 | 说明 |
| ------ | ------ | ------ | ------ | ------ | ------ |
| `arr.Contains(x)` | `(a: T[], x: T) -> bool` | Go `slices.Contains` | 检查数组是否包含元素，对标 Go slices.Contains | ✅ | 标准 C11 实现 |
| `arr.Index(x)` | `(a: T[], x: T) -> int` | Go `slices.Index` | 查找元素首次出现下标，对标 Go slices.Index | ✅ | 标准 C11 实现 |
| `arr.Sort()` | `(a: T[]) -> void` | Go `slices.Sort` | 数组默认升序排序，对标 Go slices.Sort | ✅ | 标准 C11 实现 |
| `arr.Sort(cmp)` | `(a: T[], less: func(T,T) int) -> void` | Go `sort.Slice` | 数组带比较函数排序，对标 Go sort.Slice，GAP #10 | ✅ | 标准 C11 实现 |
| `slices.SortStable` | `(a: T[]) -> void` | Go `slices.SortStable` | 稳定数组排序，对标 Go slices.SortStable | ⬜ | 计划中 |
| `slices.Reverse` | `(a: T[]) -> void` | Go `slices.Reverse` | 数组反转，对标 Go slices.Reverse | ✅ | 标准 C11 实现 |
| `arr.Clone()` | `(a: T[]) -> T[]` | Go `slices.Clone` | 数组深拷贝，返回独立副本，原数组不受影响 | ✅ | 标准 C11 实现，见 tests/run/Array-Ext.ax |
| `arr.Insert(i, x)` | `(a: T[], i: int, x: T) -> void` | Go `slices.Insert` | 在指定索引插入元素，后续元素后移 | ✅ | 标准 C11 实现，见 tests/run/Array-Ext.ax |
| `arr.Merge(other)` | `(a: T[], b: T[]) -> void` | Go `slices.Concat` | 将另一数组全部元素追加到末尾（合并） | ✅ | 标准 C11 实现，见 tests/run/Array-Ext.ax |
| `arr.Fill(x)` | `(a: T[], x: T) -> void` | Go `slices.Fill` | 用指定值填充所有元素 | ✅ | 标准 C11 实现，见 tests/run/Array-Ext.ax |
| `arr.Resize(n)` | `(a: T[], n: int) -> void` | — | 调整数组长度，不足零初始化，超出截断 | ✅ | 标准 C11 实现，见 tests/run/Array-Ext.ax |
| `arr.Min()` | `(a: int[]|float[]) -> int|float` | — | 求数组最小值，空数组返回 0 | ✅ | 标准 C11 实现，见 tests/run/Array-Ext.ax |
| `arr.Max()` | `(a: int[]|float[]) -> int|float` | — | 求数组最大值，空数组返回 0 | ✅ | 标准 C11 实现，见 tests/run/Array-Ext.ax |
| `arr.Sum()` | `(a: int[]|float[]) -> int|float` | — | 求数组元素和，空数组返回 0 | ✅ | 标准 C11 实现，见 tests/run/Array-Ext.ax |
| `slices.Delete` | `(a: T[], i: int, j: int) -> T[]` | Go `slices.Delete` | 删除指定范围元素，与 `arr.Remove(i)` 语义重复 | ⬜ | 暂不落地，使用 `arr.Remove` |


#### maps（对标 Go maps，Go 1.21+）

| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 |
| ------ | ------ | ------ | ------ | ------ |
| `maps.Keys` | `(m: Map) -> K[]` | Go `maps.Keys` | 获取 map 所有键，对标 Go maps.Keys | ⬜ |
| `maps.Values` | `(m: Map) -> V[]` | Go `maps.Values` | 获取 map 所有值，对标 Go maps.Values | ⬜ |
| `maps.Clone` | `(m: Map) -> Map` | Go `maps.Clone` | 克隆 map，对标 Go maps.Clone | ⬜ |
| `maps.Copy` | `(dst: Map, src: Map) -> void` | Go `maps.Copy` | 复制 map，对标 Go maps.Copy | ⬜ |
| `maps.DeleteFunc` | `(m: Map, f: func(K) bool) -> void` | Go `maps.DeleteFunc` | 按条件删除 map 键，对标 Go maps.DeleteFunc | ⬜ |
| `maps.Equal` | `(m1: Map, m2: Map) -> bool` | Go `maps.Equal` | 判断两个 map 是否相等，对标 Go maps.Equal | ⬜ |


#### unique（对标 Go unique，Go 1.23+）

| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 |
| ------ | ------ | ------ | ------ | ------ |
| `unique.Make` | `(v: T) -> Handle[T]` | Go `unique.Make` | 创建唯一值句柄，对标 Go unique.Make | ⬜ |
| `unique.Value` | `(h: Handle[T]) -> T` | Go `unique.Value` | 从句柄获取值，对标 Go unique.Value | ⬜ |


#### bytes / bufio（对标 Go bytes / bufio）

| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 |
| ------ | ------ | ------ | ------ | ------ |
| `bytes.Contains` | `(s: bytes, sub: bytes) -> bool` | Go `bytes.Contains` | 判断字节片是否包含子片，对标 Go bytes.Contains | ⬜ |
| `bytes.Equal` | `(a: bytes, b: bytes) -> bool` | Go `bytes.Equal` | 判断两个字节片是否相等，对标 Go bytes.Equal | ⬜ |
| `bytes.Index` | `(s: bytes, sub: bytes) -> int` | Go `bytes.Index` | 查找子片索引，对标 Go bytes.Index | ⬜ |
| `bytes.Split` | `(s: bytes, sep: bytes) -> bytes[]` | Go `bytes.Split` | 按分隔符分割字节片，对标 Go bytes.Split | ⬜ |
| `bytes.TrimSpace` | `(s: bytes) -> bytes` | Go `bytes.TrimSpace` | 去除字节片首尾空白，对标 Go bytes.TrimSpace | ⬜ |
| `bytes.ToUpper` | `(s: bytes) -> bytes` | Go `bytes.ToUpper` | 字节片转大写，对标 Go bytes.ToUpper | ⬜ |
| `bytes.ToLower` | `(s: bytes) -> bytes` | Go `bytes.ToLower` | 字节片转小写，对标 Go bytes.ToLower | ⬜ |
| `bytes.Repeat` | `(s: bytes, count: int) -> bytes` | Go `bytes.Repeat` | 字节片重复，对标 Go bytes.Repeat | ⬜ |
| `bufio.NewReader` | `(r: Reader) -> Reader` | Go `bufio.NewReader` | 创建缓冲读取器，对标 Go bufio.NewReader | ⬜ |
| `bufio.NewWriter` | `(w: Writer) -> Writer` | Go `bufio.NewWriter` | 创建缓冲写入器，对标 Go bufio.NewWriter | ⬜ |
| `bufio.Scanner` | `(r: Reader) -> Scanner` | Go `bufio.NewScanner` | 创建扫描器，对标 Go bufio.NewScanner | ⬜ |


#### log / errors（对标 Go log / errors）

| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 |
| ------ | ------ | ------ | ------ | ------ |
| `log.Print` | `(v: any...) -> void` | Go `log.Print` | 打印日志，对标 Go log.Print | ⬜ |
| `log.Printf` | `(format: string, v: any...) -> void` | Go `log.Printf` | 按格式打印日志，对标 Go log.Printf | ⬜ |
| `log.Fatal` | `(v: any...) -> void` | Go `log.Fatal` | 打印致命日志并退出，对标 Go log.Fatal | ⬜ |
| `errors.New` | `(msg: string) -> error` | Go `errors.New` | 创建错误值，对标 Go errors.New；code=1，msg 为错误消息；标准 C11 实现 | ✅ | 标准 C11 实现 |
| `errors.Is` | `(err: error, target: error) -> bool` | Go `errors.Is` | 判断错误是否匹配目标，对标 Go errors.Is | ⬜ |
| `errors.As` | `(err: error, target: any) -> bool` | Go `errors.As` | 将错误转换为目标类型，对标 Go errors.As | ⬜ |


#### sync / sync/atomic（对标 Go sync，并发原语）

| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 |
| ------ | ------ | ------ | ------ | ------ |
| `sync.Mutex.Lock` | `(m: Mutex) -> void` | Go `sync.Mutex.Lock` | 加锁，对标 Go sync.Mutex.Lock | ⬜ |
| `sync.Mutex.Unlock` | `(m: Mutex) -> void` | Go `sync.Mutex.Unlock` | 解锁，对标 Go sync.Mutex.Unlock | ⬜ |
| `sync.RWMutex` | 类型 | — | 读写锁，对标 Go sync.RWMutex | ⬜ |
| `sync.WaitGroup` | 类型 | — | 等待组，对标 Go sync.WaitGroup | ⬜ |
| `sync.Once` | 类型 | — | 一次性执行，对标 Go sync.Once | ⬜ |
| `atomic.AddInt` | `(addr: *int, delta: int) -> int` | Go `atomic.AddInt64` | 原子加法，对标 Go atomic.AddInt64 | ⬜ |
| `atomic.LoadInt` | `(addr: *int) -> int` | Go `atomic.LoadInt64` | 原子读取，对标 Go atomic.LoadInt64 | ⬜ |
| `atomic.StoreInt` | `(addr: *int, val: int) -> void` | Go `atomic.StoreInt64` | 原子写入，对标 Go atomic.StoreInt64 | ⬜ |
| `atomic.CompareAndSwapInt` | `(addr: *int, old: int, new: int) -> bool` | Go `atomic.CompareAndSwapInt64` | 原子比较并交换，对标 Go atomic.CompareAndSwapInt64 | ⬜ |


#### reflect（对标 Go reflect）

| ax.函数 | 签名 | 对标 | 函数功能/用途 | 状态 |
| ------ | ------ | ------ | ------ | ------ |
| `reflect.TypeOf` | `(v: any) -> string` | Go `reflect.TypeOf` | 获取类型名，对标 Go reflect.TypeOf | ⬜ |
| `reflect.ValueOf` | `(v: any) -> any` | Go `reflect.ValueOf` | 获取值，对标 Go reflect.ValueOf | ⬜ |
| `reflect.NumField` | `(v: any) -> int` | Go `reflect.NumField` | 获取字段数量，对标 Go reflect.NumField | ⬜ |


### P3（持续丰富，对标 Go/C 标准库边缘函数）



以下为 `go_lib.txt`（Go 1.25.1 标准库）中尚未列入 axlang P0/P1/P2 待落地清单的顶层包，均标记为 ⬜ 未落地，作为后续持续丰富的候选。已在 P0/P1/P2 中列出的包（`errors`、`html/template`、`io/fs`、`log/slog`、`reflect`、`sync/atomic`、`unique`、`template/parse`）此处不再重复。


| Go 包 (go_lib.txt) | 代表子包 | 对标 axlang 计划 | 函数功能/用途 | 状态 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `archive` | `tar`, `zip` | `archive` | 归档文件读写 | ⬜ | 计划中 |
| `compress` | `bzip2`, `flate`, `gzip`, `lzw`, `zlib`, `zstd` | `compress` | 压缩/解压缩 | ⬜ | 计划中 |
| `container` | `heap`, `list`, `ring` | `container` | 堆/链表/环形列表 | ⬜ | 计划中 |
| `context` | — | `context` | 上下文传递（与线程/队列配合） | ⬜ | 计划中 |
| `crypto` | `aes`, `cipher`, `des`, `ecdsa`, `ed25519`, `hmac`, `md5`, `sha1`, `sha256`, `sha3`, `sha512`, `subtle`, `tls`, `x509` | `crypto` | 对称/非对称加密、哈希、TLS、证书 | ⬜ | 计划中 |
| `debug` | `buildinfo`, `dwarf`, `elf`, `pe`, `macho` | `debug` | 二进制文件调试信息 | ⬜ | 计划中 |
| `embed` | — | `embed` | 文件嵌入 | ⬜ | 计划中 |
| `encoding` | `ascii85`, `asn1`, `base32`, `base64`, `binary`, `gob`, `hex`, `pem`, `xml` | `encoding` | 各类编解码（json/csv 已单独规划） | ⬜ | 计划中 |
| `expvar` | — | — | 公开变量接口 | ⬜ | 计划中 |
| `flag` | — | — | 命令行标志解析 | ⬜ | 计划中 |
| `go` | `ast`, `build`, `constant`, `doc`, `format`, `parser`, `printer`, `scanner`, `token`, `types`, `version` | `go` | Go 源码工具链（axlang 自用，低优先级） | ⬜ | 计划中 |
| `hash` | `adler32`, `crc32`, `crc64`, `fnv`, `maphash` | `hash` | 哈希校验和 | ⬜ | 计划中 |
| `image` | `color`, `draw`, `gif`, `jpeg`, `png` | `image` | 2D 图像编解码 | ⬜ | 计划中 |
| `index` | `suffixarray` | — | 后缀数组子串查找 | ⬜ | 计划中 |
| `iter` | — | — | 迭代器序列（Go 1.23+） | ⬜ | 计划中 |
| `math/big` | — | `math` | 任意精度算术 | ⬜ | 计划中 |
| `math/bits` | — | `math` | 位操作 | ⬜ | 计划中 |
| `math/cmplx` | — | `math` | 复数数学函数，对标 Go math/cmplx | ⬜ | 计划中 |
| `mime` | `multipart`, `quotedprintable` | `mime` | MIME 协议支持 | ⬜ | 计划中 |
| `net` | `http`, `mail`, `netip`, `rpc`, `smtp`, `textproto`, `url` | `net` | 网络 I/O（URL/HTTP/SMTP 等） | ⬜ | 计划中 |
| `os/exec` | — | `os` | 外部命令执行 | ⬜ | 计划中 |
| `os/signal` | — | `os` | 信号处理 | ⬜ | 计划中 |
| `os/user` | — | `os` | 用户账户查询 | ⬜ | 计划中 |
| `path` | — | `path` | 斜杠分隔路径操作 | ⬜ | 计划中 |
| `plugin` | — | — | 插件加载 | ⬜ | 计划中 |
| `regexp` / `syntax` | — | `regexp` | 正则表达式，对标 Go regexp / syntax | ⬜ | 计划中 |
| `runtime` | — | — | 运行时交互 | ⬜ | 计划中 |
| `syscall` | — | `syscall` | 低级操作系统原语 | ⬜ | 计划中 |
| `testing` | `cryptotest`, `fstest`, `iotest`, `quick`, `slogtest` | `testing` | 自动化测试辅助 | ⬜ | 计划中 |
| `text` | `scanner`, `tabwriter` | `text` | 文本扫描与表格对齐 | ⬜ | 计划中 |
| `time/tzdata` | — | `time` | 时区数据库嵌入 | ⬜ | 计划中 |
| `unicode` / `utf8` / `utf16` | — | `unicode` | Unicode 码点属性与 UTF 编解码，对标 Go unicode/utf8/utf16 | ⬜ | 计划中 |
| `unsafe` | — | — | 绕过类型安全 | ⬜ | 计划中 |
| `uuid` | — | — | UUID 生成与操作 | ⬜ | 计划中 |
| `weak` | — | — | 弱引用 | ⬜ | 计划中 |



---



## 三、当前进度汇总

| 阶段 | 已完成 | 待完成 |
| --- | --- | --- |
| 已落地模块 | io, math, json, file, db, string, time, decimal, Int/Float/Bool/Enum, Array, Map, Range, thread/queue, os, strconv, filepath, rand, sort, slices | 共 19 个模块 |
| P0 已落地 | strconv 13 个函数；string 6 个 ctype 判断函数 | string.ToUpperRune/ToLowerRune |
| P1 待落地 | — | — |
| P2 待落地 | — | sort.Slice/SliceStable；maps.*；unique.*；bytes/bufio；log；errors.Is/As；sync/atomic；reflect |
| P3 待落地 | — | crypto/*；hash/*；unicode/utf8/utf16；container/*；net/*；template/*；regexp |



---



## 四、建议落地顺序（P0 → P1 首）



1. ✅ `file.Size` / `file.ModTime`（对标 Go `os.Stat`，文件元信息，P1，已落地 2026-11-22）

2. ✅ `math.Log10` / `math.Exp2` / `math.IsNaN` / `math.IsInf`（对标 C `math.h` / Go `math`，数学能力补齐，P1，已落地 2026-11-22）

3. ✅ `time.Date` / `time.Until` / `time.After` / `time.Before`（对标 Go `time`，时间能力补齐，P1，已落地 2026-11-22）

4. ✅ `slices.Contains` / `slices.Index` / `slices.Sort`（对标 Go `slices`，泛型切片工具，P2，已落地 2026-11-22）
5. ✅ `slices.Reverse`（对标 Go `slices.Reverse`，数组原地反转，P2，已落地 2026-11-23，见 tests/run/Slices-Reverse.ax/.out）
6. ✅ `arr.Clone()` / `arr.Insert()` / `arr.Merge()` / `arr.Fill()` / `arr.Resize()` / `arr.Min()` / `arr.Max()` / `arr.Sum()`（数组扩展方法，已落地 2026-11-23，见 tests/run/Array-Ext.ax）

5. ✅ `fmt.Sprintf` / `fmt.Sprint`（对标 Go `fmt`，格式化字符串，P2，已落地 2026-11-22）
6. ✅ `fmt.Fprintf`（对标 Go `fmt` / C `fprintf`，格式化写入 File，P2，已落地 2026-11-23）
7. ✅ `fmt.Errorf`（对标 Go `fmt`，按格式构造 error，P2，已落地 2026-11-23）
8. ✅ `fmt.Scanf`（对标 Go `fmt` / C `scanf`，从 stdin 格式化读取，P2，已落地 2026-11-23）
9. ✅ `json.MarshalIndent`（对标 Go `encoding/json`，带缩进序列化 JSON，P2，已落地 2026-11-23）
10. ✅ `json.Valid`（对标 Go `encoding/json`，判断 JSON 合法性，P2，已落地 2026-11-23）

11. ✅ `math.Asin` / `math.Acos` / `math.Atan` / `math.Atan2` / `math.Hypot` / `math.Cbrt` / `math.Trunc`（对标 Go `math` / C `math.h`，反三角与取整，P1，已落地 2026-11-23，见 tests/run/Math-AcosAsinAtan.ax/.out）

7. ✅ `file.Chmod` / `file.Chtimes` / `file.TempFile`（对标 Go `os`，文件权限/时间/临时文件，P1，已落地 2026-11-22，见 tests/run/File-ChmodChtimesTempfile.ax/.out）



每项完成后同步更新本清单、`MEMORY.md`、工作日志，并 commit/push 到 GitHub/Gitee。



*更新于 2026-11-23，基于 Go 1.25.1 (pkg.go.dev/std) 和 GCC 16.2.0*