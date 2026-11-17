#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""依次跑 samples/ 下的示例，把每个程序的实际输出打出来。

    python samples/run_all.py              # 自动探测 rust/target/release → dist
    python samples/run_all.py --exe=xxx    # 指定编译器
    python samples/run_all.py -v           # 连编译命令与退出码一起打印

只做"跑一遍看效果"，不做断言——严格的逐字节断言在 tests/ 下。
"""
from __future__ import annotations

import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SAMPLES = os.path.join(ROOT, "samples")

# 10_goods 需要夹具；09_trap 故意崩（退出码 2）
FIXTURES = {
    "10_goods.ax": "id=1001,name=柴犬粮,price=19.99,stock=10",
}
EXPECTED_FAIL = {"09_trap.ax"}


def find_compiler(argv):
    for a in argv:
        if a.startswith("--exe="):
            return [a.split("=", 1)[1]]
    exe = ".exe" if os.name == "nt" else ""
    for c in (os.path.join(ROOT, "rust", "target", "release", "axc" + exe),
              os.path.join(ROOT, "dist", "axc" + exe)):
        if os.path.exists(c):
            return [c]
    return None


def main(argv):
    verbose = "-v" in argv
    cc = find_compiler(argv)
    if not cc:
        print("找不到编译器。先构建：cd rust && cargo build --release")
        return 1
    # 「模块级不能出现语句」——它不是"示例程序"，跑示例时排除，避免长期占一个红色标记。
    files = sorted(f for f in os.listdir(SAMPLES) if f.endswith(".ax"))
    # demo/ 下是多文件工程（每个子目录一个入口 main.ax），一并跑
    demo = os.path.join(SAMPLES, "demo")
    demos = []
    if os.path.isdir(demo):
        for sub in sorted(os.listdir(demo)):
            entry = os.path.join(demo, sub, "main.ax")
            if os.path.isfile(entry):
                demos.append("demo/%s/main.ax" % sub)
    files += demos
    if not files:
        print("samples/ 下没有 .ax 文件")
        return 1

    print("编译器：%s\n" % " ".join(cc))
    bad = 0
    for name in files:
        path = os.path.join(SAMPLES, name.replace("/", os.sep))
        env = dict(os.environ)
        base = os.path.basename(name)
        if base in FIXTURES:
            env["AX_DB_FIXTURE"] = FIXTURES[base]

        cmd = cc + ["run", path]
        if verbose:
            print("$ " + " ".join(cmd))
        p = subprocess.run(cmd, capture_output=True, env=env)
        out = p.stdout.decode("utf-8", "replace")
        err = p.stderr.decode("utf-8", "replace")

        bar = "-" * 62
        print("%s\n%s  (rc=%d)" % (bar, name, p.returncode))
        body = out.strip("\n")
        if body:
            print(body)
        if err.strip():
            print("[stderr] " + err.strip().replace("\n", "\n[stderr] "))

        ok = (p.returncode == 2) if base in EXPECTED_FAIL else (p.returncode == 0)
        if not ok:
            print("!! 意外退出码 %d（期望 %d）" % (p.returncode, 2 if base in EXPECTED_FAIL else 0))
            bad += 1
    print("-" * 62)
    print("\n%d 个示例，%d 个失败" % (len(files), bad))
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
