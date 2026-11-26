#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""规格书 §16 用例驱动器。

目录约定
    tests/lex|parse|sema|own/<ID>.ax     负例：首行写 `// EXPECT: E4001`
                                         断言**恰好一条**诊断且 code 相符
    tests/run/<ID>.ax                    正例：真正编译并执行
        <ID>.fixture                     可选，AX_DB_FIXTURE 的值（单行）
        <ID>.out                         期望 stdout（逐字节比较，末尾换行敏感）
        <ID>.exit                        期望退出码（默认 0）
        <ID>.errsub                      可选，stderr 必须包含的子串（如 R101）
        <ID>.args                        可选，传给程序的命令行参数（空格分隔）
    tests/mod/<ID>/main.ax              多文件正例：模块文件 + 期望输出
    samples/demo/<ID>/main.ax           可运行示例工程（多模块 + 类），同 mod 结构
        <ID>.out 或 main.out             期望 stdout（同上）

用法
    python tests/runtests.py                    跑全部
    python tests/runtests.py sema own           只跑指定类别
    python tests/runtests.py -v                 打印每条结果
    python tests/runtests.py -j 8               开 8 线程并行跑（默认串行）
    python tests/runtests.py --axc=xxx.exe      指定编译器（默认自动探测）

编译器探测顺序：`--axc=` → `rust/target/release/axc[.exe]` → `dist/axc[.exe]`。
"""
from __future__ import annotations

import concurrent.futures
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)

CATS = ["lex", "parse", "sema", "own", "run", "mod", "demo"]
# 这些类别按「一个子目录 = 一个多文件程序」组织：<cat>/<ID>/main.ax + 模块文件
MULTI_DIR_CATS = ("mod", "demo")
BUILD = os.path.join(HERE, ".build")


def read(p, default=None):
    if not os.path.exists(p):
        return default
    with open(p, encoding="utf-8", newline="") as fp:
        return fp.read()


def expect_code(path):
    first = read(path).split("\n", 1)[0]
    if "EXPECT:" not in first:
        return None
    return first.split("EXPECT:", 1)[1].strip().split()[0]


# ---------------------------------------------------------------- 后端
class ProcBackend:
    """调用编译器可执行文件。

    只依赖 CLI 契约：`check --diag=json` 输出 JSONL、`build -o` 产出可执行文件。
    """

    def __init__(self, exe):
        self.exe = exe
        self.name = os.path.basename(exe)

    def _env(self):
        return dict(os.environ)

    def error_codes(self, path):
        r = subprocess.run([self.exe, "check", path, "--diag=json"],
                           capture_output=True, env=self._env())
        # 诊断按规格书写 stderr；这里 stdout 也扫一遍，兼容实现差异
        blob = (r.stdout.decode("utf-8", "replace") + "\n"
                + r.stderr.decode("utf-8", "replace"))
        codes = []
        for line in blob.splitlines():
            line = line.strip()
            if not line.startswith("{"):
                continue
            try:
                obj = json.loads(line)
            except ValueError:
                continue
            if obj.get("severity") == "error":
                codes.append(obj.get("code"))
        return codes

    def build_exe(self, path, exe):
        os.makedirs(os.path.dirname(exe), exist_ok=True)
        r = subprocess.run([self.exe, "build", path, "-o", exe],
                           capture_output=True, env=self._env())
        if r.returncode != 0:
            tail = r.stderr.decode("utf-8", "replace").strip().splitlines()
            return False, tail[0] if tail else "build 退出码 %d" % r.returncode
        return True, "ok"


# ---------------------------------------------------------------- 用例
def run_negative(path, be):
    code = expect_code(path)
    if code is None:
        return False, "缺少 `// EXPECT: <code>` 首行"
    got = be.error_codes(path)
    if code == "PASS":
        if got:
            return False, "期望无错误，实得 %s" % got
        return True, "PASS"
    if len(got) != 1:
        return False, "期望恰好 1 条诊断，实得 %d 条：%s" % (len(got), got or "无")
    if got[0] != code:
        return False, "期望 %s，实得 %s" % (code, got[0])
    return True, code


def run_positive(path, be, stem=None):
    base = os.path.splitext(path)[0]
    name = stem or os.path.basename(base)
    os.makedirs(BUILD, exist_ok=True)
    exe = os.path.join(BUILD, name + (".exe" if os.name == "nt" else ""))
    if os.path.exists(exe):
        os.remove(exe)
    ok, msg = be.build_exe(path, exe)
    if not ok:
        return False, msg

    env = dict(os.environ)
    fx = read(base + ".fixture")
    if fx is not None:
        env["AX_DB_FIXTURE"] = fx.strip("\r\n")
    args = read(base + ".args", "").split()
    stdin_data = read(base + ".stdin")
    r = subprocess.run([exe] + args, capture_output=True, env=env,
                       input=(stdin_data.encode("utf-8") if stdin_data is not None else None))

    # 多文件用例的期望输出可以叫 `<目录名>.out`（Mod-1.out），也可以叫 main.out
    want_out = read(base + ".out")
    if want_out is None and stem:
        want_out = read(os.path.join(os.path.dirname(path), stem + ".out"))
    if want_out is not None:
        got = r.stdout.decode("utf-8", "replace").replace("\r\n", "\n")
        want_lines = want_out.replace("\r\n", "\n").split("\n")
        got_lines = got.split("\n")
        # 去掉尾部空行
        while want_lines and want_lines[-1] == "":
            want_lines.pop()
        while got_lines and got_lines[-1] == "":
            got_lines.pop()
        if len(want_lines) != len(got_lines):
            return False, "stdout 行数不符\n  期望: %r\n  实得: %r" % (want_out, got)
        for i, (wl, gl) in enumerate(zip(want_lines, got_lines)):
            if "<DYNAMIC>" in wl:
                continue  # 动态值行（如 pid、timestamp），跳过比较
            if wl != gl:
                return False, "stdout 第 %d 行不符\n  期望: %r\n  实得: %r" % (i + 1, wl, gl)
    want_exit = int((read(base + ".exit", "0") or "0").strip())
    if r.returncode != want_exit:
        return False, "退出码 %d，期望 %d" % (r.returncode, want_exit)
    sub = read(base + ".errsub")
    if sub is not None:
        err = r.stderr.decode("utf-8", "replace")
        if sub.strip() not in err:
            return False, "stderr 未包含 %r，实得 %r" % (sub.strip(), err[:200])
    return True, "ok"


def find_axc():
    """按优先级找编译器：release/bin 产物优先，其次 rust/target/release 里的直接构建产物。

    dist/ 目录已废弃，编译器统一发布到 release/bin/，标准库和运行时随同放在 release/axc/。
    """
    exe = ".exe" if os.name == "nt" else ""
    for c in (os.path.join(ROOT, "release", "bin", "axc" + exe),
              os.path.join(ROOT, "rust", "target", "release", "axc" + exe)):
        if os.path.exists(c):
            return c
    return None


def main(argv):
    verbose = "-v" in argv
    axc = None
    jobs = 1
    auto_jobs = False  # -j 单独出现时，按用例数自动分档
    rest = []
    i = 0
    while i < len(argv):
        a = argv[i]
        if a.startswith("--axc="):
            axc = a.split("=", 1)[1]
        elif a == "-j":
            i += 1
            if i < len(argv) and argv[i].lstrip("-").isdigit():
                jobs = int(argv[i])
            else:
                auto_jobs = True
        elif a.startswith("--jobs="):
            jobs = int(a.split("=", 1)[1])
        elif a.startswith("-j") and len(a) > 2:
            jobs = int(a[2:])
        else:
            rest.append(a)
        i += 1
    axc = axc or find_axc()
    if not axc:
        print("找不到编译器。先构建：cd rust && cargo build --release")
        return 1
    be = ProcBackend(axc)
    print("编译器：%s" % be.name)
    if jobs > 1:
        print("并行度：%d" % jobs)

    want = [a for a in rest if not a.startswith("-")] or CATS

    # ---------------------------------------------------------------- 收集所有用例
    all_cases = []  # (cat, name, path)
    for cat in want:
        d = os.path.join(HERE, cat)
        if cat == "demo":
            d = os.path.join(ROOT, "samples", "demo")
        if not os.path.isdir(d):
            continue
        if cat in MULTI_DIR_CATS:
            cases = sorted(
                (sub, os.path.join(d, sub, "main.ax"))
                for sub in os.listdir(d)
                if os.path.isfile(os.path.join(d, sub, "main.ax"))
            )
        else:
            files = sorted(f for f in os.listdir(d) if f.endswith(".ax"))
            cases = [(os.path.splitext(f)[0], os.path.join(d, f)) for f in files]
        for name, path in cases:
            all_cases.append((cat, name, path))

    # ---------------------------------------------------------------- 自动分档：按用例数决定并行度
    if auto_jobs:
        n = len(all_cases)
        if n <= 10:
            jobs = 2
        elif n <= 50:
            jobs = 4
        elif n <= 200:
            jobs = 8
        else:
            jobs = 16
        print("自动并行度：%d（用例数 %d）" % (jobs, n))
    elif jobs > 1:
        print("并行度：%d" % jobs)

    # ---------------------------------------------------------------- 并行/串行执行
    def run_one(cat, name, path):
        try:
            is_neg = cat in MULTI_DIR_CATS and expect_code(path) is not None
            ok, msg = run_negative(path, be) if is_neg else (
                run_positive(path, be, name) if cat in ("run",) + MULTI_DIR_CATS
                else run_negative(path, be)
            )
        except Exception as e:
            ok, msg = False, "%s: %s" % (type(e).__name__, e)
        return (cat, name, ok, msg)

    if jobs > 1:
        with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as executor:
            results = list(executor.map(
                lambda args: run_one(*args),
                all_cases,
            ))
    else:
        results = [run_one(cat, name, path) for cat, name, path in all_cases]

    # ---------------------------------------------------------------- 统计与输出
    total = len(results)
    passed = 0
    fails = []
    for cat, name, ok, msg in results:
        if ok:
            passed += 1
            if verbose:
                print("  PASS  %-6s %-14s %s" % (cat, name, msg))
        else:
            fails.append((cat, name, msg))
            print("  FAIL  %-6s %-14s %s" % (cat, name, msg))

    print()
    print("%d/%d 通过" % (passed, total))
    if fails:
        print("失败 %d 条：%s" % (len(fails), ", ".join("%s/%s" % (c, n) for c, n, _ in fails)))
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
