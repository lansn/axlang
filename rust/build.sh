#!/usr/bin/env bash
# 构建 Rust 版 axc —— 不依赖当前 shell 的 PATH 是否已刷新。
#
# 背景：Windows 的环境变量在进程启动时就固定了。如果 rustup 安装完之后
# 没有重启 IDE / 终端，老进程里 `cargo` 是不可见的，但注册表里的 PATH
# 已经是新的。这里自己找 cargo，找不到再报错。
set -euo pipefail

cd "$(dirname "$0")"

# Git Bash 里 USERPROFILE 是 C:\Users\xxx 这种反斜杠形式，转成 /c/Users/xxx
to_unix() {
    if command -v cygpath >/dev/null 2>&1; then
        cygpath -u "$1"
    else
        printf '%s' "$1" | sed -e 's|\\|/|g' -e 's|^\([A-Za-z]\):|/\L\1|'
    fi
}

found=""
if command -v cargo >/dev/null 2>&1; then
    found="$(command -v cargo)"
else
    for base in "${USERPROFILE:-}" "$HOME"; do
        [ -n "$base" ] || continue
        dir="$(to_unix "$base")/.cargo/bin"
        if [ -x "$dir/cargo.exe" ] || [ -x "$dir/cargo" ]; then
            export PATH="$dir:$PATH"
            found="$dir/cargo"
            break
        fi
    done
fi

if [ -z "$found" ]; then
    echo "找不到 cargo。请确认已安装 Rust，或把 ~/.cargo/bin 加入 PATH。" >&2
    exit 1
fi

echo "cargo: $found"
exec "$found" build --release "$@"
