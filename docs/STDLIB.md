<!-- 本文件由 tools/gen_stdlib_doc.py 生成，请勿手改；内建方法变更后重新运行。 -->
<!-- 生成时间：2026-11-27 16:07:26 -->

# axlang 内置库与标准库

本文档由 `tools/gen_stdlib_doc.py` 从编译器源码（`rust/src/sema/mod.rs` 的
`INTERNAL_BUILTINS` / `STDLIB_MODULES` 及 `expr.rs` 的 `module_sig()`）
自动抽取生成。内建与标准库模块互斥，不得并存；模块归类以编译器源码为准。

## 全局函数

| 函数 | 签名 / 说明 |
|---|---|
| `print` | print(s: string) / print(x: 可插值)——输出不换行；实参非字符串时等价于 `print("{x}")` |
| `println` | println(s: string) / println(x: 可插值)——输出并换行；实参非字符串时等价于 `println("{x}")` |
| `ok` | ok(v: T) -> result<T>——构造成功结果，配合 try/throw 使用 |
| `fail` | fail(e: error)——构造失败结果，只允许在返回 result 的函数内 |
| `parent` | parent(实参...)——在子类 constructor 第一条语句调用父类构造器 |
| `typeof` | typeof(expr: 表达式) -> string——编译期常量，返回实参静态类型显示名（int / int8 / Dog / int[] / int? / map<string, object>）；生成字符串字面量，零运行时开销 |
| `sizeof` | sizeof(expr: 表达式) -> int——编译期大小：实参类型在 C 侧的存储字节数（int=8 / uint8=1 / string=16 / int[]=24 / class=指针 8）；对象本身的大小不在此问 |

## 内建模块（`INTERNAL_BUILTINS`）

以下模块为编译器 Rust 端内建，无对应 ax 源码，裸模块名直接使用，
无需 `import`（如 `fmt.printf("pi = %.2f\n", 3.14159)`）。
与标准库模块互斥，不得并存。

### db

| `db.query` | db.query(sql: string) -> result<Row[]>——执行查询，返回行数组 |

### fmt

| `fmt.print` | fmt.print(s: string)——输出不换行；与内建 print 同实现 |
| `fmt.println` | fmt.println(s: string)——输出并换行；与内建 println 同实现 |
| `fmt.printf` | fmt.printf(format: string, ...)——格式化占位符输出（%d/%f/%.Nf/%s/%t/%v/%%），编译期核对格式串 |

### log

| `log.print` | log.print(...)——多实参打印，复用 fmt.printf 实现 |
| `log.printf` | log.printf(format: string, ...)——格式化打印，复用 fmt.printf 实现 |
| `log.fatal` | log.fatal(...)——打印致命错误并退出 |

### maps

*此内建模块当前没有通过 `module_sig()` 注册的符号。*

### reflect

| `reflect.typeOf` | reflect.typeOf(v: object) -> string——编译期返回运行时类型名，零运行时开销 |
| `reflect.numField` | reflect.numField(v: object) -> int——返回结构体/对象字段数量 |

### sort

| `sort.ints` | sort.ints(a: int[])——对 int 数组升序排序 |
| `sort.strings` | sort.strings(a: string[])——对 string 数组升序排序 |
| `sort.float64s` | sort.float64s(a: float[])——对 float 数组升序排序 |
| `sort.intsAreSorted` | sort.intsAreSorted(a: int[]) -> bool——判断 int 数组是否已升序 |
| `sort.stringsAreSorted` | sort.stringsAreSorted(a: string[]) -> bool——判断 string 数组是否已升序 |
| `sort.searchInts` | sort.searchInts(a: int[], x: int) -> int——在有序 int 数组中二分查找 x |
| `sort.searchStrings` | sort.searchStrings(a: string[], x: string) -> int——在有序 string 数组中二分查找 x |
| `sort.slice` | sort.slice(a: object[], cmp: func(int, int) int)——按自定义比较函数对 object 数组排序 |
| `sort.sliceStable` | sort.sliceStable(a: object[], cmp: func(int, int) int)——按自定义比较函数稳定排序 object 数组 |

