#!/usr/bin/env python3
# 批量更新 tests/run/*.ax 中的模块函数调用：PascalCase -> 小写 C 风格
# 只替换 module.Name( 这种模块调用形式，以及类型级静态方法 Type.Name(
# 不替换 实例方法 (如 s.Contains(), t.Join(), row.GetString())

import glob
import os

TEST_DIR = r"D:\Project\axlang\tests\run"

# 模块函数名映射：old -> new
# 仅替换 module.Func( 这种形式（带左括号，避免误替换类型名等）
MODULE_RENAMES = [
    # io
    ("io.ReadLine(", "io.read_line("),
    ("io.ReadFile(", "io.read_file("),
    ("io.Write(", "io.write("),
    ("io.WriteLine(", "io.write_line("),
    # math
    ("math.Abs(", "math.abs("),
    ("math.Compare(", "math.compare("),
    ("math.Equals(", "math.equals("),
    ("math.Max(", "math.max("),
    ("math.Min(", "math.min("),
    ("math.Sqrt(", "math.sqrt("),
    ("math.Pow(", "math.pow("),
    ("math.Exp(", "math.exp("),
    ("math.Log(", "math.log("),
    ("math.IsNaN(", "math.is_nan("),
    ("math.IsInf(", "math.is_inf("),
    ("math.Sin(", "math.sin("),
    ("math.Cos(", "math.cos("),
    ("math.Tan(", "math.tan("),
    ("math.Ceil(", "math.ceil("),
    ("math.Floor(", "math.floor("),
    ("math.Round(", "math.round("),
    ("math.Sign(", "math.sign("),
    ("math.Mod(", "math.mod("),
    ("math.Exp2(", "math.exp2("),
    ("math.Log2(", "math.log2("),
    ("math.Log10(", "math.log10("),
    ("math.Log1p(", "math.log1p("),
    ("math.Sinh(", "math.sinh("),
    ("math.Cosh(", "math.cosh("),
    ("math.Tanh(", "math.tanh("),
    ("math.Asin(", "math.asin("),
    ("math.Acos(", "math.acos("),
    ("math.Atan(", "math.atan("),
    ("math.Atan2(", "math.atan2("),
    ("math.Hypot(", "math.hypot("),
    ("math.Cbrt(", "math.cbrt("),
    ("math.Trunc(", "math.trunc("),
    # json
    ("json.Parse(", "json.parse("),
    ("json.Stringify(", "json.stringify("),
    ("json.MarshalIndent(", "json.marshal_indent("),
    ("json.Valid(", "json.valid("),
    # file
    ("file.Create(", "file.create("),
    ("file.Open(", "file.open("),
    ("file.Append(", "file.append("),
    ("file.Remove(", "file.remove("),
    ("file.Rename(", "file.rename("),
    ("file.Exists(", "file.exists("),
    ("file.Mkdir(", "file.mkdir("),
    ("file.IsDir(", "file.is_dir("),
    ("file.IsFile(", "file.is_file("),
    ("file.Size(", "file.size("),
    ("file.ModTime(", "file.mod_time("),
    ("file.ReadAll(", "file.read_all("),
    ("file.WriteAll(", "file.write_all("),
    ("file.Chmod(", "file.chmod("),
    ("file.Chtimes(", "file.chtimes("),
    ("file.TempFile(", "file.temp_file("),
    # fmt
    ("fmt.Printf(", "fmt.printf("),
    ("fmt.Print(", "fmt.print("),
    ("fmt.Println(", "fmt.println("),
    ("fmt.Sprintf(", "fmt.sprintf("),
    ("fmt.Sprint(", "fmt.sprint("),
    ("fmt.Fprintf(", "fmt.fprintf("),
    ("fmt.Errorf(", "fmt.errorf("),
    ("fmt.Scanf(", "fmt.scanf("),
    # log
    ("log.Print(", "log.print("),
    ("log.Printf(", "log.printf("),
    ("log.Fatal(", "log.fatal("),
    # os
    ("os.Args(", "os.args("),
    ("os.Getenv(", "os.getenv("),
    ("os.Exit(", "os.exit("),
    ("os.Getwd(", "os.getwd("),
    ("os.Chdir(", "os.chdir("),
    ("os.Getpid(", "os.getpid("),
    ("os.Getppid(", "os.getppid("),
    ("os.Hostname(", "os.hostname("),
    ("os.Environ(", "os.environ("),
    ("os.Setenv(", "os.setenv("),
    ("os.Unsetenv(", "os.unsetenv("),
    ("os.UserHomeDir(", "os.user_home_dir("),
    # rand
    ("rand.Int(", "rand.int("),
    ("rand.Float(", "rand.float("),
    ("rand.Seed(", "rand.seed("),
    # strconv
    ("strconv.Atoi(", "strconv.atoi("),
    ("strconv.Itoa(", "strconv.itoa("),
    ("strconv.ParseFloat(", "strconv.parse_float("),
    ("strconv.FormatFloat(", "strconv.format_float("),
    ("strconv.ParseInt(", "strconv.parse_int("),
    ("strconv.FormatInt(", "strconv.format_int("),
    ("strconv.ParseUint(", "strconv.parse_uint("),
    ("strconv.FormatUint(", "strconv.format_uint("),
    ("strconv.ParseBool(", "strconv.parse_bool("),
    ("strconv.FormatBool(", "strconv.format_bool("),
    ("strconv.CanBackquote(", "strconv.can_backquote("),
    ("strconv.Quote(", "strconv.quote("),
    ("strconv.Unquote(", "strconv.unquote("),
    # string (模块级静态方法)
    ("string.Compare(", "string.compare("),
    ("string.Fields(", "string.fields("),
    ("string.FromCodepoint(", "string.from_codepoint("),
    ("string.ToUpperRune(", "string.to_upper_rune("),
    ("string.ToLowerRune(", "string.to_lower_rune("),
    # time
    ("time.NowMs(", "time.now_ms("),
    ("time.Now(", "time.now("),
    ("time.FromUnixMs(", "time.from_unix_ms("),
    ("time.Sleep(", "time.sleep("),
    ("time.Unix(", "time.unix("),
    ("time.UnixMilli(", "time.unix_milli("),
    ("time.UnixMicro(", "time.unix_micro("),
    ("time.UnixNano(", "time.unix_nano("),
    ("time.Since(", "time.since("),
    ("time.Date(", "time.date("),
    ("time.Until(", "time.until("),
    ("time.Parse(", "time.parse("),
    # bytes
    ("bytes.Contains(", "bytes.contains("),
    ("bytes.Equal(", "bytes.equal("),
    ("bytes.Index(", "bytes.index("),
    ("bytes.Split(", "bytes.split("),
    ("bytes.TrimSpace(", "bytes.trim_space("),
    ("bytes.ToUpper(", "bytes.to_upper("),
    ("bytes.ToLower(", "bytes.to_lower("),
    ("bytes.Repeat(", "bytes.repeat("),
    # bufio
    ("bufio.NewReader(", "bufio.new_reader("),
    ("bufio.NewWriter(", "bufio.new_writer("),
    ("bufio.Scanner(", "bufio.scanner("),
    # errors
    ("errors.New(", "errors.new("),
    ("errors.Is(", "errors.is("),
    ("errors.As(", "errors.as("),
    # csv
    ("csv.ReadAll(", "csv.read_all("),
    ("csv.Write(", "csv.write("),
    # filepath
    ("filepath.Join(", "filepath.join("),
    ("filepath.Dir(", "filepath.dir("),
    ("filepath.Base(", "filepath.base("),
    ("filepath.Ext(", "filepath.ext("),
    ("filepath.Split(", "filepath.split("),
    ("filepath.Clean(", "filepath.clean("),
    ("filepath.Abs(", "filepath.abs("),
    ("filepath.IsAbs(", "filepath.is_abs("),
    # maps
    ("maps.Keys(", "maps.keys("),
    ("maps.Values(", "maps.values("),
    ("maps.Clone(", "maps.clone("),
    ("maps.Copy(", "maps.copy("),
    ("maps.Delete(", "maps.delete("),
    ("maps.Equal(", "maps.equal("),
    # unique
    ("unique.Make(", "unique.make("),
    ("unique.Value(", "unique.value("),
    # atomic
    ("atomic.AddInt(", "atomic.add_int("),
    ("atomic.LoadInt(", "atomic.load_int("),
    ("atomic.StoreInt(", "atomic.store_int("),
    ("atomic.CompareAndSwapInt(", "atomic.compare_and_swap_int("),
    # sort
    ("sort.Ints(", "sort.ints("),
    ("sort.Strings(", "sort.strings("),
    ("sort.Float64s(", "sort.float64s("),
    ("sort.IntsAreSorted(", "sort.ints_are_sorted("),
    ("sort.StringsAreSorted(", "sort.strings_are_sorted("),
    ("sort.SearchInts(", "sort.search_ints("),
    ("sort.SearchStrings(", "sort.search_strings("),
    # db
    ("db.Query(", "db.query("),
]

