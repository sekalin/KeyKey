@echo off
chcp 65001 >nul
title KeyKey 琦琦注音 - Windows 11 安裝程式

:: Check for Administrator privileges and elevate if needed
net session >nul 2>&1
if %errorLevel% NEQ 0 (
    echo [KeyKey] 正在請求系統管理員權限以向 Windows 11 註冊輸入法...
    powershell -NoProfile -Command "Start-Process cmd -ArgumentList '/c \"\"%~f0\"\"' -Verb RunAs"
    exit /b
)

pushd "%~dp0"

if not exist "target\release\keykey_tsf.dll" (
    echo [提示] 尚未建置 64-bit 與 32-bit 輸入法核心，正在執行雙架構編譯...
    powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\build_all.ps1"
) else (
    if not exist "target\release\keykey_tsf_32.dll" (
        echo [提示] 尚未建置 32-bit (Win32/WOW64) 輸入法核心，正在執行雙架構編譯...
        powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\build_all.ps1"
    )
)

echo ===================================================================
echo     KeyKey (琦琦注音) - Windows 11 TSF 輸入法安裝與啟用程式
echo ===================================================================
echo.

echo [1/3] 正在向 Windows 11 系統註冊 TSF 輸入法伺服器...
.\target\release\keykey-tools.exe register-elevated
if %errorLevel% NEQ 0 (
    echo [錯誤] 註冊失敗，請確認檔案完整性。
    pause
    exit /b 1
)

echo.
echo [2/3] 正在同步 Windows 11 語言清單與啟用輸入法...
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\enable_ime.ps1"

echo.
echo [3/3] 正在重新載入 Windows 文字服務 (ctfmon)...
taskkill /f /im ctfmon.exe >nul 2>&1
start ctfmon.exe

echo.
echo ===================================================================
echo  [成功] 琦琦注音 (KeyKey) 已成功安裝並啟用！
echo  現在您可以按下快速鍵 [Win + Space] 或 [Shift + Alt] 切換使用。
echo ===================================================================
echo.
pause