### unique

*此内建模块当前没有通过 `module_sig()` 注册的符号。*

## 标准库模块（`STDLIB_MODULES`）

以下模块用 ax 语法编写，位于 `axc/std/*.ax`，通过 `import std.xxx` 导入，
对应 ax 层与 C 运行时 `ax_std_*` 接口的桥梁。模块名统一以 `std.` 前缀开头，
如 `std.string`、`std.file`、`std.io` 等。

### std.string

*对应 `axc/std/string.ax`（标准库 ax 源码），符号定义见各模块源文件。*


### std.file

*对应 `axc/std/file.ax`（标准库 ax 源码），符号定义见各模块源文件。*


### std.sync

*对应 `axc/std/sync.ax`（标准库 ax 源码），符号定义见各模块源文件。*


### std.atomic

*对应 `axc/std/atomic.ax`（标准库 ax 源码），符号定义见各模块源文件。*


### std.io

*对应 `axc/std/io.ax`（标准库 ax 源码），符号定义见各模块源文件。*


### std.json

*对应 `axc/std/json.ax`（标准库 ax 源码），符号定义见各模块源文件。*


### std.os

*对应 `axc/std/os.ax`（标准库 ax 源码），符号定义见各模块源文件。*


### std.strconv

*对应 `axc/std/strconv.ax`（标准库 ax 源码），符号定义见各模块源文件。*


### std.math

*对应 `axc/std/math.ax`（标准库 ax 源码），符号定义见各模块源文件。*


### std.errors

*对应 `axc/std/errors.ax`（标准库 ax 源码），符号定义见各模块源文件。*


### std.time

*对应 `axc/std/time.ax`（标准库 ax 源码），符号定义见各模块源文件。*


### std.rand

*对应 `axc/std/rand.ax`（标准库 ax 源码），符号定义见各模块源文件。*


### std.bytes

*对应 `axc/std/bytes.ax`（标准库 ax 源码），符号定义见各模块源文件。*


### std.bufio

*对应 `axc/std/bufio.ax`（标准库 ax 源码），符号定义见各模块源文件。*


### std.csv

*对应 `axc/std/csv.ax`（标准库 ax 源码），符号定义见各模块源文件。*


### std.filepath

*对应 `axc/std/filepath.ax`（标准库 ax 源码），符号定义见各模块源文件。*


## 内建类型实例方法

### string

