//! End-to-End (E2E) Test Suite for KeyKey Windows TSF (Text Services Framework)
//! Tests real-world input scenarios with Windows TSF ThreadMgr, Context,
//! Candidate UI Elements, EditSession locks, and mode compartments.

use keykey_core::engine::InputMode;
use keykey_tsf::KeyKeyTip;

use windows::core::{Interface, Result};
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::TextServices::*;

/// Harness that initializes a live Windows TSF Thread Manager and Document Context
#[allow(dead_code)]
struct TsfTestHarness {
    thread_mgr: ITfThreadMgr,
    client_id: u32,
    context: ITfContext,
    edit_cookie: u32,
    tip_processor: ITfTextInputProcessorEx,
    key_sink: ITfKeyEventSink,
    tip_raw: KeyKeyTip,
}

impl TsfTestHarness {
    fn new() -> Result<Self> {
        unsafe {
            let _ = CoInitialize(None);
            let thread_mgr: ITfThreadMgr =
                CoCreateInstance(&CLSID_TF_ThreadMgr, None, CLSCTX_INPROC_SERVER)?;
            let client_id = thread_mgr.Activate()?;

            let doc_mgr = thread_mgr.CreateDocumentMgr()?;
            let mut context_opt: Option<ITfContext> = None;
            let mut edit_cookie = 0u32;
            doc_mgr.CreateContext(
                client_id,
                0,
                None,
                &mut context_opt,
                &mut edit_cookie,
            )?;
            let context = context_opt.expect("Failed to create ITfContext");
            doc_mgr.Push(&context)?;
            let _ = thread_mgr.SetFocus(&doc_mgr);

            let tip = KeyKeyTip::new();
            let tip_raw = KeyKeyTip {
                client_id: tip.client_id.clone(),
                thread_mgr: tip.thread_mgr.clone(),
                engine: tip.engine.clone(),
                active_composition: tip.active_composition.clone(),
                candidate_ui: tip.candidate_ui.clone(),
                shift_alone: std::cell::Cell::new(false),
                compartment_cookie: std::cell::Cell::new(None),
                has_focus: std::cell::Cell::new(true),
            };

            let tip_processor: ITfTextInputProcessorEx = tip.into();
            tip_processor.Activate(Some(&thread_mgr), client_id)?;

            let key_sink: ITfKeyEventSink = tip_processor.cast()?;

            Ok(Self {
                thread_mgr,
                client_id,
                context,
                edit_cookie,
                tip_processor,
                key_sink,
                tip_raw,
            })
        }
    }
}

impl Drop for TsfTestHarness {
    fn drop(&mut self) {
        unsafe {
            let _ = self.tip_processor.Deactivate();
            let _ = self.thread_mgr.Deactivate();
        }
    }
}

