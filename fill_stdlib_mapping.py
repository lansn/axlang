#!/usr/bin/env python3
# fill_stdlib_mapping.py —— 基于 C 标准库和 Go 标准库，为 STDLIB-STATUS.json 的每个函数
# 填充 c_counterpart（C 对应函数）、go_counterpart（Go 对应函数/方法）、function（中文功能说明）、description（额外说明）。
#
# 规则：
#   - 对标 C：C 标准库具体函数名（如 fabs、strlen、strcmp），无对应则 "—"
#   - 对标 Go：Go 标准库具体函数/方法名（如 math.Abs、strings.Contains），无对应则 "—"
#   - 函数功能：中文说明，不能空
#   - description：额外说明，可空

import json
from pathlib import Path

JSON_PATH = Path("STDLIB-STATUS.json")

# 模块 -> {函数名: {c, go, function, description}}
MAPPING = {
    "io": {
        "read_line": {"c": "fgets", "go": "bufio.Scanner.Text", "function": "逐行读取文件内容，返回一行字符串", "description": "对标 C fgets，Go bufio.Scanner.Text；读取直到换行符或文件结束"},
        "read_file": {"c": "fread", "go": "os.ReadFile", "function": "读取整个文件内容并返回字符串", "description": "对标 C fread，Go os.ReadFile；一次性读取全部文件到内存"},
        "write": {"c": "fwrite/puts", "go": "fmt.Fprint/io.WriteString", "function": "将字符串写入标准输出", "description": "对标 C fwrite/puts，Go fmt.Fprint；不自动换行"},
        "write_line": {"c": "fputs/puts", "go": "fmt.Fprintln", "function": "将字符串写入标准输出并换行", "description": "对标 C fputs/puts，Go fmt.Fprintln；自动追加换行符"},
    },
    "fmt": {
        "print": {"c": "printf", "go": "fmt.Print", "function": "格式化输出任意参数到标准输出", "description": "对标 C printf，Go fmt.Print；参数间自动添加空格"},
        "println": {"c": "printf/puts", "go": "fmt.Println", "function": "格式化输出并换行", "description": "对标 C printf/puts，Go fmt.Println；自动换行"},
        "printf": {"c": "printf", "go": "fmt.Printf", "function": "按格式字符串格式化输出", "description": "对标 C printf，Go fmt.Printf；编译期核对占位符"},
        "sprintf": {"c": "sprintf", "go": "fmt.Sprintf", "function": "按格式字符串格式化并返回字符串", "description": "对标 C sprintf，Go fmt.Sprintf；结果以字符串返回而非输出"},
        "sprint": {"c": "snprintf/sprintf", "go": "fmt.Sprint", "function": "将任意参数格式化为字符串返回", "description": "对标 C snprintf，Go fmt.Sprint；不输出，仅返回字符串"},
        "fprintf": {"c": "fprintf", "go": "fmt.Fprintf", "function": "将格式化内容写入文件句柄", "description": "对标 C fprintf，Go fmt.Fprintf；第一个参数为文件对象"},
        "errorf": {"c": "fprintf(stderr)", "go": "fmt.Errorf", "function": "格式化错误信息并返回 error 类型", "description": "对标 C fprintf(stderr)，Go fmt.Errorf；用于构造错误对象"},
        "scanf": {"c": "scanf", "go": "fmt.Scan", "function": "从标准输入读取并按格式解析", "description": "对标 C scanf，Go fmt.Scan；从 stdin 读取格式化输入"},
    },
    "math": {
        "abs": {"c": "fabs", "go": "math.Abs", "function": "求浮点数的绝对值", "description": "对标 C fabs，Go math.Abs；返回 |x|"},
        "max": {"c": "fmax", "go": "math.Max", "function": "返回两个浮点数中的较大值", "description": "对标 C fmax，Go math.Max"},
        "min": {"c": "fmin", "go": "math.Min", "function": "返回两个浮点数中的较小值", "description": "对标 C fmin，Go math.Min"},
        "sqrt": {"c": "sqrt", "go": "math.Sqrt", "function": "求平方根", "description": "对标 C sqrt，Go math.Sqrt；x 必须非负"},
        "pow": {"c": "pow", "go": "math.Pow", "function": "求幂次 x^y", "description": "对标 C pow，Go math.Pow"},
        "exp": {"c": "exp", "go": "math.Exp", "function": "求自然指数 e^x", "description": "对标 C exp，Go math.Exp"},
        "log": {"c": "log", "go": "math.Log", "function": "求自然对数 ln(x)", "description": "对标 C log，Go math.Log；x 必须正数"},
        "sin": {"c": "sin", "go": "math.Sin", "function": "正弦函数", "description": "对标 C sin，Go math.Sin；参数单位为弧度"},
        "cos": {"c": "cos", "go": "math.Cos", "function": "余弦函数", "description": "对标 C cos，Go math.Cos；参数单位为弧度"},
        "tan": {"c": "tan", "go": "math.Tan", "function": "正切函数", "description": "对标 C tan，Go math.Tan；参数单位为弧度"},
        "ceil": {"c": "ceil", "go": "math.Ceil", "function": "向上取整", "description": "对标 C ceil，Go math.Ceil；返回不小于 x 的最小整数"},
        "floor": {"c": "floor", "go": "math.Floor", "function": "向下取整", "description": "对标 C floor，Go math.Floor；返回不大于 x 的最大整数"},
        "round": {"c": "round", "go": "math.Round", "function": "四舍五入取整", "description": "对标 C round，Go math.Round；半远离零"},
        "sign": {"c": "—", "go": "—", "function": "返回数值的符号（正1/负-1/零0）", "description": "C 标准库无直接对应函数，需自行实现；Go 亦无 math.Sign"},
        "mod": {"c": "fmod", "go": "math.Mod", "function": "浮点数取余（结果符号同被除数）", "description": "对标 C fmod，Go math.Mod；y 为 0 时返回 NaN"},
        "equals": {"c": "—", "go": "—", "function": "在容差范围内判断两个浮点数是否相等", "description": "C/Go 标准库无直接对应函数，需按 |a-b| <= eps 自行判断"},
        "compare": {"c": "—", "go": "—", "function": "比较两个浮点数大小（-1/0/1）", "description": "C/Go 标准库无直接对应函数，需手动比较"},
        "exp2": {"c": "exp2", "go": "—", "function": "求以 2 为底的指数 2^x", "description": "对标 C exp2，Go 标准库无 math.Exp2"},
        "log10": {"c": "log10", "go": "math.Log10", "function": "求以 10 为底的对数", "description": "对标 C log10，Go math.Log10"},
        "log2": {"c": "log2", "go": "math.Log2", "function": "求以 2 为底的对数", "description": "对标 C log2，Go math.Log2"},
        "log1p": {"c": "log1p", "go": "math.Log1p", "function": "求 ln(1+x)，适用于 x 很小时的精度保持", "description": "对标 C log1p，Go math.Log1p"},
        "sinh": {"c": "sinh", "go": "math.Sinh", "function": "双曲正弦", "description": "对标 C sinh，Go math.Sinh"},
        "cosh": {"c": "cosh", "go": "math.Cosh", "function": "双曲余弦", "description": "对标 C cosh，Go math.Cosh"},
        "tanh": {"c": "tanh", "go": "math.Tanh", "function": "双曲正切", "description": "对标 C tanh，Go math.Tanh"},
        "asin": {"c": "asin", "go": "math.Asin", "function": "反正弦（arc sine）", "description": "对标 C asin，Go math.Asin；返回弧度"},
        "acos": {"c": "acos", "go": "math.Acos", "function": "反余弦（arc cosine）", "description": "对标 C acos，Go math.Acos；返回弧度"},
        "atan": {"c": "atan", "go": "math.Atan", "function": "反正切（arc tangent）", "description": "对标 C atan，Go math.Atan；返回弧度"},
        "atan2": {"c": "atan2", "go": "math.Atan2", "function": "带符号的反正切 atan2(y, x)", "description": "对标 C atan2，Go math.Atan2；考虑象限"},
        "hypot": {"c": "hypot", "go": "math.Hypot", "function": "求斜边长 sqrt(x^2+y^2)，避免溢出", "description": "对标 C hypot，Go math.Hypot"},
        "cbrt": {"c": "cbrt", "go": "math.Cbrt", "function": "求立方根", "description": "对标 C cbrt，Go math.Cbrt"},
        "trunc": {"c": "trunc", "go": "math.Trunc", "function": "向零取整（截断小数部分）", "description": "对标 C trunc，Go math.Trunc"},
        "is_nan": {"c": "isnan", "go": "math.IsNaN", "function": "判断是否为非数（NaN）", "description": "对标 C isnan，Go math.IsNaN；名称为 is_nan（避免 C 关键字 isnan 冲突）"},
        "is_na_n": {"c": "isnan", "go": "math.IsNaN", "function": "判断是否为非数（NaN）", "description": "对标 C isnan，Go math.IsNaN；axlang 将 IsNaN 转为 snake_case 后名称为 is_na_n"},
        "is_inf": {"c": "isinf", "go": "math.IsInf", "function": "判断是否为无穷大，sign 参数控制正/负/任意", "description": "对标 C isinf，Go math.IsInf；sign>0 只判 +Inf，sign<0 只判 -Inf，sign==0 判任意"},
        "frexp": {"c": "frexp", "go": "math.Frexp", "function": "将浮点数分解为尾数和 2 的幂指数", "description": "对标 C frexpf，Go math.Frexp；axlang 暂不支持指针参数，分两步调用 ax_math_frexp / ax_math_frexp_exp"},
        "ldexp": {"c": "ldexp", "go": "math.Ldexp", "function": "计算 x * 2^exp，即从尾数和指数重建浮点数", "description": "对标 C ldexp，Go math.Ldexp；与 frexp 互逆"},
        "signbit": {"c": "signbit", "go": "math.Signbit", "function": "判断浮点数符号位是否为负（含 -0 / -NaN）", "description": "对标 C signbit，Go math.Signbit"},
        "copysign": {"c": "copysign", "go": "math.Copysign", "function": "返回 x 的幅值和 y 的符号组成的浮点数", "description": "对标 C copysign，Go math.Copysign"},
        "dim": {"c": "fdim", "go": "math.Dim", "function": "返回 max(x - y, 0)，即正差值", "description": "对标 C fdim，Go math.Dim"},
    },
    "json": {
        "parse": {"c": "—", "go": "json.Unmarshal", "function": "将 JSON 字符串解析为 map<string, object>", "description": "C 标准库无 JSON 解析器，Go 对标 encoding/json.Unmarshal"},
        "stringify": {"c": "—", "go": "json.Marshal", "function": "将对象序列化为 JSON 字符串", "description": "C 标准库无 JSON 序列化，Go 对标 encoding/json.Marshal"},
        "marshal_indent": {"c": "—", "go": "json.MarshalIndent", "function": "将对象序列化为带缩进的 JSON 字符串", "description": "C 标准库无对应，Go 对标 encoding/json.MarshalIndent"},
        "valid": {"c": "—", "go": "json.Valid", "function": "判断字符串是否为合法 JSON", "description": "C 标准库无对应，Go 对标 encoding/json.Valid"},
    },
    "file": {
        "open": {"c": "fopen", "go": "os.Open", "function": "打开文件并返回文件句柄", "description": "对标 C fopen，Go os.Open；返回 result<File> 类型"},
        "create": {"c": "fopen(w)", "go": "os.Create", "function": "创建或截断文件并返回句柄", "description": "对标 C fopen(w 模式)，Go os.Create"},
        "append": {"c": "fopen(a)", "go": "os.OpenFile(append)", "function": "以追加模式打开文件", "description": "对标 C fopen(a 模式)，Go os.OpenFile 追加标志"},
        "remove": {"c": "remove", "go": "os.Remove", "function": "删除文件或目录", "description": "对标 C remove，Go os.Remove"},
        "rename": {"c": "rename", "go": "os.Rename", "function": "重命名或移动文件", "description": "对标 C rename，Go os.Rename"},
        "exists": {"c": "access/stat", "go": "os.Stat", "function": "判断路径是否存在", "description": "对标 C access/stat，Go os.Stat；返回 1 表示存在，0 表示不存在"},
        "mkdir": {"c": "mkdir", "go": "os.Mkdir", "function": "创建目录", "description": "对标 C mkdir，Go os.Mkdir"},
        "read_all": {"c": "fread", "go": "os.ReadFile", "function": "读取文件全部内容并返回字符串", "description": "对标 C fread，Go os.ReadFile；返回 result<string>"},
        "write_all": {"c": "fwrite", "go": "os.WriteFile", "function": "将字符串全部写入文件", "description": "对标 C fwrite，Go os.WriteFile；返回写入字节数"},
        "is_dir": {"c": "stat/S_ISDIR", "go": "os.IsDir/Stat.IsDir", "function": "判断路径是否为目录", "description": "对标 C stat+S_ISDIR 宏，Go os.IsDir 或 os.Stat().IsDir()"},
        "is_file": {"c": "stat/S_ISREG", "go": "Stat.IsRegular", "function": "判断路径是否为普通文件", "description": "对标 C stat+S_ISREG 宏，Go os.FileInfo.IsRegular()"},
        "size": {"c": "stat/st_size", "go": "Stat.Size", "function": "获取文件大小（字节）", "description": "对标 C stat 结构体 st_size 字段，Go os.FileInfo.Size()"},
        "mod_time": {"c": "stat/st_mtime", "go": "Stat.ModTime", "function": "获取文件最后修改时间", "description": "对标 C stat 结构体 st_mtime 字段，Go os.FileInfo.ModTime()"},
        "chmod": {"c": "chmod", "go": "os.Chmod", "function": "修改文件权限模式", "description": "对标 C chmod，Go os.Chmod"},
        "chtimes": {"c": "utime", "go": "os.Chtimes", "function": "修改文件访问时间和修改时间", "description": "对标 C utime，Go os.Chtimes"},
        "temp_file": {"c": "tmpfile", "go": "os.CreateTemp", "function": "创建临时文件并返回句柄", "description": "对标 C tmpfile，Go os.CreateTemp；自动生成唯一临时文件名"},
    },
    "db": {
        "query": {"c": "—", "go": "—", "function": "执行 SQL 查询并返回结果集", "description": "axlang 特有模块，无标准 C/Go 对应；依赖底层数据库驱动实现"},
    },
    "string": {
        "from_codepoint": {"c": "—", "go": "—", "function": "根据 Unicode 码点创建字符串", "description": "C 标准库无直接对应，Go 亦无标准库函数，需手动 UTF-8 编码"},
        "compare": {"c": "strcmp", "go": "strings.Compare", "function": "按字典序比较两个字符串（-1/0/1）", "description": "对标 C strcmp，Go strings.Compare"},
        "fields": {"c": "strtok", "go": "strings.Fields", "function": "按空白字符分割字符串为字符串切片", "description": "对标 C strtok，Go strings.Fields；按任意空白符拆分"},
        "to_upper": {"c": "toupper", "go": "strings.ToUpper", "function": "将字符串转换为大写", "description": "对标 C toupper（逐字符），Go strings.ToUpper；仅 ASCII"},
        "to_lower": {"c": "tolower", "go": "strings.ToLower", "function": "将字符串转换为小写", "description": "对标 C tolower（逐字符），Go strings.ToLower；仅 ASCII"},
        "contains": {"c": "strstr", "go": "strings.Contains", "function": "判断字符串是否包含子串", "description": "对标 C strstr，Go strings.Contains"},
        "contains_any": {"c": "strpbrk", "go": "strings.ContainsAny", "function": "判断字符串是否包含任意给定字符", "description": "对标 C strpbrk，Go strings.ContainsAny"},
        "contains_rune": {"c": "—", "go": "strings.ContainsRune", "function": "判断字符串是否包含指定 Unicode 码点", "description": "C 标准库无直接对应，Go 对标 strings.ContainsRune"},
        "count": {"c": "—", "go": "strings.Count", "function": "统计子串在字符串中出现的次数（非重叠）", "description": "C 标准库无直接对应，Go 对标 strings.Count"},
        "index_of": {"c": "strstr", "go": "strings.Index", "function": "返回子串首次出现的索引，未找到返回 -1", "description": "对标 C strstr（需计算偏移），Go strings.Index"},
        "index_any": {"c": "strpbrk", "go": "strings.IndexAny", "function": "返回任意给定字符首次出现的索引", "description": "对标 C strpbrk，Go strings.IndexAny"},
        "last_index_of": {"c": "strrstr（非标准）", "go": "strings.LastIndex", "function": "返回子串最后一次出现的索引，未找到返回 -1", "description": "C 标准库无 strrstr，Go 对标 strings.LastIndex"},
        "last_index_any": {"c": "—", "go": "strings.LastIndexAny", "function": "返回任意给定字符最后一次出现的索引", "description": "C 标准库无直接对应，Go 对标 strings.LastIndexAny"},
        "split": {"c": "strtok", "go": "strings.Split", "function": "按分隔符分割字符串为字符串切片", "description": "对标 C strtok，Go strings.Split；分隔符本身不保留"},
        "trim": {"c": "—", "go": "strings.TrimSpace", "function": "去除字符串首尾空白字符", "description": "C 标准库无直接对应，Go 对标 strings.TrimSpace（仅空白符）"},
        "trim_left": {"c": "—", "go": "strings.TrimLeft", "function": "去除字符串左侧指定字符集", "description": "C 标准库无直接对应，Go 对标 strings.TrimLeft"},
        "trim_right": {"c": "—", "go": "strings.TrimRight", "function": "去除字符串右侧指定字符集", "description": "C 标准库无直接对应，Go 对标 strings.TrimRight"},
        "trim_prefix": {"c": "—", "go": "strings.TrimPrefix", "function": "去除字符串指定前缀", "description": "C 标准库无直接对应，Go 对标 strings.TrimPrefix"},
        "trim_suffix": {"c": "—", "go": "strings.TrimSuffix", "function": "去除字符串指定后缀", "description": "C 标准库无直接对应，Go 对标 strings.TrimSuffix"},
        "split_n": {"c": "—", "go": "strings.SplitN", "function": "按分隔符分割为最多 n 个字符串", "description": "C 标准库无直接对应，Go 对标 strings.SplitN"},
        "split_after": {"c": "—", "go": "strings.SplitAfter", "function": "按分隔符分割，保留分隔符在每段末尾", "description": "C 标准库无直接对应，Go 对标 strings.SplitAfter"},
        "title": {"c": "—", "go": "strings.Title", "function": "将字符串转换为标题格式（每个单词首字母大写）", "description": "C 标准库无直接对应，Go 对标 strings.Title（Go 1.18+ 已废弃，建议 cases.Title）"},
        "map": {"c": "—", "go": "strings.Map", "function": "对字符串每个字符应用映射函数并返回新字符串", "description": "C 标准库无直接对应，Go 对标 strings.Map；axlang 不支持闭包，实际受限"},
        "replace": {"c": "—", "go": "strings.Replace", "function": "将字符串中旧子串替换为新子串", "description": "C 标准库无直接对应，Go 对标 strings.Replace；支持限制替换次数"},
        "repeat": {"c": "—", "go": "strings.Repeat", "function": "将字符串重复 n 次", "description": "C 标准库无直接对应，Go 对标 strings.Repeat"},
        "starts_with": {"c": "strncmp", "go": "strings.HasPrefix", "function": "判断字符串是否以指定前缀开头", "description": "对标 C strncmp，Go strings.HasPrefix"},
        "ends_with": {"c": "strncmp", "go": "strings.HasSuffix", "function": "判断字符串是否以指定后缀结尾", "description": "对标 C strncmp，Go strings.HasSuffix"},
        "len": {"c": "strlen", "go": "len（内置）", "function": "返回字符串的字节长度", "description": "对标 C strlen，Go 内置 len()；返回 byte 数而非 rune 数"},
        "is_digit": {"c": "isdigit", "go": "unicode.IsDigit", "function": "判断字符串是否全部由数字组成", "description": "对标 C isdigit，Go unicode.IsDigit（逐字符判断）"},
        "is_alpha": {"c": "isalpha", "go": "unicode.IsLetter", "function": "判断字符串是否全部由字母组成", "description": "对标 C isalpha，Go unicode.IsLetter"},
        "is_space": {"c": "isspace", "go": "unicode.IsSpace", "function": "判断字符串是否全部由空白字符组成", "description": "对标 C isspace，Go unicode.IsSpace"},
        "is_lower": {"c": "islower", "go": "unicode.IsLower", "function": "判断字符串是否全部由小写字母组成", "description": "对标 C islower，Go unicode.IsLower"},
        "is_upper": {"c": "isupper", "go": "unicode.IsUpper", "function": "判断字符串是否全部由大写字母组成", "description": "对标 C isupper，Go unicode.IsUpper"},
        "is_alnum": {"c": "isalnum", "go": "unicode.IsLetter/IsNumber", "function": "判断字符串是否全部由字母或数字组成", "description": "对标 C isalnum，Go unicode.IsLetter 或 unicode.IsNumber"},
        "equal_fold": {"c": "strcasecmp", "go": "strings.EqualFold", "function": "忽略大小写比较两个字符串是否相等", "description": "对标 C strcasecmp，Go strings.EqualFold；UTF-8 忽略大小写比较"},
        "to_upper_rune": {"c": "toupper", "go": "unicode.ToUpper", "function": "将单个 Unicode 码点转换为大写", "description": "对标 C toupper，Go unicode.ToUpper；返回 int 类型码点"},
        "to_lower_rune": {"c": "tolower", "go": "unicode.ToLower", "function": "将单个 Unicode 码点转换为小写", "description": "对标 C tolower，Go unicode.ToLower；返回 int 类型码点"},
    },
    "time": {
        "to_string": {"c": "ctime/strftime", "go": "time.Time.String", "function": "将时间格式化为默认字符串表示", "description": "对标 C ctime/strftime，Go time.Time.String()；输出类似 '2006-01-02 15:04:05'"},
        "format": {"c": "strftime", "go": "time.Time.Format", "function": "按自定义格式字符串格式化时间", "description": "对标 C strftime，Go time.Time.Format；axlang 使用 Go 风格格式串（如 '2006-01-02'）"},
        "now_ms": {"c": "clock_gettime/time", "go": "time.Now().UnixMilli", "function": "返回当前 Unix 时间戳（毫秒）", "description": "对标 C clock_gettime/time，Go time.Now().UnixMilli()；返回 int 毫秒数"},
        "now": {"c": "time", "go": "time.Now", "function": "返回当前本地时间", "description": "对标 C time()，Go time.Now()；返回 time 类型（C 层 int64_t 毫秒）"},
        "from_unix_ms": {"c": "—", "go": "time.UnixMilli", "function": "根据毫秒级 Unix 时间戳创建时间", "description": "C 标准库无直接对应，Go 对标 time.UnixMilli()"},
        "sleep": {"c": "sleep/usleep", "go": "time.Sleep", "function": "使当前协程/线程休眠指定毫秒数", "description": "对标 C sleep/usleep，Go time.Sleep；阻塞当前执行流"},
        "parse": {"c": "strptime", "go": "time.Parse", "function": "解析格式化的时间字符串为 time 对象", "description": "对标 C strptime，Go time.Parse；返回 result<time> 类型，解析失败返回错误"},
        "unix": {"c": "—", "go": "time.Unix", "function": "根据秒和纳秒创建时间", "description": "C 标准库无直接对应，Go 对标 time.Unix(sec, nsec)"},
        "unix_milli": {"c": "—", "go": "time.UnixMilli", "function": "根据毫秒创建时间", "description": "C 标准库无直接对应，Go 对标 time.UnixMilli(ms)"},
        "unix_micro": {"c": "—", "go": "time.UnixMicro", "function": "根据微秒创建时间", "description": "C 标准库无直接对应，Go 对标 time.UnixMicro(us)"},
        "unix_nano": {"c": "—", "go": "time.UnixNano", "function": "根据纳秒创建时间", "description": "C 标准库无直接对应，Go 对标 time.UnixNano(nsec)"},
        "since": {"c": "difftime", "go": "time.Since", "function": "返回从指定时间到当前时间的毫秒差", "description": "对标 C difftime，Go time.Since；返回 int 毫秒数，当前时间减去参数时间"},
        "date": {"c": "mktime", "go": "time.Date", "function": "按年月日时分秒纳秒创建时间", "description": "对标 C mktime，Go time.Date；参数均为 int 类型"},
        "until": {"c": "difftime", "go": "time.Until", "function": "返回从当前时间到指定时间的毫秒差", "description": "对标 C difftime，Go time.Until；返回 int，参数时间减去当前时间"},
        "after": {"c": "—", "go": "time.After/Time.After", "function": "判断当前时间是否在参数时间之后", "description": "C 标准库无直接对应，Go 对标 time.Time.After() 方法"},
        "before": {"c": "—", "go": "Time.Before", "function": "判断当前时间是否在参数时间之前", "description": "C 标准库无直接对应，Go 对标 time.Time.Before() 方法"},
        "equal": {"c": "—", "go": "Time.Equal", "function": "判断两个时间是否相等", "description": "C 标准库无直接对应，Go 对标 time.Time.Equal() 方法"},
        "compare": {"c": "difftime", "go": "Time.Compare", "function": "比较两个时间大小（-1/0/1）", "description": "对标 C difftime，Go time.Time.Compare()；返回 -1/0/1"},
        "sub": {"c": "difftime", "go": "Time.Sub", "function": "返回两个时间的差值（毫秒）", "description": "对标 C difftime，Go time.Time.Sub()；返回当前时间减去参数时间的毫秒数"},
        "add": {"c": "—", "go": "Time.Add", "function": "返回当前时间加上指定毫秒数后的时间", "description": "C 标准库无直接对应，Go 对标 time.Time.Add(duration)"},
        "round": {"c": "—", "go": "Time.Round", "function": "将时间四舍五入到指定的时间段整数倍", "description": "C 标准库无直接对应，Go 对标 time.Time.Round() 方法"},
        "truncate": {"c": "—", "go": "time.Truncate", "function": "将时间向下取整到指定的时间段整数倍", "description": "C 标准库无直接对应，Go 对标 time.Truncate() 方法"},
    },
    "decimal": {
        "parse": {"c": "—", "go": "—", "function": "将字符串解析为高精度十进制数", "description": "axlang 特有模块，无标准 C/Go 对应；依赖自定义 decimal 实现"},
        "truncate": {"c": "—", "go": "—", "function": "将十进制数截断为整数（int8/int16/int32）", "description": "axlang 特有模块，无标准 C/Go 对应；重载支持三种整数宽度"},
    },
    "int/float/bool/enum": {
        "to_string": {"c": "—", "go": "strconv.Itoa/FormatFloat", "function": "将数值转换为字符串", "description": "C 标准库需 sprintf/itoa，Go 对标 strconv 包；泛型类型方法"},
        "wrapping_add": {"c": "—", "go": "—", "function": "带环绕的加法（溢出时回绕）", "description": "C 标准库无直接对应，Go 亦无标准库函数；axlang 语言级泛型方法"},
        "round": {"c": "round", "go": "math.Round", "function": "将浮点数四舍五入到指定小数位数", "description": "对标 C round，Go math.Round；泛型类型方法，digits 为小数位数"},
        "divide": {"c": "—", "go": "—", "function": "除法运算（泛型）", "description": "C 标准库无直接对应，Go 亦无标准库函数；axlang 语言级泛型方法"},
    },
    "array": {
        "push": {"c": "realloc/memcpy", "go": "append", "function": "在数组末尾追加元素", "description": "对标 C realloc+memcpy 扩容，Go 内置 append()；动态数组扩容"},
        "pop": {"c": "—", "go": "—", "function": "移除并返回数组最后一个元素", "description": "C 标准库无直接对应，Go 亦无标准库函数；返回 T? 可选类型"},
        "remove": {"c": "memmove", "go": "—", "function": "按索引移除数组元素", "description": "对标 C memmove 后移元素，Go 需手动切片删除；索引越界可能 panic"},
        "clear": {"c": "memset", "go": "—", "function": "清空数组所有元素", "description": "对标 C memset 清零，Go 可通过重新赋值空切片实现；长度归零但容量保留"},
        "to_string": {"c": "—", "go": "fmt.Sprintf", "function": "将数组转换为字符串表示", "description": "C 标准库无直接对应，Go 对标 fmt.Sprintf('%v', slice)；格式如 [1 2 3]"},
        "==": {"c": "memcmp", "go": "reflect.DeepEqual", "function": "判断两个数组是否相等（逐元素比较）", "description": "对标 C memcmp，Go reflect.DeepEqual 或手动逐元素比较；同类型数组可比较"},
        "!=": {"c": "memcmp", "go": "—", "function": "判断两个数组是否不相等", "description": "对标 C memcmp 取反，Go 无直接对应；等价于 !(a == b)"},
        "切片": {"c": "—", "go": "切片表达式 s[lo:hi]", "function": "数组切片操作，获取子数组视图", "description": "C 标准库无直接对应（指针偏移），Go 切片表达式；支持 [lo..hi] 和 [lo..=hi] 两种语法"},
    },
    "map": {
        "clear": {"c": "—", "go": "maps.Clear", "function": "清空 map 中所有键值对", "description": "C 标准库无 map 类型，Go 对标 maps.Clear（Go 1.21+）；删除所有条目"},
        "remove": {"c": "—", "go": "delete/maps.Delete", "function": "删除 map 中指定键的条目", "description": "C 标准库无 map 类型，Go 对标 delete() 内置或 maps.Delete；键不存在时不报错"},
        "==": {"c": "—", "go": "maps.Equal", "function": "判断两个 map 是否相等（相同键值对）", "description": "C 标准库无 map 类型，Go 对标 maps.Equal；键值对完全相同即相等"},
        "!=": {"c": "—", "go": "—", "function": "判断两个 map 是否不相等", "description": "C 标准库无 map 类型，Go 无直接对应；等价于 !(m1 == m2)"},
    },
    "range": {
        "step": {"c": "—", "go": "—", "function": "创建带步进的范围迭代器", "description": "C 标准库无直接对应，Go range 关键字；axlang 语言级特性，n 为步进值"},
    },
    "thread": {
        "push": {"c": "—", "go": "ch <- v", "function": "向通道发送数据（阻塞）", "description": "C 标准库无通道，Go 对标 chan 发送操作；channel 阻塞式写入"},
        "pop": {"c": "—", "go": "<-ch", "function": "从通道接收数据（阻塞）", "description": "C 标准库无通道，Go 对标 chan 接收操作；返回 T? 可选类型"},
        "close": {"c": "—", "go": "close(ch)", "function": "关闭通道，禁止后续发送", "description": "C 标准库无通道，Go 对标 close() 内置；关闭后只能读不能写"},
        "join": {"c": "pthread_join", "go": "sync.WaitGroup.Wait", "function": "等待线程/协程结束", "description": "对标 C pthread_join，Go 对标 sync.WaitGroup.Wait；阻塞等待完成"},
    },
    "queue": {
        "try_pop": {"c": "—", "go": "select/default", "function": "非阻塞地从队列取出元素，空队列返回 T? 无值", "description": "C 标准库无直接对应，Go 对标 select + default 语句；非阻塞尝试出队"},
    },
    "os": {
        "args": {"c": "argv", "go": "os.Args", "function": "获取命令行参数字符串切片", "description": "对标 C main 的 argv 参数，Go os.Args；第一个元素通常为程序名"},
        "chdir": {"c": "chdir", "go": "os.Chdir", "function": "更改当前工作目录", "description": "对标 C chdir，Go os.Chdir；返回 0 表示成功，非 0 表示失败"},
        "environ": {"c": "environ", "go": "os.Environ", "function": "获取所有环境变量字符串切片", "description": "对标 C environ 全局变量，Go os.Environ；返回 'KEY=VALUE' 格式的字符串数组"},
        "exit": {"c": "exit", "go": "os.Exit", "function": "终止当前进程并返回退出码", "description": "对标 C exit，Go os.Exit；立即终止程序，不执行 defer"},
        "getenv": {"c": "getenv", "go": "os.Getenv", "function": "获取指定环境变量的值", "description": "对标 C getenv，Go os.Getenv；环境变量不存在时返回空字符串"},
        "getpid": {"c": "getpid", "go": "os.Getpid", "function": "获取当前进程 ID", "description": "对标 C getpid，Go os.Getpid；返回进程标识符（整数）"},
        "getppid": {"c": "getppid", "go": "os.Getppid", "function": "获取父进程 ID", "description": "对标 C getppid，Go os.Getppid；返回父进程标识符（整数）"},
        "getwd": {"c": "getcwd", "go": "os.Getwd", "function": "获取当前工作目录路径", "description": "对标 C getcwd，Go os.Getwd；返回绝对路径字符串"},
        "hostname": {"c": "gethostname", "go": "os.Hostname", "function": "获取主机名", "description": "对标 C gethostname，Go os.Hostname；返回系统主机名字符串"},
        "setenv": {"c": "setenv", "go": "os.Setenv", "function": "设置环境变量", "description": "对标 C setenv，Go os.Setenv；返回 0 表示成功，非 0 表示失败"},
        "unsetenv": {"c": "unsetenv", "go": "os.Unsetenv", "function": "删除环境变量", "description": "对标 C unsetenv，Go os.Unsetenv；返回 0 表示成功，非 0 表示失败"},
        "user_home_dir": {"c": "getenv(HOME)", "go": "os.UserHomeDir", "function": "获取当前用户的家目录路径", "description": "对标 C getenv('HOME')，Go os.UserHomeDir；Windows 对应 %USERPROFILE%"},
        "user_cache_dir": {"c": "—", "go": "os.UserCacheDir", "function": "获取用户缓存目录路径", "description": "C 标准库无直接对应，Go 对标 os.UserCacheDir；跨平台缓存目录"},
        "getpagesize": {"c": "sysconf(_SC_PAGESIZE)", "go": "—", "function": "获取系统页面大小（字节）", "description": "对标 C sysconf(_SC_PAGESIZE)，Go 标准库无直接对应；通常返回 4096"},
        "getuid": {"c": "getuid", "go": "—", "function": "获取当前用户 ID（Unix 特有）", "description": "对标 C getuid，Go 标准库无直接对应；Windows 不支持"},
        "getgid": {"c": "getgid", "go": "—", "function": "获取当前组 ID（Unix 特有）", "description": "对标 C getgid，Go 标准库无直接对应；Windows 不支持"},
        "geteuid": {"c": "geteuid", "go": "—", "function": "获取有效用户 ID（Unix 特有）", "description": "对标 C geteuid，Go 标准库无直接对应；Windows 不支持"},
        "getegid": {"c": "getegid", "go": "—", "function": "获取有效组 ID（Unix 特有）", "description": "对标 C getegid，Go 标准库无直接对应；Windows 不支持"},
        "getgroups": {"c": "getgroups", "go": "—", "function": "获取当前用户所属的组 ID 列表", "description": "对标 C getgroups，Go 标准库无直接对应；返回 int[] 数组"},
        "executable": {"c": "—", "go": "os.Executable", "function": "获取当前可执行文件路径", "description": "C 标准库无直接对应，Go 对标 os.Executable；返回程序本身的绝对路径"},
    },
    "strconv": {
        "atoi": {"c": "atoi", "go": "strconv.Atoi", "function": "将字符串解析为整数", "description": "对标 C atoi，Go strconv.Atoi；解析失败返回 0 和错误"},
        "itoa": {"c": "itoa/sprintf", "go": "strconv.Itoa", "function": "将整数格式化为字符串", "description": "C 标准库无标准 itoa（需 sprintf 或自实现），Go 对标 strconv.Itoa"},
        "parse_bool": {"c": "—", "go": "strconv.ParseBool", "function": "将字符串解析为布尔值", "description": "C 标准库无直接对应，Go 对标 strconv.ParseBool；接受 '1','t','T','TRUE','true' 等"},
        "format_bool": {"c": "—", "go": "strconv.FormatBool", "function": "将布尔值格式化为字符串", "description": "C 标准库无直接对应，Go 对标 strconv.FormatBool；返回 'true' 或 'false'"},
        "parse_float": {"c": "atof/strtod", "go": "strconv.ParseFloat", "function": "将字符串解析为浮点数", "description": "对标 C atof/strtod，Go strconv.ParseFloat；返回 float 类型"},
        "format_float": {"c": "snprintf('%f')", "go": "strconv.FormatFloat", "function": "将浮点数格式化为字符串", "description": "对标 C snprintf %f，Go strconv.FormatFloat；支持精度控制"},
        "parse_int": {"c": "atoi/strtol", "go": "strconv.ParseInt", "function": "按指定进制将字符串解析为整数", "description": "对标 C strtol，Go strconv.ParseInt；base 参数指定进制（2-36）"},
        "format_int": {"c": "itoa/snprintf", "go": "strconv.FormatInt", "function": "将整数按指定进制格式化为字符串", "description": "C 标准库无标准 itoa，Go 对标 strconv.FormatInt；base 参数指定进制"},
        "parse_uint": {"c": "strtoul", "go": "strconv.ParseUint", "function": "按指定进制将字符串解析为无符号整数", "description": "对标 C strtoul，Go strconv.ParseUint；base 参数指定进制（2-36）"},
        "format_uint": {"c": "utoa/snprintf", "go": "strconv.FormatUint", "function": "将无符号整数按指定进制格式化为字符串", "description": "C 标准库无标准 utoa，Go 对标 strconv.FormatUint；base 参数指定进制"},
        "can_backquote": {"c": "—", "go": "strconv.CanBackquote", "function": "判断字符串是否可用反引号（原始字符串）表示", "description": "C 标准库无直接对应，Go 对标 strconv.CanBackquote；检查是否包含反引号或非打印字符"},
        "quote": {"c": "—", "go": "strconv.Quote", "function": "将字符串加双引号并转义，生成 Go 字符串字面量", "description": "C 标准库无直接对应，Go 对标 strconv.Quote；输出可直接用于 Go 源码"},
        "unquote": {"c": "—", "go": "strconv.Unquote", "function": "解析带引号的字符串字面量，去除转义", "description": "C 标准库无直接对应，Go 对标 strconv.Unquote；处理双引号和反引号字符串"},
    },
    "filepath": {
        "join": {"c": "—", "go": "filepath.Join", "function": "将多个路径片段连接为单个路径", "description": "C 标准库无直接对应，Go 对标 filepath.Join；自动处理平台分隔符"},
        "dir": {"c": "dirname", "go": "filepath.Dir", "function": "返回路径的目录部分", "description": "对标 C dirname 命令/函数，Go 对标 filepath.Dir；去掉最后一个元素"},
        "base": {"c": "basename", "go": "filepath.Base", "function": "返回路径的最后一个元素（文件名）", "description": "对标 C basename 命令/函数，Go 对标 filepath.Base；去掉目录部分"},
        "ext": {"c": "—", "go": "filepath.Ext", "function": "返回文件扩展名（包含点）", "description": "C 标准库无直接对应，Go 对标 filepath.Ext；返回 '.ext' 格式，无扩展名返回空字符串"},
        "split": {"c": "—", "go": "filepath.Split", "function": "将路径拆分为目录和文件名两部分", "description": "C 标准库无直接对应，Go 对标 filepath.Split；返回 string[2] 数组"},
        "clean": {"c": "—", "go": "filepath.Clean", "function": "规范化路径，删除冗余的斜杠和 '.' '..'", "description": "C 标准库无直接对应，Go 对标 filepath.Clean；返回最短等价路径"},
        "abs": {"c": "realpath", "go": "filepath.Abs", "function": "返回路径的绝对形式", "description": "对标 C realpath，Go 对标 filepath.Abs；基于当前工作目录解析相对路径"},
        "is_abs": {"c": "—", "go": "filepath.IsAbs", "function": "判断路径是否为绝对路径", "description": "C 标准库无直接对应，Go 对标 filepath.IsAbs；检查是否以根目录开头"},
        "eval_symlinks": {"c": "realpath", "go": "filepath.EvalSymlinks", "function": "解析符号链接，返回规范路径", "description": "对标 C realpath，Go 对标 filepath.EvalSymlinks；递归解析所有软链接"},
        "glob": {"c": "glob", "go": "filepath.Glob", "function": "按模式匹配文件名，返回匹配的字符串切片", "description": "对标 C glob，Go 对标 filepath.Glob；支持 * ? [...] 通配符"},
        "rel": {"c": "—", "go": "filepath.Rel", "function": "返回从 basepath 到 targpath 的相对路径", "description": "C 标准库无直接对应，Go 对标 filepath.Rel；计算两个路径间的相对关系"},
        "walk": {"c": "nftw", "go": "filepath.Walk", "function": "递归遍历目录树，对每个文件调用回调", "description": "对标 C nftw，Go 对标 filepath.Walk；axlang 不支持闭包，实际受限"},
    },
    "rand": {
        "int": {"c": "rand", "go": "rand.Intn", "function": "返回 [0, n) 范围内的随机整数", "description": "对标 C rand，Go math/rand.Intn；n 必须为正数"},
        "float": {"c": "rand/drand48", "go": "rand.Float64", "function": "返回 [0.0, 1.0) 范围内的随机浮点数", "description": "对标 C drand48，Go math/rand.Float64；伪随机数生成"},
        "seed": {"c": "srand", "go": "rand.Seed", "function": "设置随机数生成器种子", "description": "对标 C srand，Go math/rand.Seed；相同种子产生相同序列"},
    },
    "sort": {
        "ints": {"c": "qsort", "go": "sort.Ints", "function": "对 int 数组升序排序", "description": "对标 C qsort，Go sort.Ints；原地排序，默认升序"},
        "strings": {"c": "qsort", "go": "sort.Strings", "function": "对 string 数组升序排序", "description": "对标 C qsort，Go sort.Strings；按字典序原地排序"},
        "float64s": {"c": "qsort", "go": "sort.Float64s", "function": "对 float 数组升序排序", "description": "对标 C qsort，Go sort.Float64s；原地排序，默认升序"},
        "ints_are_sorted": {"c": "—", "go": "sort.IntsAreSorted", "function": "判断 int 数组是否已升序排序", "description": "C 标准库无直接对应，Go 对标 sort.IntsAreSorted；遍历检查，O(n)"},
        "strings_are_sorted": {"c": "—", "go": "sort.StringsAreSorted", "function": "判断 string 数组是否已升序排序", "description": "C 标准库无直接对应，Go 对标 sort.StringsAreSorted；遍历检查，O(n)"},
        "search_ints": {"c": "bsearch", "go": "sort.SearchInts", "function": "在已排序 int 数组中二分查找目标值", "description": "对标 C bsearch，Go sort.SearchInts；要求数组已排序，返回索引或插入点"},
        "search_strings": {"c": "bsearch", "go": "sort.SearchStrings", "function": "在已排序 string 数组中二分查找目标字符串", "description": "对标 C bsearch，Go sort.SearchStrings；要求数组已排序，返回索引或插入点"},
        "slice": {"c": "qsort", "go": "sort.Slice", "function": "按自定义比较函数对任意类型数组排序", "description": "对标 C qsort，Go sort.Slice；axlang 不支持闭包，cmp 参数受限，实际可用性有限"},
        "slice_stable": {"c": "—", "go": "sort.SliceStable", "function": "稳定排序（相等元素保持原始相对顺序）", "description": "C 标准库无稳定排序，Go 对标 sort.SliceStable；axlang 不支持闭包，暂不落地"},
    },
    "slices": {
        "contains": {"c": "—", "go": "slices.Contains", "function": "判断数组是否包含指定元素", "description": "C 标准库无直接对应，Go 对标 slices.Contains；线性查找，O(n)"},
        "index": {"c": "—", "go": "slices.Index", "function": "返回元素在数组中的索引，未找到返回 -1", "description": "C 标准库无直接对应，Go 对标 slices.Index；线性查找，O(n)"},
        "sort": {"c": "qsort", "go": "slices.Sort", "function": "对数组原地升序排序", "description": "对标 C qsort，Go slices.Sort；支持 int[]/string[] 默认升序，cmp 重载支持自定义比较"},
        "reverse": {"c": "—", "go": "slices.Reverse", "function": "原地反转数组元素顺序", "description": "C 标准库无直接对应，Go 对标 slices.Reverse；首尾元素交换，O(n/2)"},
        "clone": {"c": "memcpy", "go": "slices.Clone", "function": "返回数组的深拷贝副本", "description": "对标 C memcpy，Go slices.Clone；返回独立的新数组，修改不影响原数组"},
        "insert": {"c": "memmove", "go": "slices.Insert", "function": "在指定索引位置插入元素", "description": "C 标准库无直接对应，Go 对标 slices.Insert；后续元素后移，O(n)"},
        "fill": {"c": "memset", "go": "slices.Fill", "function": "用指定值填充数组所有元素", "description": "对标 C memset，Go slices.Fill；将所有元素设为相同值，O(n)"},
        "resize": {"c": "realloc", "go": "—", "function": "调整数组长度，扩容时填充零值", "description": "对标 C realloc，Go 标准库无直接对应；缩短时截断，扩长时补零"},
        "merge": {"c": "memcpy", "go": "slices.Concat", "function": "将另一数组追加到当前数组末尾", "description": "对标 C memcpy 追加，Go 对标 slices.Concat；原地修改当前数组"},
        "min": {"c": "—", "go": "slices.Min", "function": "返回数组中的最小值", "description": "C 标准库无直接对应，Go 对标 slices.Min；支持 int[]/float[]，遍历查找，O(n)"},
        "max": {"c": "—", "go": "slices.Max", "function": "返回数组中的最大值", "description": "C 标准库无直接对应，Go 对标 slices.Max；支持 int[]/float[]，遍历查找，O(n)"},
        "sum": {"c": "—", "go": "—", "function": "返回数组元素之和", "description": "C 标准库无直接对应，Go 标准库无 slices.Sum；遍历累加，O(n)"},
        "delete": {"c": "memmove", "go": "slices.Delete", "function": "删除索引 [i, j) 范围内的元素", "description": "C 标准库无直接对应，Go 对标 slices.Delete；后续元素前移，O(n)"},
    },
    "maps": {
        "keys": {"c": "—", "go": "maps.Keys", "function": "返回 map 中所有键的切片", "description": "C 标准库无 map 类型，Go 对标 maps.Keys；返回 K[] 类型，顺序不固定"},
        "values": {"c": "—", "go": "maps.Values", "function": "返回 map 中所有值的切片", "description": "C 标准库无 map 类型，Go 对标 maps.Values；返回 V[] 类型，顺序不固定"},
        "clone": {"c": "—", "go": "maps.Clone", "function": "返回 map 的深拷贝副本", "description": "C 标准库无 map 类型，Go 对标 maps.Clone；返回独立的新 map"},
        "copy": {"c": "—", "go": "maps.Copy", "function": "将 src map 的所有键值对复制到 dst map", "description": "C 标准库无 map 类型，Go 对标 maps.Copy；覆盖 dst 中已有的键"},
        "delete": {"c": "—", "go": "delete/maps.Delete", "function": "删除 map 中指定键的条目", "description": "C 标准库无 map 类型，Go 对标 delete() 内置或 maps.Delete；键不存在时不报错"},
        "delete_func": {"c": "—", "go": "maps.DeleteFunc", "function": "删除满足条件的所有键值对", "description": "C 标准库无 map 类型，Go 对标 maps.DeleteFunc；axlang 不支持闭包，暂不落地"},
        "equal": {"c": "—", "go": "maps.Equal", "function": "判断两个 map 是否相等（相同键值对）", "description": "C 标准库无 map 类型，Go 对标 maps.Equal；键值对完全相同即相等"},
    },
    "unique": {
        "make": {"c": "—", "go": "unique.Make", "function": "根据值创建唯一句柄 Handle[T]，相同值共享同一句柄", "description": "C 标准库无直接对应，Go 对标 unique.Make；Handle[T] 装箱类型，支持值比较去重"},
        "value": {"c": "—", "go": "unique.Value", "function": "从 Handle[T] 解出原始值 T", "description": "C 标准库无直接对应，Go 对标 unique.Value；解箱操作，返回内层类型 T"},
    },
    "bytes": {
        "contains": {"c": "strstr/memmem", "go": "bytes.Contains", "function": "判断字节切片是否包含子字节切片", "description": "对标 C memmem（非标准）或 strstr，Go 对标 bytes.Contains；字节级查找"},
        "equal": {"c": "memcmp", "go": "bytes.Equal", "function": "判断两个字节切片是否相等", "description": "对标 C memcmp，Go 对标 bytes.Equal；长度和内容完全相同即相等"},
        "index": {"c": "strstr/memmem", "go": "bytes.Index", "function": "返回子字节切片首次出现的索引，未找到返回 -1", "description": "对标 C memmem（非标准），Go 对标 bytes.Index；字节级查找"},
        "split": {"c": "strtok", "go": "bytes.Split", "function": "按分隔符分割字节切片", "description": "对标 C strtok，Go 对标 bytes.Split；分隔符本身不保留，返回 bytes[] 切片"},
        "trim_space": {"c": "—", "go": "bytes.TrimSpace", "function": "去除字节切片首尾空白字符", "description": "C 标准库无直接对应，Go 对标 bytes.TrimSpace；空白包括空格、制表符、换行符等"},
        "to_upper": {"c": "toupper", "go": "bytes.ToUpper", "function": "将字节切片转换为大写", "description": "对标 C toupper（逐字节），Go 对标 bytes.ToUpper；仅 ASCII 字母"},
        "to_lower": {"c": "tolower", "go": "bytes.ToLower", "function": "将字节切片转换为小写", "description": "对标 C tolower（逐字节），Go 对标 bytes.ToLower；仅 ASCII 字母"},
        "repeat": {"c": "—", "go": "bytes.Repeat", "function": "将字节切片重复 count 次", "description": "C 标准库无直接对应，Go 对标 bytes.Repeat；返回新的重复后的字节切片"},
    },
    "bufio": {
        "new_reader": {"c": "—", "go": "bufio.NewReader", "function": "创建带缓冲的文件读取器", "description": "C 标准库无直接对应，Go 对标 bufio.NewReader；基于 File 句柄 ax_file*"},
        "new_writer": {"c": "—", "go": "bufio.NewWriter", "function": "创建带缓冲的文件写入器", "description": "C 标准库无直接对应，Go 对标 bufio.NewWriter；基于 File 句柄 ax_file*"},
        "scanner": {"c": "—", "go": "bufio.NewScanner", "function": "创建扫描器，按行或自定义分隔符读取", "description": "C 标准库无直接对应，Go 对标 bufio.NewScanner；基于 File 句柄 ax_file*"},
    },
    "csv": {
        "read_all": {"c": "—", "go": "csv.Reader.ReadAll", "function": "从 CSV 阅读器读取所有记录为二维字符串切片", "description": "C 标准库无 CSV 解析器，Go 对标 encoding/csv.Reader.ReadAll；返回 string[][]"},
        "write": {"c": "—", "go": "csv.Writer.Write", "function": "将一条记录写入 CSV 文件", "description": "C 标准库无 CSV 写入器，Go 对标 encoding/csv.Writer.Write；返回写入状态"},
    },
    "errors": {
        "new": {"c": "—", "go": "errors.New", "function": "根据消息字符串创建 error 对象", "description": "C 标准库无 error 类型，Go 对标 errors.New；返回 axlang error 类型"},
        "is": {"c": "—", "go": "errors.Is", "function": "判断错误是否与目标错误匹配（支持链式包装）", "description": "C 标准库无 error 类型，Go 对标 errors.Is；axlang 无运行时反射，target 限定为 error 类型"},
        "as": {"c": "—", "go": "errors.As", "function": "将错误解析为目标类型", "description": "C 标准库无 error 类型，Go 对标 errors.As；axlang 无运行时反射，target 限定为 error 类型"},
    },
    "log": {
        "print": {"c": "printf/fputs", "go": "log.Print", "function": "输出日志到标准输出", "description": "对标 C printf/fputs，Go log.Print；输出带时间戳的普通日志"},
        "printf": {"c": "printf/fprintf", "go": "log.Printf", "function": "按格式输出格式化日志", "description": "对标 C printf/fprintf，Go log.Printf；支持格式化占位符"},
        "fatal": {"c": "printf+exit", "go": "log.Fatal", "function": "输出致命日志并退出程序", "description": "对标 C printf+exit(1)，Go log.Fatal；输出后调用 os.Exit(1) 终止程序"},
    },
    "sync": {
        "Lock": {"c": "pthread_mutex_lock", "go": "sync.Mutex.Lock", "function": "加互斥锁，保护共享资源", "description": "对标 C pthread_mutex_lock，Go sync.Mutex.Lock；sync.mutex 结构体方法"},
        "Unlock": {"c": "pthread_mutex_unlock", "go": "sync.Mutex.Unlock", "function": "释放互斥锁", "description": "对标 C pthread_mutex_unlock，Go sync.Mutex.Unlock；sync.mutex 结构体方法"},
        "RLock": {"c": "pthread_rwlock_rdlock", "go": "sync.RWMutex.RLock", "function": "加读锁，允许并发读取", "description": "对标 C pthread_rwlock_rdlock，Go sync.RWMutex.RLock；sync.rwmutex 结构体方法"},
        "Runlock": {"c": "pthread_rwlock_unlock", "go": "sync.RWMutex.RUnlock", "function": "释放读锁", "description": "对标 C pthread_rwlock_unlock，Go sync.RWMutex.RUnlock；sync.rwmutex 结构体方法"},
        "Add": {"c": "—", "go": "sync.WaitGroup.Add", "function": "给等待组增加计数器", "description": "C 标准库无直接对应，Go 对标 sync.WaitGroup.Add；sync.wait_group 结构体方法"},
        "Done": {"c": "—", "go": "sync.WaitGroup.Done", "function": "等待组计数器减 1，表示任务完成", "description": "C 标准库无直接对应，Go 对标 sync.WaitGroup.Done；sync.wait_group 结构体方法"},
        "Wait": {"c": "pthread_join/条件变量", "go": "sync.WaitGroup.Wait", "function": "阻塞等待等待组计数器归零", "description": "对标 C pthread_join/条件变量，Go 对标 sync.WaitGroup.Wait；sync.wait_group 结构体方法"},
        "Do": {"c": "pthread_once", "go": "sync.Once.Do", "function": "确保函数仅执行一次，用于单例初始化", "description": "对标 C pthread_once，Go 对标 sync.Once.Do；sync.once 结构体方法"},
    },
    "atomic": {
        "add_int": {"c": "atomic_fetch_add", "go": "atomic.AddInt64", "function": "原子地将增量加到整数地址并返回原值", "description": "对标 C atomic_fetch_add（<stdatomic.h>），Go sync/atomic.AddInt64；基于 C11 原子类型"},
        "load_int": {"c": "atomic_load", "go": "atomic.LoadInt64", "function": "原子地读取整数地址的值", "description": "对标 C atomic_load，Go sync/atomic.LoadInt64；基于 C11 原子类型"},
        "store_int": {"c": "atomic_store", "go": "atomic.StoreInt64", "function": "原子地将值写入整数地址", "description": "对标 C atomic_store，Go sync/atomic.StoreInt64；基于 C11 原子类型"},
        "compare_and_swap_int": {"c": "atomic_compare_exchange", "go": "atomic.CompareAndSwapInt64", "function": "原地比较并交换：若当前值等于 old 则更新为 new", "description": "对标 C atomic_compare_exchange_strong，Go sync/atomic.CompareAndSwapInt64；基于 C11 原子类型"},
    },
    "reflect": {
        "type_of": {"c": "—", "go": "reflect.TypeOf", "function": "返回任意值的类型名称字符串", "description": "C 标准库无运行时反射，Go 对标 reflect.TypeOf；axlang 无运行时类型反射，需设计可落地子集"},
        "value_of": {"c": "—", "go": "reflect.ValueOf", "function": "返回任意值的反射值对象", "description": "C 标准库无运行时反射，Go 对标 reflect.ValueOf；axlang 无运行时类型反射，需设计可落地子集"},
        "num_field": {"c": "—", "go": "reflect.NumField", "function": "返回结构体的字段数量", "description": "C 标准库无运行时反射，Go 对标 reflect.NumField；axlang 无运行时类型反射，需设计可落地子集"},
    },
}

