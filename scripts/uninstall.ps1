# KeyKey 注音輸入 - Windows 11 解除安裝腳本
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Host "[KeyKey] 正在請求系統管理員權限以解除安裝輸入法 (請在 UAC 視窗點擊「是」)..." -ForegroundColor Yellow
    try {
        Start-Process powershell -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`"" -Verb RunAs
        exit
    } catch {
        Write-Host "[提示] 自動請求系統管理員權限失敗。請在開始選單中以「系統管理員身分執行」開啟終端機，再執行此解除安裝檔。" -ForegroundColor Red
        if (-not [Console]::IsInputRedirected) {
            Read-Host "請按 Enter 鍵結束..."
        }
        exit 1
    }
}

$rootDir = Split-Path -Parent $PSScriptRoot
Set-Location $rootDir

Write-Host "===================================================================" -ForegroundColor Cyan
Write-Host "    KeyKey 注音輸入 - Windows 11 輸入法解除安裝程式" -ForegroundColor Cyan
Write-Host "===================================================================" -ForegroundColor Cyan
Write-Host ""

$tools = Join-Path $rootDir "target\release\keykey-tools.exe"
if (Test-Path $tools) {
    Write-Host "[1/2] 正在向 Windows 11 系統反註冊 TSF 輸入法..." -ForegroundColor Green
    & $tools unregister-elevated
}

Write-Host "[2/2] 正在自 Windows 語言清單中移除 KeyKey..." -ForegroundColor Green
$tip = '0404:{B892F1C2-88E4-4866-96D5-812C465EB86B}{6786A76E-6F3C-4318-A785-5AE4E8251268}'
$list = Get-WinUserLanguageList
$zh = $list | Where-Object { $_.LanguageTag -like 'zh-Hant*' -or $_.LanguageTag -like 'zh-TW*' -or $_.LanguageTag -like 'zh-HK*' }
if ($zh -and ($zh.InputMethodTips -contains $tip)) {
    [void]$zh.InputMethodTips.Remove($tip)
    Set-WinUserLanguageList $list -Force
}

if (-not (Get-Process ctfmon -ErrorAction SilentlyContinue)) {
    Start-Process ctfmon.exe
}

Write-Host ""
Write-Host "===================================================================" -ForegroundColor Cyan
Write-Host " [成功] KeyKey 已完全從系統移除。" -ForegroundColor Green
Write-Host "===================================================================" -ForegroundColor Cyan
Write-Host ""
if (-not [Console]::IsInputRedirected) {
    Read-Host "請按 Enter 鍵結束..."
}
