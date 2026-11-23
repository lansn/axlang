#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
为 STDLIB-CHECKLIST.md 所有表格插入 `ax库函数名` 和 `函数功能/用途` 两列，并填充内容。

处理逻辑：
1. 识别 Markdown 表格的数据行（跳过表头和分隔行）
2. 在 `状态` 列之前插入两列：ax库函数名、函数功能/用途
3. ax库函数名：从 `函数` 列提取（如 `io.ReadLine` -> `io.ReadLine`）
4. 函数功能/用途：优先从 `说明` 列提取；否则从 DEFAULT_DESC 查找默认描述
5. 同时更新表头行和分隔行的列数，保持 Markdown 表格格式正确
"""

import re
import sys

FILE = r"D:\Project\axlang\STDLIB-CHECKLIST.md"

# 默认功能描述（当说明列为空时使用）
DEFAULT_DESC = {
    "io.ReadLine": "读取一行，EOF 时返回 null",
    "io.ReadFile": "读取整个文件",
    "io.Write": "写 stdout",
    "io.WriteLine": "写 stdout 并换行",
    "fmt.Print": "打印任意参数到 stdout",
    "fmt.Println": "打印任意参数到 stdout 并换行",
    "fmt.Printf": "按格式字符串打印到 stdout，编译期核对占位符",
    "fmt.Sprintf": "按格式字符串生成字符串，对标 Go fmt.Sprintf / C snprintf",
    "fmt.Sprint": "将任意参数拼接为字符串，对标 Go fmt.Sprint",
    "fmt.Fprintf": "按格式字符串写入指定 Writer，对标 Go fmt.Fprintf / C fprintf",
    "fmt.Errorf": "按格式生成错误值，对标 Go fmt.Errorf",
    "fmt.Scanf": "从 stdin 按格式读取，对标 Go fmt.Scanf / C scanf",
    "math.Abs": "绝对值，对标 Go math.Abs / C fabs",
    "math.Max": "最大值，对标 Go math.Max / C fmax",
    "math.Min": "最小值，对标 Go math.Min / C fmin",
    "math.Sqrt": "平方根，对标 Go math.Sqrt / C sqrt",
    "math.Pow": "幂，对标 Go math.Pow / C pow",
    "math.Exp": "指数，对标 Go math.Exp / C exp",
    "math.Log": "自然对数，对标 Go math.Log / C log",
    "math.Sin": "正弦，对标 Go math.Sin / C sin",
    "math.Cos": "余弦，对标 Go math.Cos / C cos",
    "math.Tan": "正切，对标 Go math.Tan / C tan",
    "math.Ceil": "向上取整，对标 Go math.Ceil / C ceil",
    "math.Floor": "向下取整，对标 Go math.Floor / C floor",
    "math.Round": "四舍五入，对标 Go math.Round / C round",
    "math.Sign": "符号：-1/0/1，对标 Go math.Sign",
    "math.Mod": "取模，对标 Go math.Mod / C fmod",
    "math.Equals": "fabs(a-b) <= eps",
    "math.Compare": "-1/0/1",
    "math.Exp2": "2 的 x 次方，对标 Go math.Exp2 / C11 exp2",
    "math.Log10": "以 10 为底对数，对标 Go math.Log10 / C11 log10",
    "math.Log2": "以 2 为底对数，对标 Go math.Log2",
    "math.Log1p": "log(1+x)，对标 Go math.Log1p / C11 log1p",
    "math.Sinh": "双曲正弦，对标 Go math.Sinh / C sinh",
    "math.Cosh": "双曲余弦，对标 Go math.Cosh / C cosh",
    "math.Tanh": "双曲正切，对标 Go math.Tanh / C tanh",
    "math.Asin": "反正弦，对标 Go math.Asin / C asin",
    "math.Acos": "反余弦，对标 Go math.Acos / C acos",
    "math.Atan": "反正切，对标 Go math.Atan / C atan",
    "math.Atan2": "反正切（带符号的商），对标 Go math.Atan2 / C atan2",
    "math.Hypot": "平方和开方，对标 Go math.Hypot / C hypot",
    "math.Cbrt": "立方根，对标 Go math.Cbrt",
    "math.Trunc": "向零取整，对标 Go math.Trunc / C trunc",
    "math.Frexp": "分解尾数与指数，对标 Go math.Frexp / C frexp",
    "math.Ldexp": "尾数乘 2 的指数次方，对标 Go math.Ldexp / C ldexp",
    "math.IsNaN": "判断是否为 NaN，对标 Go math.IsNaN / C11 isnan",
    "math.IsInf": "判断是否为无穷大，对标 Go math.IsInf / C11 isinf",
    "math.Signbit": "判断符号位，对标 Go math.Signbit",
    "math.Copysign": "复制符号，对标 Go math.Copysign / C copysign",
    "math.Dim": "max(x-y, 0)，对标 Go math.Dim",
    "json.Parse": "解析 JSON，对标 Go json.Unmarshal",
    "json.Stringify": "序列化 JSON，对标 Go json.Marshal",
    "json.MarshalIndent": "带缩进序列化 JSON，对标 Go json.MarshalIndent",
    "json.Valid": "判断 JSON 是否合法，对标 Go json.Valid",
    "file.Open": "打开文件，模式 0=只读，对标 Go os.Open / C fopen",
    "file.Create": "创建/截断文件，模式 1=写，对标 Go os.Create / C fopen wb",
    "file.Append": "追加打开文件，模式 2=追加，对标 Go os.OpenFile 追加 / C fopen ab",
    "file.Remove": "删除文件，0=成功 1=失败，对标 Go os.Remove / C remove",
    "file.Rename": "重命名文件，0=成功 1=失败，对标 Go os.Rename / C rename",
    "file.Exists": "判断文件是否存在，1=存在 0=不存在，对标 Go os.Stat / C fopen 试探",
    "file.Mkdir": "创建目录，0=成功 1=失败，对标 Go os.Mkdir / C mkdir",
    "file.ReadAll": "便捷读取整个文件，对标 Go os.ReadFile",
    "file.WriteAll": "便捷写入整个文件，0=成功 1=失败",
    "file.IsDir": "判断是否为目录，1=目录 0=否或不存在，对标 Go os.Stat IsDir",
    "file.IsFile": "判断是否为常规文件，1=常规文件 0=否或不存在，对标 Go os.Stat",
    "file.Size": "返回文件字节数，失败返回 -1，对标 Go os.Stat Size",
    "file.ModTime": "返回文件修改时间（Unix 毫秒），失败返回 -1，对标 Go os.Stat ModTime",
    "file.Chmod": "修改文件权限，0=成功 1=失败，对标 Go os.Chmod / C chmod",
    "file.Chtimes": "修改访问/修改时间，0=成功 1=失败，对标 Go os.Chtimes",
    "file.TempFile": "创建临时文件，对标 Go os.CreateTemp",
    "db.Query": "数据库查询",
    "string.FromCodepoint": "码点转字符串",
    "string.Compare": "字典序比较，对标 Go strings.Compare",
    "string.Fields": "按空白分割，对标 Go strings.Fields",
    "string.ToUpper": "大写转换，对标 Go strings.ToUpper / C toupper",
    "string.ToLower": "小写转换，对标 Go strings.ToLower / C tolower",
    "string.Contains": "判断是否包含子串，对标 Go strings.Contains",
    "string.IndexOf": "首次出现子串索引，对标 Go strings.Index",
    "string.LastIndexOf": "最后出现子串索引，对标 Go strings.LastIndex",
    "string.Split": "按分隔符分割，对标 Go strings.Split",
    "string.Trim": "去首尾空白，对标 Go strings.TrimSpace",
    "string.Replace": "替换所有子串，对标 Go strings.ReplaceAll",
    "string.Repeat": "重复 n 次，对标 Go strings.Repeat",
    "string.StartsWith": "判断前缀，对标 Go strings.HasPrefix",
    "string.EndsWith": "判断后缀，对标 Go strings.HasSuffix",
    "string.Len": "字符串长度，对标 Go len(s) / C strlen",
    "string.IsDigit": "所有字符均为数字，对标 C isdigit",
    "string.IsAlpha": "所有字符均为字母，对标 C isalpha",
    "string.IsSpace": "所有字符均为空白，对标 C isspace",
    "string.IsLower": "所有字符均为小写，对标 C islower",
    "string.IsUpper": "所有字符均为大写，对标 C isupper",
    "string.IsAlnum": "所有字符均为字母或数字，对标 C isalnum",
    "string.ContainsAny": "判断是否包含任意字符，对标 Go strings.ContainsAny",
    "string.ContainsRune": "判断是否包含字符，对标 Go strings.ContainsRune",
    "string.Count": "统计子串出现次数，对标 Go strings.Count",
    "string.IndexAny": "首次出现任意字符的索引，对标 Go strings.IndexAny",
    "string.LastIndexAny": "最后出现任意字符的索引，对标 Go strings.LastIndexAny",
    "string.EqualFold": "忽略大小写比较，对标 Go strings.EqualFold",
    "string.TrimLeft": "去除左侧字符集，对标 Go strings.TrimLeft",
    "string.TrimRight": "去除右侧字符集，对标 Go strings.TrimRight",
    "string.TrimPrefix": "去除前缀，对标 Go strings.TrimPrefix",
    "string.TrimSuffix": "去除后缀，对标 Go strings.TrimSuffix",
    "string.SplitN": "按分隔符分割最多 n 份，对标 Go strings.SplitN",
    "string.SplitAfter": "分割并保留分隔符，对标 Go strings.SplitAfter",
    "string.Title": "标题大写，对标 Go strings.Title",
    "string.Map": "按映射函数转换字符，对标 Go strings.Map",
    "string.ToUpperRune": "单个字符大写，对标 C toupper / Go unicode.ToUpper",
    "string.ToLowerRune": "单个字符小写，对标 C tolower / Go unicode.ToLower",
    "time.NowMs": "当前 Unix 毫秒",
    "time.Now": "当前时间，对标 Go time.Now",
    "time.FromUnixMs": "毫秒转 time",
    "time.Sleep": "毫秒睡眠，对标 Go time.Sleep",
    "time.Parse": "解析时间字符串，对标 Go time.Parse",
    "time.ToString": "格式化 YYYY-MM-DD HH:MM:SS",
    "time.Format": "strftime 风格格式化，对标 C strftime",
    "time.Unix": "秒+纳秒转 time，对标 Go time.Unix",
    "time.UnixMilli": "毫秒转 time，对标 Go time.UnixMilli",
    "time.Since": "与给定时间的差值毫秒，对标 Go time.Since",
    "time.UnixMicro": "微秒转 time，对标 Go time.UnixMicro",
    "time.UnixNano": "纳秒转 time，对标 Go time.UnixNano",
    "time.Until": "到给定时间的差值毫秒，对标 Go time.Until",
    "time.After": "判断是否在给定时间之后，对标 Go time.After",
    "time.Before": "判断是否在给定时间之前，对标 Go time.Before",
    "time.Date": "按年月日时分秒纳秒组装 time，UTC，对标 Go time.Date / C 手工日历计算",
    "time.Equal": "判断两个时间是否相等，对标 Go time.Equal",
    "time.Compare": "比较两个时间，返回 -1/0/1，对标 Go time.Compare",
    "time.Sub": "两个时间之差（毫秒），对标 Go time.Sub",
    "time.Add": "时间加毫秒，对标 Go time.Add",
    "time.Round": "时间四舍五入到给定精度，对标 Go time.Round",
    "time.Truncate": "时间截断到给定精度，对标 Go time.Truncate",
    "decimal.Parse": "解析十进制",
    "int8.Truncate": "截断为 int8",
    "int16.Truncate": "截断为 int16",
    "int32.Truncate": "截断为 int32",
    "ToString": "转字符串",
    "WrappingAdd": "绕环加法",
    "Round": "四舍五入",
    "Divide": "除法",
    "Push": "尾部追加",
    "Pop": "弹出末尾，空数组触发 R101",
    "Remove": "删除指定索引",
    "Clear": "清空",
    "ToString_arr": "转字符串",
    "Eq": "逐元素比较 ==",
    "Ne": "逐元素比较 !=",
    "Slice": "切片，动态浅拷贝 / 定长深拷贝",
    "Clear_map": "清空",
    "Remove_map": "删除键",
    "Eq_map": "比较 ==",
    "Ne_map": "比较 !=",
    "Step": "步进循环，防御性兜底",
    "Push_q": "入队",
    "Pop_q": "阻塞出队",
    "Close_q": "关闭队列",
    "Join": "等待线程结束",
    "TryPop": "非阻塞出队，空队列返回 null",
    "os.Args": "命令行参数 argv[1..]，对标 Go os.Args",
    "os.Chdir": "改变当前工作目录，对标 Go os.Chdir / C chdir",
    "os.Environ": "环境变量列表 KEY=VALUE，对标 Go os.Environ",
    "os.Exit": "终止进程，对标 Go os.Exit / C exit",
    "os.Getenv": "读环境变量，未设置返回空串，对标 Go os.Getenv / C getenv",
    "os.Getpid": "当前进程 ID，对标 Go os.Getpid / C getpid",
    "os.Getppid": "父进程 ID，Windows 下返回 0，对标 Go os.Getppid / C getppid",
    "os.Getwd": "当前工作目录，对标 Go os.Getwd / C getcwd",
    "os.Hostname": "主机名，失败返回空串，对标 Go os.Hostname / C gethostname",
    "os.Setenv": "设置环境变量，0=成功 1=失败，对标 Go os.Setenv / C setenv",
    "os.Unsetenv": "删除环境变量，0=成功 1=失败，对标 Go os.Unsetenv / C unsetenv",
    "os.UserHomeDir": "用户主目录，对标 Go os.UserHomeDir",
    "os.UserCacheDir": "用户缓存目录，对标 Go os.UserCacheDir",
    "os.Getpagesize": "系统页面大小，对标 Go os.Getpagesize",
    "os.Getuid": "当前用户 ID，对标 Go os.Getuid",
    "os.Getgid": "当前组 ID，对标 Go os.Getgid",
    "os.Geteuid": "当前有效用户 ID，对标 Go os.Geteuid",
    "os.Getegid": "当前有效组 ID，对标 Go os.Getegid",
    "os.Getgroups": "当前用户所属组列表，对标 Go os.Getgroups",
    "os.Executable": "可执行文件路径，对标 Go os.Executable",
    "strconv.Atoi": "字符串转 int，失败返回 0，对标 Go strconv.Atoi",
    "strconv.Itoa": "int 转字符串，对标 Go strconv.Itoa",
    "strconv.ParseBool": "字符串转 bool，失败返回 false，对标 Go strconv.ParseBool",
    "strconv.FormatBool": "bool 转字符串，对标 Go strconv.FormatBool",
    "strconv.ParseFloat": "字符串转 float，失败返回 0.0，对标 Go strconv.ParseFloat",
    "strconv.FormatFloat": "float 转字符串，对标 Go strconv.FormatFloat",
    "strconv.ParseInt": "字符串转 int（指定进制），失败返回 0，对标 Go strconv.ParseInt",
    "strconv.FormatInt": "int 转指定进制字符串，对标 Go strconv.FormatInt",
    "strconv.ParseUint": "字符串转无符号 int（指定进制），失败返回 0，对标 Go strconv.ParseUint",
    "strconv.FormatUint": "无符号 int 转指定进制字符串，对标 Go strconv.FormatUint",
    "strconv.CanBackquote": "判断能否作为反引号字符串，对标 Go strconv.CanBackquote",
    "strconv.Quote": "双引号括起来的 Go 字符串字面量，对标 Go strconv.Quote",
    "strconv.Unquote": "解释双引号字符串字面量，失败返回空串，对标 Go strconv.Unquote",
    "filepath.Join": "用平台分隔符连接路径，对标 Go filepath.Join",
    "filepath.Dir": "返回目录部分，对标 Go filepath.Dir",
    "filepath.Base": "返回最后一段文件名，对标 Go filepath.Base",
    "filepath.Ext": "返回扩展名，对标 Go filepath.Ext",
    "filepath.Split": "拆分为 (dir, file)，对标 Go filepath.Split",
    "filepath.Clean": "路径规范化，对标 Go filepath.Clean",
    "filepath.Abs": "转为绝对路径，对标 Go filepath.Abs",
    "filepath.IsAbs": "判断是否为绝对路径，对标 Go filepath.IsAbs",
    "filepath.EvalSymlinks": "求解符号链接，对标 Go filepath.EvalSymlinks",
    "filepath.Glob": "模式匹配文件列表，对标 Go filepath.Glob",
    "filepath.Rel": "计算相对路径，对标 Go filepath.Rel",
    "filepath.Walk": "遍历目录树，对标 Go filepath.Walk",
    "rand.Int": "[0, n) 范围随机整数，对标 Go rand.Intn",
    "rand.Float": "[0, 1) 范围随机浮点数，对标 Go rand.Float64",
    "rand.Seed": "设置随机种子，对标 Go rand.Seed",
    "sort.Ints": "整数切片排序，对标 Go sort.Ints",
    "sort.Strings": "字符串切片排序，对标 Go sort.Strings",
    "sort.Float64s": "浮点数切片排序，对标 Go sort.Float64s",
    "sort.IntsAreSorted": "判断整数切片是否已排序，对标 Go sort.IntsAreSorted",
    "sort.StringsAreSorted": "判断字符串切片是否已排序，对标 Go sort.StringsAreSorted",
    "sort.SearchInts": "二分查找整数，对标 Go sort.SearchInts",
    "sort.SearchStrings": "二分查找字符串，对标 Go sort.SearchStrings",
    "sort.Slice": "通用切片排序（带 less 函数），对标 Go sort.Slice",
    "sort.SliceStable": "稳定通用切片排序，对标 Go sort.SliceStable",
    "arr.Contains(x)": "判断数组是否包含元素（arr.Contains(x)），对标 Go slices.Contains",
    "arr.Index(x)": "查找元素索引（arr.Index(x)），对标 Go slices.Index",
    "arr.Sort()": "数组排序（arr.Sort() / arr.Sort(cmp)），对标 Go slices.Sort / sort.Slice",
    "arr.Sort(cmp)": "数组带比较函数排序，对标 Go sort.Slice，GAP #10",
    "slices.SortStable": "稳定数组排序，对标 Go slices.SortStable",
    "slices.Reverse": "数组反转，对标 Go slices.Reverse",
    "slices.Clone": "数组克隆，对标 Go slices.Clone",
    "slices.Concat": "数组拼接，对标 Go slices.Concat",
    "slices.Insert": "在指定位置插入元素，对标 Go slices.Insert",
    "slices.Delete": "删除指定范围元素，对标 Go slices.Delete",
    "maps.Keys": "获取 map 所有键，对标 Go maps.Keys",
    "maps.Values": "获取 map 所有值，对标 Go maps.Values",
    "maps.Clone": "克隆 map，对标 Go maps.Clone",
    "maps.Copy": "复制 map，对标 Go maps.Copy",
    "maps.DeleteFunc": "按条件删除 map 键，对标 Go maps.DeleteFunc",
    "maps.Equal": "判断两个 map 是否相等，对标 Go maps.Equal",
    "unique.Make": "创建唯一值句柄，对标 Go unique.Make",
    "unique.Value": "从句柄获取值，对标 Go unique.Value",
    "bytes.Contains": "判断字节片是否包含子片，对标 Go bytes.Contains",
    "bytes.Equal": "判断两个字节片是否相等，对标 Go bytes.Equal",
    "bytes.Index": "查找子片索引，对标 Go bytes.Index",
    "bytes.Split": "按分隔符分割字节片，对标 Go bytes.Split",
    "bytes.TrimSpace": "去除字节片首尾空白，对标 Go bytes.TrimSpace",
    "bytes.ToUpper": "字节片转大写，对标 Go bytes.ToUpper",
    "bytes.ToLower": "字节片转小写，对标 Go bytes.ToLower",
    "bytes.Repeat": "字节片重复，对标 Go bytes.Repeat",
    "bufio.NewReader": "创建缓冲读取器，对标 Go bufio.NewReader",
    "bufio.NewWriter": "创建缓冲写入器，对标 Go bufio.NewWriter",
    "bufio.Scanner": "创建扫描器，对标 Go bufio.NewScanner",
    "log.Print": "打印日志，对标 Go log.Print",
    "log.Printf": "按格式打印日志，对标 Go log.Printf",
    "log.Fatal": "打印致命日志并退出，对标 Go log.Fatal",
    "errors.New": "创建错误值，对标 Go errors.New",
    "errors.Is": "判断错误是否匹配目标，对标 Go errors.Is",
    "errors.As": "将错误转换为目标类型，对标 Go errors.As",
    "sync.Mutex.Lock": "加锁，对标 Go sync.Mutex.Lock",
    "sync.Mutex.Unlock": "解锁，对标 Go sync.Mutex.Unlock",
    "sync.RWMutex": "读写锁，对标 Go sync.RWMutex",
    "sync.WaitGroup": "等待组，对标 Go sync.WaitGroup",
    "sync.Once": "一次性执行，对标 Go sync.Once",
    "atomic.AddInt": "原子加法，对标 Go atomic.AddInt64",
    "atomic.LoadInt": "原子读取，对标 Go atomic.LoadInt64",
    "atomic.StoreInt": "原子写入，对标 Go atomic.StoreInt64",
    "atomic.CompareAndSwapInt": "原子比较并交换，对标 Go atomic.CompareAndSwapInt64",
    "reflect.TypeOf": "获取类型名，对标 Go reflect.TypeOf",
    "reflect.ValueOf": "获取值，对标 Go reflect.ValueOf",
    "reflect.NumField": "获取字段数量，对标 Go reflect.NumField",
    "archive": "归档文件读写（tar/zip）",
    "compress": "压缩/解压缩（bzip2/gzip/zlib/zstd 等）",
    "container": "堆/链表/环形列表",
    "context": "上下文传递",
    "crypto": "对称/非对称加密、哈希、TLS、证书",
    "debug": "二进制文件调试信息",
    "embed": "文件嵌入",
    "encoding": "各类编解码",
    "expvar": "公开变量接口",
    "flag": "命令行标志解析",
    "go": "Go 源码工具链（axlang 自用）",
    "hash": "哈希校验和",
    "image": "2D 图像编解码",
    "index": "后缀数组子串查找",
    "iter": "迭代器序列",
    "math/big": "任意精度算术",
    "math/bits": "位操作",
    "math/cmplx": "复数数学函数",
    "mime": "MIME 协议支持",
    "net": "网络 I/O（URL/HTTP/SMTP 等）",
    "os/exec": "外部命令执行",
    "os/signal": "信号处理",
    "os/user": "用户账户查询",
    "path": "斜杠分隔路径操作",
    "plugin": "插件加载",
    "regexp": "正则表达式",
    "runtime": "运行时交互",
    "syscall": "低级操作系统原语",
    "testing": "自动化测试辅助",
    "text": "文本扫描与表格对齐",
    "time/tzdata": "时区数据库嵌入",
    "unicode": "Unicode 码点属性与 UTF 编解码",
    "unsafe": "绕过类型安全",
    "uuid": "UUID 生成与操作",
    "weak": "弱引用",
}


def strip_backticks(s):
    return s.strip().strip("`").strip()


def is_separator_row(line):
    """判断是否为 Markdown 表格分隔行（全是 --- : --- 等）"""
    stripped = line.strip()
    if not stripped.startswith("|") or not stripped.endswith("|"):
        return False
    inner = stripped[1:-1]
    return bool(re.match(r"^[\s\-:|]*$", inner))


def is_header_row(line):
    """判断是否为表头行（包含表头关键字，但不包含 ax库函数名 说明已处理过）"""
    stripped = line.strip()
    if not stripped.startswith("|") or not stripped.endswith("|"):
        return False
    # 如果已经有 ax库函数名，说明是已处理过的表头，跳过
    if "ax库函数名" in stripped or "函数功能/用途" in stripped:
        return True
    # 表头关键字
    header_keywords = ["函数", "状态", "说明", "签名", "对标", "Go 包", "代表子包", "对标 axlang", "优先级", "标记", "阶段", "已完成", "待完成"]
    return any(k in stripped for k in header_keywords)


def process_data_row(line):
    """处理数据行：插入两列并填充内容。"""
    stripped = line.strip()
    newline = "\n" if line.endswith("\n") else ""

    # 分割单元格
    parts = [p.strip() for p in stripped.strip("|").split("|")]
    num_cols = len(parts)

    if num_cols < 3:
        return line

    # 确定插入位置：在 "状态" 列之前
    status_idx = None
    for i, p in enumerate(parts):
        if p in ("✅", "🔄", "⬜", "状态"):
            status_idx = i
            break

    if status_idx is None:
        return line

    # 提取函数名（第一列，去反引号）
    func_name_raw = strip_backticks(parts[0])

    # ax库函数名：直接使用函数名
    ax_name = func_name_raw

    # 函数功能/用途：优先从说明列（状态列之后）获取
    func_desc = ""
    if status_idx + 1 < len(parts):
        src_desc = parts[status_idx + 1]
        if src_desc and src_desc not in ("✅", "🔄", "⬜"):
            func_desc = strip_backticks(src_desc)
    if not func_desc:
        func_desc = DEFAULT_DESC.get(ax_name, f"{ax_name} 函数")

    # 构建新的单元格列表：在 status_idx 位置插入两列
    new_parts = parts[:status_idx] + [ax_name, func_desc] + parts[status_idx:]

    return "| " + " | ".join(new_parts) + " |" + newline


def process_header_row(line):
    """处理表头行：在 状态 列之前插入两列标题。"""
    stripped = line.strip()
    newline = "\n" if line.endswith("\n") else ""

    parts = [p.strip() for p in stripped.strip("|").split("|")]

    # 找到 状态 列的位置
    status_idx = None
    for i, p in enumerate(parts):
        if p == "状态":
            status_idx = i
            break

    if status_idx is None:
        return line

    # 插入两列标题
    new_parts = parts[:status_idx] + ["ax库函数名", "函数功能/用途"] + parts[status_idx:]
    return "| " + " | ".join(new_parts) + " |" + newline


def process_separator_row(line, expected_cols):
    """处理分隔行：确保列数与数据行一致。"""
    stripped = line.strip()
    newline = "\n" if line.endswith("\n") else ""

    parts = [p.strip() for p in stripped.strip("|").split("|")]
    # 调整到 expected_cols 列
    while len(parts) < expected_cols:
        parts.append("------")
    while len(parts) > expected_cols:
        parts.pop()
    return "| " + " | ".join(parts) + " |" + newline


def main():
    with open(FILE, "r", encoding="utf-8") as f:
        lines = f.readlines()

    new_lines = []
    data_filled = 0
    header_fixed = 0
    sep_fixed = 0

    # 先遍历确定每个表格的最大列数（处理后的）
    # 我们需要先处理数据行来知道每个表的列数，然后再处理分隔行
    # 采用两遍扫描：第一遍处理数据行和表头，记录每个表的列数；第二遍处理分隔行

    # 第一遍：处理数据行和表头行
    pass1_lines = []
    table_col_counts = []  # 每个表格的列数（处理后）
    current_table_cols = None

    for line in lines:
        if is_separator_row(line):
            # 分隔行暂时保留，第二遍处理
            pass1_lines.append(("SEP", line))
            continue
        if is_header_row(line):
            new_line = process_header_row(line)
            if new_line != line:
                header_fixed += 1
            # 计算表头列数
            stripped = new_line.strip()
            parts = [p.strip() for p in stripped.strip("|").split("|")]
            current_table_cols = len(parts)
            table_col_counts.append(current_table_cols)
            pass1_lines.append(("HEADER", new_line))
            continue
        if line.strip().startswith("|") and line.strip().endswith("|"):
            # 数据行
            # 如果已经有 ax库函数名 列，跳过
            if "ax库函数名" in line.strip():
                pass1_lines.append(("DATA", line))
                continue
            new_line = process_data_row(line)
            if new_line != line:
                data_filled += 1
            # 计算列数
            stripped = new_line.strip()
            parts = [p.strip() for p in stripped.strip("|").split("|")]
            current_table_cols = len(parts)
            pass1_lines.append(("DATA", new_line))
            continue
        pass1_lines.append(("OTHER", line))

    # 第二遍：处理分隔行，调整列数
    sep_index = 0
    final_lines = []
    for kind, line in pass1_lines:
        if kind == "SEP":
            # 使用最近的表格列数
            expected = current_table_cols if current_table_cols else len([p for p in line.strip().strip("|").split("|") if p.strip()])
            new_line = process_separator_row(line, expected)
            if new_line != line:
                sep_fixed += 1
            final_lines.append(new_line)
        else:
            final_lines.append(line)
            # 更新 current_table_cols 从 HEADER 或 DATA 行
            stripped = line.strip()
            if stripped.startswith("|") and stripped.endswith("|"):
                parts = [p.strip() for p in stripped.strip("|").split("|")]
                current_table_cols = len(parts)

    with open(FILE, "w", encoding="utf-8") as f:
        f.writelines(final_lines)

    print(f"处理完成：数据行填充 {data_filled} 行，表头修正 {header_fixed} 行，分隔行修正 {sep_fixed} 行。")


if __name__ == "__main__":
    main()