# sync 模块的 methods 是字符串列表，需要特殊处理
# 按结构体类型区分：mutex/rwmutex/wait_group/once 有不同的 C/Go 对应
SYNC_METHODS = {
    "mutex": {
        "Lock":   {"c": "pthread_mutex_lock",   "go": "sync.Mutex.Lock",   "function": "加互斥锁，保护共享资源", "description": "对标 C pthread_mutex_lock，Go sync.Mutex.Lock；sync.mutex 结构体方法"},
        "Unlock": {"c": "pthread_mutex_unlock", "go": "sync.Mutex.Unlock", "function": "释放互斥锁", "description": "对标 C pthread_mutex_unlock，Go sync.Mutex.Unlock；sync.mutex 结构体方法"},
    },
    "rwmutex": {
        "Lock":   {"c": "pthread_rwlock_wrlock", "go": "sync.RWMutex.Lock",   "function": "加写锁，独占访问共享资源", "description": "对标 C pthread_rwlock_wrlock，Go sync.RWMutex.Lock；sync.rwmutex 结构体方法"},
        "Unlock": {"c": "pthread_rwlock_unlock", "go": "sync.RWMutex.Unlock", "function": "释放写锁", "description": "对标 C pthread_rwlock_unlock，Go sync.RWMutex.Unlock；sync.rwmutex 结构体方法"},
        "RLock":  {"c": "pthread_rwlock_rdlock", "go": "sync.RWMutex.RLock",  "function": "加读锁，允许并发读取", "description": "对标 C pthread_rwlock_rdlock，Go sync.RWMutex.RLock；sync.rwmutex 结构体方法"},
        "Runlock": {"c": "pthread_rwlock_unlock", "go": "sync.RWMutex.RUnlock", "function": "释放读锁", "description": "对标 C pthread_rwlock_unlock，Go sync.RWMutex.RUnlock；sync.rwmutex 结构体方法"},
    },
    "wait_group": {
        "Add":    {"c": "—", "go": "sync.WaitGroup.Add", "function": "给等待组增加计数器", "description": "C 标准库无直接对应，Go 对标 sync.WaitGroup.Add；sync.wait_group 结构体方法"},
        "Done":   {"c": "—", "go": "sync.WaitGroup.Done", "function": "等待组计数器减 1，表示任务完成", "description": "C 标准库无直接对应，Go 对标 sync.WaitGroup.Done；sync.wait_group 结构体方法"},
        "Wait":   {"c": "pthread_join/条件变量", "go": "sync.WaitGroup.Wait", "function": "阻塞等待等待组计数器归零", "description": "对标 C pthread_join/条件变量，Go 对标 sync.WaitGroup.Wait；sync.wait_group 结构体方法"},
    },
    "once": {
        "Do": {"c": "pthread_once", "go": "sync.Once.Do", "function": "确保函数仅执行一次，用于单例初始化", "description": "对标 C pthread_once，Go 对标 sync.Once.Do；sync.once 结构体方法"},
    },
}

