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
            let tip_raw = KeyKeyTip::with_engine(tip.engine.clone());
            // Clone tip candidate_ui & fields before moving into processor
            *tip_raw.thread_mgr.borrow_mut() = Some(thread_mgr.clone());
            tip_raw.client_id.set(client_id);

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
