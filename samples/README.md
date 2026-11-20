# samples —— 可以直接跑的 axlang 示例

每个文件都是完整程序，一个命令就能编译并运行：

```
dist\axc.exe run samples\01_hello.ax
```

只看诊断不运行：

```
dist\axc.exe check samples\01_hello.ax
```

## 示例清单

| 文件 | 讲什么 | 关键点 |
|---|---|---|
| `01_hello.ax` | 变量、字符串插值 | `{name}` 插值、`{{ }}` 转义、`{price:.2}` 格式、按码点遍历字符串 |
| `02_types.ax` | 类型系统一览 | 整数家族带位宽、`float` 与 `decimal` 分工、**零隐式转换**、`T(x)` 显式转换 |
| `03_control.ax` | 控制流 | 只有 `if` / `for`，没有 `while`；`0..n` 与 `0..=n`；标签 `break outer` |
| `04_class.ax` | 面向对象 | `class` / `constructor` / `parent(...)` / `extend` / `override` / `implement` |
| `05_result.ax` | 错误处理 | `result<T>` + `try` + `throw` + `match`，错误是值不是异常 |
| `06_collections.ax` | 容器 | 变长数组、定长数组、`map` 插入序、`record` 键固定 |
| `07_decimal.ax` | 金额计算 | 为什么 `0.1+0.2` 用 float 会失真、`round` / `divide` 的显式舍入 |
| `08_defer.ax` | `defer` | 逆序执行，`return` / `throw` 也会触发 |
| `09_trap.ax` | 崩溃演示 | 七类运行时崩溃 `R101..R107`，固定四行输出、退出码 2 |
| `10_goods.ax` | 综合业务 | 文稿 §12 原文：类 + 继承 + 接口 + `db` 查询 + 金额舍入 |
| `11_try_catch.ax` | 就地捕获错误 | `try` / `catch` 按 error 变体过滤 / `finally` 必执行 / catch 内改写再抛 / 只写 finally 也不吞错误 |
| `demo/OrderDemo/` | 多模块工程 | 4 个模块 + 入口：`catalog.product`、`catalog.inventory`、`checkout.bill`（具名导入）、`checkout.wholesale`（跨模块继承）；`import` 之后裸类名直接 `new Product(...)` |
| `demo/Package/` | 多文件模块（文件即是模块） | `main.ax` 显式 `import shop.inventory` / `import shop.stock`；`shop/inventory.ax` 与 `shop/stock.ax` 各自为独立模块，通过 `shop.inventory.VALUE` / `shop.stock.greet()` 等多段路径访问跨模块 `public` 符号 |

## demo：多模块 + 类的引入与调用

`demo/OrderDemo/` 是一个目录工程，入口是 `main.ax`：

```
dist\axc.exe run samples\demo\OrderDemo\main.ax
```

```
demo/OrderDemo/
  main.ax                 入口：import 四个模块，构造 / 调用 / 输出
  catalog/product.ax      class Product：字段 + 构造 + 方法 + 静态工厂 + 模块常量
  catalog/inventory.ax    数组字段 items、Push、可空返回 Find(sku)
  checkout/bill.ax        具名导入 import catalog.product.{Product, TAX_RATE}
  checkout/wholesale.ax   跨模块继承 extend product.Product
  main.out                期望输出（tests/runtests.py 逐字节比对）
```

类型名三种写法都合法：`Product`（裸名）、`model.Product`（末段短名）、`shop.model.Product`（全路径）。
两个已导入模块导出同名类型时裸名报 `E3042`，此时写全路径。

## 一次跑完

```
python samples/run_all.py
```

## 关于 `09_trap.ax` 和 `10_goods.ax`

- `09_trap.ax` 故意触发崩溃，**退出码是 2**，这是设计如此（七类崩溃是语言允许的唯一崩溃路径）。改首行的 `const int TRAP = 1` 可切换 `R101`…`R107`。
- `10_goods.ax` 要用数据库夹具，缺夹具时会提示 `no db fixture`。带上夹具运行：

```
dist\axc.exe run samples\10_goods.ax
```

PowerShell：

```powershell
$env:AX_DB_FIXTURE="id=1001,name=柴犬粮,price=19.99,stock=10"; dist\axc.exe run samples\10_goods.ax
```

cmd：

```bat
set AX_DB_FIXTURE=id=1001,name=柴犬粮,price=19.99,stock=10
dist\axc.exe run samples\10_goods.ax
```

Git Bash：

```bash
AX_DB_FIXTURE="id=1001,name=柴犬粮,price=19.99,stock=10" ./dist/axc.exe run samples/10_goods.ax
```

期望输出：

```
折后小计 53.97
柴犬粮 ¥19.99（库存 10）
```

## 语言里没有的东西（不是编译器缺）

这些是文稿明确删掉的，不是没实现：

- **`while` / `do-while` / `goto`**：只有 `for`（区间、遍历、无头三种）。条件循环写 `for { ... if c { break } ... }`。
- **`char` 类型**：字符串按码点遍历，元素是长度 1 的 `string`。
- **`switch`**：用 `match`，且穷尽性由编译器检查。
- **`try-catch`**：用 `result<T>` + `try`，错误是返回值。
- **字符串下标**：`s[i]` 不支持，要遍历就 `for c in s`。