def main():
    with open(JSON_PATH, encoding='utf-8') as f:
        data = json.load(f)

    filled_c = filled_go = filled_func = filled_desc = 0

    for mod in data["modules"]:
        mname = mod["name"]
        mod_map = MAPPING.get(mname, {})

        for key in ("functions", "static_functions", "instance_functions", "module_functions"):
            if key in mod and isinstance(mod[key], list):
                for fn in mod[key]:
                    fname = fn["name"]
                    m = mod_map.get(fname)
                    if m:
                        # 强制填充：无论当前值如何，都从 mapping 写入（保证一致性）
                        fn["c_counterpart"] = m["c"]; filled_c += 1
                        fn["go_counterpart"] = m["go"]; filled_go += 1
                        fn["function"] = m["function"]; filled_func += 1
                        fn["description"] = m["description"]; filled_desc += 1

        # sync 模块的 methods 是字符串列表（如 ["Lock", "Unlock"]），需转为 dict 后填充
        # 按结构体类型名（t["name"]）区分 mutex/rwmutex/wait_group/once
        if mname == "sync" and "types" in mod and isinstance(mod["types"], list):
            for t in mod["types"]:
                tname = t.get("name", "")
                type_map = SYNC_METHODS.get(tname, {})
                if "methods" in t and isinstance(t["methods"], list):
                    new_methods = []
                    for mm in t["methods"]:
                        if isinstance(mm, str):
                            fname = mm
                            m = type_map.get(fname)
                            if m:
                                md = {"name": fname, "signature": "", "status": "implemented",
                                      "c_counterpart": m["c"], "go_counterpart": m["go"],
                                      "function": m["function"], "description": m["description"],
                                      "note": ""}
                                filled_c += 1; filled_go += 1; filled_func += 1; filled_desc += 1
                            else:
                                md = {"name": fname, "signature": "", "status": "implemented",
                                      "c_counterpart": "—", "go_counterpart": "—",
                                      "function": "—", "description": "—", "note": ""}
                            new_methods.append(md)
                        elif isinstance(mm, dict):
                            new_methods.append(mm)
                    t["methods"] = new_methods

    with open(JSON_PATH, 'w', encoding='utf-8') as f:
        json.dump(data, f, ensure_ascii=False, indent=2)
        f.write('\n')

    print(f"已填充 c_counterpart: {filled_c}, go_counterpart: {filled_go}, function: {filled_func}, description: {filled_desc}")

if __name__ == "__main__":
    main()
