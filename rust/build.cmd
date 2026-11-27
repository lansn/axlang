@echo off
rem 构建 Rust 版 axc —— 同时输出 Windows / Linux / macOS 三个平台的二进制。
rem
rem 不依赖已打开的终端窗口里 PATH 是否已刷新。
rem 环境变量在进程启动时固定；rustup 装完后老窗口看不到 cargo，但注册表已更新。
rem
rem 产物：
rem   dist/axc.exe         Windows 版（宿主平台）
rem   dist/axc-linux       Linux 版（x86_64-unknown-linux-musl，静态链接）
rem   dist/axc-macos       macOS 版（x86_64-apple-darwin，需 macOS SDK，在 Windows 上构建会跳过）
rem
rem macOS 版在 Windows 宿主机上无法直接构建（缺 macOS SDK/链接器），脚本会尝试构建，
rem 失败时自动跳过并给出提示。如需 macOS 版，请在 macOS 机器上运行构建，
rem 或使用 GitHub Actions / CI 交叉编译。
setlocal

cd /d "%~dp0"

where /q cargo
if %errorlevel%==0 goto build

if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
    set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
    goto build
)

echo 找不到 cargo。请确认已安装 Rust，或把 %USERPROFILE%\.cargo\bin 加入 PATH。
exit /b 1

:build
echo cargo: %PATH%

rem 确保交叉编译目标已安装
echo 检查并安装编译目标...
rustup target add x86_64-pc-windows-msvc    > "%TEMP%\axc_rustup_msvc.log" 2>&1 || true
rustup target add x86_64-unknown-linux-musl > "%TEMP%\axc_rustup_musl.log" 2>&1 || true
rustup target add x86_64-apple-darwin       > "%TEMP%\axc_rustup_darwin.log" 2>&1 || true

rem 确保 dist 目录存在
if not exist "..\dist" mkdir "..\dist"
set "DIST_DIR=%~dp0..\dist"

rem ===== 1. Windows 版（宿主平台） =====
echo.
echo === 构建 Windows 版 axc ===
cargo build --release %*
if %errorlevel% neq 0 (
    echo Windows 版构建失败
    exit /b 1
)
copy /y "target\release\axc.exe" "%DIST_DIR%\axc.exe" > "%TEMP%\axc_copy_win.log"
echo 已输出: %DIST_DIR%\axc.exe

rem ===== 2. Linux 版（musl 静态链接） =====
echo.
echo === 构建 Linux 版 axc (x86_64-unknown-linux-musl) ===
cargo build --release --target x86_64-unknown-linux-musl %*
if %errorlevel% neq 0 (
    echo Linux 版构建失败
    exit /b 1
)
copy /y "target\x86_64-unknown-linux-musl\release\axc" "%DIST_DIR%\axc-linux" > "%TEMP%\axc_copy_linux.log"
echo 已输出: %DIST_DIR%\axc-linux

rem ===== 3. macOS 版（可选，需 macOS SDK） =====
echo.
echo === 尝试构建 macOS 版 axc (x86_64-apple-darwin) ===
cargo build --release --target x86_64-apple-darwin %* > "%TEMP%\axc_build_darwin.log" 2>&1
if %errorlevel% equ 0 (
    copy /y "target\x86_64-apple-darwin\release\axc" "%DIST_DIR%\axc-macos" > "%TEMP%\axc_copy_macos.log"
    echo 已输出: %DIST_DIR%\axc-macos
) else (
    echo macOS 版构建跳过（缺 macOS SDK/链接器，请在 macOS 机器上构建：cd rust && cargo build --release --target x86_64-apple-darwin）
)

echo.
echo 全部完成。产物目录: %DIST_DIR%
dir /b "%DIST_DIR%"