#[test]
fn test_e2e_tsf_typing_and_commit() {
    let harness = match TsfTestHarness::new() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Skipping live TSF test (COM/TSF not available): {:?}", e);
            return;
        }
    };

    unsafe {
        // Test typing "不" -> ㄅ(1) ㄨ(j) ˋ(4) then candidate 1
        // 1. Initial ㄅ ('1')
        let test1 = harness.key_sink.OnTestKeyDown(
            Some(&harness.context),
            WPARAM(b'1' as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(test1, BOOL(1)); // Intercepted

        let down1 = harness.key_sink.OnKeyDown(
            Some(&harness.context),
            WPARAM(b'1' as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(down1, BOOL(1));

        // 2. Medial ㄨ ('j')
        let test2 = harness.key_sink.OnTestKeyDown(
            Some(&harness.context),
            WPARAM(b'J' as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(test2, BOOL(1));

        let down2 = harness.key_sink.OnKeyDown(
            Some(&harness.context),
            WPARAM(b'J' as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(down2, BOOL(1));

        // 3. Tone ˋ ('4') -> Opens Candidate UI
        let test3 = harness.key_sink.OnTestKeyDown(
            Some(&harness.context),
            WPARAM(b'4' as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(test3, BOOL(1));

        let down3 = harness.key_sink.OnKeyDown(
            Some(&harness.context),
            WPARAM(b'4' as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(down3, BOOL(1));

        // 4. Select Candidate #1 ("不")
        let test4 = harness.key_sink.OnTestKeyDown(
            Some(&harness.context),
            WPARAM(b'1' as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(test4, BOOL(1));

        let down4 = harness.key_sink.OnKeyDown(
            Some(&harness.context),
            WPARAM(b'1' as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(down4, BOOL(1)); // Committed!
    }
}

#[test]
fn test_e2e_tsf_backspace_handling_in_app() {
    let harness = match TsfTestHarness::new() {
        Ok(h) => h,
        Err(_) => return,
    };

    unsafe {
        // --- Scenario A: Composition buffer is EMPTY ---
        // Pressing Backspace must NOT be swallowed; it must return BOOL(0)
        // so the Windows application can delete previous characters naturally.
        let test_empty = harness.key_sink.OnTestKeyDown(
            Some(&harness.context),
            WPARAM(VK_BACK.0 as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(test_empty, BOOL(0), "Empty buffer must pass Backspace to application");

        let down_empty = harness.key_sink.OnKeyDown(
            Some(&harness.context),
            WPARAM(VK_BACK.0 as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(down_empty, BOOL(0), "Empty buffer must not consume Backspace");

        // --- Scenario B: Composing a syllable ---
        // Type ㄏ ('c') and ㄠ ('l')
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'C' as usize), LPARAM(0));
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'L' as usize), LPARAM(0));

        // Now buffer has ㄏㄠ. Backspace MUST be consumed by KeyKey to edit the syllable!
        let test_composing = harness.key_sink.OnTestKeyDown(
            Some(&harness.context),
            WPARAM(VK_BACK.0 as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(test_composing, BOOL(1), "Composing buffer must intercept Backspace");

        let down_bs1 = harness.key_sink.OnKeyDown(
            Some(&harness.context),
            WPARAM(VK_BACK.0 as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(down_bs1, BOOL(1), "Consumed Backspace to edit syllable (ㄠ removed)");

        // Backspace once more to remove ㄏ
        let down_bs2 = harness.key_sink.OnKeyDown(
            Some(&harness.context),
            WPARAM(VK_BACK.0 as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(down_bs2, BOOL(1), "Consumed Backspace to edit syllable (ㄏ removed)");

        // Now buffer is empty again -> Next Backspace MUST pass through to app!
        let test_after_cleared = harness.key_sink.OnTestKeyDown(
            Some(&harness.context),
            WPARAM(VK_BACK.0 as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(test_after_cleared, BOOL(0), "Cleared buffer must immediately pass Backspace through");
    }
}

#[test]
fn test_e2e_tsf_candidate_ui_mouse_click_finalize() {
    let harness = match TsfTestHarness::new() {
        Ok(h) => h,
        Err(_) => return,
    };

    unsafe {
        // Type ㄅ(1) ㄨ(j) ˋ(4) -> triggers candidate list
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'1' as usize), LPARAM(0));
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'J' as usize), LPARAM(0));
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'4' as usize), LPARAM(0));

        // Candidate UI window is open
        assert!(harness.tip_raw.engine.borrow().candidate_list.is_some());

        // Simulate user clicking Candidate #2 ("部") with mouse in Windows 11 Candidate Window
        let cand_element = harness.tip_raw.candidate_ui.ui_element.clone();
        let behavior: ITfCandidateListUIElementBehavior = cand_element.cast().unwrap();

        behavior.SetSelection(1).unwrap(); // Select 2nd candidate (0-indexed = 1)
        behavior.Finalize().unwrap();      // Win11 fires Finalize() on mouse click

        // Candidate UI closes and engine resets
        assert!(harness.tip_raw.engine.borrow().candidate_list.is_none());
        assert!(harness.tip_raw.engine.borrow().syllable.is_empty());
    }
}

#[test]
fn test_e2e_tsf_shift_mode_toggle_and_tray_sync() {
    let harness = match TsfTestHarness::new() {
        Ok(h) => h,
        Err(_) => return,
    };

    unsafe {
        // 1. Initially in Bopomofo mode
        assert_eq!(harness.tip_raw.engine.borrow().mode, InputMode::Bopomofo);

        // 2. Press and release Shift alone
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(VK_SHIFT.0 as usize), LPARAM(0));
        let _ = harness.key_sink.OnKeyUp(Some(&harness.context), WPARAM(VK_SHIFT.0 as usize), LPARAM(0));

        // Switched to English mode
        assert_eq!(harness.tip_raw.engine.borrow().mode, InputMode::English);

        // In English mode, keys pass through directly to Windows app
        let test_eng_char = harness.key_sink.OnTestKeyDown(
            Some(&harness.context),
            WPARAM(b'A' as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(test_eng_char, BOOL(0), "English mode must pass letters directly to app");

        let test_eng_num = harness.key_sink.OnTestKeyDown(
            Some(&harness.context),
            WPARAM(b'1' as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(test_eng_num, BOOL(0), "English mode must pass numbers directly to app");

        // 3. User clicks taskbar system tray "中" icon -> Triggers OnChange on GUID_COMPARTMENT_KEYBOARD_OPENCLOSE
        let comp_sink: ITfCompartmentEventSink = harness.tip_processor.cast().unwrap();
        comp_sink.OnChange(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE).unwrap();

        // 4. Press Shift alone again to toggle back to Chinese
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(VK_LSHIFT.0 as usize), LPARAM(0));
        let _ = harness.key_sink.OnKeyUp(Some(&harness.context), WPARAM(VK_LSHIFT.0 as usize), LPARAM(0));
        assert_eq!(harness.tip_raw.engine.borrow().mode, InputMode::Bopomofo);
    }
}

#[test]
fn test_e2e_tsf_fullwidth_punctuation_and_uppercase() {
    let harness = match TsfTestHarness::new() {
        Ok(h) => h,
        Err(_) => return,
    };

    unsafe {
        // 1. Direct punctuation: VK_OEM_5 ('\') -> '、' (全形頓號)
        let test_pause = harness.key_sink.OnTestKeyDown(
            Some(&harness.context),
            WPARAM(VK_OEM_5.0 as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(test_pause, BOOL(1));

        let down_pause = harness.key_sink.OnKeyDown(
            Some(&harness.context),
            WPARAM(VK_OEM_5.0 as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(down_pause, BOOL(1));

        // 2. Shifted uppercase English letter in Chinese mode:
        // map_virtual_key with is_shift = true for 'A' -> CommitDirect("A")
        let stroke_a = harness.tip_raw.map_virtual_key(WPARAM(b'A' as usize), true);
        assert_eq!(
            stroke_a,
            Some(keykey_tsf::KeyStroke::CommitDirect("A".to_string()))
        );

        // 3. Taiwanese punctuation keys:
        // Shift + , -> "，"
        let stroke_comma = harness.tip_raw.map_virtual_key(WPARAM(VK_OEM_COMMA.0 as usize), true);
        assert_eq!(
            stroke_comma,
            Some(keykey_tsf::KeyStroke::CommitDirect("，".to_string()))
        );

        // Shift + . -> "。"
        let stroke_period = harness.tip_raw.map_virtual_key(WPARAM(VK_OEM_PERIOD.0 as usize), true);
        assert_eq!(
            stroke_period,
            Some(keykey_tsf::KeyStroke::CommitDirect("。".to_string()))
        );

        // Shift + / -> "？"
        let stroke_question = harness.tip_raw.map_virtual_key(WPARAM(VK_OEM_2.0 as usize), true);
        assert_eq!(
            stroke_question,
            Some(keykey_tsf::KeyStroke::CommitDirect("？".to_string()))
        );

        // Shift + ; -> "："
        let stroke_colon = harness.tip_raw.map_virtual_key(WPARAM(VK_OEM_1.0 as usize), true);
        assert_eq!(
            stroke_colon,
            Some(keykey_tsf::KeyStroke::CommitDirect("：".to_string()))
        );

        // Shift + 1 -> "！"
        let stroke_excl = harness.tip_raw.map_virtual_key(WPARAM(b'1' as usize), true);
        assert_eq!(
            stroke_excl,
            Some(keykey_tsf::KeyStroke::CommitDirect("！".to_string()))
        );

        // Shift + [ -> "「"
        let stroke_bracket_l = harness.tip_raw.map_virtual_key(WPARAM(VK_OEM_4.0 as usize), true);
        assert_eq!(
            stroke_bracket_l,
            Some(keykey_tsf::KeyStroke::CommitDirect("「".to_string()))
        );

        // Shift + ] -> "」"
        let stroke_bracket_r = harness.tip_raw.map_virtual_key(WPARAM(VK_OEM_6.0 as usize), true);
        assert_eq!(
            stroke_bracket_r,
            Some(keykey_tsf::KeyStroke::CommitDirect("」".to_string()))
        );
    }
}

#[test]
fn test_e2e_tsf_escape_and_cancel() {
    let harness = match TsfTestHarness::new() {
        Ok(h) => h,
        Err(_) => return,
    };

    unsafe {
        // Type ㄅ(1) ㄨ(j)
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'1' as usize), LPARAM(0));
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'J' as usize), LPARAM(0));
        assert!(!harness.tip_raw.engine.borrow().syllable.is_empty());

        // Press Escape
        let test_esc = harness.key_sink.OnTestKeyDown(
            Some(&harness.context),
            WPARAM(VK_ESCAPE.0 as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(test_esc, BOOL(1));

        let down_esc = harness.key_sink.OnKeyDown(
            Some(&harness.context),
            WPARAM(VK_ESCAPE.0 as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(down_esc, BOOL(1));

        // Syllable buffer cancelled and cleared
        assert!(harness.tip_raw.engine.borrow().syllable.is_empty());
    }
}

#[test]
fn test_e2e_anti_hang_rapid_keystroke_stress() {
    let harness = match TsfTestHarness::new() {
        Ok(h) => h,
        Err(_) => return,
    };

    unsafe {
        let keys_to_stress = [
            WPARAM(b'1' as usize),
            WPARAM(b'J' as usize),
            WPARAM(b'4' as usize),
            WPARAM(VK_SPACE.0 as usize),
            WPARAM(VK_BACK.0 as usize),
            WPARAM(VK_ESCAPE.0 as usize),
            WPARAM(b'A' as usize),
            WPARAM(VK_OEM_COMMA.0 as usize),
            WPARAM(0xFFFF), // Invalid virtual key
            WPARAM(0),      // Null virtual key
        ];

        let start = std::time::Instant::now();
        const ITERATIONS: usize = 10_000;

        for i in 0..ITERATIONS {
            let vk = keys_to_stress[i % keys_to_stress.len()];
            // Both OnTestKeyDown and OnKeyDown must complete without stall
            let _ = harness.key_sink.OnTestKeyDown(Some(&harness.context), vk, LPARAM(0));
            let _ = harness.key_sink.OnKeyDown(Some(&harness.context), vk, LPARAM(0));
            if i % 100 == 0 {
                let _ = harness.key_sink.OnKeyUp(Some(&harness.context), WPARAM(VK_SHIFT.0 as usize), LPARAM(0));
            }
        }

        let elapsed = start.elapsed();
        // 10,000 keystrokes with live Win32 GDI window operations must complete within 5.0s in debug mode
        assert!(
            elapsed.as_secs_f64() < 5.0,
            "Anti-hang stress test took too long: {:?}",
            elapsed
        );

        // Verify IME remains 100% operational after intense stress
        let test_post = harness.key_sink.OnTestKeyDown(
            Some(&harness.context),
            WPARAM(b'1' as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(test_post, BOOL(1));
    }
}

#[test]
fn test_e2e_anti_hang_chaos_input_and_underflow() {
    let harness = match TsfTestHarness::new() {
        Ok(h) => h,
        Err(_) => return,
    };

    unsafe {
        // 1. Spam 1,000 Backspaces on completely empty buffer - must pass through to app and never hang
        for _ in 0..1000 {
            let res = harness.key_sink.OnKeyDown(
                Some(&harness.context),
                WPARAM(VK_BACK.0 as usize),
                LPARAM(0),
            ).unwrap();
            assert_eq!(res, BOOL(0)); // Pass-through to host app
        }

        // 2. Spam 1,000 Escapes on empty buffer - must pass through and never hang
        for _ in 0..1000 {
            let res = harness.key_sink.OnKeyDown(
                Some(&harness.context),
                WPARAM(VK_ESCAPE.0 as usize),
                LPARAM(0),
            ).unwrap();
            assert_eq!(res, BOOL(0));
        }

        // 3. Repeated rapid mode switches while sending keystrokes
        for _ in 0..500 {
            let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(VK_SHIFT.0 as usize), LPARAM(0));
            let _ = harness.key_sink.OnKeyUp(Some(&harness.context), WPARAM(VK_SHIFT.0 as usize), LPARAM(0));
            let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'1' as usize), LPARAM(0));
        }

        // IME must still be alive and responsive
        assert!(harness.tip_processor.Deactivate().is_ok());
    }
}

#[test]
fn test_e2e_tsf_candidate_backspace_and_enter_handling() {
    let harness = match TsfTestHarness::new() {
        Ok(h) => h,
        Err(_) => return,
    };

    unsafe {
        // 1. Type ㄅ(1) ㄨ(j) ˋ(4) -> Opens candidate list for 1j4
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'1' as usize), LPARAM(0));
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'J' as usize), LPARAM(0));
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'4' as usize), LPARAM(0));

        assert!(harness.tip_raw.engine.borrow().candidate_list.is_some());
        assert!(harness.tip_raw.candidate_ui.is_visible());

        // 2. While candidate list is open, Backspace must be intercepted (BOOL 1)
        let test_bs = harness.key_sink.OnTestKeyDown(
            Some(&harness.context),
            WPARAM(VK_BACK.0 as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(test_bs, BOOL(1), "Backspace must be intercepted while candidate window is open");

        let down_bs = harness.key_sink.OnKeyDown(
            Some(&harness.context),
            WPARAM(VK_BACK.0 as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(down_bs, BOOL(1));

        // Candidate window must be closed, and tone removed (leaving ㄅㄨ)
        assert!(harness.tip_raw.engine.borrow().candidate_list.is_none());
        assert_eq!(harness.tip_raw.engine.borrow().syllable.to_cin_code(), "1j");

        // 3. Re-enter tone '4' to reopen candidate window
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'4' as usize), LPARAM(0));
        assert!(harness.tip_raw.engine.borrow().candidate_list.is_some());

        // 4. While candidate list is open, Enter must be intercepted (BOOL 1) and commit raw symbols
        let test_enter = harness.key_sink.OnTestKeyDown(
            Some(&harness.context),
            WPARAM(VK_RETURN.0 as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(test_enter, BOOL(1), "Enter must be intercepted while candidate window is open");

        let down_enter = harness.key_sink.OnKeyDown(
            Some(&harness.context),
            WPARAM(VK_RETURN.0 as usize),
            LPARAM(0),
        ).unwrap();
        assert_eq!(down_enter, BOOL(1));

        // Candidate window must close, and engine resets
        assert!(harness.tip_raw.engine.borrow().candidate_list.is_none());
        assert!(harness.tip_raw.engine.borrow().syllable.is_empty());
    }
}


#[test]
fn test_e2e_tsf_continuous_typing_after_manual_candidate_selection() {
    let harness = match TsfTestHarness::new() {
        Ok(h) => h,
        Err(_) => return,
    };

    unsafe {
        // --- 1. Type first syllable: ㄅ(1) ㄨ(j) ˋ(4) ---
        let _ = harness.key_sink.OnTestKeyDown(Some(&harness.context), WPARAM(b'1' as usize), LPARAM(0));
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'1' as usize), LPARAM(0));

        let _ = harness.key_sink.OnTestKeyDown(Some(&harness.context), WPARAM(b'J' as usize), LPARAM(0));
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'J' as usize), LPARAM(0));

        let _ = harness.key_sink.OnTestKeyDown(Some(&harness.context), WPARAM(b'4' as usize), LPARAM(0));
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'4' as usize), LPARAM(0));

        assert!(harness.tip_raw.engine.borrow().candidate_list.is_some());

        // Check candidate items
        let initial_cand0 = harness.tip_raw.engine.borrow().candidate_list.as_ref().unwrap().select_by_index(0).unwrap().to_string();
        let initial_cand1 = harness.tip_raw.engine.borrow().candidate_list.as_ref().unwrap().select_by_index(1).unwrap().to_string();

        // --- 2. Manually select candidate #2 (key '2') ---
        let test_sel = harness.key_sink.OnTestKeyDown(Some(&harness.context), WPARAM(b'2' as usize), LPARAM(0)).unwrap();
        assert_eq!(test_sel, BOOL(1));

        let down_sel = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'2' as usize), LPARAM(0)).unwrap();
        assert_eq!(down_sel, BOOL(1));

        // Candidate UI must close, engine syllable reset
        assert!(harness.tip_raw.engine.borrow().candidate_list.is_none());
        assert!(harness.tip_raw.engine.borrow().syllable.is_empty());

        // --- 3. CRITICAL: Continue typing immediately after manual candidate selection ---
        // Type second syllable: ㄅ(1) ㄨ(j) ˋ(4)
        let test_c1 = harness.key_sink.OnTestKeyDown(Some(&harness.context), WPARAM(b'1' as usize), LPARAM(0)).unwrap();
        assert_eq!(test_c1, BOOL(1), "Next keystroke after candidate selection must NOT be blocked");

        let down_c1 = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'1' as usize), LPARAM(0)).unwrap();
        assert_eq!(down_c1, BOOL(1));

        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'J' as usize), LPARAM(0));
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'4' as usize), LPARAM(0));

        assert!(harness.tip_raw.engine.borrow().candidate_list.is_some());

        // --- 4. Verify MRU Candidate Rearrangement ---
        // The previously manually selected candidate (initial_cand1) must now be promoted to index 0!
        let new_cand0 = harness.tip_raw.engine.borrow().candidate_list.as_ref().unwrap().select_by_index(0).unwrap().to_string();
        assert_eq!(new_cand0, initial_cand1, "Manually selected word must be rearranged to candidate #1");

        // The previous first candidate is now at index 1
        let new_cand1 = harness.tip_raw.engine.borrow().candidate_list.as_ref().unwrap().select_by_index(1).unwrap().to_string();
        assert_eq!(new_cand1, initial_cand0);

        // --- 5. Test Continuous Typing without manual selection ---
        // When candidate window is showing, directly pressing next consonant 'S' (ㄉ)
        // must auto-commit candidate #1 and start the next syllable!
        let test_cont = harness.key_sink.OnTestKeyDown(Some(&harness.context), WPARAM(b'S' as usize), LPARAM(0)).unwrap();
        assert_eq!(test_cont, BOOL(1));

        let down_cont = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'S' as usize), LPARAM(0)).unwrap();
        assert_eq!(down_cont, BOOL(1));

        // Candidate list should be closed, and engine has 'ㄉ' ('s') in progress!
        assert!(harness.tip_raw.engine.borrow().candidate_list.is_none());
        assert_eq!(harness.tip_raw.engine.borrow().syllable.to_cin_code(), "s");

        // --- 6. Complete and commit with Space ---
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'J' as usize), LPARAM(0));
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'4' as usize), LPARAM(0));
        assert!(harness.tip_raw.engine.borrow().candidate_list.is_some());

        // Pressing Space should select Candidate #1
        let test_sp = harness.key_sink.OnTestKeyDown(Some(&harness.context), WPARAM(VK_SPACE.0 as usize), LPARAM(0)).unwrap();
        assert_eq!(test_sp, BOOL(1));
        let down_sp = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(VK_SPACE.0 as usize), LPARAM(0)).unwrap();
        assert_eq!(down_sp, BOOL(1));

        assert!(harness.tip_raw.engine.borrow().candidate_list.is_none());
        assert!(harness.tip_raw.engine.borrow().syllable.is_empty());
    }
}

