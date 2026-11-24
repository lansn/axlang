# axc — axlang Compiler

axlang is a statically-typed compiled language generated entirely by AI, including compiler/library/docs.

Implemented according to `axlang编译器实现规格书.md`; the sole source of truth for the language is `axlang语法设计文稿.md`.
Gaps and deviations are documented in `GAP-REPORT.md`.

> **Implementation language: Rust** (`rust/`, single exe approx. 1.1 MB, zero runtime dependencies).
>
> No longer involved in build and acceptance; migration and per-stage acceptance records are in `RUST-MIGRATION.md`.
>
> **Taking over this project**: read `ARCHITECTURE.md` (architecture and development guide) first; version history in `CHANGELOG.md`.

## Usage

```bash
dist\axc.exe check samples\01_hello.ax        # lexical / syntax / semantic check
dist\axc.exe build samples\01_hello.ax -o hi  # generate C11 + invoke gcc to produce executable
dist\axc.exe run   samples\01_hello.ax        # build then execute immediately
```

Options: `-o <path>`, `--emit=c`, `--dump=ast|types|c`, `--diag=text|json`, `-Werror`, `--cc=<path>`, `--keep`, `--lang=en|zh` (diagnostic language, default English; also via `AXC_LANG` or `axc.toml`), `-V/--version`.
Exit codes: `0` success, `1` error-level diagnostics, `2` target program crashed at runtime (R101–R107).

## Build

```bash
cd rust && cargo build --release    # → rust/target/release/axc.exe (approx. 1.1 MB)
cp target/release/axc.exe ../dist/axc.exe
```

`rust\build.cmd` is the equivalent script under cmd (first searches PATH, falls back to `~/.cargo/bin` — Windows environment variables are fixed at process start, so after rustup installation `cargo` is still invisible in **already open** terminals; see `RUST-MIGRATION.md`). Toolchain configuration (machine lacks `link.exe`, switched to GNU target + `rust-lld`) is also recorded there.

## Examples

`samples/` contains 12 directly runnable programs plus multi-module projects under `samples/demo/`, covering types, control flow, classes and interfaces, error handling (`result` + `match`, `try`/`catch`/`finally`), containers, amounts, `defer`, crash demonstrations, and complete business examples:

```bash
python samples/run_all.py     # run all at once, printing each program's output
```

See `samples/README.md` for details.

## Structure

```
axc/rt/axrt.{h,c}           Runtime: strings, decimal, arrays, map (insertion order), Row/db fixtures,
                            console UTF-8 (ax_console_utf8, interactive terminal code page switch only)
rust/src/main.rs            Entry + crate-level overview (pipeline, module responsibilities, exit code contract)
rust/src/cli.rs             Subcommands, C compilation, execution, exit codes, runtime directory detection
rust/src/diag.rs            Diagnostic collection + text/JSONL output (sort key path,line,col,code)
rust/src/types.rs           Type representation, canonical, assignable — single entry point
rust/src/frontend/          Frontend: lexer.rs lexer (39 keywords: added extern/unsafe, three string types, interpolation fragmentization,
                            code point line/column), parser.rs syntax (nl/expect_term division, G-10…G-16
                            resolution and recovery), ast.rs AST nodes + --dump=ast serialization
rust/src/sema/              Semantics: mod.rs main class and semantic slots, stmt.rs statement-level (including
                            try/catch checking), expr.rs expression-level and coerce, member.rs
                            member access/method calls/constant evaluation
rust/src/backend/           C11 backend: mod.rs program-level (class flattening, vtable, main wrapper),
                            codegen_fn.rs statement-level (defer/try/catch expansion),
                            codegen_expr.rs expression-level + match
dist/axc.exe                Distribution artifact (release build copy)
legacy/                     Archived Python reference implementation (zip, not part of build)
samples/                    Directly runnable examples + run_all.py
tests/runtests.py           §16 test driver
tests/{lex,parse,sema,own,run,mod} (mod is multi-file module test cases: `tests/mod/<ID>/main.ax`)
```

The `//!` comments at the head of each source file describe responsibilities, inputs/outputs, and key invariants — they are the first entry point for deep understanding.

The runtime `axc/rt/` is implementation-language agnostic: generated code `#include "axrt.h"`, and `build` copies it into a temp directory to compile together.

## §15 FFI / unsafe

axlang supports `extern "C" { ... }` to declare external C functions, and `unsafe { ... }` blocks to call them. The design follows the "obvious from code" principle: `extern` functions are signature-only declarations with no function body; `unsafe` blocks expand block contents directly in the C backend (C itself is unsafe), with zero runtime overhead.

