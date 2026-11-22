# axlang 标准库清单（对标 C / Go）

本文档列出 axlang 内建模块与 C 标准库（stdio/stdlib/string/math/time/file）及 Go 标准库（fmt/os/strings/strconv/encoding/json/math/time/os/filepath）的对标关系，作为标准库落地进度的检视参考。目标：尽可能丰富，满足开发需要。

## 状态图例
- `✅ 已落地`：已在 axrt.h/axrt.c 实现、sema/codegen 绑定、测试用例齐备、回归通过
- `🔄 部分落地`：核心函数已有，部分扩展函数待补
- `⬜ 未落地`：尚未开始

---

## 已落地模块

### io（对标 C stdio / Go io）
| 函数 | 签名 | 状态 | 说明 |
|------|------|------|------|
| `io.ReadLine` | `() -> string?` | ✅ | 读取一行，EOF 时 has=0 |
| `io.ReadFile` | `(path: string) -> result<string>` | ✅ | 读取整个文件 |
| `io.Write` | `(s: string) -> void` | ✅ | 写 stdout |
| `io.WriteLine` | `(s: string) -> void` | ✅ | 写 stdout 并换行 |

### math（对标 C math.h / Go math）
| 函数 | 签名 | 状态 | 说明 |
|------|------|------|------|
| `math.Equals` | `(a: float, b: float, eps: float) -> bool` | ✅ | fabs(a-b) <= eps |
| `math.Compare` | `(a: float, b: float) -> int` | ✅ | -1/0/1 |
| `math.Abs` | `(x: float) -> float` | ✅ | fabs |
| `math.Max` | `(a: float, b: float) -> float` | ✅ | fmax |
| `math.Min` | `(a: float, b: float) -> float` | ✅ | fmin |
| `math.Sqrt` | `(x: float) -> float` | ✅ | sqrt |
| `math.Pow` | `(a: float, b: float) -> float` | ✅ | pow |
| `math.Exp` | `(x: float) -> float` | ✅ | exp |
| `math.Log` | `(x: float) -> float` | ✅ | log |
| `math.Sin` | `(x: float) -> float` | ✅ | sin |
| `math.Cos` | `(x: float) -> float` | ✅ | cos |
| `math.Tan` | `(x: float) -> float` | ✅ | tan |
| `math.Ceil` | `(x: float) -> float` | ✅ | ceil |
| `math.Floor` | `(x: float) -> float` | ✅ | floor |
| `math.Round` | `(x: float) -> float` | ✅ | round |

### json（对标 Go encoding/json）
| 函数 | 签名 | 状态 | 说明 |
|------|------|------|------|
| `json.Parse` | `(s: string) -> result<map<string, object>>` | ✅ | 解析 JSON |
| `json.Stringify` | `(v: object) -> string` | ✅ | 序列化 JSON |

### file（对标 C stdio/stdlib / Go os）
| 函数 | 签名 | 状态 | 说明 |
|------|------|------|------|
| `file.Open` | `(path: string) -> result<File>` | ✅ | 模式 0=只读 |
| `file.Create` | `(path: string) -> result<File>` | ✅ | 模式 1=写/截断 |
| `file.Append` | `(path: string) -> result<File>` | ✅ | 模式 2=追加 |
| `file.Remove` | `(path: string) -> int` | ✅ | 0=成功 1=失败 |
| `file.Rename` | `(old: string, new: string) -> int` | ✅ | 0=成功 1=失败 |
| `file.Exists` | `(path: string) -> int` | ✅ | 1=存在 0=不存在 |
| `file.Mkdir` | `(path: string) -> int` | ✅ | 0=成功 1=失败 |

### db
| 函数 | 签名 | 状态 | 说明 |
|------|------|------|------|
| `db.Query` | `(sql: string) -> result<Row>` | ✅ | 数据库查询 |

