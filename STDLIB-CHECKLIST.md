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

## 一、编译器内建函数（长期保留在 Rust 编译器端）

以下函数涉及编译器魔法（变参、编译期占位符核对、泛型类型推导、运行时反射等），**无法**用纯 ax 语法独立实现，长期保留在编译器 Rust 端，不迁移到 ax_stdlib。

## 二、标准库函数（ax_stdlib）

以下函数已迁移至 `axc/std/` 下的 ax 语法源码，通过 `extern "C"` 调用 axrt C 原语实现。

### ax 函数清单（统一表格）

| ax.函数 | 类型 | 签名 | 用法举例 | 对标 C | 对标 Go | 函数功能 | 说明 | 状态 |
| ------ | ------ | ------ | ------ | ------ | ------ | ------ | ------ | ----- |
| `array.!=` | 内建库 | `(other: T[]) -> bool` | arr1 != arr2 | memcmp | — | 判断两个数组是否不相等 | 对标 C memcmp 取反，Go 无直接对应；等价于 !(a == b) | ✅ 已落地 |
| `array.==` | 内建库 | `(other: T[]) -> bool` | arr1 == arr2 | memcmp | reflect.DeepEqual | 判断两个数组是否相等（逐元素比较） | 对标 C memcmp，Go reflect.DeepEqual 或手动逐元素比较；同类型数组可比较 | ✅ 已落地 |
| `array.at` | 内建库 | `(index: int) -> T?` | arr.at(0) | — | — | 按索引访问数组元素，越界返回 null | 安全访问数组元素，越界返回 T? 可选类型 | ✅ 已落地 |
| `array.clear` | 内建库 | `() -> void` | arr.clear() | memset | — | 清空数组所有元素 | 对标 C memset 清零，Go 可通过重新赋值空切片实现；长度归零但容量保留 | ✅ 已落地 |
| `array.clone` | 内建库 | `() -> T[]` | arr.clone() | memcpy | — | 返回数组的深拷贝副本 | 返回独立的新数组副本，原数组不受影响 | ✅ 已落地 |
| `array.contains` | 内建库 | `(elem: T) -> bool` | arr.contains(5) | — | — | 判断数组是否包含指定元素 | 线性查找，返回是否存在指定元素 | ✅ 已落地 |
| `array.fill` | 内建库 | `(elem: T) -> void` | arr.fill(0) | memset | — | 用指定值填充所有元素 | 将数组所有元素设为指定值 | ✅ 已落地 |
| `array.index` | 内建库 | `(elem: T) -> int` | arr.index(5) | — | — | 返回元素首次出现的索引，未找到返回 -1 | 线性查找元素索引 | ✅ 已落地 |
| `array.insert` | 内建库 | `(index: int, elem: T) -> void` | arr.insert(0, 1) | memmove | append | 在指定索引处插入元素 | 在索引处插入元素，后续元素后移 | ✅ 已落地 |
| `array.len` | 内建库 | `() -> int` | arr.len() | — | len | 返回数组元素个数 | 对标 C sizeof/arr_len，Go 内置 len()；返回元素数量 | ✅ 已落地 |
| `array.max` | 内建库 | `() -> T` | arr.max() | — | — | 返回数组中的最大值 | 仅适用于可比较类型（int/float 等） | ✅ 已落地 |
| `array.merge` | 内建库 | `(other: T[]) -> void` | arr.merge(other) | memcpy | append | 将另一个数组合并到当前数组 | 将其他数组的所有元素追加到末尾 | ✅ 已落地 |
| `array.min` | 内建库 | `() -> T` | arr.min() | — | — | 返回数组中的最小值 | 仅适用于可比较类型（int/float 等） | ✅ 已落地 |
| `array.pop` | 内建库 | `() -> T?` | arr.pop() | — | — | 移除并返回数组最后一个元素 | C 标准库无直接对应，Go 亦无标准库函数；返回 T? 可选类型 | ✅ 已落地 |
| `array.push` | 内建库 | `(elem: T) -> void` | arr.push(1) | realloc/memcpy | append | 在数组末尾追加元素 | 对标 C realloc+memcpy 扩容，Go 内置 append()；动态数组扩容 | ✅ 已落地 |
| `array.remove` | 内建库 | `(index: int) -> void` | arr.remove(0) | memmove | — | 按索引移除数组元素 | 对标 C memmove 后移元素，Go 需手动切片删除；索引越界可能 panic | ✅ 已落地 |
| `array.resize` | 内建库 | `(n: int) -> void` | arr.resize(10) | realloc | — | 调整数组长度 | 不足时零初始化，超出时截断 | ✅ 已落地 |
| `array.reverse` | 内建库 | `() -> void` | arr.reverse() | — | sort.Reverse | 将数组元素反转 | 原地反转数组元素顺序 | ✅ 已落地 |
| `array.sort` | 内建库 | `() -> void` | arr.sort() | qsort | sort.Ints | 对数组元素升序排序 | 对标 C qsort，Go sort.Ints；原地排序 | ✅ 已落地 |
| `array.sort` | 内建库 | `(cmp) -> void` | arr.sort(func(a, b int) bool { ... }) | qsort_r | sort.Slice | 按自定义比较函数对数组排序 | 支持自定义比较函数，但 axlang 不支持闭包，实际受限 | ✅ 已落地 |
| `array.sum` | 内建库 | `() -> T` | arr.sum() | — | — | 返回数组所有元素之和 | 仅适用于数值类型（int/float 等） | ✅ 已落地 |
| `array.to_string` | 内建库 | `() -> string` | arr.to_string() | — | fmt.Sprintf | 将数组转换为字符串表示 | C 标准库无直接对应，Go 对标 fmt.Sprintf('%v', slice)；格式如 [1 2 3] | ✅ 已落地 |
| `array.切片` | 内建库 | `[lo..hi] / [lo..=hi]` | arr[1..3] | — | 切片表达式 s[lo:hi] | 数组切片操作，获取子数组视图 | C 标准库无直接对应（指针偏移），Go 切片表达式；支持 [lo..hi] 和 [lo..=hi] 两种语法 | ✅ 已落地 |
| `atomic.add_int` | 标准库 | `(addr: *int, delta: int) -> int` | — | atomic_fetch_add | atomic.AddInt64 | 原子地将增量加到整数地址并返回原值 | 对标 C atomic_fetch_add（<stdatomic.h>），Go sync/atomic.AddInt64；基于 C11 原子类型 | ✅ 已落地 |
| `atomic.compare_and_swap_int` | 标准库 | `(addr: *int, old: int, new: int) -> bool` | — | atomic_compare_exchange | atomic.CompareAndSwapInt64 | 原地比较并交换：若当前值等于 old 则更新为 new | 对标 C atomic_compare_exchange_strong，Go sync/atomic.CompareAndSwapInt64；基于 C11 原子类型 | ✅ 已落地 |
| `atomic.load_int` | 标准库 | `(addr: *int) -> int` | — | atomic_load | atomic.LoadInt64 | 原子地读取整数地址的值 | 对标 C atomic_load，Go sync/atomic.LoadInt64；基于 C11 原子类型 | ✅ 已落地 |
| `atomic.store_int` | 标准库 | `(addr: *int, val: int) -> void` | — | atomic_store | atomic.StoreInt64 | 原子地将值写入整数地址 | 对标 C atomic_store，Go sync/atomic.StoreInt64；基于 C11 原子类型 | ✅ 已落地 |
| `bufio.new_reader` | 标准库 | `(r: File) -> File` | — | — | bufio.NewReader | 创建带缓冲的文件读取器 | C 标准库无直接对应，Go 对标 bufio.NewReader；基于 File 句柄 ax_file* | ✅ 已落地 |
| `bufio.new_writer` | 标准库 | `(w: File) -> File` | — | — | bufio.NewWriter | 创建带缓冲的文件写入器 | C 标准库无直接对应，Go 对标 bufio.NewWriter；基于 File 句柄 ax_file* | ✅ 已落地 |
| `bufio.scanner` | 标准库 | `(r: File) -> File` | — | — | bufio.NewScanner | 创建扫描器，按行或自定义分隔符读取 | C 标准库无直接对应，Go 对标 bufio.NewScanner；基于 File 句柄 ax_file* | ✅ 已落地 |
| `print` | 内建库 | `(args: any...) -> void` | print("hello", 42, true) | printf | fmt.Print | 输出任意参数到标准输出，参数间自动添加空格 | axlang 语言内建原语，长期保留在编译器 Rust 端；对标 C printf，Go fmt.Print | ✅ 已落地 |
| `println` | 内建库 | `(args: any...) -> void` | println("hello", "world") | printf+putc | fmt.Println | 输出任意参数到标准输出并换行，参数间自动添加空格 | axlang 语言内建原语，长期保留在编译器 Rust 端；对标 C printf+putc，Go fmt.Println | ✅ 已落地 |
| `sizeof` | 内建库 | `(T) -> int` | sizeof(int) | sizeof | unsafe.Sizeof | 返回类型或表达式的字节大小 | axlang 语言内建原语，长期保留在编译器 Rust 端；对标 C sizeof，Go unsafe.Sizeof；编译期求值 | ✅ 已落地 |
| `typeof` | 内建库 | `(v: any) -> string` | typeof(42) | — | reflect.TypeOf | 返回值的类型名称字符串 | axlang 语言内建原语，长期保留在编译器 Rust 端；对标 Go reflect.TypeOf；无运行时反射，编译期求值 | ✅ 已落地 |
| `bytes.contains` | 标准库 | `(s: bytes, sub: bytes) -> bool` | — | strstr/memmem | bytes.Contains | 判断字节切片是否包含子字节切片 | 对标 C memmem（非标准）或 strstr，Go 对标 bytes.Contains；字节级查找 | ✅ 已落地 |
| `bytes.equal` | 标准库 | `(a: bytes, b: bytes) -> bool` | — | memcmp | bytes.Equal | 判断两个字节切片是否相等 | 对标 C memcmp，Go 对标 bytes.Equal；长度和内容完全相同即相等 | ✅ 已落地 |
| `bytes.index` | 标准库 | `(s: bytes, sub: bytes) -> int` | — | strstr/memmem | bytes.Index | 返回子字节切片首次出现的索引，未找到返回 -1 | 对标 C memmem（非标准），Go 对标 bytes.Index；字节级查找 | ✅ 已落地 |
| `bytes.repeat` | 标准库 | `(s: bytes, count: int) -> bytes` | — | — | bytes.Repeat | 将字节切片重复 count 次 | C 标准库无直接对应，Go 对标 bytes.Repeat；返回新的重复后的字节切片 | ✅ 已落地 |
| `bytes.split` | 标准库 | `(s: bytes, sep: bytes) -> bytes[]` | — | strtok | bytes.Split | 按分隔符分割字节切片 | 对标 C strtok，Go 对标 bytes.Split；分隔符本身不保留，返回 bytes[] 切片 | ✅ 已落地 |
| `bytes.to_lower` | 标准库 | `(s: bytes) -> bytes` | — | tolower | bytes.ToLower | 将字节切片转换为小写 | 对标 C tolower（逐字节），Go 对标 bytes.ToLower；仅 ASCII 字母 | ✅ 已落地 |
| `bytes.to_upper` | 标准库 | `(s: bytes) -> bytes` | — | toupper | bytes.ToUpper | 将字节切片转换为大写 | 对标 C toupper（逐字节），Go 对标 bytes.ToUpper；仅 ASCII 字母 | ✅ 已落地 |
| `bytes.trim_space` | 标准库 | `(s: bytes) -> bytes` | — | — | bytes.TrimSpace | 去除字节切片首尾空白字符 | C 标准库无直接对应，Go 对标 bytes.TrimSpace；空白包括空格、制表符、换行符等 | ✅ 已落地 |
| `csv.read_all` | 标准库 | `(r: Reader) -> string[][]` | — | — | csv.Reader.ReadAll | 从 CSV 阅读器读取所有记录为二维字符串切片 | C 标准库无 CSV 解析器，Go 对标 encoding/csv.Reader.ReadAll；返回 string[][] | ✅ 已落地 |
| `csv.write` | 标准库 | `(w: Writer, record: string[]) -> int` | — | — | csv.Writer.Write | 将一条记录写入 CSV 文件 | C 标准库无 CSV 写入器，Go 对标 encoding/csv.Writer.Write；返回写入状态 | ✅ 已落地 |
| `db.query` | 内建库 | `(sql: string) -> result<Row>` | db.query("SELECT * FROM t") | — | — | 执行 SQL 查询并返回结果集 | axlang 特有模块，无标准 C/Go 对应；依赖底层数据库驱动实现 | ⬜ 计划中 |
| `decimal.divide` | 内建库 | `(other: decimal, rounding: Rounding) -> decimal` | d.divide(e, Rounding.HalfUp) | — | — | 高精度除法，支持自定义舍入模式 | 对标 C 无直接对应，需自定义 decimal 实现 | ✅ 已落地 |
| `decimal.parse` | 内建库 | `(s: string) -> result<decimal>` | decimal.parse("123.45") | ax_dec_parse | — | 将字符串解析为高精度十进制数 | axlang 特有模块，无标准 C/Go 对应；依赖自定义 decimal 实现 | ✅ 已落地 |
| `decimal.round` | 内建库 | `(digits: int, rounding: Rounding) -> decimal` | d.round(2, Rounding.HalfUp) | — | — | 按指定精度和舍入模式四舍五入 | 对标 C 无直接对应，需自定义 decimal 实现 | ✅ 已落地 |
| `decimal.to_string` | 内建库 | `() -> string` | d.to_string() | — | — | 将 decimal 转换为字符串表示 | 对标 C sprintf，Go fmt.Sprintf | ✅ 已落地 |
| `decimal.truncate` | 内建库 | `(v: int) -> int8` | d.truncate(8) | ax_narrow(ax_dec_to_i64) | — | 将十进制数截断为整数（int8/int16/int32） | axlang 特有模块，无标准 C/Go 对应；重载支持三种整数宽度 | ✅ 已落地 |
| `decimal.truncate` | 内建库 | `(v: int) -> int16` | d.truncate(16) | ax_narrow(ax_dec_to_i64) | — | 将十进制数截断为整数（int8/int16/int32） | axlang 特有模块，无标准 C/Go 对应；重载支持三种整数宽度 | ✅ 已落地 |
| `decimal.truncate` | 内建库 | `(v: int) -> int32` | d.truncate(32) | ax_narrow(ax_dec_to_i64) | — | 将十进制数截断为整数（int8/int16/int32） | axlang 特有模块，无标准 C/Go 对应；重载支持三种整数宽度 | ✅ 已落地 |
| `errors.as` | 标准库 | `(err: error, target: error) -> bool` | — | — | errors.As | 将错误解析为目标类型 | C 标准库无 error 类型，Go 对标 errors.As；axlang 无运行时反射，target 限定为 error 类型 | ✅ 已落地 |
| `errors.is` | 标准库 | `(err: error, target: error) -> bool` | — | — | errors.Is | 判断错误是否与目标错误匹配（支持链式包装） | C 标准库无 error 类型，Go 对标 errors.Is；axlang 无运行时反射，target 限定为 error 类型 | ✅ 已落地 |
| `errors.new` | 标准库 | `(msg: string) -> error` | — | — | errors.New | 根据消息字符串创建 error 对象 | C 标准库无 error 类型，Go 对标 errors.New；返回 axlang error 类型 | ✅ 已落地 |
| `file.append` | 标准库 | `(path: string) -> result<File>` | — | fopen(a) | os.OpenFile(append) | 以追加模式打开文件 | 对标 C fopen(a 模式)，Go os.OpenFile 追加标志 | ✅ 已落地 |
| `file.chmod` | 标准库 | `(path: string, mode: int) -> int` | — | chmod | os.Chmod | 修改文件权限模式 | 对标 C chmod，Go os.Chmod | ✅ 已落地 |
| `file.chtimes` | 标准库 | `(path: string, atime: int, mtime: int) -> int` | — | utime | os.Chtimes | 修改文件访问时间和修改时间 | 对标 C utime，Go os.Chtimes | ✅ 已落地 |
| `file.create` | 标准库 | `(path: string) -> result<File>` | — | fopen(w) | os.Create | 创建或截断文件并返回句柄 | 对标 C fopen(w 模式)，Go os.Create | ✅ 已落地 |
| `file.exists` | 标准库 | `(path: string) -> int` | — | access/stat | os.Stat | 判断路径是否存在 | 对标 C access/stat，Go os.Stat；返回 1 表示存在，0 表示不存在 | ✅ 已落地 |
| `file.is_dir` | 标准库 | `(path: string) -> int` | — | stat/S_ISDIR | os.IsDir/Stat.IsDir | 判断路径是否为目录 | 对标 C stat+S_ISDIR 宏，Go os.IsDir 或 os.Stat().IsDir() | ✅ 已落地 |
| `file.is_file` | 标准库 | `(path: string) -> int` | — | stat/S_ISREG | Stat.IsRegular | 判断路径是否为普通文件 | 对标 C stat+S_ISREG 宏，Go os.FileInfo.IsRegular() | ✅ 已落地 |
| `file.mkdir` | 标准库 | `(path: string) -> int` | — | mkdir | os.Mkdir | 创建目录 | 对标 C mkdir，Go os.Mkdir | ✅ 已落地 |
| `file.mod_time` | 标准库 | `(path: string) -> int` | — | stat/st_mtime | Stat.ModTime | 获取文件最后修改时间 | 对标 C stat 结构体 st_mtime 字段，Go os.FileInfo.ModTime() | ✅ 已落地 |
| `file.open` | 标准库 | `(path: string) -> result<File>` | — | fopen | os.Open | 打开文件并返回文件句柄 | 对标 C fopen，Go os.Open；返回 result<File> 类型 | ✅ 已落地 |
| `file.read_all` | 标准库 | `(path: string) -> result<string>` | — | fread | os.ReadFile | 读取文件全部内容并返回字符串 | 对标 C fread，Go os.ReadFile；返回 result<string> | ✅ 已落地 |
| `file.remove` | 标准库 | `(path: string) -> int` | — | remove | os.Remove | 删除文件或目录 | 对标 C remove，Go os.Remove | ✅ 已落地 |
| `file.rename` | 标准库 | `(old: string, new: string) -> int` | — | rename | os.Rename | 重命名或移动文件 | 对标 C rename，Go os.Rename | ✅ 已落地 |
| `file.size` | 标准库 | `(path: string) -> int` | — | stat/st_size | Stat.Size | 获取文件大小（字节） | 对标 C stat 结构体 st_size 字段，Go os.FileInfo.Size() | ✅ 已落地 |
| `file.temp_file` | 标准库 | `(dir: string, prefix: string) -> result<File>` | — | tmpfile | os.CreateTemp | 创建临时文件并返回句柄 | 对标 C tmpfile，Go os.CreateTemp；自动生成唯一临时文件名 | ✅ 已落地 |
| `file.write_all` | 标准库 | `(path: string, s: string) -> int` | — | fwrite | os.WriteFile | 将字符串全部写入文件 | 对标 C fwrite，Go os.WriteFile；返回写入字节数 | ✅ 已落地 |
| `filepath.abs` | 标准库 | `(path: string) -> string` | — | realpath | filepath.Abs | 返回路径的绝对形式 | 对标 C realpath，Go 对标 filepath.Abs；基于当前工作目录解析相对路径 | ✅ 已落地 |
| `filepath.base` | 标准库 | `(path: string) -> string` | — | basename | filepath.Base | 返回路径的最后一个元素（文件名） | 对标 C basename 命令/函数，Go 对标 filepath.Base；去掉目录部分 | ✅ 已落地 |
| `filepath.clean` | 标准库 | `(path: string) -> string` | — | — | filepath.Clean | 规范化路径，删除冗余的斜杠和 '.' '..' | C 标准库无直接对应，Go 对标 filepath.Clean；返回最短等价路径 | ✅ 已落地 |
| `filepath.dir` | 标准库 | `(path: string) -> string` | — | dirname | filepath.Dir | 返回路径的目录部分 | 对标 C dirname 命令/函数，Go 对标 filepath.Dir；去掉最后一个元素 | ✅ 已落地 |
| `filepath.eval_symlinks` | 标准库 | `(path: string) -> string` | — | realpath | filepath.EvalSymlinks | 解析符号链接，返回规范路径 | 对标 C realpath，Go 对标 filepath.EvalSymlinks；递归解析所有软链接 | ✅ 已落地 |
| `filepath.ext` | 标准库 | `(path: string) -> string` | — | — | filepath.Ext | 返回文件扩展名（包含点） | C 标准库无直接对应，Go 对标 filepath.Ext；返回 '.ext' 格式，无扩展名返回空字符串 | ✅ 已落地 |
| `filepath.glob` | 标准库 | `(pattern: string) -> string[]` | — | glob | filepath.Glob | 按模式匹配文件名，返回匹配的字符串切片 | 对标 C glob，Go 对标 filepath.Glob；支持 * ? [...] 通配符 | ✅ 已落地 |
| `filepath.is_abs` | 标准库 | `(path: string) -> bool` | — | — | filepath.IsAbs | 判断路径是否为绝对路径 | C 标准库无直接对应，Go 对标 filepath.IsAbs；检查是否以根目录开头 | ✅ 已落地 |
| `filepath.join` | 标准库 | `(parts: string[]) -> string` | — | — | filepath.Join | 将多个路径片段连接为单个路径 | C 标准库无直接对应，Go 对标 filepath.Join；自动处理平台分隔符 | ✅ 已落地 |
| `filepath.rel` | 标准库 | `(basepath, targpath: string) -> string` | — | — | filepath.Rel | 返回从 basepath 到 targpath 的相对路径 | C 标准库无直接对应，Go 对标 filepath.Rel；计算两个路径间的相对关系 | ✅ 已落地 |
| `filepath.split` | 标准库 | `(path: string) -> string[2]` | — | — | filepath.Split | 将路径拆分为目录和文件名两部分 | C 标准库无直接对应，Go 对标 filepath.Split；返回 string[2] 数组 | ✅ 已落地 |
| `filepath.walk` | 标准库 | `(root: string, fn: func) -> int` | — | nftw | filepath.Walk | 递归遍历目录树，对每个文件调用回调 | 对标 C nftw，Go 对标 filepath.Walk；axlang 不支持闭包，实际受限 | ✅ 已落地 |
| `fmt.errorf` | 标准库 | `(format: string, args: any...) -> error` | — | fprintf(stderr) | fmt.Errorf | 格式化错误信息并返回 error 类型 | 对标 C fprintf(stderr)，Go fmt.Errorf；用于构造错误对象 | ✅ 已落地 |
| `fmt.fprintf` | 标准库 | `(w: File, format: string, args: any...) -> int` | — | fprintf | fmt.Fprintf | 将格式化内容写入文件句柄 | 对标 C fprintf，Go fmt.Fprintf；第一个参数为文件对象 | ✅ 已落地 |
| `fmt.print` | 标准库 | `(args: any...) -> void` | — | printf | fmt.Print | 格式化输出任意参数到标准输出 | 对标 C printf，Go fmt.Print；参数间自动添加空格 | ✅ 已落地 |
| `fmt.printf` | 标准库 | `(format: string, args: any...) -> void` | — | printf | fmt.Printf | 按格式字符串格式化输出 | 对标 C printf，Go fmt.Printf；编译期核对占位符 | ✅ 已落地 |
| `fmt.println` | 标准库 | `(args: any...) -> void` | — | printf/puts | fmt.Println | 格式化输出并换行 | 对标 C printf/puts，Go fmt.Println；自动换行 | ✅ 已落地 |
| `fmt.scanf` | 标准库 | `(format: string, args: any...) -> int` | — | scanf | fmt.Scan | 从标准输入读取并按格式解析 | 对标 C scanf，Go fmt.Scan；从 stdin 读取格式化输入 | ✅ 已落地 |
| `fmt.sprint` | 标准库 | `(args: any...) -> string` | — | snprintf/sprintf | fmt.Sprint | 将任意参数格式化为字符串返回 | 对标 C snprintf，Go fmt.Sprint；不输出，仅返回字符串 | ✅ 已落地 |
| `fmt.sprintf` | 标准库 | `(format: string, args: any...) -> string` | — | sprintf | fmt.Sprintf | 按格式字符串格式化并返回字符串 | 对标 C sprintf，Go fmt.Sprintf；结果以字符串返回而非输出 | ✅ 已落地 |
| `int/float/bool/enum.divide` | 内建库 | `(other: T) -> T` | x.divide(y) | / | — | 除法运算（泛型） | C 标准库 / 运算符，Go / 运算符；axlang 语言级泛型方法 | ✅ 已落地 |
| `int/float/bool/enum.round` | 内建库 | `(digits: int) -> float` | x.round(2) | round/pow | math.Round | 将浮点数四舍五入到指定小数位数 | 对标 C round/pow，Go math.Round；仅 Float 类型，digits 为小数位数 | ✅ 已落地 |
| `int/float/bool/enum.to_string` | 内建库 | `() -> string` | x.to_string() | ax_str_of_int/ax_str_of_f64/ax_str_of_bool/enum_name | strconv.Itoa/FormatFloat | 将数值转换为字符串 | C 标准库需 sprintf/itoa，Go 对标 strconv 包；泛型类型方法 | ✅ 已落地 |
| `int/float/bool/enum.wrapping_add` | 内建库 | `(other: T) -> T` | x.wrapping_add(y) | uint64_t 溢出回绕 | — | 带环绕的加法（溢出时回绕） | C 标准库无直接对应，Go 亦无标准库函数；axlang 语言级泛型方法 | ✅ 已落地 |
| `io.read_file` | 标准库 | `(path: string) -> result<string>` | — | fread | os.ReadFile | 读取整个文件内容并返回字符串 | 对标 C fread，Go os.ReadFile；一次性读取全部文件到内存 | ✅ 已落地 |
| `io.read_line` | 标准库 | `() -> string?` | — | fgets | bufio.Scanner.Text | 逐行读取文件内容，返回一行字符串 | 对标 C fgets，Go bufio.Scanner.Text；读取直到换行符或文件结束 | ✅ 已落地 |
| `io.write` | 标准库 | `(s: string) -> void` | — | fwrite/puts | fmt.Fprint/io.WriteString | 将字符串写入标准输出 | 对标 C fwrite/puts，Go fmt.Fprint；不自动换行 | ✅ 已落地 |
| `io.write_line` | 标准库 | `(s: string) -> void` | — | fputs/puts | fmt.Fprintln | 将字符串写入标准输出并换行 | 对标 C fputs/puts，Go fmt.Fprintln；自动追加换行符 | ✅ 已落地 |
| `json.marshal_indent` | 标准库 | `(v: object, prefix: string, indent: string) -> string` | — | — | json.MarshalIndent | 将对象序列化为带缩进的 JSON 字符串 | C 标准库无对应，Go 对标 encoding/json.MarshalIndent | ✅ 已落地 |
| `json.parse` | 标准库 | `(s: string) -> result<map<string, object>>` | — | — | json.Unmarshal | 将 JSON 字符串解析为 map<string, object> | C 标准库无 JSON 解析器，Go 对标 encoding/json.Unmarshal | ✅ 已落地 |
| `json.stringify` | 标准库 | `(v: object) -> string` | — | — | json.Marshal | 将对象序列化为 JSON 字符串 | C 标准库无 JSON 序列化，Go 对标 encoding/json.Marshal | ✅ 已落地 |
| `json.valid` | 标准库 | `(s: string) -> bool` | — | — | json.Valid | 判断字符串是否为合法 JSON | C 标准库无对应，Go 对标 encoding/json.Valid | ✅ 已落地 |
| `log.fatal` | 内建库 | `(v: any...) -> void` | log.fatal("error", err) | printf+exit | log.Fatal | 输出致命日志并退出程序 | 对标 C printf+exit(1)，Go log.Fatal；输出后调用 os.Exit(1) 终止程序 | ✅ 已落地 |
| `log.print` | 内建库 | `(v: any...) -> void` | log.print("info", "started") | printf/fputs | log.Print | 输出日志到标准输出 | 对标 C printf/fputs，Go log.Print；输出带时间戳的普通日志 | ✅ 已落地 |
| `log.printf` | 内建库 | `(format: string, v: any...) -> void` | log.printf("count=%d", n) | printf/fprintf | log.Printf | 按格式输出格式化日志 | 对标 C printf/fprintf，Go log.Printf；支持格式化占位符 | ✅ 已落地 |
| `map.!=` | 内建库 | `(other: Map) -> bool` | m1 != m2 | — | — | 判断两个 map 是否不相等 | C 标准库无 map 类型，Go 无直接对应；等价于 !(m1 == m2) | ✅ 已落地 |
| `map.==` | 内建库 | `(other: Map) -> bool` | m1 == m2 | — | maps.Equal | 判断两个 map 是否相等（相同键值对） | C 标准库无 map 类型，Go 对标 maps.Equal；键值对完全相同即相等 | ✅ 已落地 |
| `map.clear` | 内建库 | `() -> void` | m.clear() | — | maps.Clear | 清空 map 中所有键值对 | C 标准库无 map 类型，Go 对标 maps.Clear（Go 1.21+）；删除所有条目 | ✅ 已落地 |
| `map.get` | 内建库 | `(key: K) -> V?` | m.get("key") | — | — | 按键获取 map 值，未找到返回 null | 安全访问 map 元素，键不存在返回 V? 可选类型 | ✅ 已落地 |
| `map.has` | 内建库 | `(key: K) -> bool` | m.has("key") | — | — | 判断 map 是否包含指定键 | 检查 map 中是否存在指定键 | ✅ 已落地 |
| `map.keys` | 内建库 | `() -> K[]` | m.keys() | — | maps.Keys | 返回 map 中所有键的切片 | C 标准库无 map 类型，Go 对标 maps.Keys；返回 K[] 类型，顺序不固定 | ✅ 已落地 |
| `map.len` | 内建库 | `() -> int` | m.len() | — | len | 返回 map 的键值对数量 | 对标 C 无直接对应，Go 内置 len() | ✅ 已落地 |
| `map.remove` | 内建库 | `(key: K) -> void` | m.remove("key") | — | delete/maps.Delete | 删除 map 中指定键的条目 | C 标准库无 map 类型，Go 对标 delete() 内置或 maps.Delete；键不存在时不报错 | ✅ 已落地 |
| `maps.clone` | 内建库 | `(m: Map) -> Map` | maps.clone(m) | — | maps.Clone | 返回 map 的深拷贝副本 | C 标准库无 map 类型，Go 对标 maps.Clone；返回独立的新 map | ✅ 已落地 |
| `maps.copy` | 内建库 | `(dst: Map, src: Map) -> void` | maps.copy(dst, src) | — | maps.Copy | 将 src map 的所有键值对复制到 dst map | C 标准库无 map 类型，Go 对标 maps.Copy；覆盖 dst 中已有的键 | ✅ 已落地 |
| `maps.delete` | 内建库 | `(m: Map, key: K) -> void` | maps.delete(m, key) | — | delete/maps.Delete | 删除 map 中指定键的条目 | C 标准库无 map 类型，Go 对标 delete() 内置或 maps.Delete；键不存在时不报错 | ✅ 已落地 |
| `maps.delete_func` | 内建库 | `(m: Map, f: func(K) bool) -> void` | maps.delete_func(m, func(k) bool { ... }) | — | maps.DeleteFunc | 删除满足条件的所有键值对 | C 标准库无 map 类型，Go 对标 maps.DeleteFunc；axlang 不支持闭包，暂不落地 | ⬜ 未落地 |
| `maps.equal` | 内建库 | `(m1: Map, m2: Map) -> bool` | maps.equal(m1, m2) | — | maps.Equal | 判断两个 map 是否相等（相同键值对） | C 标准库无 map 类型，Go 对标 maps.Equal；键值对完全相同即相等 | ✅ 已落地 |
| `maps.keys` | 内建库 | `(m: Map) -> K[]` | maps.keys(m) | — | maps.Keys | 返回 map 中所有键的切片 | C 标准库无 map 类型，Go 对标 maps.Keys；返回 K[] 类型，顺序不固定 | ✅ 已落地 |
| `maps.values` | 内建库 | `(m: Map) -> V[]` | maps.values(m) | — | maps.Values | 返回 map 中所有值的切片 | C 标准库无 map 类型，Go 对标 maps.Values；返回 V[] 类型，顺序不固定 | ✅ 已落地 |
| `math.abs` | 标准库 | `(x: float) -> float` | — | fabs | math.Abs | 求浮点数的绝对值 | 对标 C fabs，Go math.Abs；返回 \|x\| | ✅ 已落地 |
| `math.acos` | 标准库 | `(x: float) -> float` | — | acos | math.Acos | 反余弦（arc cosine） | 对标 C acos，Go math.Acos；返回弧度 | ✅ 已落地 |
| `math.asin` | 标准库 | `(x: float) -> float` | — | asin | math.Asin | 反正弦（arc sine） | 对标 C asin，Go math.Asin；返回弧度 | ✅ 已落地 |
| `math.atan` | 标准库 | `(x: float) -> float` | — | atan | math.Atan | 反正切（arc tangent） | 对标 C atan，Go math.Atan；返回弧度 | ✅ 已落地 |
| `math.atan2` | 标准库 | `(y: float, x: float) -> float` | — | atan2 | math.Atan2 | 带符号的反正切 atan2(y, x) | 对标 C atan2，Go math.Atan2；考虑象限 | ✅ 已落地 |
| `math.cbrt` | 标准库 | `(x: float) -> float` | — | cbrt | math.Cbrt | 求立方根 | 对标 C cbrt，Go math.Cbrt | ✅ 已落地 |
| `math.ceil` | 标准库 | `(x: float) -> float` | — | ceil | math.Ceil | 向上取整 | 对标 C ceil，Go math.Ceil；返回不小于 x 的最小整数 | ✅ 已落地 |
| `math.compare` | 标准库 | `(a: float, b: float) -> int` | — | — | — | 比较两个浮点数大小（-1/0/1） | C/Go 标准库无直接对应函数，需手动比较 | ✅ 已落地 |
| `math.copysign` | 标准库 | `(f: float, sign: float) -> float` | — | copysign | math.Copysign | 返回 x 的幅值和 y 的符号组成的浮点数 | 对标 C copysign，Go math.Copysign | ✅ 已落地 |
| `math.cos` | 标准库 | `(x: float) -> float` | — | cos | math.Cos | 余弦函数 | 对标 C cos，Go math.Cos；参数单位为弧度 | ✅ 已落地 |
| `math.cosh` | 标准库 | `(x: float) -> float` | — | cosh | math.Cosh | 双曲余弦 | 对标 C cosh，Go math.Cosh | ✅ 已落地 |
| `math.dim` | 标准库 | `(x: float, y: float) -> float` | — | fdim | math.Dim | 返回 max(x - y, 0)，即正差值 | 对标 C fdim，Go math.Dim | ✅ 已落地 |
| `math.equals` | 标准库 | `(a: float, b: float, eps: float) -> bool` | — | — | — | 在容差范围内判断两个浮点数是否相等 | C/Go 标准库无直接对应函数，需按 \|a-b\| <= eps 自行判断 | ✅ 已落地 |
| `math.exp` | 标准库 | `(x: float) -> float` | — | exp | math.Exp | 求自然指数 e^x | 对标 C exp，Go math.Exp | ✅ 已落地 |
| `math.exp2` | 标准库 | `(x: float) -> float` | — | exp2 | — | 求以 2 为底的指数 2^x | 对标 C exp2，Go 标准库无 math.Exp2 | ✅ 已落地 |
| `math.floor` | 标准库 | `(x: float) -> float` | — | floor | math.Floor | 向下取整 | 对标 C floor，Go math.Floor；返回不大于 x 的最大整数 | ✅ 已落地 |
| `math.frexp` | 标准库 | `(f: float) -> (frac: float, exp: int)` | — | frexp | math.Frexp | 将浮点数分解为尾数和 2 的幂指数 | 对标 C frexpf，Go math.Frexp；axlang 暂不支持指针参数，分两步调用 ax_math_frexp / ax_math_frexp_exp | ✅ 已落地 |
| `math.hypot` | 标准库 | `(p: float, q: float) -> float` | — | hypot | math.Hypot | 求斜边长 sqrt(x^2+y^2)，避免溢出 | 对标 C hypot，Go math.Hypot | ✅ 已落地 |
| `math.is_inf` | 标准库 | `(f: float, sign: int) -> bool` | — | isinf | math.IsInf | 判断是否为无穷大，sign 参数控制正/负/任意 | 对标 C isinf，Go math.IsInf；sign>0 只判 +Inf，sign<0 只判 -Inf，sign==0 判任意 | ✅ 已落地 |
| `math.is_na_n` | 标准库 | `(f: float) -> bool` | — | isnan | math.IsNaN | 判断是否为非数（NaN） | 对标 C isnan，Go math.IsNaN；axlang 将 IsNaN 转为 snake_case 后名称为 is_na_n | ✅ 已落地 |
| `math.ldexp` | 标准库 | `(frac: float, exp: int) -> float` | — | ldexp | math.Ldexp | 计算 x * 2^exp，即从尾数和指数重建浮点数 | 对标 C ldexp，Go math.Ldexp；与 frexp 互逆 | ✅ 已落地 |
| `math.log` | 标准库 | `(x: float) -> float` | — | log | math.Log | 求自然对数 ln(x) | 对标 C log，Go math.Log；x 必须正数 | ✅ 已落地 |
| `math.log10` | 标准库 | `(x: float) -> float` | — | log10 | math.Log10 | 求以 10 为底的对数 | 对标 C log10，Go math.Log10 | ✅ 已落地 |
| `math.log1p` | 标准库 | `(x: float) -> float` | — | log1p | math.Log1p | 求 ln(1+x)，适用于 x 很小时的精度保持 | 对标 C log1p，Go math.Log1p | ✅ 已落地 |
| `math.log2` | 标准库 | `(x: float) -> float` | — | log2 | math.Log2 | 求以 2 为底的对数 | 对标 C log2，Go math.Log2 | ✅ 已落地 |
| `math.max` | 标准库 | `(a: float, b: float) -> float` | — | fmax | math.Max | 返回两个浮点数中的较大值 | 对标 C fmax，Go math.Max | ✅ 已落地 |
| `math.min` | 标准库 | `(a: float, b: float) -> float` | — | fmin | math.Min | 返回两个浮点数中的较小值 | 对标 C fmin，Go math.Min | ✅ 已落地 |
| `math.mod` | 标准库 | `(x: float, y: float) -> float` | — | fmod | math.Mod | 浮点数取余（结果符号同被除数） | 对标 C fmod，Go math.Mod；y 为 0 时返回 NaN | ✅ 已落地 |
| `math.pow` | 标准库 | `(a: float, b: float) -> float` | — | pow | math.Pow | 求幂次 x^y | 对标 C pow，Go math.Pow | ✅ 已落地 |
| `math.round` | 标准库 | `(x: float) -> float` | — | round | math.Round | 四舍五入取整 | 对标 C round，Go math.Round；半远离零 | ✅ 已落地 |
| `math.sign` | 标准库 | `(x: float) -> int` | — | — | — | 返回数值的符号（正1/负-1/零0） | C 标准库无直接对应函数，需自行实现；Go 亦无 math.Sign | ✅ 已落地 |
| `math.signbit` | 标准库 | `(f: float) -> bool` | — | signbit | math.Signbit | 判断浮点数符号位是否为负（含 -0 / -NaN） | 对标 C signbit，Go math.Signbit | ✅ 已落地 |
| `math.sin` | 标准库 | `(x: float) -> float` | — | sin | math.Sin | 正弦函数 | 对标 C sin，Go math.Sin；参数单位为弧度 | ✅ 已落地 |
| `math.sinh` | 标准库 | `(x: float) -> float` | — | sinh | math.Sinh | 双曲正弦 | 对标 C sinh，Go math.Sinh | ✅ 已落地 |
| `math.sqrt` | 标准库 | `(x: float) -> float` | — | sqrt | math.Sqrt | 求平方根 | 对标 C sqrt，Go math.Sqrt；x 必须非负 | ✅ 已落地 |
| `math.tan` | 标准库 | `(x: float) -> float` | — | tan | math.Tan | 正切函数 | 对标 C tan，Go math.Tan；参数单位为弧度 | ✅ 已落地 |
| `math.tanh` | 标准库 | `(x: float) -> float` | — | tanh | math.Tanh | 双曲正切 | 对标 C tanh，Go math.Tanh | ✅ 已落地 |
| `math.trunc` | 标准库 | `(x: float) -> float` | — | trunc | math.Trunc | 向零取整（截断小数部分） | 对标 C trunc，Go math.Trunc | ✅ 已落地 |
| `os.args` | 标准库 | `() -> string[]` | — | argv | os.Args | 获取命令行参数字符串切片 | 对标 C main 的 argv 参数，Go os.Args；第一个元素通常为程序名 | ✅ 已落地 |
| `os.chdir` | 标准库 | `(path: string) -> int` | — | chdir | os.Chdir | 更改当前工作目录 | 对标 C chdir，Go os.Chdir；返回 0 表示成功，非 0 表示失败 | ✅ 已落地 |
| `os.environ` | 标准库 | `() -> string[]` | — | environ | os.Environ | 获取所有环境变量字符串切片 | 对标 C environ 全局变量，Go os.Environ；返回 'KEY=VALUE' 格式的字符串数组 | ✅ 已落地 |
| `os.executable` | 标准库 | `() -> string` | — | — | os.Executable | 获取当前可执行文件路径 | C 标准库无直接对应，Go 对标 os.Executable；返回程序本身的绝对路径 | ✅ 已落地 |
| `os.exit` | 标准库 | `(code: int) -> void` | — | exit | os.Exit | 终止当前进程并返回退出码 | 对标 C exit，Go os.Exit；立即终止程序，不执行 defer | ✅ 已落地 |
| `os.getegid` | 标准库 | `() -> int` | — | getegid | — | 获取有效组 ID（Unix 特有） | 对标 C getegid，Go 标准库无直接对应；Windows 不支持 | ✅ 已落地 |
| `os.getenv` | 标准库 | `(name: string) -> string` | — | getenv | os.Getenv | 获取指定环境变量的值 | 对标 C getenv，Go os.Getenv；环境变量不存在时返回空字符串 | ✅ 已落地 |
| `os.geteuid` | 标准库 | `() -> int` | — | geteuid | — | 获取有效用户 ID（Unix 特有） | 对标 C geteuid，Go 标准库无直接对应；Windows 不支持 | ✅ 已落地 |
| `os.getgid` | 标准库 | `() -> int` | — | getgid | — | 获取当前组 ID（Unix 特有） | 对标 C getgid，Go 标准库无直接对应；Windows 不支持 | ✅ 已落地 |
| `os.getgroups` | 标准库 | `() -> int[]` | — | getgroups | — | 获取当前用户所属的组 ID 列表 | 对标 C getgroups，Go 标准库无直接对应；返回 int[] 数组 | ✅ 已落地 |
| `os.getpagesize` | 标准库 | `() -> int` | — | sysconf(_SC_PAGESIZE) | — | 获取系统页面大小（字节） | 对标 C sysconf(_SC_PAGESIZE)，Go 标准库无直接对应；通常返回 4096 | ✅ 已落地 |
| `os.getpid` | 标准库 | `() -> int` | — | getpid | os.Getpid | 获取当前进程 ID | 对标 C getpid，Go os.Getpid；返回进程标识符（整数） | ✅ 已落地 |
| `os.getppid` | 标准库 | `() -> int` | — | getppid | os.Getppid | 获取父进程 ID | 对标 C getppid，Go os.Getppid；返回父进程标识符（整数） | ✅ 已落地 |
| `os.getuid` | 标准库 | `() -> int` | — | getuid | — | 获取当前用户 ID（Unix 特有） | 对标 C getuid，Go 标准库无直接对应；Windows 不支持 | ✅ 已落地 |
| `os.getwd` | 标准库 | `() -> string` | — | getcwd | os.Getwd | 获取当前工作目录路径 | 对标 C getcwd，Go os.Getwd；返回绝对路径字符串 | ✅ 已落地 |
| `os.hostname` | 标准库 | `() -> string` | — | gethostname | os.Hostname | 获取主机名 | 对标 C gethostname，Go os.Hostname；返回系统主机名字符串 | ✅ 已落地 |
| `os.setenv` | 标准库 | `(name: string, value: string) -> int` | — | setenv | os.Setenv | 设置环境变量 | 对标 C setenv，Go os.Setenv；返回 0 表示成功，非 0 表示失败 | ✅ 已落地 |
| `os.unsetenv` | 标准库 | `(name: string) -> int` | — | unsetenv | os.Unsetenv | 删除环境变量 | 对标 C unsetenv，Go os.Unsetenv；返回 0 表示成功，非 0 表示失败 | ✅ 已落地 |
| `os.user_cache_dir` | 标准库 | `() -> string` | — | — | os.UserCacheDir | 获取用户缓存目录路径 | C 标准库无直接对应，Go 对标 os.UserCacheDir；跨平台缓存目录 | ✅ 已落地 |
| `os.user_home_dir` | 标准库 | `() -> string` | — | getenv(HOME) | os.UserHomeDir | 获取当前用户的家目录路径 | 对标 C getenv('HOME')，Go os.UserHomeDir；Windows 对应 %USERPROFILE% | ✅ 已落地 |
| `queue.close` | 内建库 | `() -> void` | q.close() | — | — | 关闭队列 | 关闭队列，后续无法入队 | ✅ 已落地 |
| `queue.pop` | 内建库 | `() -> T?` | q.pop() | — | — | 将元素出队 | 从队列头部移除并返回元素，空队列返回 null | ✅ 已落地 |
| `queue.push` | 内建库 | `(v: T) -> void` | q.push(v) | — | — | 将元素入队 | 向队列尾部添加元素 | ✅ 已落地 |
| `queue.try_pop` | 内建库 | `() -> T?` | q.try_pop() | — | select/default | 非阻塞地从队列取出元素，空队列返回 T? 无值 | C 标准库无直接对应，Go 对标 select + default 语句；非阻塞尝试出队 | ✅ 已落地 |
| `rand.float` | 标准库 | `() -> float` | — | rand/drand48 | rand.Float64 | 返回 [0.0, 1.0) 范围内的随机浮点数 | 对标 C drand48，Go math/rand.Float64；伪随机数生成 | ✅ 已落地 |
| `rand.int` | 标准库 | `(n: int) -> int` | — | rand | rand.Intn | 返回 [0, n) 范围内的随机整数 | 对标 C rand，Go math/rand.Intn；n 必须为正数 | ✅ 已落地 |
| `rand.seed` | 标准库 | `(seed: int) -> void` | — | srand | rand.Seed | 设置随机数生成器种子 | 对标 C srand，Go math/rand.Seed；相同种子产生相同序列 | ✅ 已落地 |
| `range.step` | 内建库 | `(n: int) -> Range` | range(0, 10).step(2) | — | — | 创建带步进的范围迭代器 | C 标准库无直接对应，Go range 关键字；axlang 语言级特性，n 为步进值 | ✅ 已落地 |
| `reflect.num_field` | 内建库 | `(v: any) -> int` | reflect.num_field(v) | — | reflect.NumField | 返回结构体的字段数量 | C 标准库无运行时反射，Go 对标 reflect.NumField；axlang 无运行时类型反射，需设计可落地子集 | ✅ 已落地 |
| `reflect.type_of` | 内建库 | `(v: any) -> string` | reflect.type_of(v) | — | reflect.TypeOf | 返回任意值的类型名称字符串 | C 标准库无运行时反射，Go 对标 reflect.TypeOf；axlang 无运行时类型反射，需设计可落地子集 | ✅ 已落地 |
| `reflect.value_of` | 内建库 | `(v: any) -> any` | reflect.value_of(v) | — | reflect.ValueOf | 返回任意值的反射值对象 | C 标准库无运行时反射，Go 对标 reflect.ValueOf；axlang 无运行时类型反射，需设计可落地子集 | ⬜ 未落地 |
| `slices.clone` | 内建库 | `() -> T[]` | arr.clone() | memcpy | slices.Clone | 返回数组的深拷贝副本 | 对标 C memcpy，Go slices.Clone；返回独立的新数组，修改不影响原数组 | ✅ 已落地 |
| `slices.contains` | 内建库 | `(x: T) -> bool` | slices.contains(arr, x) | — | slices.Contains | 判断数组是否包含指定元素 | C 标准库无直接对应，Go 对标 slices.Contains；线性查找，O(n) | ✅ 已落地 |
| `slices.delete` | 内建库 | `(i, j) -> T[]` | arr.delete(i, j) | memmove | slices.Delete | 删除索引 [i, j) 范围内的元素 | C 标准库无直接对应，Go 对标 slices.Delete；后续元素前移，O(n)；底层 ax_arr_delete_range | ✅ 已落地 |
| `slices.fill` | 内建库 | `(x) -> void` | arr.fill(0) | memset | slices.Fill | 用指定值填充数组所有元素 | 对标 C memset，Go slices.Fill；将所有元素设为相同值，O(n) | ✅ 已落地 |
| `slices.index` | 内建库 | `(x: T) -> int` | slices.index(arr, x) | — | slices.Index | 返回元素在数组中的索引，未找到返回 -1 | C 标准库无直接对应，Go 对标 slices.Index；线性查找，O(n) | ✅ 已落地 |
| `slices.insert` | 内建库 | `(i, x) -> void` | arr.insert(0, x) | memmove | slices.Insert | 在指定索引位置插入元素 | C 标准库无直接对应，Go 对标 slices.Insert；后续元素后移，O(n) | ✅ 已落地 |
| `slices.max` | 内建库 | `() -> T` | arr.max() | — | slices.Max | 返回数组中的最大值 | C 标准库无直接对应，Go 对标 slices.Max；支持 int[]/float[]，遍历查找，O(n) | ✅ 已落地 |
| `slices.merge` | 内建库 | `(other) -> void` | arr.merge(other) | memcpy | slices.Concat | 将另一数组追加到当前数组末尾 | 对标 C memcpy 追加，Go 对标 slices.Concat；原地修改当前数组 | ✅ 已落地 |
| `slices.min` | 内建库 | `() -> T` | arr.min() | — | slices.Min | 返回数组中的最小值 | C 标准库无直接对应，Go 对标 slices.Min；支持 int[]/float[]，遍历查找，O(n) | ✅ 已落地 |
| `slices.resize` | 内建库 | `(n) -> void` | arr.resize(10) | realloc | — | 调整数组长度，扩容时填充零值 | 对标 C realloc，Go 标准库无直接对应；缩短时截断，扩长时补零 | ✅ 已落地 |
| `slices.reverse` | 内建库 | `() -> void` | arr.reverse() | — | slices.Reverse | 原地反转数组元素顺序 | C 标准库无直接对应，Go 对标 slices.Reverse；首尾元素交换，O(n/2) | ✅ 已落地 |
| `slices.sort` | 内建库 | `() -> void` | arr.sort() | qsort | slices.Sort | 对数组原地升序排序 | 对标 C qsort，Go slices.Sort；支持 int[]/string[] 默认升序，cmp 重载支持自定义比较 | ✅ 已落地 |
| `slices.sort` | 内建库 | `(cmp) -> void` | arr.sort() | qsort | slices.Sort | 对数组原地升序排序 | 对标 C qsort，Go slices.Sort；支持 int[]/string[] 默认升序，cmp 重载支持自定义比较 | ✅ 已落地 |
| `slices.sum` | 内建库 | `() -> T` | arr.sum() | — | — | 返回数组元素之和 | C 标准库无直接对应，Go 标准库无 slices.Sum；遍历累加，O(n) | ✅ 已落地 |
| `sort.float64s` | 内建库 | `(a: float[]) -> void` | sort.float64s(arr) | qsort | sort.Float64s | 对 float 数组升序排序 | 对标 C qsort，Go sort.Float64s；原地排序，默认升序 | ✅ 已落地 |
| `sort.ints` | 内建库 | `(a: int[]) -> void` | sort.ints(arr) | qsort | sort.Ints | 对 int 数组升序排序 | 对标 C qsort，Go sort.Ints；原地排序，默认升序 | ✅ 已落地 |
| `sort.ints_are_sorted` | 内建库 | `(a: int[]) -> bool` | sort.ints_are_sorted(arr) | — | sort.IntsAreSorted | 判断 int 数组是否已升序排序 | C 标准库无直接对应，Go 对标 sort.IntsAreSorted；遍历检查，O(n) | ✅ 已落地 |
| `sort.search_ints` | 内建库 | `(a: int[], x: int) -> int` | sort.search_ints(arr, 5) | bsearch | sort.SearchInts | 在已排序 int 数组中二分查找目标值 | 对标 C bsearch，Go sort.SearchInts；要求数组已排序，返回索引或插入点 | ✅ 已落地 |
| `sort.search_strings` | 内建库 | `(a: string[], x: string) -> int` | sort.search_strings(arr, "key") | bsearch | sort.SearchStrings | 在已排序 string 数组中二分查找目标字符串 | 对标 C bsearch，Go sort.SearchStrings；要求数组已排序，返回索引或插入点 | ✅ 已落地 |
| `sort.slice` | 内建库 | `(a: any[], less: func(i, j int) bool) -> void` | sort.slice(arr, less) | qsort | sort.Slice | 按自定义比较函数对任意类型数组排序 | 对标 C qsort，Go sort.Slice；底层复用 qsort + 文件级比较函数适配器；less 参数须为顶层函数（axlang 不支持闭包） | ✅ 已落地 |
| `sort.slice_stable` | 内建库 | `(a: any[], less: func(i, j int) bool) -> void` | sort.slice_stable(arr, less) | qsort | sort.SliceStable | 稳定排序（相等元素保持原始相对顺序） | 对标 C qsort，Go sort.SliceStable；底层复用 qsort（C 标准库无稳定排序，同 slice 实现）；less 参数须为顶层函数 | ✅ 已落地 |
| `sort.strings` | 内建库 | `(a: string[]) -> void` | sort.strings(arr) | qsort | sort.Strings | 对 string 数组升序排序 | 对标 C qsort，Go sort.Strings；按字典序原地排序 | ✅ 已落地 |
| `sort.strings_are_sorted` | 内建库 | `(a: string[]) -> bool` | sort.strings_are_sorted(arr) | — | sort.StringsAreSorted | 判断 string 数组是否已升序排序 | C 标准库无直接对应，Go 对标 sort.StringsAreSorted；遍历检查，O(n) | ✅ 已落地 |
| `strconv.atoi` | 标准库 | `(s: string) -> int` | — | atoi | strconv.Atoi | 将字符串解析为整数 | 对标 C atoi，Go strconv.Atoi；解析失败返回 0 和错误 | ✅ 已落地 |
| `strconv.can_backquote` | 标准库 | `(s: string) -> bool` | — | — | strconv.CanBackquote | 判断字符串是否可用反引号（原始字符串）表示 | C 标准库无直接对应，Go 对标 strconv.CanBackquote；检查是否包含反引号或非打印字符 | ✅ 已落地 |
| `strconv.format_bool` | 标准库 | `(v: bool) -> string` | — | — | strconv.FormatBool | 将布尔值格式化为字符串 | C 标准库无直接对应，Go 对标 strconv.FormatBool；返回 'true' 或 'false' | ✅ 已落地 |
| `strconv.format_float` | 标准库 | `(v: float) -> string` | — | snprintf('%f') | strconv.FormatFloat | 将浮点数格式化为字符串 | 对标 C snprintf %f，Go strconv.FormatFloat；支持精度控制 | ✅ 已落地 |
| `strconv.format_int` | 标准库 | `(v: int, base: int) -> string` | — | itoa/snprintf | strconv.FormatInt | 将整数按指定进制格式化为字符串 | C 标准库无标准 itoa，Go 对标 strconv.FormatInt；base 参数指定进制 | ✅ 已落地 |
| `strconv.format_uint` | 标准库 | `(v: int, base: int) -> string` | — | utoa/snprintf | strconv.FormatUint | 将无符号整数按指定进制格式化为字符串 | C 标准库无标准 utoa，Go 对标 strconv.FormatUint；base 参数指定进制 | ✅ 已落地 |
| `strconv.itoa` | 标准库 | `(v: int) -> string` | — | itoa/sprintf | strconv.Itoa | 将整数格式化为字符串 | C 标准库无标准 itoa（需 sprintf 或自实现），Go 对标 strconv.Itoa | ✅ 已落地 |
| `strconv.parse_bool` | 标准库 | `(s: string) -> bool` | — | — | strconv.ParseBool | 将字符串解析为布尔值 | C 标准库无直接对应，Go 对标 strconv.ParseBool；接受 '1','t','T','TRUE','true' 等 | ✅ 已落地 |
| `strconv.parse_float` | 标准库 | `(s: string) -> float` | — | atof/strtod | strconv.ParseFloat | 将字符串解析为浮点数 | 对标 C atof/strtod，Go strconv.ParseFloat；返回 float 类型 | ✅ 已落地 |
| `strconv.parse_int` | 标准库 | `(s: string, base: int) -> int` | — | atoi/strtol | strconv.ParseInt | 按指定进制将字符串解析为整数 | 对标 C strtol，Go strconv.ParseInt；base 参数指定进制（2-36） | ✅ 已落地 |
| `strconv.parse_uint` | 标准库 | `(s: string, base: int) -> int` | — | strtoul | strconv.ParseUint | 按指定进制将字符串解析为无符号整数 | 对标 C strtoul，Go strconv.ParseUint；base 参数指定进制（2-36） | ✅ 已落地 |
| `strconv.quote` | 标准库 | `(s: string) -> string` | — | — | strconv.Quote | 将字符串加双引号并转义，生成 Go 字符串字面量 | C 标准库无直接对应，Go 对标 strconv.Quote；输出可直接用于 Go 源码 | ✅ 已落地 |
| `strconv.unquote` | 标准库 | `(s: string) -> string` | — | — | strconv.Unquote | 解析带引号的字符串字面量，去除转义 | C 标准库无直接对应，Go 对标 strconv.Unquote；处理双引号和反引号字符串 | ✅ 已落地 |
| `string.compare` | 内建库 | `(a: string, b: string) -> int` | — | strcmp | strings.Compare | 按字典序比较两个字符串（-1/0/1） | 对标 C strcmp，Go strings.Compare | ✅ 已落地 |
| `string.contains` | 内建库 | `(sub: string) -> bool` | — | strstr | strings.Contains | 判断字符串是否包含子串 | 对标 C strstr，Go strings.Contains | ✅ 已落地 |
| `string.contains_any` | 内建库 | `(chars: string) -> bool` | — | strpbrk | strings.ContainsAny | 判断字符串是否包含任意给定字符 | 对标 C strpbrk，Go strings.ContainsAny | ✅ 已落地 |
| `string.contains_rune` | 内建库 | `(r: int) -> bool` | — | — | strings.ContainsRune | 判断字符串是否包含指定 Unicode 码点 | C 标准库无直接对应，Go 对标 strings.ContainsRune | ✅ 已落地 |
| `string.count` | 内建库 | `(substr: string) -> int` | — | — | strings.Count | 统计子串在字符串中出现的次数（非重叠） | C 标准库无直接对应，Go 对标 strings.Count | ✅ 已落地 |
| `string.ends_with` | 内建库 | `(suffix: string) -> bool` | — | strncmp | strings.HasSuffix | 判断字符串是否以指定后缀结尾 | 对标 C strncmp，Go strings.HasSuffix | ✅ 已落地 |
| `string.equal_fold` | 内建库 | `(t: string) -> bool` | — | strcasecmp | strings.EqualFold | 忽略大小写比较两个字符串是否相等 | 对标 C strcasecmp，Go strings.EqualFold；UTF-8 忽略大小写比较 | ✅ 已落地 |
| `string.fields` | 内建库 | `(s: string) -> string[]` | — | strtok | strings.Fields | 按空白字符分割字符串为字符串切片 | 对标 C strtok，Go strings.Fields；按任意空白符拆分 | ✅ 已落地 |
| `string.from_codepoint` | 内建库 | `(cp: int) -> result<string>` | — | — | — | 根据 Unicode 码点创建字符串 | C 标准库无直接对应，Go 亦无标准库函数，需手动 UTF-8 编码 | ✅ 已落地 |
| `string.index_any` | 内建库 | `(chars: string) -> int` | — | strpbrk | strings.IndexAny | 返回任意给定字符首次出现的索引 | 对标 C strpbrk，Go strings.IndexAny | ✅ 已落地 |
| `string.index_of` | 内建库 | `(sub: string) -> int` | — | strstr | strings.Index | 返回子串首次出现的索引，未找到返回 -1 | 对标 C strstr（需计算偏移），Go strings.Index | ✅ 已落地 |
| `string.is_alnum` | 内建库 | `() -> bool` | — | isalnum | unicode.IsLetter/IsNumber | 判断字符串是否全部由字母或数字组成 | 对标 C isalnum，Go unicode.IsLetter 或 unicode.IsNumber | ✅ 已落地 |
| `string.is_alpha` | 内建库 | `() -> bool` | — | isalpha | unicode.IsLetter | 判断字符串是否全部由字母组成 | 对标 C isalpha，Go unicode.IsLetter | ✅ 已落地 |
| `string.is_digit` | 内建库 | `() -> bool` | — | isdigit | unicode.IsDigit | 判断字符串是否全部由数字组成 | 对标 C isdigit，Go unicode.IsDigit（逐字符判断） | ✅ 已落地 |
| `string.is_lower` | 内建库 | `() -> bool` | — | islower | unicode.IsLower | 判断字符串是否全部由小写字母组成 | 对标 C islower，Go unicode.IsLower | ✅ 已落地 |
| `string.is_space` | 内建库 | `() -> bool` | — | isspace | unicode.IsSpace | 判断字符串是否全部由空白字符组成 | 对标 C isspace，Go unicode.IsSpace | ✅ 已落地 |
| `string.is_upper` | 内建库 | `() -> bool` | — | isupper | unicode.IsUpper | 判断字符串是否全部由大写字母组成 | 对标 C isupper，Go unicode.IsUpper | ✅ 已落地 |
| `string.last_index_any` | 内建库 | `(chars: string) -> int` | — | — | strings.LastIndexAny | 返回任意给定字符最后一次出现的索引 | C 标准库无直接对应，Go 对标 strings.LastIndexAny | ✅ 已落地 |
| `string.last_index_of` | 内建库 | `(sub: string) -> int` | — | strrstr（非标准） | strings.LastIndex | 返回子串最后一次出现的索引，未找到返回 -1 | C 标准库无 strrstr，Go 对标 strings.LastIndex | ✅ 已落地 |
| `string.len` | 内建库 | `() -> int` | — | strlen | len（内置） | 返回字符串的字节长度 | 对标 C strlen，Go 内置 len()；返回 byte 数而非 rune 数 | ✅ 已落地 |
| `string.map` | 内建库 | `(mapping: func(int) int) -> string` | — | — | strings.Map | 对字符串每个字符应用映射函数并返回新字符串 | C 标准库无直接对应，Go 对标 strings.Map；axlang 不支持闭包，实际受限 | ✅ 已落地 |
| `string.repeat` | 内建库 | `(n: int) -> string` | — | — | strings.Repeat | 将字符串重复 n 次 | C 标准库无直接对应，Go 对标 strings.Repeat | ✅ 已落地 |
| `string.replace` | 内建库 | `(old: string, new: string) -> string` | — | — | strings.Replace | 将字符串中旧子串替换为新子串 | C 标准库无直接对应，Go 对标 strings.Replace；支持限制替换次数 | ✅ 已落地 |
| `string.split` | 内建库 | `(sep: string) -> string[]` | — | strtok | strings.Split | 按分隔符分割字符串为字符串切片 | 对标 C strtok，Go strings.Split；分隔符本身不保留 | ✅ 已落地 |
| `string.split_after` | 内建库 | `(sep: string) -> string[]` | — | — | strings.SplitAfter | 按分隔符分割，保留分隔符在每段末尾 | C 标准库无直接对应，Go 对标 strings.SplitAfter | ✅ 已落地 |
| `string.split_n` | 内建库 | `(sep: string, n: int) -> string[]` | — | — | strings.SplitN | 按分隔符分割为最多 n 个字符串 | C 标准库无直接对应，Go 对标 strings.SplitN | ✅ 已落地 |
| `string.starts_with` | 内建库 | `(prefix: string) -> bool` | — | strncmp | strings.HasPrefix | 判断字符串是否以指定前缀开头 | 对标 C strncmp，Go strings.HasPrefix | ✅ 已落地 |
| `string.title` | 内建库 | `() -> string` | — | — | strings.Title | 将字符串转换为标题格式（每个单词首字母大写） | C 标准库无直接对应，Go 对标 strings.Title（Go 1.18+ 已废弃，建议 cases.Title） | ✅ 已落地 |
| `string.to_lower` | 内建库 | `() -> string` | — | tolower | strings.ToLower | 将字符串转换为小写 | 对标 C tolower（逐字符），Go strings.ToLower；仅 ASCII | ✅ 已落地 |
| `string.to_lower_rune` | 内建库 | `(r: int) -> int` | — | tolower | unicode.ToLower | 将单个 Unicode 码点转换为小写 | 对标 C tolower，Go unicode.ToLower；返回 int 类型码点 | ✅ 已落地 |
| `string.to_upper` | 内建库 | `() -> string` | — | toupper | strings.ToUpper | 将字符串转换为大写 | 对标 C toupper（逐字符），Go strings.ToUpper；仅 ASCII | ✅ 已落地 |
| `string.to_upper_rune` | 内建库 | `(r: int) -> int` | — | toupper | unicode.ToUpper | 将单个 Unicode 码点转换为大写 | 对标 C toupper，Go unicode.ToUpper；返回 int 类型码点 | ✅ 已落地 |
| `string.trim` | 内建库 | `() -> string` | — | — | strings.TrimSpace | 去除字符串首尾空白字符 | C 标准库无直接对应，Go 对标 strings.TrimSpace（仅空白符） | ✅ 已落地 |
| `string.trim_left` | 内建库 | `(cutset: string) -> string` | — | — | strings.TrimLeft | 去除字符串左侧指定字符集 | C 标准库无直接对应，Go 对标 strings.TrimLeft | ✅ 已落地 |
| `string.trim_prefix` | 内建库 | `(prefix: string) -> string` | — | — | strings.TrimPrefix | 去除字符串指定前缀 | C 标准库无直接对应，Go 对标 strings.TrimPrefix | ✅ 已落地 |
| `string.trim_right` | 内建库 | `(cutset: string) -> string` | — | — | strings.TrimRight | 去除字符串右侧指定字符集 | C 标准库无直接对应，Go 对标 strings.TrimRight | ✅ 已落地 |
| `string.trim_suffix` | 内建库 | `(suffix: string) -> string` | — | — | strings.TrimSuffix | 去除字符串指定后缀 | C 标准库无直接对应，Go 对标 strings.TrimSuffix | ✅ 已落地 |
| `sync.mutex.lock` | 标准库 | `` | — | pthread_mutex_lock | sync.Mutex.Lock | 加互斥锁，保护共享资源 | 对标 C pthread_mutex_lock，Go sync.Mutex.Lock；sync.mutex 结构体方法 | ✅ 已落地 |
| `sync.mutex.unlock` | 标准库 | `` | — | pthread_mutex_unlock | sync.Mutex.Unlock | 释放互斥锁 | 对标 C pthread_mutex_unlock，Go sync.Mutex.Unlock；sync.mutex 结构体方法 | ✅ 已落地 |
| `sync.once.do` | 标准库 | `` | — | pthread_once | sync.Once.Do | 确保函数仅执行一次，用于单例初始化 | 对标 C pthread_once，Go 对标 sync.Once.Do；sync.once 结构体方法 | ✅ 已落地 |
| `sync.rwmutex.lock` | 标准库 | `` | — | pthread_mutex_lock | sync.Mutex.Lock | 加互斥锁，保护共享资源 | 对标 C pthread_mutex_lock，Go sync.Mutex.Lock；sync.mutex 结构体方法 | ✅ 已落地 |
| `sync.rwmutex.rlock` | 标准库 | `` | — | pthread_rwlock_rdlock | sync.RWMutex.RLock | 加读锁，允许并发读取 | 对标 C pthread_rwlock_rdlock，Go sync.RWMutex.RLock；sync.rwmutex 结构体方法 | ✅ 已落地 |
| `sync.rwmutex.runlock` | 标准库 | `` | — | pthread_rwlock_unlock | sync.RWMutex.RUnlock | 释放读锁 | 对标 C pthread_rwlock_unlock，Go sync.RWMutex.RUnlock；sync.rwmutex 结构体方法 | ✅ 已落地 |
| `sync.rwmutex.unlock` | 标准库 | `` | — | pthread_mutex_unlock | sync.Mutex.Unlock | 释放互斥锁 | 对标 C pthread_mutex_unlock，Go sync.Mutex.Unlock；sync.mutex 结构体方法 | ✅ 已落地 |
| `sync.wait_group.add` | 标准库 | `` | — | — | sync.WaitGroup.Add | 给等待组增加计数器 | C 标准库无直接对应，Go 对标 sync.WaitGroup.Add；sync.wait_group 结构体方法 | ✅ 已落地 |
| `sync.wait_group.done` | 标准库 | `` | — | — | sync.WaitGroup.Done | 等待组计数器减 1，表示任务完成 | C 标准库无直接对应，Go 对标 sync.WaitGroup.Done；sync.wait_group 结构体方法 | ✅ 已落地 |
| `sync.wait_group.wait` | 标准库 | `` | — | pthread_join/条件变量 | sync.WaitGroup.Wait | 阻塞等待等待组计数器归零 | 对标 C pthread_join/条件变量，Go 对标 sync.WaitGroup.Wait；sync.wait_group 结构体方法 | ✅ 已落地 |
| `thread.close` | 内建库 | `() -> void` | ch.close() | — | close(ch) | 关闭通道，禁止后续发送 | C 标准库无通道，Go 对标 close() 内置；关闭后只能读不能写 | ✅ 已落地 |
| `thread.join` | 内建库 | `() -> void` | ch.join() | pthread_join | sync.WaitGroup.Wait | 等待线程/协程结束 | 对标 C pthread_join，Go 对标 sync.WaitGroup.Wait；阻塞等待完成 | ✅ 已落地 |
| `thread.pop` | 内建库 | `() -> T?` | ch.pop() | — | <-ch | 从通道接收数据（阻塞） | C 标准库无通道，Go 对标 chan 接收操作；返回 T? 可选类型 | ✅ 已落地 |
| `thread.push` | 内建库 | `(v: T) -> void` | ch.push(v) | — | ch <- v | 向通道发送数据（阻塞） | C 标准库无通道，Go 对标 chan 发送操作；channel 阻塞式写入 | ✅ 已落地 |
| `time.add` | 标准库 | `(d: int) -> time` | — | — | Time.Add | 返回当前时间加上指定毫秒数后的时间 | C 标准库无直接对应，Go 对标 time.Time.Add(duration) | ✅ 已落地 |
| `time.after` | 标准库 | `(u: time) -> bool` | — | — | time.After/Time.After | 判断当前时间是否在参数时间之后 | C 标准库无直接对应，Go 对标 time.Time.After() 方法 | ✅ 已落地 |
| `time.before` | 标准库 | `(u: time) -> bool` | — | — | Time.Before | 判断当前时间是否在参数时间之前 | C 标准库无直接对应，Go 对标 time.Time.Before() 方法 | ✅ 已落地 |
| `time.compare` | 标准库 | `(u: time) -> int` | — | difftime | Time.Compare | 比较两个时间大小（-1/0/1） | 对标 C difftime，Go time.Time.Compare()；返回 -1/0/1 | ✅ 已落地 |
| `time.date` | 标准库 | `(year, month, day, hour, min, sec, nsec: int) -> time` | — | mktime | time.Date | 按年月日时分秒纳秒创建时间 | 对标 C mktime，Go time.Date；参数均为 int 类型 | ✅ 已落地 |
| `time.equal` | 标准库 | `(u: time) -> bool` | — | — | Time.Equal | 判断两个时间是否相等 | C 标准库无直接对应，Go 对标 time.Time.Equal() 方法 | ✅ 已落地 |
| `time.format` | 标准库 | `(fmt: string) -> string` | — | strftime | time.Time.Format | 按自定义格式字符串格式化时间 | 对标 C strftime，Go time.Time.Format；axlang 使用 Go 风格格式串（如 '2006-01-02'） | ✅ 已落地 |
| `time.from_unix_ms` | 标准库 | `(ms: int) -> time` | — | — | time.UnixMilli | 根据毫秒级 Unix 时间戳创建时间 | C 标准库无直接对应，Go 对标 time.UnixMilli() | ✅ 已落地 |
| `time.now` | 标准库 | `() -> time` | — | time | time.Now | 返回当前本地时间 | 对标 C time()，Go time.Now()；返回 time 类型（C 层 int64_t 毫秒） | ✅ 已落地 |
| `time.now_ms` | 标准库 | `() -> int` | — | clock_gettime/time | time.Now().UnixMilli | 返回当前 Unix 时间戳（毫秒） | 对标 C clock_gettime/time，Go time.Now().UnixMilli()；返回 int 毫秒数 | ✅ 已落地 |
| `time.parse` | 标准库 | `(s: string, fmt: string) -> result<time>` | — | strptime | time.Parse | 解析格式化的时间字符串为 time 对象 | 对标 C strptime，Go time.Parse；返回 result<time> 类型，解析失败返回错误 | ✅ 已落地 |
| `time.round` | 标准库 | `(d: int) -> time` | — | — | Time.Round | 将时间四舍五入到指定的时间段整数倍 | C 标准库无直接对应，Go 对标 time.Time.Round() 方法 | ✅ 已落地 |
| `time.since` | 标准库 | `(t: time) -> int` | — | difftime | time.Since | 返回从指定时间到当前时间的毫秒差 | 对标 C difftime，Go time.Since；返回 int 毫秒数，当前时间减去参数时间 | ✅ 已落地 |
| `time.sleep` | 标准库 | `(ms: int) -> void` | — | sleep/usleep | time.Sleep | 使当前协程/线程休眠指定毫秒数 | 对标 C sleep/usleep，Go time.Sleep；阻塞当前执行流 | ✅ 已落地 |
| `time.sub` | 标准库 | `(u: time) -> int` | — | difftime | Time.Sub | 返回两个时间的差值（毫秒） | 对标 C difftime，Go time.Time.Sub()；返回当前时间减去参数时间的毫秒数 | ✅ 已落地 |
| `time.to_string` | 标准库 | `() -> string` | — | ctime/strftime | time.Time.String | 将时间格式化为默认字符串表示 | 对标 C ctime/strftime，Go time.Time.String()；输出类似 '2006-01-02 15:04:05' | ✅ 已落地 |
| `time.truncate` | 标准库 | `(d: int) -> time` | — | — | time.Truncate | 将时间向下取整到指定的时间段整数倍 | C 标准库无直接对应，Go 对标 time.Truncate() 方法 | ✅ 已落地 |
| `time.unix` | 标准库 | `(sec: int, nsec: int) -> time` | — | — | time.Unix | 根据秒和纳秒创建时间 | C 标准库无直接对应，Go 对标 time.Unix(sec, nsec) | ✅ 已落地 |
| `time.unix_micro` | 标准库 | `(us: int) -> time` | — | — | time.UnixMicro | 根据微秒创建时间 | C 标准库无直接对应，Go 对标 time.UnixMicro(us) | ✅ 已落地 |
| `time.unix_milli` | 标准库 | `(ms: int) -> time` | — | — | time.UnixMilli | 根据毫秒创建时间 | C 标准库无直接对应，Go 对标 time.UnixMilli(ms) | ✅ 已落地 |
| `time.unix_nano` | 标准库 | `(nsec: int) -> time` | — | — | time.UnixNano | 根据纳秒创建时间 | C 标准库无直接对应，Go 对标 time.UnixNano(nsec) | ✅ 已落地 |
| `time.until` | 标准库 | `(t: time) -> int` | — | difftime | time.Until | 返回从当前时间到指定时间的毫秒差 | 对标 C difftime，Go time.Until；返回 int，参数时间减去当前时间 | ✅ 已落地 |
| `unique.make` | 内建库 | `(v: T) -> Handle[T]` | unique.make(v) | — | unique.Make | 根据值创建唯一句柄 Handle[T]，相同值共享同一句柄 | C 标准库无直接对应，Go 对标 unique.Make；Handle[T] 装箱类型，支持值比较去重 | ✅ 已落地 |
| `unique.value` | 内建库 | `(h: Handle[T]) -> T` | unique.value(h) | — | unique.Value | 从 Handle[T] 解出原始值 T | C 标准库无直接对应，Go 对标 unique.Value；解箱操作，返回内层类型 T | ✅ 已落地 |

---

## 三、当前进度汇总

| 阶段 | 已完成 | 待完成 |
| --- | --- | --- |
| 编译器内建函数（保留） | 131 个 | 长期保留在编译器 Rust 端 |
| 标准库函数（已迁移） | 170 个 | 已在 axc/std/ 下以 ax 源码实现 |
| 标准库函数（待迁移） | 0 个 | 计划从编译器内建迁移到 ax_stdlib |
| 已落地函数 | 298 / 301 | — |


*更新于 2026-11-25，基于 STDLIB-STATUS.json 自动生成*
