# axc/stdlib/build.ps1 —— axlang 标准库预转译脚本
#
# 用途：将 axc/stdlib/*.ax 预转译为标准 C 代码，输出到 axc/stdlib/generated/，
# 随编译器发布，通过 gcc 与用户代码一起编译链接。
#
# 预转译原理：调用 axc 编译器的 --emit=c 模式，将 ax 标准库源码转译为 C。
# 标准库 ax 源码通过 extern "C" 调用 axrt C 运行时原语（ax_mutex_*、ax_atomic_* 等），
# ax 层仅做包装，不引入 ax 编译器内建能力（print/println/fmt、泛型等仍由编译器内建）。
#
# 用法（在 axlang 仓库根目录下执行）：
#   pwsh -File axc/stdlib/build.ps1
#
# 可选参数：
#   -AxC <path>   指定 axc 编译器路径（默认尝试 dist/axc.exe 或 rust/target/release/axc.exe）
#   -Clean      预转译前清空 generated/ 目录

param(
    [string]$AxC = "",
    [switch]$Clean
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$RootDir = Resolve-Path (Join-Path $ScriptDir "..\..")
$StdlibDir = Join-Path $RootDir "axc\stdlib"
$GenDir = Join-Path $StdlibDir "generated"
$HeaderFile = Join-Path $StdlibDir "axc_stdlib.h"

# --- 确定 axc 编译器路径 ---
if (-not $AxC) {
    $candidates = @(
        Join-Path $RootDir "dist\axc.exe",
        Join-Path $RootDir "rust\target\release\axc.exe",
        Join-Path $RootDir "rust\target\debug\axc.exe"
    )
    foreach ($c in $candidates) {
        if (Test-Path $c) { $AxC = $c; break }
    }
}
if (-not $AxC -or -not (Test-Path $AxC)) {
    Write-Error "找不到 axc 编译器，请通过 -AxC 指定路径，或确保 dist/axc.exe 存在。"
    exit 1
}
Write-Host "使用 axc 编译器: $AxC" -ForegroundColor Cyan

# --- 清空生成目录（可选）---
if ($Clean) {
    if (Test-Path $GenDir) {
        Remove-Item -Recurse -Force $GenDir
    }
}

# --- 确保生成目录存在 ---
if (-not (Test-Path $GenDir)) {
    New-Item -ItemType Directory -Path $GenDir | Out-Null
}

# --- 复制标准库 C 接口头文件 ---
$DestHeader = Join-Path $GenDir "axc_stdlib.h"
Copy-Item -Path $HeaderFile -Destination $DestHeader -Force
Write-Host "已复制头文件: axc_stdlib.h" -ForegroundColor Green

# --- 预转译每个 .ax 文件 ---
$axFiles = Get-ChildItem -Path $StdlibDir -Filter "*.ax" -Name
if ($axFiles.Count -eq 0) {
    Write-Warning "未在 $StdlibDir 中找到任何 .ax 文件。"
    exit 0
}

$success = 0
$failed = 0

foreach ($axFile in $axFiles) {
    $axPath = Join-Path $StdlibDir $axFile
    $baseName = [System.IO.Path]::GetFileNameWithoutExtension($axFile)
    $cFileName = "$baseName.c"
    $cPath = Join-Path $GenDir $cFileName

    Write-Host "预转译: $axFile -> generated/$cFileName" -ForegroundColor Gray

    # 调用 axc --emit=c 生成 C 源码
    $process = Start-Process -FilePath $AxC -ArgumentList @("--emit=c", $axPath) -RedirectStandardOutput $cPath -RedirectStandardError "stderr_$baseName.txt" -Wait -NoNewWindow -PassThru

    if ($process.ExitCode -eq 0) {
        Write-Host "  ✓ 成功" -ForegroundColor Green
        $success++
    } else {
        Write-Host "  ✗ 失败 (退出码 $($process.ExitCode))" -ForegroundColor Red
        if (Test-Path "stderr_$baseName.txt") {
            Get-Content "stderr_$baseName.txt"
            Remove-Item "stderr_$baseName.txt"
        }
        $failed++
    }
}

Write-Host ""
Write-Host "预转译完成: 成功 $success, 失败 $failed" -ForegroundColor Cyan
if ($failed -gt 0) {
    exit 1
}
