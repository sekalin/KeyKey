use keykey_core::cin::CinTable;
use keykey_core::engine::{EngineAction, KeyCode, KeyKeyEngine};
use std::path::PathBuf;
use std::time::Instant;

#[test]
fn test_load_real_bpmf_cin() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("data");
    path.push("bpmf.cin");

    assert!(path.exists(), "bpmf.cin must exist at {:?}", path);

    let start = Instant::now();
    let table = CinTable::load_from_file(&path).expect("Failed to load bpmf.cin");
    let elapsed = start.elapsed();
    println!("Loaded bpmf.cin in {:?}, total char_defs keys: {}", elapsed, table.char_defs.len());
    if let Some((k, v)) = table.char_defs.iter().next() {
        println!("Sample key: {:?}, candidates: {:?}", k, v);
    }
    for (k, v) in table.char_defs.iter().take(10) {
        println!("Key {:?} -> {:?}", k, v);
    }

    assert_eq!(table.cname, "注音");

    // "1j4" -> ㄅㄨˋ
    let candidates = table.lookup("1j4").expect("Candidates for 1j4 must exist");
    println!("Candidates for 1j4 (ㄅㄨˋ): {:?}", &candidates[..candidates.len().min(10)]);
    assert!(candidates.contains(&"不".to_string()));

    // Test with Engine
    let mut engine = KeyKeyEngine::new(table);
    // Type 1, j, 4
    engine.handle_key(KeyCode::Char('1'));
    engine.handle_key(KeyCode::Char('j'));
    let actions = engine.handle_key(KeyCode::Char('4'));

    let mut found_candidate_window = false;
    for action in actions {
        if let EngineAction::ShowCandidates { candidates, .. } = action {
            found_candidate_window = true;
            println!("Engine candidate page 1: {:?}", candidates);
            assert!(candidates.iter().any(|(_, text)| text == "不" || text == "部"));
        }
    }
    assert!(found_candidate_window);

    // Select candidate '1'
    let commit_actions = engine.handle_key(KeyCode::Char('1'));
    assert!(commit_actions.iter().any(|a| matches!(a, EngineAction::Commit(_))));
}

#[test]
fn test_load_real_bpmf_ext_cin() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("data");
    path.push("bpmf-ext.cin");

    assert!(path.exists(), "bpmf-ext.cin must exist at {:?}", path);

    let start = Instant::now();
    let ext_table = CinTable::load_from_file(&path).expect("Failed to load bpmf-ext.cin");
    let elapsed = start.elapsed();
    println!("Loaded bpmf-ext.cin (98k lines) in {:?}, total keys: {}", elapsed, ext_table.char_defs.len());

    let total_entries: usize = ext_table.char_defs.values().map(|v| v.len()).sum();
    println!("Total phonetic syllable keys: {}, Total candidate entries: {}", ext_table.char_defs.len(), total_entries);

    assert!(ext_table.char_defs.len() > 1000);
    assert!(total_entries > 50000);
}