```ax
extern "C" {
    int puts(string s);
}

int main() {
    unsafe {
        puts("hello from axlang FFI")
    }
    return 0
}
```

- `extern "C" { int foo(string s); }`: declare an external C function, `ret` and `params` resolved per axlang type system; function body is not allowed (reports `E3046`).
- `unsafe { ... }`: wraps calls to extern functions, generating direct C-level calls with no wrapper or runtime check.
- Currently `extern` functions generate local empty function bodies (stubs), not linked to real C symbols, so calls produce no runtime output (see `tests/run/FFI-1.out`).
- Tests: `tests/run/FFI-1.ax` + `FFI-1.out`.

## Testing

```bash
python tests/runtests.py            # full suite (auto-detects compiler)
python tests/runtests.py run -v     # end-to-end only, prints each case
python tests/runtests.py --axc=xxx  # specify compiler (default rust/target/release → dist)
```

- Negative cases (`lex|parse|sema|own`): first line `// EXPECT: E4001`, asserts **exactly one** diagnostic with matching code; `// EXPECT: PASS` means no errors expected.
- Positive cases (`run`): place `<ID>.fixture` (`AX_DB_FIXTURE`), `<ID>.out` (expected stdout, byte-exact), `<ID>.exit`, `<ID>.errsub`, `<ID>.args` in the same directory.

Current: **192/192 passing** (including try/catch, visibility, constructor pass, static `this` interception, multi-file modules, anonymous functions and their scope isolation, array mutable methods persisted to disk, `print`/`println` newline differences and multi-arg space-joined concatenation, `fmt.Printf` compile-time placeholder checking, `File` read/write and `Append`), including §16.10 `RUN-1` (`Post-discount subtotal 53.97` / `柴犬粮 ¥19.99 (stock 10)`, exit code 0), `RUN-2`, `RUN-3`, `RUN-4`, `Rt-01…Rt-16` seven crash categories and numeric/ordering/precision cases, `TRY-1…TRY-4` / `Try-1…Try-3` error handling positive/negative cases, `Vis-1…Vis-3` visibility explicit negative cases, and `Ctor-1…Ctor-2` / run `Ctor-1` no constructor cannot new positive/negative cases, `Scope-1` anonymous functions don't pollute outer scope, `Mut-1` array mutable methods effective, `IO-1` output newline differences, `File-1` full read/write flow, `Out-1` old name `Print` migration hint, `Top-1`/`Top-2` top-level statements and top-level variables explicit diagnostics, `Mod-neg-builtin` builtin module name reserved (`E3039`), `Mod-3` three-level module path drilling, `Null-1` nullable value interpolation (value shown / `null` shown for null, including `decimal?`/`float?` literal landing), `Interp-1` composite type interpolation reports `E5006`, `Mod-4` named import binding functions/constants/types, `Mod-neg-named` named import whitelist negative case, `Main-1` `void main()` signature negative case (`check` immediately reports `E3025`), `Mod-5` import last-segment short name (`import net.http` then `http.ping()`), `Mod-neg-short` short name conflict `E3040`, `Mod-6` cross-module instantiation (3-segment path / `new` / static factory / struct positional construction), `Struct-1` struct value semantics, `Top-3` implicit `main` (top-level statements exempt from writing `main`), `Mod-neg-top` module top-level statement negative case, `demo/OrderDemo` multi-module demo project (`samples/demo/OrderDemo`: `catalog.product` / `catalog.inventory` / `checkout.bill` named imports / `checkout.wholesale` cross-module inheritance, byte-exact comparison against `main.out`), `Mod-7` bare type names (`import model` then `new Product(...)` / `Product.Sample()` / `Point(3,4)` / `Color.Red`), `Mod-neg-ambig` same-name type ambiguity `E3042`, `Mod-8` cross-module interface polymorphism (`implement model.Describable` / interface variable reassigned to subclass instance dispatched to override / interface as cross-module parameter), `Upcast-1` upcast assignment explicit cast (declaration / pass / return three places, `-Werror` zero warnings), `Iface-1` interface polymorphism (interface variables/parameters/return values/arrays, fat pointer dynamic dispatch), `Iface-2` interface extend (parent interface methods called via child interface variable, implementing child interface implicitly implements parent interface), `Box-1` object[] heterogeneous array (boxing / auto-unboxing / object intermediary, tag mismatch `R107` terminates), `Sort-1` array `Sort(cmp)` three comparators (anonymous function / named function / class element), `Printable-1` class directly interpolated after implementing `Printable.ToString` (interface variable dynamic dispatch / nullable with value / `null`), `Comp-1` `Comparable.Compare(Self)` dispatched via shim vtable + `Hashable.Hash()`, `Printable-neg-1/2/3` builtin interface redeclaration `E3018` / interpolation not implemented `E5006` / missing implementation `E3019`, `Json-1` json/time materialization (`json.Parse` parsing + various type unboxing + JSON null key absence + syntax error fail + `json.Stringify` round-trip with escaping + `time.NowMs`), `Pat-1` type pattern four sources (class patterns on object box by boxed tag, class patterns on class reference by object header real tag, scalar patterns on object box), `Pat-neg-1` type pattern used on scalar value `E4001`, `Print-1` `print`/`println` args can be numbers/booleans/enums/classes implementing `Printable` (equivalent to adding interpolation braces), `Print-neg-1` composite type arg `E4001`, `Typeof-1` `typeof(expression)` compile-time type name (scalar/class/array/nullable + interpolation + const), `Typeof-neg-1` bare `typeof` hints `E3003`, `Lit-1` `0x`/`0b`/`0o` radix literals, `Str-1` code point round-trip (`CodepointAt` out of bounds `null` / `FromCodepoint` / iterate by code point), `Sizeof-1` `sizeof(expression)` storage size of eleven types (class by reference gives pointer size), `Sizeof-neg-1` bare `sizeof` hints `E3003`, `GX-1` generic functions (inference / explicit args / multiple instances coexist / nested instantiation / anonymous functions inside instances), `GX-2` generic classes and structs (fields / methods / nested instances / instance class implements interface fat pointer dispatch), `GX-3` generic type aliases, `GX-neg-1..4` constraint rejection (E4036) / invariance (E4001) / recursion limit (E4038), `O-01…O-12` ownership negative cases (use after move / borrow conflict / borrow-period move / borrow target / const borrow / branch and loop conservative merge / arena escape), `Borrow-1` borrow positive cases (multiple `&const` coexist / sequential borrow / resurrection / `W1003`), `Branch-1` branch-loop merge positive case, `Arena-1` arena and ArenaRef graph structure (cycle reference / same-region aliasing / whole-block release), `ArenaRef-neg-1..3` escape and `Alloc` type negative cases, `Math-1` module member PascalCase (`math.Compare`/`math.Equals`), `IO-2` `io.ReadFile` read-back, `Name-neg-1` lowercase old-style `io.readLine()` name hint (`E3003`), `Name-neg-2` lowercase member declaration `read` name hint (`E3043`), `G-10` `print(a++)` exactly one `E2010` (fix recovery spam), `Print-neg-2` multi-arg `print` exactly one `E4031`, `Fmt-1` `fmt.Printf`/`fmt.Print`/`fmt.Println` positive cases, `Fmt-neg-1..5` placeholder type/count and illegal placeholder negative cases, `StrSlice-1` `string.Left`/`Right`/`Mid` slicing (including out-of-bounds safe truncation).