| `len` | s.len() -> int——字符数（按码点） |
| `byteLen` | s.byteLen() -> int——UTF-8 字节数 |
| `at` | s.at(i: int) -> string?——第 i 个字符；越界返回 null |
| `chars` | s.chars() -> string[]——拆成单字符数组 |
| `codepointAt` | s.codepointAt(i: int) -> int?——第 i 个字符的码点；越界返回 null |
| `contains` | s.contains(sub: string) -> bool——是否包含子串 |
| `containsRune` | s.containsRune(cp: int) -> bool——是否包含指定 Unicode 码点 |
| `containsAny` | s.containsAny(chars: string) -> bool——是否包含 chars 中任意字符 |
| `split` | s.split(sep: string) -> string[]——按分隔符拆分 |
| `indexOf` | s.indexOf(sub: string) -> int——子串位置，找不到返回 -1 |
| `indexAny` | s.indexAny(chars: string) -> int——chars 中任意字符首次出现的位置，找不到返回 -1 |
| `lastIndexOf` | s.lastIndexOf(sub: string) -> int——子串最后出现的位置，找不到返回 -1 |
| `lastIndexAny` | s.lastIndexAny(chars: string) -> int——chars 中任意字符最后出现的位置，找不到返回 -1 |
| `count` | s.count(sub: string) -> int——子串非重叠出现次数 |
| `toString` | s.toString() -> string——自身 |
| `trim` | s.trim() -> string——去除两端空白 |
| `trimLeft` | s.trimLeft(cutset: string) -> string——去除左侧任意在 cutset 中的字符 |
| `trimRight` | s.trimRight(cutset: string) -> string——去除右侧任意在 cutset 中的字符 |
| `trimPrefix` | s.trimPrefix(prefix: string) -> string——若以 prefix 开头则去除，否则原样返回 |
| `trimSuffix` | s.trimSuffix(suffix: string) -> string——若以 suffix 结尾则去除，否则原样返回 |
| `left` | s.left(n: int) -> string——取左侧 n 个字符 |
| `right` | s.right(n: int) -> string——取右侧 n 个字符 |
| `mid` | s.mid(start: int, end: int) -> string——按码点区间取子串 |
| `upper` | s.upper() -> string——转大写（仅 ASCII，UTF-8 多字节原样保留） |
| `lower` | s.lower() -> string——转小写（仅 ASCII，UTF-8 多字节原样保留） |
| `startsWith` | s.startsWith(p: string) -> bool——是否以 p 开头 |
| `endsWith` | s.endsWith(p: string) -> bool——是否以 p 结尾 |
| `equalFold` | s.equalFold(t: string) -> bool——忽略大小写判断两字符串是否相等（仅 ASCII） |
| `splitN` | s.splitN(sep: string, n: int) -> string[]——按 sep 分割，最多返回 n 段 |
| `splitAfter` | s.splitAfter(sep: string) -> string[]——在每个 sep 之后切分 |
| `title` | s.title() -> string——标题式大写（首字母大写，其余小写） |
| `map` | s.map(f: func(int) int) -> string——对每个字符码点应用函数 f 并重建字符串 |
| `join` | s.join(parts: string[]) -> string——用 s 作为分隔符连接数组 |
| `replace` | s.replace(old: string, new: string) -> string——替换所有 old 子串为 new |
| `repeat` | s.repeat(n: int) -> string——重复 s n 次 |
| `isDigit` | s.isDigit() -> bool——是否全部为数字字符 |
| `isAlpha` | s.isAlpha() -> bool——是否全部为字母字符 |
| `isSpace` | s.isSpace() -> bool——是否全部为空白字符 |
| `isLower` | s.isLower() -> bool——是否全部为小写字母 |
| `isUpper` | s.isUpper() -> bool——是否全部为大写字母 |
| `isAlnum` | s.isAlnum() -> bool——是否全部为字母或数字 |

### array

| `len` | a.len() -> int——元素个数 |
| `push` | a.push(v: T)——尾部追加（就地修改） |
| `pop` | a.pop() -> T——弹出并返回尾元素 |
| `remove` | a.remove(i: int)——按下标删除 |
| `delete` | a.delete(i: int, j: int)——删除下标区间 [i, j) 的元素（就地修改） |
| `clear` | a.clear()——清空 |
| `at` | a.at(i: int) -> T?——按下标取；越界返回 null |
| `contains` | a.contains(v: T) -> bool——是否包含元素 v |
| `index` | a.index(v: T) -> int——元素 v 首次出现的下标，找不到返回 -1 |
| `sort` | a.sort(cmp: func(T, T) int)——按比较函数稳定排序（cmp<0 表示前者在前） |
| `reverse` | a.reverse()——原地逆序数组元素 |
| `clone` | a.clone() -> T[]——浅拷贝数组 |
| `insert` | a.insert(i: int, v: T)——在下标 i 处插入元素 v |
| `fill` | a.fill(v: T)——用元素 v 填充整个数组 |
| `resize` | a.resize(n: int)——调整数组长度为 n，不足零初始化，超出截断 |
| `merge` | a.merge(b: T[])——将数组 b 的所有元素追加到 a |
| `min` | a.min() -> T——数组中的最小元素（需 T 可比较） |
| `max` | a.max() -> T——数组中的最大元素（需 T 可比较） |
| `sum` | a.sum() -> T——数组元素求和（需 T 支持加法） |
| `toString` | a.toString() -> string——转为展示字符串 |

