use std::env;
use keykey_core::cin::CinTable;
use keykey_core::engine::{EngineAction, KeyCode, KeyKeyEngine};

fn is_elevated() -> bool {
    let subkey: Vec<u16> = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion".encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey = windows::Win32::System::Registry::HKEY::default();
    unsafe {
        windows::Win32::System::Registry::RegOpenKeyExW(
            windows::Win32::System::Registry::HKEY_LOCAL_MACHINE,
            windows::core::PCWSTR(subkey.as_ptr()),
            0,
            windows::Win32::System::Registry::KEY_WRITE,
            &mut hkey,
        ).is_ok()
    }
}

fn sync_language_list_and_refresh(enable: bool) {
    let tip = "0404:{B892F1C2-88E4-4866-96D5-812C465EB86B}{6786A76E-6F3C-4318-A785-5AE4E8251268}";
    let script = if enable {
        format!(
            "$tip = '{}'; $list = Get-WinUserLanguageList; $zh = $list | Where-Object {{ $_.LanguageTag -like 'zh-Hant*' -or $_.LanguageTag -like 'zh-TW*' }}; if ($zh -and ($zh.InputMethodTips -notcontains $tip)) {{ $zh.InputMethodTips.Add($tip); Set-WinUserLanguageList $list -Force }}; Stop-Process -Name ctfmon -Force -ErrorAction SilentlyContinue; Start-Process ctfmon.exe",
            tip
        )
    } else {
        format!(
            "$tip = '{}'; $list = Get-WinUserLanguageList; $zh = $list | Where-Object {{ $_.LanguageTag -like 'zh-Hant*' -or $_.LanguageTag -like 'zh-TW*' }}; if ($zh -and ($zh.InputMethodTips -contains $tip)) {{ [void]$zh.InputMethodTips.Remove($tip); Set-WinUserLanguageList $list -Force }}; Stop-Process -Name ctfmon -Force -ErrorAction SilentlyContinue; Start-Process ctfmon.exe",
            tip
        )
    };
    let _ = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output();
}

fn request_elevation(arg: &str) -> bool {
    if let Ok(val) = env::var("KEYKEY_TEST_UAC") {
        return val == "1";
    }
    if let Ok(exe_path) = env::current_exe() {
        let script = format!(
            "Start-Process -FilePath '{}' -ArgumentList '{}' -Verb RunAs -Wait",
            exe_path.display(),
            arg
        );
        let status = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .status();
        if let Ok(st) = status {
            return st.success();
        }
    }
    false
}

fn register_core() -> i32 {
    println!("Registering KeyKey TSF TIP with Windows 11...");
    match keykey_tsf::register::register_server() {
        Ok(_) => {
            sync_language_list_and_refresh(true);
            println!("Successfully registered KeyKey with Windows 11!");
            0
        }
        Err(e) => {
            eprintln!("Failed to register: {:?}", e);
            1
        }
    }
}

fn unregister_core() -> i32 {
    println!("Unregistering KeyKey TSF TIP from Windows 11...");
    match keykey_tsf::register::unregister_server() {
        Ok(_) => {
            sync_language_list_and_refresh(false);
            println!("Successfully unregistered KeyKey.");
            0
        }
        Err(e) => {
            eprintln!("Failed to unregister: {:?}", e);
            1
        }
    }
}