# 类型级静态方法（Type.Method(）：这些也在 check_builtin_static 中处理
TYPE_STATIC_RENAMES = [
    ("int.Parse(", "int.parse("),
    ("int.Truncate(", "int.truncate("),
    ("float.Parse(", "float.parse("),
    ("float.Truncate(", "float.truncate("),
    ("decimal.Parse(", "decimal.parse("),
    ("decimal.Truncate(", "decimal.truncate("),
    ("int8.Truncate(", "int8.truncate("),
    ("int16.Truncate(", "int16.truncate("),
    ("int32.Truncate(", "int32.truncate("),
    # Row / File 静态方法（在 check_static_call 中处理）
    ("Row.GetString(", "row.get_string("),
    ("Row.GetInt(", "row.get_int("),
    ("Row.GetDecimal(", "row.get_decimal("),
]

ALL_RENAMES = MODULE_RENAMES + TYPE_STATIC_RENAMES

def process_file(path):
    with open(path, "r", encoding="utf-8") as f:
        content = f.read()

    original = content
    for old, new in ALL_RENAMES:
        content = content.replace(old, new)

    if content != original:
        with open(path, "w", encoding="utf-8") as f:
            f.write(content)
        # 统计改动
        count = sum(1 for old, _ in ALL_RENAMES if old in original)
        print(f"[已更新] {os.path.basename(path)}")
    else:
        pass  # 无变化，不打印

if __name__ == "__main__":
    files = sorted(glob.glob(os.path.join(TEST_DIR, "*.ax")))
    updated = 0
    for path in files:
        before = open(path, "r", encoding="utf-8").read()
        process_file(path)
        after = open(path, "r", encoding="utf-8").read()
        if before != after:
            updated += 1
    print(f"\n完成：{updated} 个文件已更新。")
