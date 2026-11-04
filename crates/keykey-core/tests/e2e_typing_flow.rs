use keykey_core::cin::CinTable;
use keykey_core::engine::{EngineAction, InputMode, KeyCode, KeyKeyEngine};
use std::path::PathBuf;

fn load_real_cin() -> CinTable {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("data");
    path.push("bpmf.cin");

    CinTable::load_from_file(&path).expect("Failed to load bpmf.cin")
}

/// Helper simulator to feed a sequence of KeyCodes into the engine and collect committed text
fn simulate_typing(engine: &mut KeyKeyEngine, keys: &[KeyCode]) -> (String, String) {
    let mut committed = String::new();
    let mut current_composition = String::new();

    for key in keys {
        let actions = engine.handle_key(key.clone());
        for action in actions {
            match action {
                EngineAction::UpdateComposition { display_text, .. } => {
                    current_composition = display_text;
                }
                EngineAction::Commit(text) => {
                    committed.push_str(&text);
                    current_composition.clear();
                }
                EngineAction::HideCandidates => {}
                _ => {}
            }
        }
    }

    (committed, current_composition)
}

#[test]
fn test_e2e_full_sentence_typing() {
    let table = load_real_cin();
    let mut engine = KeyKeyEngine::new(table);

    // Sentence: "今天天氣很好"
    // 今: ㄐ(r) ㄧ(u) ㄣ(p) 1聲(Space) -> cand 1
    // 天: ㄊ(w) ㄧ(u) ㄢ(0) 1聲(Space) -> cand 1
    // 天: ㄊ(w) ㄧ(u) ㄢ(0) 1聲(Space) -> cand 1
    // 氣: ㄑ(f) ㄧ(u) ˋ(4) -> cand 1
    // 很: ㄏ(c) ㄣ(p) ˇ(3) -> cand 1
    // 好: ㄏ(c) ㄠ(l) ˇ(3) -> cand 1

    let keys = vec![
        // 今
        KeyCode::Char('r'), KeyCode::Char('u'), KeyCode::Char('p'), KeyCode::Space, KeyCode::Char('1'),
        // 天
        KeyCode::Char('w'), KeyCode::Char('u'), KeyCode::Char('0'), KeyCode::Space, KeyCode::Char('1'),
        // 天
        KeyCode::Char('w'), KeyCode::Char('u'), KeyCode::Char('0'), KeyCode::Space, KeyCode::Char('1'),
        // 氣 (fu4 -> cand 2 is 氣, cand 1 is 企)
        KeyCode::Char('f'), KeyCode::Char('u'), KeyCode::Char('4'), KeyCode::Char('2'),
        // 很
        KeyCode::Char('c'), KeyCode::Char('p'), KeyCode::Char('3'), KeyCode::Char('1'),
        // 好
        KeyCode::Char('c'), KeyCode::Char('l'), KeyCode::Char('3'), KeyCode::Char('1'),
    ];

    let (committed, composition) = simulate_typing(&mut engine, &keys);

    println!("E2E Committed sentence: '{}'", committed);
    assert_eq!(committed, "今天天氣很好");
    assert_eq!(composition, "");
}

#[test]
fn test_e2e_candidate_selection_options() {
    let table = load_real_cin();
    let mut engine = KeyKeyEngine::new(table);

    // ㄅ(1) ㄨ(j) ˋ(4)
    // Option 1 is "不"
    // Option 2 is "部"
    // Option 3 is "布"

    let keys_bu2 = vec![
        KeyCode::Char('1'), KeyCode::Char('j'), KeyCode::Char('4'), KeyCode::Char('2'),
    ];
    let (committed2, _) = simulate_typing(&mut engine, &keys_bu2);
    assert_eq!(committed2, "部");

    let keys_bu3 = vec![
        KeyCode::Char('1'), KeyCode::Char('j'), KeyCode::Char('4'), KeyCode::Char('3'),
    ];
    let (committed3, _) = simulate_typing(&mut engine, &keys_bu3);
    assert_eq!(committed3, "布");
}

#[test]
fn test_e2e_backspace_editing_and_correction() {
    let table = load_real_cin();
    let mut engine = KeyKeyEngine::new(table);

    // User intended to type "好" (ㄏ ㄠ ˇ = c l 3), but accidentally typed "ㄏ ㄣ" (c p)
    engine.handle_key(KeyCode::Char('c')); // ㄏ
    engine.handle_key(KeyCode::Char('p')); // ㄣ -> composition: ㄏㄣ
    assert_eq!(engine.cin_table.keys_to_symbols(&engine.syllable.to_cin_code()), "ㄏㄣ");

    // Press Backspace to remove ㄣ
    engine.handle_key(KeyCode::Backspace);
    assert_eq!(engine.cin_table.keys_to_symbols(&engine.syllable.to_cin_code()), "ㄏ");

    // Type correct rhyme ㄠ (l) and tone ˇ (3)
    engine.handle_key(KeyCode::Char('l'));
    let actions = engine.handle_key(KeyCode::Char('3'));

    // Verify candidate window opened with "好"
    assert!(actions.iter().any(|a| matches!(a, EngineAction::ShowCandidates { .. })));

    // Select candidate 1 ("好")
    let commit_actions = engine.handle_key(KeyCode::Char('1'));
    assert!(commit_actions.contains(&EngineAction::Commit("好".to_string())));
}

#[test]
fn test_e2e_escape_cancellation() {
    let table = load_real_cin();
    let mut engine = KeyKeyEngine::new(table);

    // Type partial syllable
    engine.handle_key(KeyCode::Char('1')); // ㄅ
    engine.handle_key(KeyCode::Char('j')); // ㄨ

    // Escape clears the buffer
    let actions = engine.handle_key(KeyCode::Escape);
    assert_eq!(
        actions,
        vec![
            EngineAction::UpdateComposition {
                display_text: String::new(),
                cursor_pos: 0,
            },
            EngineAction::HideCandidates,
        ]
    );
    assert!(engine.syllable.is_empty());
}

#[test]
fn test_e2e_english_mode_switching() {
    let table = load_real_cin();
    let mut engine = KeyKeyEngine::new(table);

    // Type in Chinese mode
    let keys = vec![
        KeyCode::Char('w'), KeyCode::Char('u'), KeyCode::Char('0'), KeyCode::Space, KeyCode::Char('1'),
    ];
    let (committed, _) = simulate_typing(&mut engine, &keys);
    assert_eq!(committed, "天");

    // Toggle to English mode
    assert_eq!(engine.toggle_mode(), InputMode::English);

    // English characters pass through (Ignored)
    for ch in "hello".chars() {
        assert_eq!(engine.handle_key(KeyCode::Char(ch)), vec![EngineAction::Ignored]);
    }

    // Toggle back to Chinese
    assert_eq!(engine.toggle_mode(), InputMode::Bopomofo);
    let keys2 = vec![
        KeyCode::Char('f'), KeyCode::Char('u'), KeyCode::Char('4'), KeyCode::Char('2'),
    ];
    let (committed2, _) = simulate_typing(&mut engine, &keys2);
    assert_eq!(committed2, "氣");
}
