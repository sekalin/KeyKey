use std::env;
use keykey_core::cin::CinTable;
use keykey_core::engine::{EngineAction, KeyCode, KeyKeyEngine};

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    match command {
        "register" => {
            println!("Registering KeyKey TSF TIP with Windows 11...");
            match keykey_tsf::register::register_server() {
                Ok(_) => println!("Successfully registered KeyKey with Windows 11!"),
                Err(e) => eprintln!("Failed to register: {:?}", e),
            }
        }
        "unregister" => {
            println!("Unregistering KeyKey TSF TIP from Windows 11...");
            match keykey_tsf::register::unregister_server() {
                Ok(_) => println!("Successfully unregistered KeyKey."),
                Err(e) => eprintln!("Failed to unregister: {:?}", e),
            }
        }
        "test-type" => {
            println!("=== KeyKey Typing Simulator ===");
            let default_cin: &[u8] =
                include_bytes!("../../../YahooKeyKey-Source-1.1.2528/DataTables/bpmf.cin");
            let cin_table = CinTable::load_from_reader(default_cin).expect("Failed to load embedded cin");
            let mut engine = KeyKeyEngine::new(cin_table);

            let sequence = ['1', 'j', '4', '1']; // ㄅ + ㄨ + ˋ, then select candidate 1
            println!("Simulating input sequence: 1 (ㄅ), j (ㄨ), 4 (ˋ), 1 (選第1字)");

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
        }
        _ => {
            println!("KeyKey Control CLI Tool");
            println!("Usage:");
            println!("  keykey-tools register    - Register KeyKey as a Windows 11 TSF IME");
            println!("  keykey-tools unregister  - Unregister KeyKey from Windows 11");
            println!("  keykey-tools test-type   - Run simulated typing test in console");
        }
    }
}