#[test]
fn test_e2e_tsf_caps_lock_mode_toggle_and_tray_sync() {
    let harness = match TsfTestHarness::new() {
        Ok(h) => h,
        Err(_) => return,
    };

    unsafe {
        // Initialize compartment to match initial Bopomofo mode
        harness.tip_raw.set_openclose_compartment(true);
        assert_eq!(harness.tip_raw.engine.borrow().mode, InputMode::Bopomofo);
        assert_eq!(harness.tip_raw.get_openclose_compartment(), Some(true));

        // 1. Press Caps Lock -> toggles to English mode
        let test_caps1 = harness.key_sink.OnTestKeyDown(Some(&harness.context), WPARAM(VK_CAPITAL.0 as usize), LPARAM(0)).unwrap();
        assert_eq!(test_caps1, BOOL(1));

        let down_caps1 = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(VK_CAPITAL.0 as usize), LPARAM(0)).unwrap();
        assert_eq!(down_caps1, BOOL(1));

        assert_eq!(harness.tip_raw.engine.borrow().mode, InputMode::English);
        assert_eq!(harness.tip_raw.get_openclose_compartment(), Some(false), "Taskbar tray must update to English ('英')");

        // While in English mode, normal keystrokes are passed directly to app (BOOL 0)
        let test_eng = harness.key_sink.OnTestKeyDown(Some(&harness.context), WPARAM(b'A' as usize), LPARAM(0)).unwrap();
        assert_eq!(test_eng, BOOL(0));

        let down_eng = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'A' as usize), LPARAM(0)).unwrap();
        assert_eq!(down_eng, BOOL(0));

        // 2. Press Caps Lock again while in English mode -> toggles back to Bopomofo mode
        let test_caps2 = harness.key_sink.OnTestKeyDown(Some(&harness.context), WPARAM(VK_CAPITAL.0 as usize), LPARAM(0)).unwrap();
        assert_eq!(test_caps2, BOOL(1), "Caps Lock must still be intercepted when in English mode");

        let down_caps2 = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(VK_CAPITAL.0 as usize), LPARAM(0)).unwrap();
        assert_eq!(down_caps2, BOOL(1));

        assert_eq!(harness.tip_raw.engine.borrow().mode, InputMode::Bopomofo);
        assert_eq!(harness.tip_raw.get_openclose_compartment(), Some(true), "Taskbar tray must update to Chinese ('中')");

        // Now keystrokes are intercepted by Bopomofo engine again
        let test_bpmf = harness.key_sink.OnTestKeyDown(Some(&harness.context), WPARAM(b'1' as usize), LPARAM(0)).unwrap();
        assert_eq!(test_bpmf, BOOL(1));
    }
}

