# KeyKey 注音輸入 - Windows 11 TSF 原生重構版

[![CI](https://github.com/sekalin/KeyKey/actions/workflows/rust.yml/badge.svg)](https://github.com/sekalin/KeyKey/actions)
[![License](https://img.shields.io/badge/license-BSD--3--Clause-blue.svg)](LICENSE.txt)

> **KeyKey 注音輸入** 專案源自於經典開源輸入法 **Yahoo! 奇摩輸入法 (Yahoo! KeyKey)**。  
> 本專案以 **100% 純 Rust** 重新打造，全面擁抱 **Windows 11 原生 Text Services Framework (TSF)** 標準架構，擺脫過往 IMM32 舊式視窗相容性問題，並同時原生支援 **Win64 (64 位元)** 與 **Win32 (32 位元 WOW64)** 應用程式。

---

##  核心特色

* **純 Rust 打造**：零 C++、零 C# / .NET 依賴，無外加執行階段（Runtime），記憶體精確安全且極度輕量。
* **Win32 + Win64 雙架構全面支援**：
  * **64 位元核心**：`keykey_tsf.dll` 注入原生 64 位元應用程式（如 Edge、Chrome、VS Code、Windows 記事本、Office 64-bit）。
  * **32 位元核心**：`keykey_tsf_32.dll` 注入 32 位元 WOW64 應用程式（如 Steam、舊型企業軟體、32 位元遊戲），徹底解決「32 位元應用程式打不出字或輸入法失效」的常見問題。
  * **COM 註冊雙重重定向**：同一個 CLSID 同步登記於 `HKCR\CLSID` 與 `HKCR\WOW6432Node\CLSID`。
* **雙軌候選字視窗架構（Win11 原生 Shell + 純 Rust 自繪相容視窗）**：
  * **Windows 11 原生 Shell 整合 (`ITfCandidateListUIElement`)**：於現代 XAML / UWP 應用程式直接由系統渲染 Fluent 候選字清單。
  * **純 Rust 自繪 Fluent 懸浮視窗 (`CustomCandidateWindow`)**：針對舊式 Win32 應用程式、傳統終端機、控制台或未啟動 Modern Shell 的遊戲，以 100% 純 Rust + GDI 雙緩衝無閃爍自繪，套用 Windows 11 DWM 圓角（`DWMWCP_ROUND`）、陰影與膠囊型選取效果，具備 `WS_EX_NOACTIVATE | WS_EX_TOPMOST` 絕對不搶焦點保證，徹底解決同類專案在 Win11「看不到候選字視窗」的問題。
  * **智慧游標追蹤**：支援 TSF `GetTextExt` -> Win32 Caret (`GetGUIThreadInfo`) -> 游標座標三層式精確定位，並具備螢幕邊界自動翻轉（避免超出螢幕底部或邊緣）。
* **內建現代擴充詞庫（整合 polobread/KeyKey 專業分類詞庫）**：
  * 除了原版 Yahoo! KeyKey 單字表，內建編譯 3,925 筆現代專業與生活詞庫（涵蓋人工智慧、軟體開發、半導體、資訊安全、金融商務與現代高頻生活詞彙）。
  * 支援多字詞音節編碼查詢與首字音節關聯候選，開箱即用免手動匯入。
* **純標準 TSF 編輯作業 (`ITfEditSession`) 與零當機保證 (Anti-Hang Architecture)**：
  * 徹底摒棄 `SendInput`、`keybd_event` 或偽造 `WM_CHAR` 等易遭攔截或丟字的落後做法。
  * 透過同步讀寫鎖 (`TF_ES_READWRITE | TF_ES_SYNC`) 直接操作文件文字範圍 (`ITfRange`)，文字上字 100% 準確不丟字。
  * 退格鍵（Backspace）於注音拼音期間精確遞減聲母、韻母、聲調，清空時自然透傳至目標程式，杜絕雙重刪除或刪字異常。
  * 所有 COM 介面與視窗回呼皆以 `catch_unwind` 與非阻塞鎖全面防護，通過 10,000 次急促擊鍵與混沌注入壓力測試，絕不掛死主應用程式。
* **無縫中英切換與系統匣狀態同步**：
  * 按下單獨的 <kbd>Shift</kbd> 鍵瞬切「中文（注音）」與「英文直通」。
  * 串接 `ITfCompartmentEventSink` 與 `GUID_COMPARTMENT_KEYBOARD_OPENCLOSE`，按下 Shift 或點擊工作列系統匣時，工作列的「中／英」圖示雙向即時同步。
* **台灣慣用全形標點符號與大寫字母直通**：
  * 大千式注音鍵盤上的無 Shift 按鍵 `, . / ; -` 為韻母（`ㄝ ㄡ ㄥ ㄤ ㄦ`）。
  * 支援快速輸入標點：`Shift + ,`（`，`）、`Shift + .`（`。`）、`Shift + /`（`？`）、`Shift + ;`（`：`）、`Shift + 1`（`！`）、`Shift + [`（`「`）、`Shift + ]`（`」`）、`Shift + \` 或 `\`（`、`）、`Shift + '`（`”`）、`Shift + -`（`——`）。
  * 中文模式下按下 `Shift + A~Z` 直接輸出大寫英文字母，不干擾拼音。

---

## 詞庫與外部資料來源致謝 (References & Data Sources)

本專案之字表、注音對應與詞庫架構奠基並整合以下開源資源：

1. **原版 Yahoo! 奇摩輸入法 (Yahoo! KeyKey)**:
   * 官方開源專案版本：`YahooKeyKey-Source-1.1.2528` (New BSD License)。
   * 本專案的核心基礎注音字表（`bpmf.cin`）與補充詞庫對照表（`Keke-AddBpmf.txt`）直接源自原版 Yahoo! KeyKey。
   * 官方網站歷史存檔：[Yahoo! KeyKey](http://tw.media.yahoo.com/keykey/)。
2. **現代專業分類詞庫整合來源 ([polobread/KeyKey](https://github.com/polobread/KeyKey))**:
   * 參考並整合社群專案 `polobread/KeyKey` 所收錄之現代台灣專業用語詞庫集合（`chichi77Collection`），包含：
     * `phrase.ai-data-science.tsv` (人工智慧與資料科學詞庫)
     * `phrase.software.tsv` (軟體工程與資訊技術詞庫)
     * `phrase.semiconductor.tsv` (半導體製造與硬體產業詞庫)
     * `phrase.network-security.tsv` (網路通訊與資訊安全詞庫)
     * `phrase.finance.tsv` (現代金融與加密貨幣詞庫)
     * `phrase.business.tsv` (企業商務與管理名詞)
     * `phrase.general.tsv` (台灣現代高頻生活常用語)
   * 由自動化工具 [`scripts/generate_phrases.py`](file:///D:/dev/KeyKey/scripts/generate_phrases.py) 將上述詞庫與注音轉換為大千式 CIN 編碼，編譯為 [`crates/keykey-core/data/phrases.cin`](file:///D:/dev/KeyKey/crates/keykey-core/data/phrases.cin) 並內建於輸入法核心，啟動時自動與基礎字表雙向無縫合併。
3. **中央研究院語言學研究所平衡語料庫 (Sinica Corpus)**:
   * 台灣繁體中文詞頻與詞庫關聯資料基礎。
4. **OpenVanilla 專案 (OpenVanilla Input Method Project)**:
   * 提供開源輸入法架構與 `.cin` 字表標準規範。

---

## 專案結構

```text
KeyKey/
├── crates/
│   ├── keykey-core/           # 純 Rust 注音輸入法核心（音節組字、字表解析、候選字分頁）
│   ├── keykey-tsf/            # Windows 11 TSF TIP 實作（COM 服務、EditSession、候選字 UI）
│   └── keykey-tools/          # 輸入法註冊管理與測試 CLI 工具
├── DataSource-1.1.2528/       # Yahoo! KeyKey 原始字詞庫與語料庫資料
├── YahooKeyKey-Source-1.1.2528/# Yahoo! KeyKey 歷史 C++ 原始碼與資料表
├── scripts/
│   ├── build_all.ps1          # 一鍵雙架構 (x86_64 + i686) Release 建置腳本
│   └── enable_ime.ps1         # Windows 11 語言清單同步腳本
├── install_admin.bat          # 系統管理員一鍵安裝與啟用
├── uninstall_admin.bat        # 系統管理員一鍵解除安裝
└── README.md
```

---

## 安裝與使用方式

### 方式 1：一鍵安裝（推薦）
1. 在專案根目錄中，對 [`install_admin.bat`](file:///D:/dev/KeyKey/install_admin.bat) 按右鍵選擇**「以系統管理員身分執行」**（或直接點擊執行，程式會自動請求 UAC 提升）。
2. 安裝程式將會：
   * 自動編譯並檢查 64 位元與 32 位元核心。
   * 將輸入法 COM 伺服器註冊至 Windows 11 系統（包含 64 位元與 WOW6432Node）。
   * 將 KeyKey 登記至 Windows 11 語言清單 (`0404:{B892F1C2-88E4-4866-96D5-812C465EB86B}{6786A76E-6F3C-4318-A785-5AE4E8251268}`)。
   * 重啟 Windows 文字服務（`ctfmon.exe`）。
3. 安裝完成後，即可按下 <kbd>Win</kbd> + <kbd>Space</kbd> 切換為 **「中文（台灣）- KeyKey 注音輸入」** 進行打字！

### 方式 2：解除安裝
* 執行 [`uninstall_admin.bat`](file:///D:/dev/KeyKey/uninstall_admin.bat)，將會自動反註冊 COM 伺服器並自語言清單中安全移除。

---

## 手動編譯與測試

### 1. 安裝 Rust 目標架構
```powershell
rustup target add x86_64-pc-windows-msvc
rustup target add i686-pc-windows-msvc
```

### 2. 一鍵建置雙架構二進位檔
```powershell
powershell -ExecutionPolicy Bypass -File scripts\build_all.ps1
```
建置完成後，於 `target/release/` 下可取得：
* `keykey_tsf.dll` (Win64)
* `keykey_tsf_32.dll` (Win32 / WOW64)
* `keykey-tools.exe` (管理 CLI 工具)

### 3. 執行全單元測試與覆蓋率檢驗
```powershell
# 執行全部 42 項單元測試、整合測試與 E2E 測試
cargo test --workspace

# 驗證 32 位元 i686 架構測試
cargo test --target i686-pc-windows-msvc --package keykey-tsf

# 產生 LLVM 測試覆蓋率報告（Line Coverage > 95%）
cargo llvm-cov --workspace
```

---

##  授權條款 (License)

本專案採 **New BSD License** 釋出，請參閱 [LICENSE.txt](LICENSE.txt) 取得完整內容。
