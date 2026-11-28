#!/usr/bin/env bash
# 构建 Rust 版 axc —— 同时输出 Windows / Linux / macOS 三个平台的二进制。
#
# 不依赖当前 shell 的 PATH 是否已刷新。Windows 的环境变量在进程启动时
# 就固定了，rustup 装完后已开着的终端里 cargo 可能不可见，这里自己找 cargo。
#
# 产物：
#   dist/axc/axc.exe         Windows 版（宿主平台，MSVC/GNU）
#   dist/axc/axc-linux       Linux 版（x86_64-unknown-linux-musl，静态链接，可在大多数 Linux 发行版运行）
#   dist/axc/axc-macos       macOS 版（x86_64-apple-darwin，需 macOS SDK，在 Windows 上构建会跳过）
#
# macOS 版在 Windows 宿主机上无法直接构建（缺 macOS SDK/链接器），脚本会尝试构建，
# 失败时自动跳过并给出提示。如需 macOS 版，请在 macOS 机器上运行本脚本，
# 或使用 GitHub Actions / CI 交叉编译。
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

# 确保交叉编译目标已安装
echo "检查并安装编译目标..."
rustup target add x86_64-pc-windows-msvc    >/dev/null 2>&1 || true
rustup target add x86_64-unknown-linux-musl >/dev/null 2>&1 || true
rustup target add x86_64-apple-darwin       >/dev/null 2>&1 || true

# 确保 dist/axc 目录存在
DIST_BIN_DIR="$(cd "$(dirname "$0")/.." && pwd)/dist/axc"
mkdir -p "$DIST_BIN_DIR"

# ===== 1. Windows 版（宿主平台） =====
echo ""
echo "=== 构建 Windows 版 axc ==="
"$found" build --release "$@"
if [ $? -ne 0 ]; then
    echo "Windows 版构建失败" >&2
    exit 1
fi
cp "target/release/axc.exe" "$DIST_BIN_DIR/axc.exe"
echo "已输出: $DIST_BIN_DIR/axc.exe"

# ===== 2. Linux 版（musl 静态链接） =====
echo ""
echo "=== 构建 Linux 版 axc (x86_64-unknown-linux-musl) ==="
"$found" build --release --target x86_64-unknown-linux-musl "$@"
if [ $? -ne 0 ]; then
    echo "Linux 版构建失败" >&2
    exit 1
fi
cp "target/x86_64-unknown-linux-musl/release/axc" "$DIST_BIN_DIR/axc-linux"
echo "已输出: $DIST_BIN_DIR/axc-linux"

# ===== 3. macOS 版（可选，需 macOS SDK） =====
echo ""
echo "=== 尝试构建 macOS 版 axc (x86_64-apple-darwin) ==="
if "$found" build --release --target x86_64-apple-darwin "$@" 2>/dev/null; then
    cp "target/x86_64-apple-darwin/release/axc" "$DIST_BIN_DIR/axc-macos"
    echo "已输出: $DIST_BIN_DIR/axc-macos"
else
    echo "macOS 版构建跳过（缺 macOS SDK/链接器，请在 macOS 机器上构建：cd rust && cargo build --release --target x86_64-apple-darwin）"
fi

echo ""
echo "全部完成。产物目录: $DIST_BIN_DIR"
ls -la "$DIST_BIN_DIR"