#[test]
fn test_e2e_tsf_shifted_punctuation_and_uppercase_auto_commits_prefix() {
    let harness = match TsfTestHarness::new() {
        Ok(h) => h,
        Err(_) => return,
    };

    unsafe {
        // 1. Compose ㄅ (1) + ㄨ (j) + ˋ (4) -> Candidate window opens with "不" as Candidate 1
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'1' as usize), LPARAM(0));
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'J' as usize), LPARAM(0));
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'4' as usize), LPARAM(0));
        assert!(harness.tip_raw.engine.borrow().candidate_list.is_some());

        // Now simulate user typing Shift + 1 ('！') while candidate window is open:
        // Set KEYKEY_TEST_SHIFT_DOWN so map_virtual_key and is_shift_down() evaluate with Shift active
        keykey_tsf::set_test_shift_down(Some(true));
        let res_excl = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'1' as usize), LPARAM(0)).unwrap();
        assert_eq!(res_excl, BOOL(1));
        keykey_tsf::set_test_shift_down(None);

        // The candidate list must now be closed and engine reset
        assert!(harness.tip_raw.engine.borrow().candidate_list.is_none());
        assert!(harness.tip_raw.engine.borrow().syllable.is_empty());

        // 2. Compose partial syllable: ㄅ (1) without tone (no candidate window)
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'1' as usize), LPARAM(0));
        assert!(!harness.tip_raw.engine.borrow().syllable.is_empty());
        assert!(harness.tip_raw.engine.borrow().candidate_list.is_none());

        // Now simulate user typing Shift + A ('A') while composing
        keykey_tsf::set_test_shift_down(Some(true));
        let res_a = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'A' as usize), LPARAM(0)).unwrap();
        assert_eq!(res_a, BOOL(1));
        keykey_tsf::set_test_shift_down(None);

        assert!(harness.tip_raw.engine.borrow().syllable.is_empty());
    }
}

