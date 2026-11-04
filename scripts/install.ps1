# KeyKey 注音輸入 - Windows 11 安裝與啟用腳本
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Host "[KeyKey] 正在請求系統管理員權限以向 Windows 11 註冊輸入法 (請在 UAC 視窗點擊「是」)..." -ForegroundColor Yellow
    try {
        Start-Process powershell -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`"" -Verb RunAs
        exit
    } catch {
        Write-Host "[提示] 自動請求系統管理員權限失敗。請在開始選單中以「系統管理員身分執行」開啟終端機，再執行此安裝檔。" -ForegroundColor Red
        if (-not [Console]::IsInputRedirected) {
            Read-Host "請按 Enter 鍵結束..."
        }
        exit 1
    }
}

$rootDir = Split-Path -Parent $PSScriptRoot
Set-Location $rootDir

Write-Host "===================================================================" -ForegroundColor Cyan
Write-Host "    KeyKey 注音輸入 - Windows 11 TSF 原生輸入法安裝程式" -ForegroundColor Cyan
Write-Host "===================================================================" -ForegroundColor Cyan
Write-Host ""

$dll64 = Join-Path $rootDir "target\release\keykey_tsf.dll"
$dll32 = Join-Path $rootDir "target\release\keykey_tsf_32.dll"
$tools = Join-Path $rootDir "target\release\keykey-tools.exe"

if (-not (Test-Path $dll64) -or -not (Test-Path $dll32) -or -not (Test-Path $tools)) {
    Write-Host "[提示] 正在編譯 64-bit 與 32-bit (Win32/WOW64) 輸入法核心元件..." -ForegroundColor Green
    & "$PSScriptRoot\build_all.ps1"
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[錯誤] 編譯失敗，請確認 Rust 工具鏈環境。" -ForegroundColor Red
        if (-not [Console]::IsInputRedirected) {
            Read-Host "請按 Enter 鍵結束..."
        }
        exit 1
    }
}

Write-Host "[1/3] 正在向 Windows 11 系統註冊 TSF 輸入法 COM 伺服器..." -ForegroundColor Green
& $tools register-elevated
if ($LASTEXITCODE -ne 0) {
    Write-Host "[錯誤] 註冊失敗，請確認檔案完整性。" -ForegroundColor Red
    if (-not [Console]::IsInputRedirected) {
        Read-Host "請按 Enter 鍵結束..."
    }
    exit 1
}

Write-Host ""
Write-Host "[2/3] 正在同步 Windows 11 語言清單並加入 KeyKey..." -ForegroundColor Green
& "$PSScriptRoot\enable_ime.ps1"

Write-Host ""
Write-Host "[3/3] 正在確認 Windows 系統文字服務 (ctfmon) 運行狀態..." -ForegroundColor Green
if (-not (Get-Process ctfmon -ErrorAction SilentlyContinue)) {
    Start-Process ctfmon.exe
}

Write-Host ""
Write-Host "===================================================================" -ForegroundColor Cyan
Write-Host " [成功] KeyKey 注音輸入法 已成功安裝並啟用！" -ForegroundColor Green
Write-Host " 現在您可以按下快捷鍵 [Win + Space] 或 [Shift + Alt] 切換使用。" -ForegroundColor White
Write-Host "===================================================================" -ForegroundColor Cyan
Write-Host ""
if (-not [Console]::IsInputRedirected) {
    Read-Host "請按 Enter 鍵結束..."
}