pub fn run(args: &[String]) -> i32 {
    unsafe {
        let _ = windows::Win32::System::Com::CoInitialize(None);
    }
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    match command {
        "register" => {
            if !is_elevated() {
                println!("提示：註冊 Windows 11 原生輸入法需要系統管理員權限 (Administrator)。");
                println!("正在自動請求系統管理員權限以完成註冊 (請在出現的 UAC 提示視窗中點擊「是」)...");
                if request_elevation("register-elevated") {
                    println!("已成功取得權限並完成註冊！現在可按 Win + Space 切換為「琦琦注音」。");
                    return 0;
                }
                println!("若未授權 UAC，您也可以在開始選單中以「系統管理員身分」開啟終端機後再執行 register。");
                return 1;
            }
            register_core()
        }
        "register-elevated" => register_core(),
        "unregister" => {
            if !is_elevated() {
                println!("提示：解除安裝 Windows 11 輸入法需要系統管理員權限 (Administrator)。");
                println!("正在自動請求系統管理員權限 (請在出現的 UAC 提示視窗中點擊「是」)...");
                if request_elevation("unregister-elevated") {
                    println!("已成功解除安裝 KeyKey。");
                    return 0;
                }
                return 1;
            }
            unregister_core()
        }
        "unregister-elevated" => unregister_core(),
        "test-type" => {
            println!("=== KeyKey Typing Simulator ===");
            let default_cin: &[u8] =
                include_bytes!("../../../YahooKeyKey-Source-1.1.2528/DataTables/bpmf.cin");
            let cin_table = CinTable::load_from_reader(default_cin).expect("Failed to load embedded cin");
            let mut engine = KeyKeyEngine::new(cin_table);

            let sequence = ['a', '1', 'j', '4', '1']; // 'a' (ignored/fallback), then ㄅ + ㄨ + ˋ, then select candidate 1
            println!("Simulating input sequence: a (英文直通), 1 (ㄅ), j (ㄨ), 4 (ˋ), 1 (選第1字)");

            for key in sequence {
                println!("--- Input key: '{}' ---", key);
                let actions = engine.handle_key(KeyCode::Char(key));
                for action in actions {
                    match action {
                        EngineAction::UpdateComposition { display_text, cursor_pos } => {
                            println!("  [組字區] '{}' (游標: {})", display_text, cursor_pos);
                        }
                        EngineAction::ShowCandidates { candidates, page, total_pages } => {
                            println!("  [候選字] 頁碼: {}/{}", page + 1, total_pages);
                            for (sel, cand) in candidates {
                                print!("  {}.{} ", sel, cand);
                            }
                            println!();
                        }
                        EngineAction::Commit(text) => {
                            println!("  ===> [上字輸出 COMMIT] '{}' <===", text);
                        }
                        EngineAction::HideCandidates => {
                            println!("  [隱藏候選字框]");
                        }
                        _ => {}
                    }
                }
            }

            // Also simulate Escape to test HideCandidates
            let esc_actions = engine.handle_key(KeyCode::Escape);
            for action in esc_actions {
                if let EngineAction::HideCandidates = action {
                    println!("  [隱藏候選字框]");
                }
            }
            0
        }
        _ => {
            println!("KeyKey Control CLI Tool");
            println!("Usage:");
            println!("  keykey-tools register    - Register KeyKey as a Windows 11 TSF IME");
            println!("  keykey-tools unregister  - Unregister KeyKey from Windows 11");
            println!("  keykey-tools test-type   - Run simulated typing test in console");
            0
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let code = run(&args);
    if code != 0 {
        std::process::exit(code);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_help() {
        assert_eq!(run(&["keykey-tools".to_string()]), 0);
        assert_eq!(run(&["keykey-tools".to_string(), "help".to_string()]), 0);
        assert_eq!(run(&["keykey-tools".to_string(), "unknown-cmd".to_string()]), 0);
    }

    #[test]
    fn test_cli_test_type() {
        assert_eq!(run(&["keykey-tools".to_string(), "test-type".to_string()]), 0);
    }

    #[test]
    fn test_cli_register_and_unregister() {
        let reg_code = run(&["keykey-tools".to_string(), "register-elevated".to_string()]);
        assert_eq!(reg_code, 0);

        let unreg_code = run(&["keykey-tools".to_string(), "unregister-elevated".to_string()]);
        assert_eq!(unreg_code, 0);

        let _ = is_elevated();

        // Test non-elevated simulation with UAC success
        env::set_var("KEYKEY_TEST_UAC", "1");
        assert_eq!(run(&["keykey-tools".to_string(), "register".to_string()]), 0);
        assert_eq!(run(&["keykey-tools".to_string(), "unregister".to_string()]), 0);

        // Test non-elevated simulation with UAC denial
        env::set_var("KEYKEY_TEST_UAC", "0");
        assert_eq!(run(&["keykey-tools".to_string(), "register".to_string()]), 1);
        assert_eq!(run(&["keykey-tools".to_string(), "unregister".to_string()]), 1);

        env::remove_var("KEYKEY_TEST_UAC");
    }

    #[test]
    fn test_main_entrypoint() {
        main();
    }
}
