@echo off
rem 构建 Rust 版 axc —— 不依赖已打开的终端窗口里 PATH 是否已刷新。
rem 环境变量在进程启动时固定；rustup 装完后老窗口看不到 cargo，但注册表已更新。
setlocal

cd /d "%~dp0"

where cargo >nul 2>nul
if %errorlevel%==0 goto build

if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
    set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
    goto build
)

echo 找不到 cargo。请确认已安装 Rust，或把 %USERPROFILE%\.cargo\bin 加入 PATH。
exit /b 1

:build
cargo build --release %*
if %errorlevel% neq 0 exit /b %errorlevel%
echo.
echo 产物: target\release\axc.exe