### string（对标 C string.h / Go strings）
| 函数 | 类型 | 签名 | 状态 | 说明 |
|------|------|------|------|------|
| `string.FromCodepoint` | 静态 | `(cp: int) -> result<string>` | ✅ | 码点转字符串 |
| `string.ToUpper` | 实例 | `() -> string` | ✅ | 大写转换 |
| `string.ToLower` | 实例 | `() -> string` | ✅ | 小写转换 |
| `string.Contains` | 实例 | `(sub: string) -> bool` | ✅ | 包含子串 |
| `string.IndexOf` | 实例 | `(sub: string) -> int` | ✅ | 首次出现索引 |
| `string.LastIndexOf` | 实例 | `(sub: string) -> int` | ✅ | 最后出现索引 |
| `string.Split` | 实例 | `(sep: string) -> string[]` | ✅ | 分割 |
| `string.Trim` | 实例 | `() -> string` | ✅ | 去首尾空白 |
| `string.Replace` | 实例 | `(old: string, new: string) -> string` | ✅ | 替换所有 |
| `string.Repeat` | 实例 | `(n: int) -> string` | ✅ | 重复 n 次 |
| `string.StartsWith` | 实例 | `(prefix: string) -> bool` | ✅ | 前缀判断 |
| `string.EndsWith` | 实例 | `(suffix: string) -> bool` | ✅ | 后缀判断 |
| 其他实例方法（共 20 个） | 实例 | — | ✅ | 含 Len 等 |

### time（对标 C time.h / Go time）
| 函数 | 类型 | 签名 | 状态 | 说明 |
|------|------|------|------|------|
| `time.NowMs` | 模块 | `() -> int` | ✅ | 当前 Unix 毫秒 |
| `time.Now` | 模块 | `() -> time` | ✅ | 当前时间 |
| `time.FromUnixMs` | 模块 | `(ms: int) -> time` | ✅ | 毫秒转时间 |
| `time.Sleep` | 模块 | `(ms: int) -> void` | ✅ | 毫秒睡眠 |
| `time.Parse` | 模块 | `(s: string, fmt: string) -> result<time>` | ✅ | 解析时间字符串 |
| `time.ToString` | 实例 | `() -> string` | ✅ | 格式化为 YYYY-MM-DD HH:MM:SS |
| `time.Format` | 实例 | `(fmt: string) -> string` | ✅ | strftime 风格格式化 |
| `time.Unix` | 模块 | `(sec: int, nsec: int) -> time` | ✅ | 秒+纳秒转 time（对标 Go time.Unix） |
| `time.UnixMilli` | 模块 | `(ms: int) -> time` | ✅ | 毫秒转 time（对标 Go time.UnixMilli） |
| `time.Since` | 模块 | `(t: time) -> int` | ✅ | 与给定时间的差值毫秒（对标 Go time.Since） |

### decimal
| 函数 | 类型 | 签名 | 状态 | 说明 |
|------|------|------|------|------|
| `decimal.Parse` | 静态 | `(s: string) -> result<decimal>` | ✅ | 解析十进制 |
| `int8.Truncate` | 实例 | `(v: int) -> int8` | ✅ | 截断（溢出触发 R107） |
| `int16.Truncate` | 实例 | `(v: int) -> int16` | ✅ | |
| `int32.Truncate` | 实例 | `(v: int) -> int32` | ✅ | |

### Int/Float/Bool/Enum
| 函数 | 类型 | 签名 | 状态 | 说明 |
|------|------|------|------|------|
| `ToString` | 各类型 | `() -> string` | ✅ | 转字符串 |
| `WrappingAdd` | 整型 | `(other: T) -> T` | ✅ | 绕环加法 |
| `Round` | 浮点 | `(digits: int) -> float` | ✅ | 四舍五入 |
| `Divide` | 数值型 | `(other: T) -> T` | ✅ | 除法 |

### Array
| 函数 | 签名 | 状态 | 说明 |
|------|------|------|------|
| `Push` | `(elem: T) -> void` | ✅ | 尾部追加 |
| `Pop` | `() -> T?` | ✅ | 弹出末尾，空数组触发 R101 |
| `Remove` | `(index: int) -> void` | ✅ | 删除指定索引 |
| `Clear` | `() -> void` | ✅ | 清空 |
| `ToString` | `() -> string` | ✅ | 转字符串 |
| `==/!=` | `(other: T[]) -> bool` | ✅ | 逐元素比较 |
| 切片 | `[lo..hi]` / `[lo..=hi]` | ✅ | 动态浅拷贝 / 定长深拷贝 |