#[test]
fn test_e2e_tsf_arrow_keys_page_navigation() {
    let harness = match TsfTestHarness::new() {
        Ok(h) => h,
        Err(_) => return,
    };

    unsafe {
        // Compose ㄅ (1) + ㄨ (j) + ˋ (4)
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'1' as usize), LPARAM(0));
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'J' as usize), LPARAM(0));
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'4' as usize), LPARAM(0));
        assert!(harness.tip_raw.engine.borrow().candidate_list.is_some());

        // Right arrow and Left arrow must be intercepted (BOOL 1)
        let test_right = harness.key_sink.OnTestKeyDown(Some(&harness.context), WPARAM(VK_RIGHT.0 as usize), LPARAM(0)).unwrap();
        assert_eq!(test_right, BOOL(1));

        let down_right = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(VK_RIGHT.0 as usize), LPARAM(0)).unwrap();
        assert_eq!(down_right, BOOL(1));

        let test_left = harness.key_sink.OnTestKeyDown(Some(&harness.context), WPARAM(VK_LEFT.0 as usize), LPARAM(0)).unwrap();
        assert_eq!(test_left, BOOL(1));

        let down_left = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(VK_LEFT.0 as usize), LPARAM(0)).unwrap();
        assert_eq!(down_left, BOOL(1));
    }
}

#[test]
fn test_e2e_tsf_shift_alone_cancels_composition_and_toggles_mode() {
    let harness = match TsfTestHarness::new() {
        Ok(h) => h,
        Err(_) => return,
    };

    unsafe {
        // Start composing: '1' (ㄅ)
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(b'1' as usize), LPARAM(0));
        assert!(!harness.tip_raw.engine.borrow().syllable.is_empty());

        // Press Shift down
        let _ = harness.key_sink.OnKeyDown(Some(&harness.context), WPARAM(VK_SHIFT.0 as usize), LPARAM(0));

        // Release Shift up -> toggles to English and cancels composition
        let _ = harness.key_sink.OnKeyUp(Some(&harness.context), WPARAM(VK_SHIFT.0 as usize), LPARAM(0));

        assert_eq!(harness.tip_raw.engine.borrow().mode, InputMode::English);
        assert!(harness.tip_raw.engine.borrow().syllable.is_empty());
        assert!(harness.tip_raw.engine.borrow().candidate_list.is_none());
    }
}
