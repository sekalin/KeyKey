@echo off
chcp 65001 >nul
title KeyKey 琦琦注音 - Windows 11 解除安裝程式

net session >nul 2>&1
if %errorLevel% NEQ 0 (
    echo [KeyKey] 正在請求系統管理員權限...
    powershell -NoProfile -Command "Start-Process cmd -ArgumentList '/c \"\"%~f0\"\"' -Verb RunAs"
    exit /b
)

pushd "%~dp0"

echo ===================================================================
echo     KeyKey (琦琦注音) - Windows 11 輸入法解除安裝程式
echo ===================================================================
echo.

echo [1/2] 正在從 Windows 11 反註冊 TSF 輸入法...
.\target\release\keykey-tools.exe unregister-elevated

echo.
echo [2/2] 正在自語言清單移除並重新整理...
powershell -NoProfile -ExecutionPolicy Bypass -Command "$tip = '0404:{B892F1C2-88E4-4866-96D5-812C465EB86B}{6786A76E-6F3C-4318-A785-5AE4E8251268}'; $list = Get-WinUserLanguageList; $zh = $list | Where-Object { $_.LanguageTag -like 'zh-Hant*' -or $_.LanguageTag -like 'zh-TW*' }; if ($zh -and ($zh.InputMethodTips -contains $tip)) { [void]$zh.InputMethodTips.Remove($tip); Set-WinUserLanguageList $list -Force }; Stop-Process -Name ctfmon -Force -ErrorAction SilentlyContinue; Start-Process ctfmon.exe"

echo.
echo [成功] KeyKey 已完全從系統移除。
echo.
pause
