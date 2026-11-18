<!-- 本文件由 tools/gen_stdlib_doc.py 生成，请勿手改；内建方法变更后重新运行。 -->

# axlang 内置库与标准库

本文档由 `tools/gen_stdlib_doc.py` 从编译器源码（`rust/src/sema/member.rs` 与
`expr.rs` 的内建签名表）自动抽取生成。给内建表新增方法后重新运行该脚本，
新方法会自动列出；描述在脚本的 `DESC` 字典里维护，缺描述的条目会标「签名待补充」。

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

## 内建模块（`BUILTIN_MODULES`）

`db` / `file` / `io` / `json` / `math` / `string` / `time` 是编译器内建模块，
与用户模块同名会被拒绝（`E3039`）。

### db

| `db.Query` | db.Query(sql: string) -> result<Row[]>——执行查询，返回行数组 |

### file

| `file.Open` | file.Open(path: string) -> result<File>——以读写打开文件 |
| `file.Create` | file.Create(path: string) -> result<File>——创建/截断文件 |
| `file.Append` | file.Append(path: string) -> result<File>——以追加打开文件 |

### io

| `io.ReadLine` | io.ReadLine() -> string?——从标准输入读一行；EOF 返回 null |
| `io.ReadFile` | io.ReadFile(path: string) -> result<string>——读整个文本文件 |

### json

| `json.Parse` | json.Parse(s: string) -> result<map<string, object>>——解析 JSON 文本 |
| `json.Stringify` | json.Stringify(v: object) -> string——序列化 object 为 JSON |

### math

| `math.Equals` | math.Equals(a: float, b: float, eps: float) -> bool——浮点带容差比较 |
| `math.Compare` | math.Compare(a: float, b: float) -> int——浮点比较：-1/0/1 |

### string

| `string.FromCodepoint` | string.FromCodepoint(cp: int) -> result<string>——码点转字符（`string` 既是类型也是模块，与静态方法同名同拼写） |

### time

| `time.NowMs` | time.NowMs() -> int——当前 Unix 时间（毫秒） |
| `time.Now` | time.Now() -> time——当前本地时间（Unix 毫秒） |
| `time.FromUnixMs` | time.FromUnixMs(ms: int) -> time——按 Unix 毫秒数构造时间 |
| `UnixMs` | t.UnixMs() -> int——Unix 毫秒数 |
| `ToString` | t.ToString() -> string——按 "YYYY-MM-DD HH:MM:SS" 格式化 |
| `Format` | t.Format(fmt: string) -> string——按 strftime 风格格式串格式化 |

## 内建类型实例方法

### string

| `Len` | s.Len() -> int——字符数（按码点） |
| `ByteLen` | s.ByteLen() -> int——UTF-8 字节数 |
| `At` | s.At(i: int) -> string?——第 i 个字符；越界返回 null |
| `Chars` | s.Chars() -> string[]——拆成单字符数组 |
| `CodepointAt` | s.CodepointAt(i: int) -> int?——第 i 个字符的码点；越界返回 null |
| `Contains` | s.Contains(sub: string) -> bool——是否包含子串 |
| `Split` | s.Split(sep: string) -> string[]——按分隔符拆分 |
| `ToString` | s.ToString() -> string——自身 |
| `Trim` | s.Trim() -> string——去除两端空白 |
| `Left` | *签名待补充*（在 tools/gen_stdlib_doc.py 的 DESC 里补） |
| `Right` | *签名待补充*（在 tools/gen_stdlib_doc.py 的 DESC 里补） |
| `Mid` | *签名待补充*（在 tools/gen_stdlib_doc.py 的 DESC 里补） |
| `Upper` | s.Upper() -> string——转大写 |
| `Lower` | s.Lower() -> string——转小写 |
| `StartsWith` | s.StartsWith(p: string) -> bool——前缀判断 |
| `EndsWith` | s.EndsWith(p: string) -> bool——后缀判断 |
| `Join` | s.Join(parts: string[]) -> string——用 s 连接数组 |
| `Replace` | s.Replace(old: string, new: string) -> string——替换全部子串 |
| `Repeat` | s.Repeat(n: int) -> string——重复 n 次 |

### array

| `Len` | a.Len() -> int——元素个数 |
| `Push` | a.Push(v: T)——尾部追加（就地修改） |
| `Pop` | a.Pop() -> T——弹出并返回尾元素 |
| `Remove` | a.Remove(i: int)——按下标删除 |
| `Clear` | a.Clear()——清空 |
| `At` | a.At(i: int) -> T?——按下标取；越界返回 null |
| `Sort` | a.Sort(cmp: func(T, T) int)——按比较函数稳定排序（cmp<0 表示前者在前） |
| `ToString` | a.ToString() -> string——转为展示字符串 |

### map

| `Len` | m.Len() -> int——键值对个数 |
| `Get` | m.Get(k: K) -> V?——取值；键不存在返回 null |
| `Keys` | m.Keys() -> K[]——全部键 |
| `Remove` | m.Remove(k: K)——删除键 |
| `Has` | m.Has(k: K) -> bool——键是否存在 |

### decimal

| `Round` | d.Round(scale: int, mode: Rounding) -> decimal——按位数与舍入模式取整 |
| `Divide` | d.Divide(other: decimal, scale: int, mode: Rounding) -> decimal——显式指定精度与舍入的除法 |
| `ToString` | d.ToString() -> string——转为字符串 |

### int

| `ToString` | x.ToString() -> string——转为字符串 |
| `WrappingAdd` | x.WrappingAdd(y: int) -> int——回绕加法（溢出不报错） |

### float

| `ToString` | x.ToString() -> string——转为字符串 |

### bool

| `ToString` | b.ToString() -> string——"true"/"false" |

### enum

| `ToString` | x.ToString() -> string——变体名 |

## 静态方法

| 方法 | 签名 / 说明 |
|---|---|
| `Parse` | T.Parse(s: string) -> result<T>——字符串解析为数值（int/float/int8/16/32/64 等） |
| `Truncate` | int8/int16/int32.Truncate(x: int) -> T——把 int 截断到窄类型 |
| `FromCodepoint` | string.FromCodepoint(cp: int) -> result<string>——码点转字符（与模块 string 同名方法等价） |

## 数据库行：`Row`（`db.Query` 的元素）

| 方法 | 签名 / 说明 |
|---|---|
| `Empty` | row.Empty() -> bool——行是否为空 |
| `GetString` | row.GetString(col: string) -> string——取字符串列 |
| `GetInt` | row.GetInt(col: string) -> int——取整数列 |
| `GetDecimal` | row.GetDecimal(col: string) -> decimal——取金额列 |

## 文件句柄：`File`（`file.Open / file.Create / file.Append` 的返回值）

| 方法 | 签名 / 说明 |
|---|---|
| `Close` | f.Close()——关闭文件 |
| `Read` | f.Read() -> result<string>——读全部内容 |
| `ReadLine` | f.ReadLine() -> string?——读一行；EOF 返回 null |
| `Write` | f.Write(s: string) -> result<int>——写入，返回字节数 |
| `WriteLine` | f.WriteLine(s: string) -> result<int>——写入一行，返回字节数 |

## 待补充

以下条目已存在于编译器内建表，但 `DESC` 字典还没有描述：

- string.Left
- string.Right
- string.Mid
