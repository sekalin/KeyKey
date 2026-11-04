use std::collections::HashMap;

use crate::bopomofo::{classify_key, ComponentType, SyllableBuffer};
use crate::candidate::CandidateList;
use crate::cin::CinTable;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Bopomofo,
    English,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyCode {
    Char(char),
    Backspace,
    Enter,
    Escape,
    Space,
    Down,
    Up,
    PageDown,
    PageUp,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineAction {
    /// Commit text into the target application
    Commit(String),
    /// Update the composition/inline text
    UpdateComposition {
        display_text: String,
        cursor_pos: usize,
    },
    /// Show the candidate list UI
    ShowCandidates {
        candidates: Vec<(char, String)>,
        page: usize,
        total_pages: usize,
    },
    /// Hide the candidate list UI
    HideCandidates,
    /// The keystroke was consumed by the engine
    Consumed,
    /// The keystroke was not handled; pass to the application
    Ignored,
}

pub struct KeyKeyEngine {
    pub mode: InputMode,
    pub cin_table: CinTable,
    pub syllable: SyllableBuffer,
    pub candidate_list: Option<CandidateList>,
    pub user_freq: HashMap<String, Vec<String>>,
}

impl KeyKeyEngine {
    pub fn new(cin_table: CinTable) -> Self {
        Self {
            mode: InputMode::Bopomofo,
            cin_table,
            syllable: SyllableBuffer::new(),
            candidate_list: None,
            user_freq: HashMap::new(),
        }
    }

    pub fn toggle_mode(&mut self) -> InputMode {
        self.mode = match self.mode {
            InputMode::Bopomofo => InputMode::English,
            InputMode::English => InputMode::Bopomofo,
        };
        self.reset();
        self.mode
    }

    pub fn reset(&mut self) {
        self.syllable.clear();
        self.candidate_list = None;
    }

    /// Look up candidates for a Bopomofo syllable code, prioritizing user-frequently-used candidates (MRU)
    pub fn lookup_candidates(&self, code: &str) -> Option<Vec<String>> {
        let base_cands = self.cin_table.lookup(code)?;
        if let Some(user_ordered) = self.user_freq.get(code) {
            let mut result = Vec::with_capacity(base_cands.len());
            // 1. Add user-selected candidates in MRU order (if they belong to this syllable)
            for cand in user_ordered {
                if base_cands.contains(cand) && !result.contains(cand) {
                    result.push(cand.clone());
                }
            }
            // 2. Append the rest of standard candidates
            for cand in base_cands {
                if !result.contains(cand) {
                    result.push(cand.clone());
                }
            }
            Some(result)
        } else {
            Some(base_cands.to_vec())
        }
    }

    /// Record a candidate selection to promote it to highest frequency (MRU)
    pub fn record_candidate_selection(&mut self, code: &str, candidate: &str) {
        let list = self.user_freq.entry(code.to_string()).or_default();
        if let Some(pos) = list.iter().position(|c| c == candidate) {
            list.remove(pos);
        }
        list.insert(0, candidate.to_string());
    }

    /// Process a keystroke and return the appropriate action.
    pub fn handle_key(&mut self, key: KeyCode) -> Vec<EngineAction> {
        if self.mode == InputMode::English {
            return vec![EngineAction::Ignored];
        }

        // If candidate list is currently active
        if let Some(ref mut candidates) = self.candidate_list {
            match key {
                KeyCode::Char(c) if self.cin_table.selkey.contains(c) => {
                    if let Some(selected) = candidates.select_by_key(c).map(|s| s.to_string()) {
                        let code = self.syllable.to_cin_code();
                        self.record_candidate_selection(&code, &selected);
                        self.reset();
                        return vec![
                            EngineAction::Commit(selected),
                            EngineAction::HideCandidates,
                        ];
                    }
                    return vec![EngineAction::Consumed];
                }
                KeyCode::Space => {
                    // Space selects candidate 1 (default candidate)
                    if let Some(selected) = candidates.select_by_index(0).map(|s| s.to_string()) {
                        let code = self.syllable.to_cin_code();
                        self.record_candidate_selection(&code, &selected);
                        self.reset();
                        return vec![
                            EngineAction::Commit(selected),
                            EngineAction::HideCandidates,
                        ];
                    }
                    return vec![EngineAction::Consumed];
                }
                KeyCode::Down | KeyCode::PageDown | KeyCode::Right => {
                    if candidates.next_page() {
                        let items: Vec<(char, String)> = candidates
                            .current_page_items()
                            .into_iter()
                            .map(|(k, s)| (k, s.to_string()))
                            .collect();
                        let page = candidates.current_page();
                        let total_pages = candidates.total_pages();
                        return vec![EngineAction::ShowCandidates {
                            candidates: items,
                            page,
                            total_pages,
                        }];
                    }
                    return vec![EngineAction::Consumed];
                }
                KeyCode::Up | KeyCode::PageUp | KeyCode::Left => {
                    if candidates.prev_page() {
                        let items: Vec<(char, String)> = candidates
                            .current_page_items()
                            .into_iter()
                            .map(|(k, s)| (k, s.to_string()))
                            .collect();
                        let page = candidates.current_page();
                        let total_pages = candidates.total_pages();
                        return vec![EngineAction::ShowCandidates {
                            candidates: items,
                            page,
                            total_pages,
                        }];
                    }
                    return vec![EngineAction::Consumed];
                }
                KeyCode::Escape => {
                    self.candidate_list = None;
                    let display_text = self.cin_table.keys_to_symbols(&self.syllable.to_cin_code());
                    let cursor_pos = display_text.chars().count();
                    return vec![
                        EngineAction::HideCandidates,
                        EngineAction::UpdateComposition {
                            display_text,
                            cursor_pos,
                        },
                    ];
                }
                KeyCode::Backspace => {
                    self.candidate_list = None;
                    self.syllable.backspace();
                    let display_text = self.cin_table.keys_to_symbols(&self.syllable.to_cin_code());
                    let cursor_pos = display_text.chars().count();
                    return vec![
                        EngineAction::HideCandidates,
                        EngineAction::UpdateComposition {
                            display_text,
                            cursor_pos,
                        },
                    ];
                }
                KeyCode::Enter => {
                    let display_text = self.cin_table.keys_to_symbols(&self.syllable.to_cin_code());
                    self.reset();
                    return vec![
                        EngineAction::Commit(display_text),
                        EngineAction::HideCandidates,
                    ];
                }
                KeyCode::Char(c) => {
                    let comp = classify_key(c);
                    if matches!(
                        comp,
                        ComponentType::Consonant | ComponentType::Medial | ComponentType::Rhyme
                    ) {
                        // Continuous typing: auto-commit Candidate 1 and start next syllable!
                        let selected = candidates.select_by_index(0).map(|s| s.to_string()).unwrap_or_default();
                        let code = self.syllable.to_cin_code();
                        self.record_candidate_selection(&code, &selected);
                        self.reset();
                        let _ = self.syllable.feed_key(c);
                        let next_code = self.syllable.to_cin_code();
                        let display_text = self.cin_table.keys_to_symbols(&next_code);
                        let cursor_pos = display_text.chars().count();
                        return vec![
                            EngineAction::Commit(selected),
                            EngineAction::HideCandidates,
                            EngineAction::UpdateComposition {
                                display_text,
                                cursor_pos,
                            },
                        ];
                    }
                    return vec![EngineAction::Consumed];
                }
            }
        }

        // When candidate list is not active
        match key {
            KeyCode::Char(c) => {
                if self.syllable.feed_key(c).unwrap_or(false) {
                    if self.syllable.is_complete() {
                        // Look up candidates with MRU frequency reordering
                        let code = self.syllable.to_cin_code();
                        if let Some(cand_vec) = self.lookup_candidates(&code).filter(|v| !v.is_empty()) {
                            let cand_list = CandidateList::new(cand_vec, &self.cin_table.selkey);
                            let items: Vec<(char, String)> = cand_list
                                .current_page_items()
                                .into_iter()
                                .map(|(k, s)| (k, s.to_string()))
                                .collect();
                            let total_pages = cand_list.total_pages();
                            self.candidate_list = Some(cand_list);

                            let display_text = self.cin_table.keys_to_symbols(&code);
                            let cursor_pos = display_text.chars().count();

                            return vec![
                                EngineAction::UpdateComposition {
                                    display_text,
                                    cursor_pos,
                                },
                                EngineAction::ShowCandidates {
                                    candidates: items,
                                    page: 0,
                                    total_pages,
                                },
                            ];
                        } else {
                            // No candidate for this syllable
                            let display_text = self.cin_table.keys_to_symbols(&code);
                            let cursor_pos = display_text.chars().count();
                            return vec![EngineAction::UpdateComposition {
                                display_text,
                                cursor_pos,
                            }];
                        }
                    } else {
                        // Incomplete syllable
                        let code = self.syllable.to_cin_code();
                        let display_text = self.cin_table.keys_to_symbols(&code);
                        let cursor_pos = display_text.chars().count();
                        return vec![EngineAction::UpdateComposition {
                            display_text,
                            cursor_pos,
                        }];
                    }
                } else {
                    // Key not accepted by Bopomofo syllable buffer
                    // If current syllable was already complete (e.g. no candidates found for an invalid tone combination),
                    // typing a new syllable starter commits the raw symbols and begins the next syllable!
                    if self.syllable.is_complete() {
                        let comp = classify_key(c);
                        if matches!(
                            comp,
                            ComponentType::Consonant | ComponentType::Medial | ComponentType::Rhyme
                        ) {
                            let raw = self.cin_table.keys_to_symbols(&self.syllable.to_cin_code());
                            self.syllable.clear();
                            let _ = self.syllable.feed_key(c);
                            let code = self.syllable.to_cin_code();
                            let display_text = self.cin_table.keys_to_symbols(&code);
                            let cursor_pos = display_text.chars().count();
                            return vec![
                                EngineAction::Commit(raw),
                                EngineAction::UpdateComposition {
                                    display_text,
                                    cursor_pos,
                                },
                            ];
                        }
                    }
                    if self.syllable.is_empty() {
                        vec![EngineAction::Ignored]
                    } else {
                        vec![EngineAction::Consumed]
                    }
                }
            }
            KeyCode::Space => {
                if !self.syllable.is_empty() {
                    // Space acts as 1st tone!
                    return self.handle_key(KeyCode::Char(' '));
                } else {
                    vec![EngineAction::Ignored]
                }
            }
            KeyCode::Backspace => {
                if self.syllable.backspace() {
                    if self.syllable.is_empty() {
                        vec![
                            EngineAction::UpdateComposition {
                                display_text: String::new(),
                                cursor_pos: 0,
                            },
                            EngineAction::HideCandidates,
                        ]
                    } else {
                        let code = self.syllable.to_cin_code();
                        let display_text = self.cin_table.keys_to_symbols(&code);
                        let cursor_pos = display_text.chars().count();
                        vec![EngineAction::UpdateComposition {
                            display_text,
                            cursor_pos,
                        }]
                    }
                } else {
                    vec![EngineAction::Ignored]
                }
            }
            KeyCode::Escape => {
                if !self.syllable.is_empty() {
                    self.reset();
                    vec![
                        EngineAction::UpdateComposition {
                            display_text: String::new(),
                            cursor_pos: 0,
                        },
                        EngineAction::HideCandidates,
                    ]
                } else {
                    vec![EngineAction::Ignored]
                }
            }
            KeyCode::Enter => {
                if !self.syllable.is_empty() {
                    // Commit raw symbols or clear
                    let code = self.syllable.to_cin_code();
                    let raw = self.cin_table.keys_to_symbols(&code);
                    self.reset();
                    vec![
                        EngineAction::Commit(raw),
                        EngineAction::HideCandidates,
                    ]
                } else {
                    vec![EngineAction::Ignored]
                }
            }
            _ => vec![EngineAction::Ignored],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CIN: &str = r#"
%ename Phonetic
%cname 注音
%selkey 123456789
%keyname begin
1 ㄅ
j ㄨ
4 ˋ
z ㄈ
%keyname end
%chardef begin
1j4 不
1j4 布
1j4 部
1j 逋
%chardef end
"#;

    #[test]
    fn test_engine_flow() {
        let table = CinTable::load_from_reader(TEST_CIN.as_bytes()).unwrap();
        let mut engine = KeyKeyEngine::new(table);

        // Type '1' (ㄅ)
        let actions = engine.handle_key(KeyCode::Char('1'));
        assert_eq!(
            actions,
            vec![EngineAction::UpdateComposition {
                display_text: "ㄅ".to_string(),
                cursor_pos: 1,
            }]
        );

        // Type 'j' (ㄨ)
        let actions = engine.handle_key(KeyCode::Char('j'));
        assert_eq!(
            actions,
            vec![EngineAction::UpdateComposition {
                display_text: "ㄅㄨ".to_string(),
                cursor_pos: 2,
            }]
        );

        // Type '4' (ˋ - 4th tone) -> Completes syllable, triggers candidates
        let actions = engine.handle_key(KeyCode::Char('4'));
        assert!(actions.contains(&EngineAction::UpdateComposition {
            display_text: "ㄅㄨˋ".to_string(),
            cursor_pos: 3,
        }));

        // Select candidate '1' ('不')
        let actions = engine.handle_key(KeyCode::Char('1'));
        assert!(actions.contains(&EngineAction::Commit("不".to_string())));
    }

    #[test]
    fn test_engine_space_tone1() {
        let table = CinTable::load_from_reader(TEST_CIN.as_bytes()).unwrap();
        let mut engine = KeyKeyEngine::new(table);

        engine.handle_key(KeyCode::Char('1')); // ㄅ
        engine.handle_key(KeyCode::Char('j')); // ㄨ
        let actions = engine.handle_key(KeyCode::Space); // 1st tone!

        // Should find "逋" and open candidate list
        assert!(actions.iter().any(|a| matches!(a, EngineAction::ShowCandidates { .. })));

        // Select candidate '1' ('逋')
        let actions = engine.handle_key(KeyCode::Char('1'));
        assert!(actions.contains(&EngineAction::Commit("逋".to_string())));
    }

    #[test]
    fn test_engine_mode_toggle() {
        let table = CinTable::load_from_reader(TEST_CIN.as_bytes()).unwrap();
        let mut engine = KeyKeyEngine::new(table);

        assert_eq!(engine.mode, InputMode::Bopomofo);
        assert_eq!(engine.toggle_mode(), InputMode::English);

        // In English mode, keys are ignored
        assert_eq!(engine.handle_key(KeyCode::Char('a')), vec![EngineAction::Ignored]);

        assert_eq!(engine.toggle_mode(), InputMode::Bopomofo);
    }

    #[test]
    fn test_engine_editing_keys() {
        let table = CinTable::load_from_reader(TEST_CIN.as_bytes()).unwrap();
        let mut engine = KeyKeyEngine::new(table);

        // Backspace, Enter, Space, Escape on empty buffer
        assert_eq!(engine.handle_key(KeyCode::Backspace), vec![EngineAction::Ignored]);
        assert_eq!(engine.handle_key(KeyCode::Enter), vec![EngineAction::Ignored]);
        assert_eq!(engine.handle_key(KeyCode::Space), vec![EngineAction::Ignored]);
        assert_eq!(engine.handle_key(KeyCode::Escape), vec![EngineAction::Ignored]);
        assert_eq!(engine.handle_key(KeyCode::Down), vec![EngineAction::Ignored]);

        // Non-bopomofo key on empty buffer
        assert_eq!(engine.handle_key(KeyCode::Char('!')), vec![EngineAction::Ignored]);

        // Feed '1' (ㄅ), then backspace to empty
        engine.handle_key(KeyCode::Char('1'));
        let actions = engine.handle_key(KeyCode::Backspace);
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

        // Feed '1' and 'j', then backspace partial
        engine.handle_key(KeyCode::Char('1'));
        engine.handle_key(KeyCode::Char('j'));
        let actions = engine.handle_key(KeyCode::Backspace);
        assert_eq!(
            actions,
            vec![EngineAction::UpdateComposition {
                display_text: "ㄅ".to_string(),
                cursor_pos: 1,
            }]
        );

        // Feed non-bopomofo char while composing -> consumed
        assert_eq!(engine.handle_key(KeyCode::Char('!')), vec![EngineAction::Consumed]);

        // Escape cancels composition
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

        // Enter on non-empty buffer commits raw phonetic symbols
        engine.handle_key(KeyCode::Char('1'));
        engine.handle_key(KeyCode::Char('j'));
        let actions = engine.handle_key(KeyCode::Enter);
        assert!(actions.contains(&EngineAction::Commit("ㄅㄨ".to_string())));
    }

    #[test]
    fn test_engine_candidate_navigation_and_cancel() {
        let table = CinTable::load_from_reader(TEST_CIN.as_bytes()).unwrap();
        let mut engine = KeyKeyEngine::new(table);

        engine.handle_key(KeyCode::Char('1'));
        engine.handle_key(KeyCode::Char('j'));
        engine.handle_key(KeyCode::Char('4')); // ㄅㄨˋ -> candidate list open

        // Navigation when only 1 page
        assert_eq!(engine.handle_key(KeyCode::Down), vec![EngineAction::Consumed]);
        assert_eq!(engine.handle_key(KeyCode::Up), vec![EngineAction::Consumed]);

        // Other non-bopomofo key when candidate list open
        assert_eq!(engine.handle_key(KeyCode::Char('!')), vec![EngineAction::Consumed]);

        // Escape from candidate list hides candidates and keeps composition
        let actions = engine.handle_key(KeyCode::Escape);
        assert_eq!(
            actions,
            vec![
                EngineAction::HideCandidates,
                EngineAction::UpdateComposition {
                    display_text: "ㄅㄨˋ".to_string(),
                    cursor_pos: 3,
                },
            ]
        );
    }

    #[test]
    fn test_engine_candidate_multipage() {
        // Create CIN with 12 candidates for code 1j4
        let mut table = CinTable::new();
        table.selkey = "123456789".to_string();
        table.key_names.insert('1', "ㄅ".to_string());
        table.key_names.insert('j', "ㄨ".to_string());
        table.key_names.insert('4', "ˋ".to_string());
        let cands: Vec<String> = (1..=12).map(|i| format!("字{}", i)).collect();
        table.char_defs.insert("1j4".to_string(), cands);

        let mut engine = KeyKeyEngine::new(table);
        engine.handle_key(KeyCode::Char('1'));
        engine.handle_key(KeyCode::Char('j'));
        engine.handle_key(KeyCode::Char('4')); // candidate list page 0 (9 items)

        // Down / PageDown -> flips to page 1
        let actions = engine.handle_key(KeyCode::Down);
        assert!(actions.iter().any(|a| matches!(a, EngineAction::ShowCandidates { page: 1, .. })));

        // Up / PageUp -> flips back to page 0
        let actions = engine.handle_key(KeyCode::Up);
        assert!(actions.iter().any(|a| matches!(a, EngineAction::ShowCandidates { page: 0, .. })));

        // PageDown
        let actions = engine.handle_key(KeyCode::PageDown);
        assert!(actions.iter().any(|a| matches!(a, EngineAction::ShowCandidates { page: 1, .. })));

        // PageUp
        let actions = engine.handle_key(KeyCode::PageUp);
        assert!(actions.iter().any(|a| matches!(a, EngineAction::ShowCandidates { page: 0, .. })));
    }

    #[test]
    fn test_engine_no_candidates_syllable() {
        let table = CinTable::load_from_reader(TEST_CIN.as_bytes()).unwrap();
        let mut engine = KeyKeyEngine::new(table);

        // Type '1' (ㄅ) + '4' (ˋ) -> ㄅˋ (not in TEST_CIN)
        engine.handle_key(KeyCode::Char('1'));
        let actions = engine.handle_key(KeyCode::Char('4'));

        // Should update composition without opening candidates
        assert_eq!(
            actions,
            vec![EngineAction::UpdateComposition {
                display_text: "ㄅˋ".to_string(),
                cursor_pos: 2,
            }]
        );

        // Typing another consonant commits the raw unmapped syllable and starts the new one
        let next_actions = engine.handle_key(KeyCode::Char('1'));
        assert_eq!(
            next_actions,
            vec![
                EngineAction::Commit("ㄅˋ".to_string()),
                EngineAction::UpdateComposition {
                    display_text: "ㄅ".to_string(),
                    cursor_pos: 1,
                },
            ]
        );
    }

    #[test]
    fn test_engine_selkey_out_of_range() {
        let table = CinTable::load_from_reader(TEST_CIN.as_bytes()).unwrap();
        let mut engine = KeyKeyEngine::new(table);

        // 1j4 has 3 candidates ('1': 不, '2': 布, '3': 部)
        engine.handle_key(KeyCode::Char('1'));
        engine.handle_key(KeyCode::Char('j'));
        engine.handle_key(KeyCode::Char('4'));

        // Press '9' which is in selkey "123456789" but index 8 doesn't exist
        let actions = engine.handle_key(KeyCode::Char('9'));
        assert_eq!(actions, vec![EngineAction::Consumed]);
    }

    #[test]
    fn test_engine_candidate_backspace_and_enter() {
        let table = CinTable::load_from_reader(TEST_CIN.as_bytes()).unwrap();
        let mut engine = KeyKeyEngine::new(table);

        // 1. Backspace when candidates are open
        engine.handle_key(KeyCode::Char('1'));
        engine.handle_key(KeyCode::Char('j'));
        engine.handle_key(KeyCode::Char('4')); // candidate window open
        assert!(engine.candidate_list.is_some());

        let bs_actions = engine.handle_key(KeyCode::Backspace);
        assert!(engine.candidate_list.is_none());
        assert_eq!(
            bs_actions,
            vec![
                EngineAction::HideCandidates,
                EngineAction::UpdateComposition {
                    display_text: "ㄅㄨ".to_string(), // tone 'ˋ' removed
                    cursor_pos: 2,
                },
            ]
        );

        // 2. Enter when candidates are open commits raw phonetic symbols
        engine.handle_key(KeyCode::Char('4')); // reopen candidate window
        assert!(engine.candidate_list.is_some());

        let enter_actions = engine.handle_key(KeyCode::Enter);
        assert!(engine.candidate_list.is_none());
        assert!(engine.syllable.is_empty());
        assert_eq!(
            enter_actions,
            vec![
                EngineAction::Commit("ㄅㄨˋ".to_string()),
                EngineAction::HideCandidates,
            ]
        );
    }

    #[test]
    fn test_engine_candidate_mru_rearrangement() {
        let table = CinTable::load_from_reader(TEST_CIN.as_bytes()).unwrap();
        let mut engine = KeyKeyEngine::new(table);

        // 1j4 has 3 candidates ('1': 不, '2': 布, '3': 部)
        // First lookup: candidate 1 is '不'
        engine.handle_key(KeyCode::Char('1'));
        engine.handle_key(KeyCode::Char('j'));
        let actions1 = engine.handle_key(KeyCode::Char('4'));
        if let Some(EngineAction::ShowCandidates { candidates, .. }) = actions1.iter().find(|a| matches!(a, EngineAction::ShowCandidates { .. })) {
            assert_eq!(candidates[0].1, "不");
            assert_eq!(candidates[1].1, "布");
            assert_eq!(candidates[2].1, "部");
        } else {
            panic!("Expected ShowCandidates");
        }

        // Manually select candidate '2' ('布')
        let select_actions = engine.handle_key(KeyCode::Char('2'));
        assert_eq!(
            select_actions,
            vec![
                EngineAction::Commit("布".to_string()),
                EngineAction::HideCandidates,
            ]
        );

        // Second lookup for same syllable 1j4 (ㄅㄨˋ):
        // '布' MUST now be rearranged to candidate #1!
        engine.handle_key(KeyCode::Char('1'));
        engine.handle_key(KeyCode::Char('j'));
        let actions2 = engine.handle_key(KeyCode::Char('4'));
        if let Some(EngineAction::ShowCandidates { candidates, .. }) = actions2.iter().find(|a| matches!(a, EngineAction::ShowCandidates { .. })) {
            assert_eq!(candidates[0].1, "布", "布 should be promoted to #1 candidate (MRU)");
            assert_eq!(candidates[1].1, "不");
            assert_eq!(candidates[2].1, "部");
        } else {
            panic!("Expected ShowCandidates");
        }
    }

    #[test]
    fn test_engine_continuous_typing_on_candidates() {
        let table = CinTable::load_from_reader(TEST_CIN.as_bytes()).unwrap();
        let mut engine = KeyKeyEngine::new(table);

        // Type '1' 'j' '4' -> candidate window opens
        engine.handle_key(KeyCode::Char('1'));
        engine.handle_key(KeyCode::Char('j'));
        engine.handle_key(KeyCode::Char('4'));
        assert!(engine.candidate_list.is_some());

        // Now without pressing 1-9 or space, immediately type consonant 'z' (ㄈ) for next syllable
        let cont_actions = engine.handle_key(KeyCode::Char('z'));
        // Candidate 1 ('不') should be committed, candidate window closed, and new composition started with 'ㄈ'
        assert_eq!(
            cont_actions,
            vec![
                EngineAction::Commit("不".to_string()),
                EngineAction::HideCandidates,
                EngineAction::UpdateComposition {
                    display_text: "ㄈ".to_string(),
                    cursor_pos: 1,
                },
            ]
        );
        assert!(engine.candidate_list.is_none());
        assert_eq!(engine.syllable.to_cin_code(), "z");
    }

    #[test]
    fn test_engine_space_selects_first_candidate() {
        let table = CinTable::load_from_reader(TEST_CIN.as_bytes()).unwrap();
        let mut engine = KeyKeyEngine::new(table);

        engine.handle_key(KeyCode::Char('1'));
        engine.handle_key(KeyCode::Char('j'));
        engine.handle_key(KeyCode::Char('4')); // Candidates: 不, 布, 部
        assert!(engine.candidate_list.is_some());

        // Pressing Space should select the default first candidate ("不")
        let space_actions = engine.handle_key(KeyCode::Space);
        assert_eq!(
            space_actions,
            vec![
                EngineAction::Commit("不".to_string()),
                EngineAction::HideCandidates,
            ]
        );
        assert!(engine.candidate_list.is_none());
        assert!(engine.syllable.is_empty());
    }

    #[test]
    fn test_engine_arrow_left_right_page_navigation() {
        let mut table = CinTable::new();
        table.selkey = "123456789".to_string();
        table.key_names.insert('1', "ㄅ".to_string());
        table.key_names.insert('j', "ㄨ".to_string());
        table.key_names.insert('4', "ˋ".to_string());
        let cands: Vec<String> = (1..=25).map(|i| format!("字{}", i)).collect();
        table.char_defs.insert("1j4".to_string(), cands);
        let mut engine = KeyKeyEngine::new(table);

        engine.handle_key(KeyCode::Char('1'));
        engine.handle_key(KeyCode::Char('j'));
        engine.handle_key(KeyCode::Char('4'));
        assert!(engine.candidate_list.is_some());
        assert_eq!(engine.candidate_list.as_ref().unwrap().current_page(), 0);

        // Right arrow moves to next page
        let nav_right = engine.handle_key(KeyCode::Right);
        assert!(matches!(nav_right[0], EngineAction::ShowCandidates { page: 1, .. }));
        assert_eq!(engine.candidate_list.as_ref().unwrap().current_page(), 1);

        // Left arrow moves back to previous page
        let nav_left = engine.handle_key(KeyCode::Left);
        assert!(matches!(nav_left[0], EngineAction::ShowCandidates { page: 0, .. }));
        assert_eq!(engine.candidate_list.as_ref().unwrap().current_page(), 0);

        // Left arrow at page 0 is safely consumed
        let consumed = engine.handle_key(KeyCode::Left);
        assert_eq!(consumed, vec![EngineAction::Consumed]);
    }
}