### map

| `len` | m.len() -> int——键值对个数 |
| `get` | m.get(k: K) -> V?——取值；键不存在返回 null |
| `keys` | m.keys() -> K[]——全部键 |
| `remove` | m.remove(k: K)——删除键 |
| `has` | m.has(k: K) -> bool——键是否存在 |
| `clear` | m.clear()——清空所有键值对 |

### decimal

| `round` | d.round(scale: int, mode: Rounding) -> decimal——按位数与舍入模式取整 |
| `divide` | d.divide(other: decimal, scale: int, mode: Rounding) -> decimal——显式指定精度与舍入的除法 |
| `truncate` | d.truncate(scale: int) -> decimal——按指定小数位截断（向零舍入） |
| `toString` | d.toString() -> string——转为字符串 |

### time

| `unixMs` | t.unixMs() -> int——Unix 毫秒数 |
| `toString` | t.toString() -> string——按 "YYYY-MM-DD HH:MM:SS" 格式化 |
| `format` | t.format(fmt: string) -> string——按 strftime 风格格式串格式化 |
| `after` | t.after(other: time) -> bool——t 是否在 other 之后 |
| `before` | t.before(other: time) -> bool——t 是否在 other 之前 |
| `equal` | t.equal(other: time) -> bool——t 是否等于 other |
| `compare` | t.compare(other: time) -> int——比较两个时间，返回 -1/0/1 |
| `sub` | t.sub(other: time) -> int——两个时间的时间差（毫秒） |
| `add` | t.add(ms: int) -> time——t 加上指定毫秒数后的时间 |
| `round` | t.round(scale: int) -> time——按指定精度取整时间 |
| `truncate` | t.truncate(scale: int) -> time——按指定精度截断时间 |

### int

| `toString` | x.toString() -> string——转为字符串 |
| `wrappingAdd` | x.wrappingAdd(y: int) -> int——回绕加法（溢出不报错） |

### float

| `toString` | x.toString() -> string——转为字符串 |

### bool

| `toString` | b.toString() -> string——"true"/"false" |

### enum

| `toString` | x.toString() -> string——变体名 |

## 静态方法

| 方法 | 签名 / 说明 |
|---|---|
| `parse` | T.parse(s: string) -> result<T>——字符串解析为数值（int/float/int8/16/32/64 等） |
| `truncate` | int8/int16/int32.truncate(x: int) -> T——把 int 截断到窄类型 |
| `fromCodepoint` | string.fromCodepoint(cp: int) -> result<string>——码点转字符（与模块 string 同名方法等价） |
| `compare` | string.compare(s1: string, s2: string) -> int——比较两个字符串，返回 -1/0/1 |
| `fields` | string.fields(s: string) -> string[]——按空白字符拆分字符串为字段数组 |
| `toUpperRune` | string.toUpperRune(cp: int) -> int——码点转大写后的码点 |
| `toLowerRune` | string.toLowerRune(cp: int) -> int——码点转小写后的码点 |

## 数据库行：`Row`（`db.Query` 的元素）

| 方法 | 签名 / 说明 |
|---|---|
| `empty` | row.empty() -> bool——行是否为空 |
| `getString` | row.getString(col: string) -> string——取字符串列 |
| `getInt` | row.getInt(col: string) -> int——取整数列 |
| `getDecimal` | row.getDecimal(col: string) -> decimal——取金额列 |

## 文件句柄：`File`（`file.Open / file.Create / file.Append` 的返回值）

| 方法 | 签名 / 说明 |
|---|---|
| `close` | f.close()——关闭文件 |
| `read` | f.read() -> result<string>——读全部内容 |
| `readLine` | f.readLine() -> string?——读一行；EOF 返回 null |
| `write` | f.write(s: string) -> result<int>——写入，返回字节数 |
| `writeLine` | f.writeLine(s: string) -> result<int>——写入一行，返回字节数 |
