use std::env;
use keykey_core::cin::CinTable;
use keykey_core::engine::{EngineAction, KeyCode, KeyKeyEngine};

pub fn run(args: &[String]) -> i32 {
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    match command {
        "register" => {
            println!("Registering KeyKey TSF TIP with Windows 11...");
            match keykey_tsf::register::register_server() {
                Ok(_) => {
                    println!("Successfully registered KeyKey with Windows 11!");
                    0
                }
                Err(e) => {
                    eprintln!("Failed to register: {:?}", e);
                    1
                }
            }
        }
        "unregister" => {
            println!("Unregistering KeyKey TSF TIP from Windows 11...");
            match keykey_tsf::register::unregister_server() {
                Ok(_) => {
                    println!("Successfully unregistered KeyKey.");
                    0
                }
                Err(e) => {
                    eprintln!("Failed to unregister: {:?}", e);
                    1
                }
            }
        }
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
        unsafe {
            let _ = windows::Win32::System::Com::CoInitialize(None);
        }
        let reg_code = run(&["keykey-tools".to_string(), "register".to_string()]);
        assert_eq!(reg_code, 0);

        let unreg_code = run(&["keykey-tools".to_string(), "unregister".to_string()]);
        assert_eq!(unreg_code, 0);
    }

    #[test]
    fn test_main_entrypoint() {
        main();
    }
}