### Map
| 函数 | 签名 | 状态 | 说明 |
|------|------|------|------|
| `Clear` | `() -> void` | ✅ | 清空 |
| `Remove` | `(key: K) -> void` | ✅ | 删除键 |
| `==/!=` | `(other: Map) -> bool` | ✅ | 比较 |

### Range
| 函数 | 签名 | 状态 | 说明 |
|------|------|------|------|
| `Step` | `(n: int) -> Range` | ✅ | 步进循环（防御性兜底） |

### thread / queue
| 函数 | 类型 | 签名 | 状态 | 说明 |
|------|------|------|------|------|
| `thread.Push` | 队列 | `(v: T) -> void` | ✅ | 入队 |
| `thread.Pop` | 队列 | `() -> T?` | ✅ | 阻塞出队 |
| `thread.Close` | 队列 | `() -> void` | ✅ | 关闭队列 |
| `thread.Join` | 线程 | `() -> void` | ✅ | 等待线程结束 |
| `queue.TryPop` | 队列 | `() -> T?` | ✅ | 非阻塞出队，空队列返回 null |

### os（对标 C stdlib / Go os）
| 函数 | 签名 | 状态 | 说明 |
|------|------|------|------|
| `os.Getenv` | `(name: string) -> string` | ✅ | 读环境变量，未设置返回空串 |
| `os.Exit` | `(code: int) -> void` | ✅ | 终止进程（C exit） |
| `os.Args` | `() -> string[]` | ✅ | 命令行参数（argv[1..]） |

### strconv（对标 Go strconv）
| 函数 | 签名 | 状态 | 说明 |
|------|------|------|------|
| `strconv.Atoi` | `(s: string) -> int` | ✅ | 字符串转 int（失败返回 0） |
| `strconv.Itoa` | `(v: int) -> string` | ✅ | int 转字符串 |
| `strconv.ParseFloat` | `(s: string) -> float` | ✅ | 字符串转 float（失败返回 0.0） |
| `strconv.FormatFloat` | `(v: float) -> string` | ✅ | float 转字符串 |

### filepath（对标 Go path/filepath）
| 函数 | 签名 | 状态 | 说明 |
|------|------|------|------|
| `filepath.Join` | `(parts: string[]) -> string` | ✅ | 用平台分隔符连接 |
| `filepath.Dir` | `(path: string) -> string` | ✅ | 返回目录部分 |
| `filepath.Base` | `(path: string) -> string` | ✅ | 返回最后一段文件名 |

---

## 待落地清单（P0 → P2）

### P0（核心，立即开始）

#### strconv 扩展
| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `strconv.ParseInt` | `(s: string, base: int) -> int` | Go `strconv.ParseInt` | ✅ 已落地（2026-11-22） |
| `strconv.FormatInt` | `(v: int, base: int) -> string` | Go `strconv.FormatInt` | ✅ 已落地（2026-11-22） |
| `strconv.ParseBool` | `(s: string) -> bool` | Go `strconv.ParseBool` | ✅ 已落地（2026-11-22） |
| `strconv.FormatBool` | `(v: bool) -> string` | Go `strconv.FormatBool` | ✅ 已落地（2026-11-22） |

#### filepath 扩展
| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `filepath.Ext` | `(path: string) -> string` | Go `filepath.Ext` | ✅ 已落地（2026-11-22） |
| `filepath.Split` | `(path: string) -> string[2]` | Go `filepath.Split` | ✅ 已落地（2026-11-22） |
| `filepath.Clean` | `(path: string) -> string` | Go `filepath.Clean` | ✅ 已落地（2026-11-22） |
| `filepath.Abs` | `(path: string) -> string` | Go `filepath.Abs` | ✅ 已落地（2026-11-22） |
| `filepath.IsAbs` | `(path: string) -> bool` | Go `filepath.IsAbs` | ✅ 已落地（2026-11-22） |

### P1

#### string 静态方法补充
| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `string.Compare` | `(a: string, b: string) -> int` | Go `strings.Compare` | ✅ 已落地（2026-11-22） |
| `string.Fields` | `(s: string) -> string[]` | Go `strings.Fields` | ✅ 已落地（2026-11-22） |

