use crate::bopomofo::SyllableBuffer;
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
}

impl KeyKeyEngine {
    pub fn new(cin_table: CinTable) -> Self {
        Self {
            mode: InputMode::Bopomofo,
            cin_table,
            syllable: SyllableBuffer::new(),
            candidate_list: None,
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
                        self.reset();
                        return vec![
                            EngineAction::HideCandidates,
                            EngineAction::UpdateComposition {
                                display_text: String::new(),
                                cursor_pos: 0,
                            },
                            EngineAction::Commit(selected),
                        ];
                    }
                    return vec![EngineAction::Consumed];
                }
                KeyCode::Space | KeyCode::Down | KeyCode::PageDown => {
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
                KeyCode::Up | KeyCode::PageUp => {
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
                _ => return vec![EngineAction::Consumed],
            }
        }

        // When candidate list is not active
        match key {
            KeyCode::Char(c) => {
                if self.syllable.feed_key(c).unwrap_or(false) {
                    if self.syllable.is_complete() {
                        // Look up candidates
                        let code = self.syllable.to_cin_code();
                        if let Some(candidates) = self.cin_table.lookup(&code) {
                            let cand_vec: Vec<String> = candidates.to_vec();
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
                        EngineAction::UpdateComposition {
                            display_text: String::new(),
                            cursor_pos: 0,
                        },
                        EngineAction::HideCandidates,
                        EngineAction::Commit(raw),
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
}