## Standard Library (Stdlib)

axlang 标准库以 ax 语法编写源码，位于 `axc/std/`（随编译器发布至 `release/axc/std/`），由 axc 编译器解析语义并生成 C11 代码。标准库 .c 文件预编译为 `.o` 目标文件，缓存至 `release/axc/stdlib/obj/`，用户编译时由 gcc 链接所有 `.o` 文件，避免每次重新编译标准库。详细落地状态见 `STDLIB-CHECKLIST.md`（由 `generate_checklist.py` 从 `STDLIB-STATUS.json` 自动生成）。

发行包布局（`release/`）：
- `bin/axc` — 编译器可执行文件
- `gcc/bin/` — 平台随发的 gcc（Windows 为 `gcc.exe`），axc 优先在此查找，未找到再回落到 `$PATH`
- `axc/rt/` — C 运行时源码（axrt.h、axrt_core.c、axrt_str.c、axrt_file.c、axrt_json.c、axrt_os.c、axrt_sync.c、axrt_atomic.c、axrt_time.c、axrt_strconv.c、axrt_csv.c、axrt_decimal.c、axrt_filepath.c、axrt_rand.c、axrt_sort.c 等）
- `axc/stdlib/` — 标准库 C 声明头文件 `axc_stdlib.h`
- `axc/stdlib/obj/` — 标准库 `.o` 目标文件缓存（用户编译时复用）
- `axc/std/` — 标准库 ax 源码文件（io.ax、json.ax、os.ax、strconv.ax、math.ax、errors.ax、fmt.ax、string.ax、file.ax、sync.ax、atomic.ax）