#### math 扩展
| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `math.Sign` | `(x: float) -> int` | Go `math.Sign` | ✅ 已落地（2026-11-22） |
| `math.Mod` | `(x: float, y: float) -> float` | Go `math.Mod` | ✅ 已落地（2026-11-22） |

#### os 扩展
| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `os.Getwd` | `() -> string` | Go `os.Getwd` | ✅ 已落地（2026-11-22） |
| `os.Chdir` | `(path: string) -> int` | Go `os.Chdir` | ✅ 已落地（2026-11-22） |

#### file 便捷读写
| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `file.ReadAll` | `(path: string) -> result<string>` | — | ✅ 已落地（2026-11-22） |
| `file.WriteAll` | `(path: string, s: string) -> int` | — | ✅ 已落地（2026-11-22） |

### P2

#### rand（对标 Go math/rand）
| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `rand.Int` | `(n: int) -> int` | Go `rand.Intn` | ✅ 已落地（2026-11-22） |
| `rand.Float` | `() -> float` | Go `rand.Float64` | ✅ 已落地（2026-11-22） |
| `rand.Seed` | `(seed: int) -> void` | Go `rand.Seed` | ✅ 已落地（2026-11-22） |

#### sort（对标 Go sort）
| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `sort.Ints` | `(a: int[]) -> void` | Go `sort.Ints` | ✅ 已落地（2026-11-22） |
| `sort.Strings` | `(a: string[]) -> void` | Go `sort.Strings` | ✅ 已落地（2026-11-22） |

---

### P3（持续丰富，对标 C / Go 标准库）

#### strconv 扩展
| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `strconv.ParseUint` | `(s: string, base: int) -> int` | Go `strconv.ParseUint` | ✅ 已落地（2026-11-22） |
| `strconv.FormatUint` | `(v: int, base: int) -> string` | Go `strconv.FormatUint` | ✅ 已落地（2026-11-22） |

#### os 扩展
| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `os.Getpid` | `() -> int` | Go `os.Getpid` | ✅ 已落地（2026-11-22） |
| `os.Getppid` | `() -> int` | Go `os.Getppid` | ✅ 已落地（2026-11-22） |
| `os.Hostname` | `() -> string` | Go `os.Hostname` | ✅ 已落地（2026-11-22） |
| `os.Environ` | `() -> string[]` | Go `os.Environ` | ✅ 已落地（2026-11-22） |

#### time 扩展
| 函数 | 签名 | 对标 | 状态 |
|------|------|------|------|
| `time.Unix` | `(sec: int, nsec: int) -> time` | Go `time.Unix` | ✅ 已落地（2026-11-22） |
| `time.UnixMilli` | `(ms: int) -> time` | Go `time.UnixMilli` | ✅ 已落地（2026-11-22） |
| `time.Since` | `(t: time) -> int` | Go `time.Since` | ✅ 已落地（2026-11-22） |

---

## 落地规范（每项必须完成）

1. **axrt.h**：新增 C 函数声明（标准 C11，无 GNU/Clang 扩展）
2. **axrt.c**：新增 C 函数实现
3. **rust/src/sema/mod.rs**：`BUILTIN_MODULES` 新增模块名（若新模块）
4. **rust/src/sema/expr.rs**：`module_sig` 新增签名
5. **rust/src/backend/codegen_expr.rs**：`module_call` 新增代码生成分支
6. **rust/src/backend/mod.rs**：如需新增 result 类型 C typedef（如 `result<time>` 走 `ax_res_time`）
7. **tests/run/*.ax + .out**：新增测试用例，覆盖正例和负例
8. **回归验证**：`python tests/runtests.py` 零新增回归
9. **提交推送**：Git commit + GitHub/Gitee 推送
10. **更新文档**：本清单状态标记，`MEMORY.md` 同步更新

---

## 当前进度

| 阶段 | 已完成 | 待完成 |
|------|--------|--------|
| 已落地模块 | io, math, json, file, db, string, time, decimal, Int/Float/Bool/Enum, Array, Map, Range, thread/queue, os, strconv, filepath, rand, sort | — |
| P0 待落地 | — | strconv.ParseInt/FormatInt, ParseBool/FormatBool; filepath.Ext/Split/Clean/Abs/IsAbs |
| P1 待落地 | — | — |
| P2 待落地 | — | — |

*更新于 2026-11-22*
