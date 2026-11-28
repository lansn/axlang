@echo off
:: axc 发行版复制与验证脚本
:: 用途：将 dist/ 完整复制到目标目录，并验证 axc.exe 可正常启动
:: 平时只做验证及测试，决定分发时使用
:: 
:: 用法：
::   copy_axc_release.bat <目标目录>
::   示例：copy_axc_release.bat "C:\Users\lansn-lan\Desktop\test\ax"

setlocal enabledelayedexpansion

:: 当前脚本所在目录的上一级应为 axlang 项目根目录
set "SCRIPT_DIR=%~dp0"
set "PROJECT_ROOT=%SCRIPT_DIR%.."
set "DIST_DIR=%PROJECT_ROOT%dist"

if "%~1"=="" (
    echo 用法: %~nx0 <目标目录>
    echo 示例: %~nx0 "C:\Users\lansn-lan\Desktop\test\ax"
    goto :end
)

set "TARGET_DIR=%~1"

echo ========================================
echo  axc 发行版复制与验证脚本
echo ========================================
echo.

:: 检查 dist/ 是否存在
if not exist "%DIST_DIR%" (
    echo [错误] 找不到 dist/ 目录：%DIST_DIR%
    echo 请确保在 axlang 项目根目录下运行此脚本
    goto :end
)

:: 检查 axc.exe 是否存在
if not exist "%DIST_DIR%\axc.exe" (
    echo [错误] 找不到 %DIST_DIR%\axc.exe
    goto :end
)

:: 检查 axc/ 子目录是否存在
if not exist "%DIST_DIR%\axc\" (
    echo [错误] 找不到 %DIST_DIR%\axc\ 子目录
    echo 预编译缓存文件（.o + .hash）缺失，无法正常运行
    goto :end
)

echo [1/3] 检查 dist/ 完整性...
if exist "%DIST_DIR%\axc\rt\obj\axrt_atomic.o" (
    echo   OK: 运行时缓存已就绪
) else (
    echo   警告: 未找到运行时预编译缓存，可能需要先运行 axc precompile
)
if exist "%DIST_DIR%\axc\stdlib\obj\atomic.o" (
    echo   OK: 标准库缓存已就绪
) else (
    echo   警告: 未找到标准库预编译缓存，可能需要先运行 axc precompile
)

echo.
echo [2/3] 复制 dist/ 到目标目录：%TARGET_DIR%
if not exist "%TARGET_DIR%\" (
    mkdir "%TARGET_DIR%"
    if errorlevel 1 (
        echo [错误] 无法创建目标目录：%TARGET_DIR%
        goto :end
    )
)

xcopy /E /I /Y "%DIST_DIR%\*" "%TARGET_DIR%\*"
if errorlevel 1 (
    echo [错误] 复制失败
    goto :end
)
echo   OK: 已复制

echo.
echo [3/3] 验证目标目录中的 axc.exe
set "TARGET_AXC=%TARGET_DIR%\axc.exe"
if exist "%TARGET_AXC%" (
    echo   执行：%TARGET_AXC% --version
    "%TARGET_AXC%" --version
    if errorlevel 1 (
        echo [错误] axc.exe 无法启动
        goto :end
    )
    echo   OK: axc.exe 启动正常
) else (
    echo [错误] 目标目录中找不到 axc.exe
    goto :end
)

echo.
echo ========================================
echo  分发包准备就绪：%TARGET_DIR%
echo  使用方式：%TARGET_AXC% <file.ax>
echo ========================================

:end
endlocal
pause