已落地模块（版本 0.0.28，按优先级 P0→P3）：
io、fmt、math、json、file、os、strconv、string、sync、atomic、errors、decimal、Int/Float/Bool/Enum、Array、Map、Range、thread/queue、filepath、rand、sort、slices、bytes、unique、bufio、csv、log。

标准库函数名统一采用小写 C 风格（snake_case，下划线分隔），例如：
- `io.read_line`、`io.read_file`、`io.write`、`io.write_line`
- `math.abs`、`math.max`、`math.sqrt`、`math.pow`、`math.sin`、`math.cos`、`math.tan`、`math.exp`、`math.log`、`math.isnan`、`math.isinf`
- `json.parse`、`json.stringify`、`json.marshal_indent`、`json.valid`
- `os.getenv`、`os.exit`、`os.getwd`、`os.chdir`、`os.getpid`、`os.getppid`、`os.hostname`、`os.setenv`、`os.unsetenv`
- `strconv.atoi`、`strconv.itoa`、`strconv.parse_bool`、`strconv.format_bool`、`strconv.parse_float`、`strconv.format_float`、`strconv.parse_int`、`strconv.format_int`、`strconv.parse_uint`、`strconv.format_uint`、`strconv.can_backquote`、`strconv.quote`、`strconv.unquote`
- `string.compare`、`string.contains`、`string.index_of`、`string.trim`、`string.to_upper`、`string.to_lower`、`string.repeat`、`string.replace`、`string.startswith`、`string.endswith`、`string.is_digit`、`string.is_alpha`、`string.is_space`、`string.is_lower`、`string.is_upper`、`string.is_alnum`
- `file.open`、`file.create`、`file.append`、`file.remove`、`file.rename`、`file.exists`、`file.mkdir`、`file.is_dir`、`file.is_file`、`file.size`、`file.mod_time`、`file.write_all`、`file.chmod`、`file.chtimes`、`file.temp_file`
- `sync.mutex.lock`、`sync.mutex.unlock`、`sync.rwmutex.lock`、`sync.rwmutex.unlock`、`sync.rwmutex.r_lock`、`sync.rwmutex.runlock`、`sync.waitgroup.add`、`sync.waitgroup.done`、`sync.waitgroup.wait`、`sync.once.do`
- `atomic.add_int`、`atomic.load_int`、`atomic.store_int`、`atomic.compare_and_swap_int`
- `errors.new`、`errors.is`、`errors.as`
- `fmt.print`、`fmt.println`、`fmt.errorf`

所有 ax 源码通过 `extern "C"` 声明底层 `ax_*` C 原语，由 `unsafe {}` 块调用，底层依赖 `axc/rt/axrt.h/axrt_*.c` 提供的 C 运行时原语。

## Notes

- `gcc` is required (specifiable via `--cc` or `$AX_CC`). `dist\axc.exe` is a standalone Rust-compiled program and does not depend on Python.
- Running `tests/runtests.py` / `samples/run_all.py` requires Python 3 (driver scripts only, not the compiler implementation).
- `decimal` uses standard C11 struct `ax_dec` (`ax_i128 { int64_t hi; uint64_t lo }` coefficient + `int32_t scale`, 24 bytes actual / 32 bytes aligned; `CONFLICT-5`收口), semantics are 128-bit decimal fixed-point, no GCC/Clang extensions.
- Unimplemented list (generic monomorphization, complete borrow lifetime, arena, anonymous function backend, json/file/time modules) see `GAP-REPORT.md` §3.2.

## Windows Chinese Display

Generated executables output Chinese in the Windows console without manually running `chcp`: the `main` entry calls `ax_console_utf8()`, switching the console code page from the system default (936/GBK on Chinese systems) to UTF-8, and restores the original value on exit (including runtime crash `exit(2)`).

- Switching happens only when **truly attached to a console** (`_isatty` check). When redirected to a file or pipe, bytes are written as-is, code page unchanged, so `axc run x.ax > out.txt` yields pure UTF-8 text, and byte-exact comparisons in tests are unaffected.
- If a terminal still shows boxes/question marks, it's because the **console font** lacks Chinese glyphs (select Consolas or NSimSun), not an encoding issue.
- **Source files must be UTF-8**. `.ax` files saved as "ANSI" or GBK by Notepad will directly report `axc: 源文件不是合法 UTF-8` (intentional: never guess encoding, avoid silently producing garbled strings).
