//! KeyKey Windows TSF (Text Services Framework) Text Input Processor (TIP)

pub mod candidate_ui;
pub mod composition;
pub mod register;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicIsize, Ordering};

use keykey_core::cin::CinTable;
use keykey_core::engine::{EngineAction, KeyCode, KeyKeyEngine};

use windows::core::{implement, Result, GUID, HRESULT};
use windows_core::Interface;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::SystemServices::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::TextServices::*;

use crate::candidate_ui::CandidateUIManager;
use crate::composition::{EditAction, KeyKeyEditSession};

// KeyKey Text Service CLSID: {B892F1C2-88E4-4866-96D5-812C465EB86B}
pub const CLSID_KEYKEY_TEXT_SERVICE: GUID =
    GUID::from_u128(0xB892F1C2_88E4_4866_96D5_812C465EB86B);

// KeyKey Profile GUID: {6786A76E-6F3C-4318-A785-5AE4E8251268}
pub const GUID_KEYKEY_PROFILE: GUID =
    GUID::from_u128(0x6786A76E_6F3C_4318_A785_5AE4E8251268);

pub static DLL_INSTANCE: AtomicIsize = AtomicIsize::new(0);

// Embedded default Bopomofo table from DataTables/bpmf.cin
const DEFAULT_BPMF_CIN: &[u8] =
    include_bytes!("../../../YahooKeyKey-Source-1.1.2528/DataTables/bpmf.cin");

#[no_mangle]
pub extern "system" fn DllMain(
    hinst: HINSTANCE,
    reason: u32,
    _reserved: *mut std::ffi::c_void,
) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        DLL_INSTANCE.store(hinst.0 as isize, Ordering::SeqCst);
    }
    BOOL(1)
}

#[implement(IClassFactory)]
pub struct KeyKeyClassFactory;

impl IClassFactory_Impl for KeyKeyClassFactory_Impl {
    fn CreateInstance(
        &self,
        punkouter: Option<&windows::core::IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut std::ffi::c_void,
    ) -> Result<()> {
        if punkouter.is_some() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let tip: ITfTextInputProcessorEx = KeyKeyTip::new().into();
        unsafe { tip.query(riid, ppvobject).ok() }
    }

    fn LockServer(&self, _flock: BOOL) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyStroke {
    Engine(KeyCode),
    CommitDirect(String),
}

#[implement(
    ITfTextInputProcessorEx,
    ITfKeyEventSink,
    ITfCompositionSink,
    ITfCompartmentEventSink
)]
pub struct KeyKeyTip {
    pub client_id: Rc<Cell<u32>>,
    pub thread_mgr: Rc<RefCell<Option<ITfThreadMgr>>>,
    pub engine: Rc<RefCell<KeyKeyEngine>>,
    pub active_composition: Rc<RefCell<Option<ITfComposition>>>,
    pub candidate_ui: CandidateUIManager,
    pub shift_alone: Cell<bool>,
    pub compartment_cookie: Cell<Option<u32>>,
}

impl KeyKeyTip {
    pub fn new() -> Self {
        // Load default CinTable from embedded bpmf.cin
        let cin_table = CinTable::load_from_reader(DEFAULT_BPMF_CIN)
            .unwrap_or_else(|_| CinTable::new());

        let engine = KeyKeyEngine::new(cin_table);
        Self::with_engine(Rc::new(RefCell::new(engine)))
    }

    pub fn with_engine(engine: Rc<RefCell<KeyKeyEngine>>) -> Self {
        let client_id = Rc::new(Cell::new(0));
        let thread_mgr: Rc<RefCell<Option<ITfThreadMgr>>> = Rc::new(RefCell::new(None));
        let active_composition: Rc<RefCell<Option<ITfComposition>>> = Rc::new(RefCell::new(None));
        let candidate_ui = CandidateUIManager::new();

        // Wire candidate list mouse finalization callback
        let cand_mgr_clone = candidate_ui.clone();
        let tm_clone = thread_mgr.clone();
        let eng_clone = engine.clone();
        let comp_clone = active_composition.clone();
        let cid_clone = client_id.clone();

        candidate_ui.set_on_finalize(move |idx| {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if let Ok(tm_lock) = tm_clone.try_borrow() {
                    if let Some(ref tm) = *tm_lock {
                        unsafe {
                            if let Ok(doc_mgr) = tm.GetFocus() {
                                if let Ok(context) = doc_mgr.GetTop() {
                                    let text = cand_mgr_clone.data.candidates.try_borrow().ok().and_then(|c| c.get(idx as usize).cloned());
                                    if let Some(text) = text {
                                        let session: ITfEditSession = KeyKeyEditSession::new(
                                            context.clone(),
                                            EditAction::Commit(text),
                                            comp_clone.clone(),
                                            None,
                                        ).into();
                                        let _ = context.RequestEditSession(cid_clone.get(), &session, TF_ES_READWRITE | TF_ES_SYNC);
                                    }
                                }
                            }
                            let _ = cand_mgr_clone.close(tm);
                        }
                    }
                }
                if let Ok(mut eng) = eng_clone.try_borrow_mut() {
                    eng.reset();
                }
            }));
        });

        Self {
            client_id,
            thread_mgr,
            engine,
            active_composition,
            candidate_ui,
            shift_alone: Cell::new(false),
            compartment_cookie: Cell::new(None),
        }
    }

    pub fn get_openclose_compartment(&self) -> Option<bool> {
        let tm = self.thread_mgr.try_borrow().ok()?.clone()?;
        let comp_mgr: ITfCompartmentMgr = tm.cast().ok()?;
        unsafe {
            let comp = comp_mgr.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE).ok()?;
            let var = comp.GetValue().ok()?;
            let val: i32 = (&var).try_into().ok()?;
            Some(val != 0)
        }
    }

    pub fn set_openclose_compartment(&self, open: bool) {
        if let Ok(tm_lock) = self.thread_mgr.try_borrow() {
            if let Some(ref tm) = *tm_lock {
                if let Ok(comp_mgr) = tm.cast::<ITfCompartmentMgr>() {
                    if let Ok(comp) = unsafe { comp_mgr.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE) } {
                        let var = windows_core::VARIANT::from(if open { 1i32 } else { 0i32 });
                        unsafe {
                            let _ = comp.SetValue(self.client_id.get(), &var);
                        }
                    }
                }
            }
        }
    }

    pub fn map_virtual_key(&self, wparam: WPARAM, is_shift: bool) -> Option<KeyStroke> {
        let vk = wparam.0 as u16;
        if is_shift {
            match VIRTUAL_KEY(vk) {
                VK_OEM_COMMA => Some(KeyStroke::CommitDirect("，".to_string())),
                VK_OEM_PERIOD => Some(KeyStroke::CommitDirect("。".to_string())),
                VK_OEM_2 => Some(KeyStroke::CommitDirect("？".to_string())),
                VK_OEM_1 => Some(KeyStroke::CommitDirect("：".to_string())),
                VK_OEM_4 => Some(KeyStroke::CommitDirect("「".to_string())),
                VK_OEM_6 => Some(KeyStroke::CommitDirect("」".to_string())),
                VK_OEM_5 => Some(KeyStroke::CommitDirect("、".to_string())),
                VK_OEM_7 => Some(KeyStroke::CommitDirect("”".to_string())),
                VK_OEM_MINUS => Some(KeyStroke::CommitDirect("——".to_string())),
                _ => {
                    if vk == b'1' as u16 {
                        Some(KeyStroke::CommitDirect("！".to_string()))
                    } else if (b'A'..=b'Z').contains(&(vk as u8)) {
                        // Direct uppercase ASCII commit
                        Some(KeyStroke::CommitDirect(((vk as u8) as char).to_string()))
                    } else {
                        None
                    }
                }
            }
        } else {
            match VIRTUAL_KEY(vk) {
                VK_OEM_5 => Some(KeyStroke::CommitDirect("、".to_string())),
                VK_BACK => Some(KeyStroke::Engine(KeyCode::Backspace)),
                VK_RETURN => Some(KeyStroke::Engine(KeyCode::Enter)),
                VK_ESCAPE => Some(KeyStroke::Engine(KeyCode::Escape)),
                VK_SPACE => Some(KeyStroke::Engine(KeyCode::Space)),
                VK_DOWN => Some(KeyStroke::Engine(KeyCode::Down)),
                VK_UP => Some(KeyStroke::Engine(KeyCode::Up)),
                VK_PRIOR => Some(KeyStroke::Engine(KeyCode::PageUp)),
                VK_NEXT => Some(KeyStroke::Engine(KeyCode::PageDown)),
                VK_OEM_COMMA => Some(KeyStroke::Engine(KeyCode::Char(','))),
                VK_OEM_PERIOD => Some(KeyStroke::Engine(KeyCode::Char('.'))),
                VK_OEM_1 => Some(KeyStroke::Engine(KeyCode::Char(';'))),
                VK_OEM_2 => Some(KeyStroke::Engine(KeyCode::Char('/'))),
                VK_OEM_MINUS => Some(KeyStroke::Engine(KeyCode::Char('-'))),
                _ => {
                    if (b'0'..=b'9').contains(&(vk as u8)) {
                        Some(KeyStroke::Engine(KeyCode::Char((vk as u8) as char)))
                    } else if (b'A'..=b'Z').contains(&(vk as u8)) {
                        // Convert to lowercase for Bopomofo mapping
                        let ch = ((vk as u8) + 32) as char;
                        Some(KeyStroke::Engine(KeyCode::Char(ch)))
                    } else {
                        None
                    }
                }
            }
        }
    }
}

impl ITfTextInputProcessor_Impl for KeyKeyTip_Impl {
    fn Activate(&self, ptim: Option<&ITfThreadMgr>, tid: u32) -> Result<()> {
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.activate_internal(ptim, tid)
        }));
        res.unwrap_or(Ok(()))
    }

    fn Deactivate(&self) -> Result<()> {
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.deactivate_internal()
        }));
        res.unwrap_or(Ok(()))
    }
}

impl KeyKeyTip_Impl {
    fn activate_internal(&self, ptim: Option<&ITfThreadMgr>, tid: u32) -> Result<()> {
        self.client_id.set(tid);
        if let Ok(mut tm_lock) = self.thread_mgr.try_borrow_mut() {
            *tm_lock = ptim.cloned();
        }

        if let Some(tm) = ptim {
            unsafe {
                if let Ok(keystroke_mgr) = tm.cast::<ITfKeystrokeMgr>() {
                    if let Ok(key_sink) = self.cast::<ITfKeyEventSink>() {
                        let _ = keystroke_mgr.AdviseKeyEventSink(tid, &key_sink, true);
                    }
                }

                if let Ok(comp_mgr) = tm.cast::<ITfCompartmentMgr>() {
                    if let Ok(comp) = comp_mgr.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE) {
                        if let Ok(source) = comp.cast::<ITfSource>() {
                            if let Ok(sink) = self.cast::<ITfCompartmentEventSink>() {
                                if let Ok(cookie) = source.AdviseSink(&ITfCompartmentEventSink::IID, &sink) {
                                    self.compartment_cookie.set(Some(cookie));
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn deactivate_internal(&self) -> Result<()> {
        let tid = self.client_id.get();
        let tm = if let Ok(mut tm_lock) = self.thread_mgr.try_borrow_mut() {
            tm_lock.take()
        } else {
            None
        };
        if let Some(tm) = tm {
            let _ = self.candidate_ui.close(&tm);
            unsafe {
                if let Some(cookie) = self.compartment_cookie.take() {
                    if let Ok(comp_mgr) = tm.cast::<ITfCompartmentMgr>() {
                        if let Ok(comp) = comp_mgr.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE) {
                            if let Ok(source) = comp.cast::<ITfSource>() {
                                let _ = source.UnadviseSink(cookie);
                            }
                        }
                    }
                }

                if let Ok(keystroke_mgr) = tm.cast::<ITfKeystrokeMgr>() {
                    let _ = keystroke_mgr.UnadviseKeyEventSink(tid);
                }
            }
        }
        if let Ok(mut engine) = self.engine.try_borrow_mut() {
            engine.reset();
        }
        if let Ok(mut comp) = self.active_composition.try_borrow_mut() {
            *comp = None;
        }
        Ok(())
    }
}

impl ITfTextInputProcessorEx_Impl for KeyKeyTip_Impl {
    fn ActivateEx(&self, ptim: Option<&ITfThreadMgr>, tid: u32, _dwflags: u32) -> Result<()> {
        self.Activate(ptim, tid)
    }
}

impl ITfCompartmentEventSink_Impl for KeyKeyTip_Impl {
    fn OnChange(&self, rguid: *const GUID) -> Result<()> {
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if !rguid.is_null() && unsafe { *rguid } == GUID_COMPARTMENT_KEYBOARD_OPENCLOSE {
                if let Some(open) = self.get_openclose_compartment() {
                    let target_mode = if open {
                        keykey_core::engine::InputMode::Bopomofo
                    } else {
                        keykey_core::engine::InputMode::English
                    };
                    if let Ok(mut engine) = self.engine.try_borrow_mut() {
                        if engine.mode != target_mode {
                            engine.mode = target_mode;
                            engine.reset();
                        }
                    }
                }
            }
            Ok(())
        }));
        res.unwrap_or(Ok(()))
    }
}

impl ITfCompositionSink_Impl for KeyKeyTip_Impl {
    fn OnCompositionTerminated(
        &self,
        _ec: u32,
        _pcomposition: Option<&ITfComposition>,
    ) -> Result<()> {
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if let Ok(tm_lock) = self.thread_mgr.try_borrow() {
                if let Some(ref tm) = *tm_lock {
                    let _ = self.candidate_ui.close(tm);
                }
            }
            if let Ok(mut engine) = self.engine.try_borrow_mut() {
                engine.reset();
            }
            if let Ok(mut active) = self.active_composition.try_borrow_mut() {
                *active = None;
            }
            Ok(())
        }));
        res.unwrap_or(Ok(()))
    }
}

impl KeyKeyTip_Impl {
    fn on_test_key_down_internal(
        &self,
        _pic: Option<&ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        let Ok(engine) = self.engine.try_borrow() else {
            return Ok(BOOL(0));
        };
        if engine.mode == keykey_core::engine::InputMode::English {
            return Ok(BOOL(0));
        }

        let vk = wparam.0 as u16;
        if vk == VK_SHIFT.0 || vk == VK_LSHIFT.0 || vk == VK_RSHIFT.0 {
            return Ok(BOOL(0));
        }

        let is_shift = unsafe { (GetKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0 };
        if let Some(keystroke) = self.map_virtual_key(wparam, is_shift) {
            match keystroke {
                KeyStroke::CommitDirect(_) => return Ok(BOOL(1)),
                KeyStroke::Engine(keycode) => {
                    // If candidate list is active, we consume candidate selection & navigation keys
                    if engine.candidate_list.is_some() {
                        match keycode {
                            KeyCode::Char(c) if engine.cin_table.selkey.contains(c) => return Ok(BOOL(1)),
                            KeyCode::Space | KeyCode::Down | KeyCode::Up | KeyCode::PageDown | KeyCode::PageUp | KeyCode::Escape => {
                                return Ok(BOOL(1));
                            }
                            _ => return Ok(BOOL(0)),
                        }
                    }

                    // If composing a syllable
                    if !engine.syllable.is_empty() {
                        match keycode {
                            KeyCode::Backspace | KeyCode::Escape | KeyCode::Enter | KeyCode::Space => {
                                return Ok(BOOL(1));
                            }
                            KeyCode::Char(c) if keykey_core::bopomofo::classify_key(c) != keykey_core::ComponentType::Unknown => {
                                return Ok(BOOL(1));
                            }
                            _ => return Ok(BOOL(0)),
                        }
                    }

                    // If empty, check if key is a valid Bopomofo initial key
                    if let KeyCode::Char(c) = keycode {
                        if keykey_core::bopomofo::classify_key(c) != keykey_core::ComponentType::Unknown && c != ' ' {
                            return Ok(BOOL(1));
                        }
                    }
                }
            }
        }

        Ok(BOOL(0))
    }

    fn on_key_down_internal(
        &self,
        pic: Option<&ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        let vk = wparam.0 as u16;
        if vk == VK_SHIFT.0 || vk == VK_LSHIFT.0 || vk == VK_RSHIFT.0 {
            self.shift_alone.set(true);
            return Ok(BOOL(0));
        }
        self.shift_alone.set(false);

        let Ok(mut engine) = self.engine.try_borrow_mut() else {
            return Ok(BOOL(0));
        };

        if engine.mode == keykey_core::engine::InputMode::English {
            return Ok(BOOL(0));
        }

        let Some(context) = pic else {
            return Ok(BOOL(0));
        };

        let is_shift = unsafe { (GetKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0 };
        let Some(keystroke) = self.map_virtual_key(wparam, is_shift) else {
            return Ok(BOOL(0));
        };

        let sink: Option<ITfCompositionSink> = unsafe { self.cast().ok() };
        let tid = self.client_id.get();

        match keystroke {
            KeyStroke::CommitDirect(text) => {
                if engine.candidate_list.is_some() {
                    if let Ok(tm_lock) = self.thread_mgr.try_borrow() {
                        if let Some(ref tm) = *tm_lock {
                            let _ = self.candidate_ui.close(tm);
                        }
                    }
                }
                engine.reset();

                let session: ITfEditSession = KeyKeyEditSession::new(
                    context.clone(),
                    EditAction::Commit(text),
                    self.active_composition.clone(),
                    sink,
                ).into();
                unsafe {
                    let _ = context.RequestEditSession(tid, &session, TF_ES_READWRITE | TF_ES_SYNC);
                }
                return Ok(BOOL(1));
            }
            KeyStroke::Engine(keycode) => {
                let actions = engine.handle_key(keycode);
                if actions.is_empty() || actions == vec![EngineAction::Ignored] {
                    return Ok(BOOL(0));
                }

                for action in actions {
                    match action {
                        EngineAction::UpdateComposition { display_text, .. } => {
                            let session: ITfEditSession = KeyKeyEditSession::new(
                                context.clone(),
                                EditAction::UpdateComposition(display_text),
                                self.active_composition.clone(),
                                sink.clone(),
                            ).into();
                            unsafe {
                                let _ = context.RequestEditSession(tid, &session, TF_ES_READWRITE | TF_ES_SYNC);
                            }
                        }
                        EngineAction::Commit(text) => {
                            let session: ITfEditSession = KeyKeyEditSession::new(
                                context.clone(),
                                EditAction::Commit(text),
                                self.active_composition.clone(),
                                sink.clone(),
                            ).into();
                            unsafe {
                                let _ = context.RequestEditSession(tid, &session, TF_ES_READWRITE | TF_ES_SYNC);
                            }
                        }
                        EngineAction::ShowCandidates { page, .. } => {
                            if let Ok(tm_lock) = self.thread_mgr.try_borrow() {
                                if let Some(ref tm) = *tm_lock {
                                    let all_cands: Vec<String> = engine
                                        .candidate_list
                                        .as_ref()
                                        .map(|cl| cl.all_candidates().to_vec())
                                        .unwrap_or_default();
                                    let _ = self.candidate_ui.show(tm, all_cands, page);
                                }
                            }
                        }
                        EngineAction::HideCandidates => {
                            if let Ok(tm_lock) = self.thread_mgr.try_borrow() {
                                if let Some(ref tm) = *tm_lock {
                                    let _ = self.candidate_ui.close(tm);
                                }
                            }
                            if engine.syllable.is_empty() {
                                let session: ITfEditSession = KeyKeyEditSession::new(
                                    context.clone(),
                                    EditAction::Cancel,
                                    self.active_composition.clone(),
                                    sink.clone(),
                                ).into();
                                unsafe {
                                    let _ = context.RequestEditSession(tid, &session, TF_ES_READWRITE | TF_ES_SYNC);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(BOOL(1))
    }
}

impl ITfKeyEventSink_Impl for KeyKeyTip_Impl {
    fn OnSetFocus(&self, _fforeground: BOOL) -> Result<()> {
        Ok(())
    }

    fn OnTestKeyDown(
        &self,
        pic: Option<&ITfContext>,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Result<BOOL> {
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.on_test_key_down_internal(pic, wparam, lparam)
        }));
        res.unwrap_or(Ok(BOOL(0)))
    }

    fn OnKeyDown(
        &self,
        pic: Option<&ITfContext>,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Result<BOOL> {
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.on_key_down_internal(pic, wparam, lparam)
        }));
        res.unwrap_or(Ok(BOOL(0)))
    }

    fn OnTestKeyUp(
        &self,
        _pic: Option<&ITfContext>,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        Ok(BOOL(0))
    }

    fn OnKeyUp(
        &self,
        _pic: Option<&ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let vk = wparam.0 as u16;
            if vk == VK_SHIFT.0 || vk == VK_LSHIFT.0 || vk == VK_RSHIFT.0 {
                if self.shift_alone.replace(false) {
                    if let Ok(mut engine) = self.engine.try_borrow_mut() {
                        let new_mode = engine.toggle_mode();
                        self.set_openclose_compartment(new_mode == keykey_core::engine::InputMode::Bopomofo);
                    }
                }
            }
            Ok(BOOL(0))
        }));
        res.unwrap_or(Ok(BOOL(0)))
    }

    fn OnPreservedKey(
        &self,
        _pic: Option<&ITfContext>,
        _rguid: *const GUID,
    ) -> Result<BOOL> {
        Ok(BOOL(0))
    }
}

pub unsafe fn dll_get_class_object(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut std::ffi::c_void,
) -> HRESULT {
    if rclsid.is_null() || riid.is_null() || ppv.is_null() {
        return E_INVALIDARG;
    }
    if *rclsid != CLSID_KEYKEY_TEXT_SERVICE {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    let factory: IClassFactory = KeyKeyClassFactory.into();
    factory.query(riid, ppv).into()
}

pub fn dll_can_unload_now() -> HRESULT {
    S_FALSE
}

pub fn dll_register_server() -> HRESULT {
    register::register_server().into()
}

pub fn dll_unregister_server() -> HRESULT {
    register::unregister_server().into()
}

#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut std::ffi::c_void,
) -> HRESULT {
    dll_get_class_object(rclsid, riid, ppv)
}

#[no_mangle]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    dll_can_unload_now()
}

#[no_mangle]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    dll_register_server()
}

#[no_mangle]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    dll_unregister_server()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compartment_check() {
        use windows_core::VARIANT;
        let _ = GUID_COMPARTMENT_KEYBOARD_OPENCLOSE;
        let _ = ITfCompartmentEventSink::IID;
        let var = VARIANT::from(1i32);
        let val: i32 = (&var).try_into().unwrap();
        assert_eq!(val, 1);
    }

    #[test]
    fn test_map_virtual_key_all() {
        let tip = KeyKeyTip::new();

        assert_eq!(tip.map_virtual_key(WPARAM(VK_BACK.0 as usize), false), Some(KeyStroke::Engine(KeyCode::Backspace)));
        assert_eq!(tip.map_virtual_key(WPARAM(VK_RETURN.0 as usize), false), Some(KeyStroke::Engine(KeyCode::Enter)));
        assert_eq!(tip.map_virtual_key(WPARAM(VK_ESCAPE.0 as usize), false), Some(KeyStroke::Engine(KeyCode::Escape)));
        assert_eq!(tip.map_virtual_key(WPARAM(VK_SPACE.0 as usize), false), Some(KeyStroke::Engine(KeyCode::Space)));
        assert_eq!(tip.map_virtual_key(WPARAM(VK_DOWN.0 as usize), false), Some(KeyStroke::Engine(KeyCode::Down)));
        assert_eq!(tip.map_virtual_key(WPARAM(VK_UP.0 as usize), false), Some(KeyStroke::Engine(KeyCode::Up)));
        assert_eq!(tip.map_virtual_key(WPARAM(VK_PRIOR.0 as usize), false), Some(KeyStroke::Engine(KeyCode::PageUp)));
        assert_eq!(tip.map_virtual_key(WPARAM(VK_NEXT.0 as usize), false), Some(KeyStroke::Engine(KeyCode::PageDown)));

        assert_eq!(tip.map_virtual_key(WPARAM(VK_OEM_COMMA.0 as usize), false), Some(KeyStroke::Engine(KeyCode::Char(','))));
        assert_eq!(tip.map_virtual_key(WPARAM(VK_OEM_PERIOD.0 as usize), false), Some(KeyStroke::Engine(KeyCode::Char('.'))));
        assert_eq!(tip.map_virtual_key(WPARAM(VK_OEM_1.0 as usize), false), Some(KeyStroke::Engine(KeyCode::Char(';'))));
        assert_eq!(tip.map_virtual_key(WPARAM(VK_OEM_2.0 as usize), false), Some(KeyStroke::Engine(KeyCode::Char('/'))));
        assert_eq!(tip.map_virtual_key(WPARAM(VK_OEM_MINUS.0 as usize), false), Some(KeyStroke::Engine(KeyCode::Char('-'))));
        assert_eq!(tip.map_virtual_key(WPARAM(VK_OEM_5.0 as usize), false), Some(KeyStroke::CommitDirect("、".to_string())));

        assert_eq!(tip.map_virtual_key(WPARAM(b'1' as usize), false), Some(KeyStroke::Engine(KeyCode::Char('1'))));
        assert_eq!(tip.map_virtual_key(WPARAM(b'A' as usize), false), Some(KeyStroke::Engine(KeyCode::Char('a'))));
        assert_eq!(tip.map_virtual_key(WPARAM(VK_F1.0 as usize), false), None);

        // Shifted punctuations
        assert_eq!(tip.map_virtual_key(WPARAM(VK_OEM_COMMA.0 as usize), true), Some(KeyStroke::CommitDirect("，".to_string())));
        assert_eq!(tip.map_virtual_key(WPARAM(VK_OEM_PERIOD.0 as usize), true), Some(KeyStroke::CommitDirect("。".to_string())));
        assert_eq!(tip.map_virtual_key(WPARAM(VK_OEM_2.0 as usize), true), Some(KeyStroke::CommitDirect("？".to_string())));
        assert_eq!(tip.map_virtual_key(WPARAM(VK_OEM_1.0 as usize), true), Some(KeyStroke::CommitDirect("：".to_string())));
        assert_eq!(tip.map_virtual_key(WPARAM(b'1' as usize), true), Some(KeyStroke::CommitDirect("！".to_string())));
        assert_eq!(tip.map_virtual_key(WPARAM(VK_OEM_4.0 as usize), true), Some(KeyStroke::CommitDirect("「".to_string())));
        assert_eq!(tip.map_virtual_key(WPARAM(VK_OEM_6.0 as usize), true), Some(KeyStroke::CommitDirect("」".to_string())));
        assert_eq!(tip.map_virtual_key(WPARAM(VK_OEM_5.0 as usize), true), Some(KeyStroke::CommitDirect("、".to_string())));
        assert_eq!(tip.map_virtual_key(WPARAM(VK_OEM_7.0 as usize), true), Some(KeyStroke::CommitDirect("”".to_string())));
        assert_eq!(tip.map_virtual_key(WPARAM(VK_OEM_MINUS.0 as usize), true), Some(KeyStroke::CommitDirect("——".to_string())));

        // Shifted letter
        assert_eq!(tip.map_virtual_key(WPARAM(b'A' as usize), true), Some(KeyStroke::CommitDirect("A".to_string())));
        assert_eq!(tip.map_virtual_key(WPARAM(VK_F1.0 as usize), true), None);
    }

    #[test]
    fn test_tip_sink_callbacks() {
        let tip = KeyKeyTip::new();
        let key_sink: ITfKeyEventSink = tip.into();

        assert_eq!(unsafe { key_sink.OnSetFocus(BOOL(1)) }.unwrap(), ());
        assert_eq!(unsafe { key_sink.OnSetFocus(BOOL(0)) }.unwrap(), ());

        assert_eq!(unsafe { key_sink.OnTestKeyUp(None, WPARAM(0), LPARAM(0)) }.unwrap(), BOOL(0));
        assert_eq!(unsafe { key_sink.OnKeyUp(None, WPARAM(0), LPARAM(0)) }.unwrap(), BOOL(0));
        assert_eq!(unsafe { key_sink.OnPreservedKey(None, std::ptr::null()) }.unwrap(), BOOL(0));
    }

    #[test]
    fn test_on_test_key_down_branches() {
        let default_cin = CinTable::load_from_reader(DEFAULT_BPMF_CIN).unwrap();
        let engine = Rc::new(RefCell::new(KeyKeyEngine::new(default_cin)));
        let tip = KeyKeyTip::with_engine(engine.clone());
        let key_sink: ITfKeyEventSink = tip.into();

        // 1. Empty buffer
        // '1' is ㄅ (consumed)
        assert_eq!(
            unsafe { key_sink.OnTestKeyDown(None, WPARAM(b'1' as usize), LPARAM(0)) }.unwrap(),
            BOOL(1)
        );
        // Space on empty buffer (not consumed)
        assert_eq!(
            unsafe { key_sink.OnTestKeyDown(None, WPARAM(VK_SPACE.0 as usize), LPARAM(0)) }.unwrap(),
            BOOL(0)
        );
        // F1 (unmapped -> not consumed)
        assert_eq!(
            unsafe { key_sink.OnTestKeyDown(None, WPARAM(VK_F1.0 as usize), LPARAM(0)) }.unwrap(),
            BOOL(0)
        );

        // 2. Composing syllable: feed '1' (ㄅ) into engine
        engine.borrow_mut().handle_key(KeyCode::Char('1'));
        assert_eq!(
            unsafe { key_sink.OnTestKeyDown(None, WPARAM(VK_BACK.0 as usize), LPARAM(0)) }.unwrap(),
            BOOL(1)
        );
        assert_eq!(
            unsafe { key_sink.OnTestKeyDown(None, WPARAM(VK_ESCAPE.0 as usize), LPARAM(0)) }.unwrap(),
            BOOL(1)
        );
        assert_eq!(
            unsafe { key_sink.OnTestKeyDown(None, WPARAM(VK_RETURN.0 as usize), LPARAM(0)) }.unwrap(),
            BOOL(1)
        );
        assert_eq!(
            unsafe { key_sink.OnTestKeyDown(None, WPARAM(VK_SPACE.0 as usize), LPARAM(0)) }.unwrap(),
            BOOL(1)
        );
        assert_eq!(
            unsafe { key_sink.OnTestKeyDown(None, WPARAM(b'J' as usize), LPARAM(0)) }.unwrap(),
            BOOL(1)
        );
        assert_eq!(
            unsafe { key_sink.OnTestKeyDown(None, WPARAM(VK_OEM_1.0 as usize), LPARAM(0)) }.unwrap(),
            BOOL(1)
        );
        // Non-bopomofo key while composing is not consumed
        assert_eq!(
            unsafe { key_sink.OnTestKeyDown(None, WPARAM(VK_UP.0 as usize), LPARAM(0)) }.unwrap(),
            BOOL(0)
        );

        // 3. Candidate list active: feed 'j' and '4' -> triggers candidates for ㄅㄨˋ
        engine.borrow_mut().handle_key(KeyCode::Char('j'));
        engine.borrow_mut().handle_key(KeyCode::Char('4'));
        assert!(engine.borrow().candidate_list.is_some());

        // Test candidate keys
        assert_eq!(
            unsafe { key_sink.OnTestKeyDown(None, WPARAM(b'1' as usize), LPARAM(0)) }.unwrap(),
            BOOL(1)
        );
        assert_eq!(
            unsafe { key_sink.OnTestKeyDown(None, WPARAM(VK_SPACE.0 as usize), LPARAM(0)) }.unwrap(),
            BOOL(1)
        );
        assert_eq!(
            unsafe { key_sink.OnTestKeyDown(None, WPARAM(VK_DOWN.0 as usize), LPARAM(0)) }.unwrap(),
            BOOL(1)
        );
        assert_eq!(
            unsafe { key_sink.OnTestKeyDown(None, WPARAM(VK_UP.0 as usize), LPARAM(0)) }.unwrap(),
            BOOL(1)
        );
        assert_eq!(
            unsafe { key_sink.OnTestKeyDown(None, WPARAM(VK_NEXT.0 as usize), LPARAM(0)) }.unwrap(),
            BOOL(1)
        );
        assert_eq!(
            unsafe { key_sink.OnTestKeyDown(None, WPARAM(VK_PRIOR.0 as usize), LPARAM(0)) }.unwrap(),
            BOOL(1)
        );
        assert_eq!(
            unsafe { key_sink.OnTestKeyDown(None, WPARAM(VK_ESCAPE.0 as usize), LPARAM(0)) }.unwrap(),
            BOOL(1)
        );
        // Non-candidate key
        assert_eq!(
            unsafe { key_sink.OnTestKeyDown(None, WPARAM(b'Z' as usize), LPARAM(0)) }.unwrap(),
            BOOL(0)
        );

        // Test OnKeyDown with pic = None
        assert_eq!(
            unsafe { key_sink.OnKeyDown(None, WPARAM(b'1' as usize), LPARAM(0)) }.unwrap(),
            BOOL(0)
        );
        // Test OnKeyDown with unmapped key
        assert_eq!(
            unsafe { key_sink.OnKeyDown(None, WPARAM(VK_F1.0 as usize), LPARAM(0)) }.unwrap(),
            BOOL(0)
        );

        // Test OnCompositionTerminated
        let comp_sink: ITfCompositionSink = key_sink.cast().unwrap();
        assert_eq!(unsafe { comp_sink.OnCompositionTerminated(0, None) }.unwrap(), ());
        assert!(engine.borrow().syllable.is_empty());
        assert!(engine.borrow().candidate_list.is_none());
    }

    #[test]
    fn test_real_tsf_thread_mgr() {
        unsafe {
            let _ = CoInitialize(None);
            let tm: Result<ITfThreadMgr> = CoCreateInstance(&CLSID_TF_ThreadMgr, None, CLSCTX_INPROC_SERVER);
            println!("CoCreateInstance CLSID_TF_ThreadMgr: {:?}", tm.is_ok());
            if let Ok(ref mgr) = tm {
                let ui_mgr: Result<ITfUIElementMgr> = mgr.cast();
                println!("ITfUIElementMgr cast: {:?}", ui_mgr.is_ok());
                let key_mgr: Result<ITfKeystrokeMgr> = mgr.cast();
                println!("ITfKeystrokeMgr cast: {:?}", key_mgr.is_ok());

                let client_id = mgr.Activate().unwrap();
                println!("mgr.Activate returned client_id: {}", client_id);

                let doc_mgr = mgr.CreateDocumentMgr();
                println!("mgr.CreateDocumentMgr: {:?}", doc_mgr.is_ok());

                if let Ok(ref dm) = doc_mgr {
                    let mut edit_cookie = 0u32;
                    let mut context: Option<ITfContext> = None;
                    let ctx_res = dm.CreateContext(client_id, 0, None, &mut context, &mut edit_cookie);
                    assert!(ctx_res.is_ok());
                    if let Some(ref ctx) = context {
                        let push_res = dm.Push(ctx);
                        assert!(push_res.is_ok());
                        let _ = mgr.SetFocus(dm);

                        let tip = KeyKeyTip::new();
                        let cand_ui = tip.candidate_ui.clone();
                        let processor: ITfTextInputProcessorEx = tip.into();
                        assert!(processor.Activate(Some(mgr), client_id).is_ok());

                        let tip2 = KeyKeyTip::new();
                        *tip2.thread_mgr.borrow_mut() = Some(mgr.clone());
                        let _ = tip2.get_openclose_compartment();
                        tip2.set_openclose_compartment(false);
                        tip2.set_openclose_compartment(true);

                        let key_sink: ITfKeyEventSink = processor.cast().unwrap();

                        // 1. Start composition: '1' (ㄅ) -> UpdateComposition
                        let kd1 = key_sink.OnKeyDown(Some(ctx), WPARAM(b'1' as usize), LPARAM(0));
                        assert_eq!(kd1.unwrap(), BOOL(1));

                        // 2. Medial: 'j' (ㄨ) -> UpdateComposition
                        let kd2 = key_sink.OnKeyDown(Some(ctx), WPARAM(b'J' as usize), LPARAM(0));
                        assert_eq!(kd2.unwrap(), BOOL(1));

                        // 3. Tone: '4' (ˋ) -> ShowCandidates
                        let kd3 = key_sink.OnKeyDown(Some(ctx), WPARAM(b'4' as usize), LPARAM(0));
                        assert_eq!(kd3.unwrap(), BOOL(1));

                        // 4. Select candidate: '1' ("不") -> Commit
                        let kd4 = key_sink.OnKeyDown(Some(ctx), WPARAM(b'1' as usize), LPARAM(0));
                        assert_eq!(kd4.unwrap(), BOOL(1));

                        // 5. Cancel flow: type '1' then Escape -> HideCandidates
                        let _ = key_sink.OnKeyDown(Some(ctx), WPARAM(b'1' as usize), LPARAM(0));
                        // 5b. Invalid combination: type 'q' after '1' -> triggers EngineAction::Beep
                        let kd_beep = key_sink.OnKeyDown(Some(ctx), WPARAM(b'Q' as usize), LPARAM(0));
                        assert_eq!(kd_beep.unwrap(), BOOL(1));

                        let kd_esc = key_sink.OnKeyDown(Some(ctx), WPARAM(VK_ESCAPE.0 as usize), LPARAM(0));
                        assert_eq!(kd_esc.unwrap(), BOOL(1));

                        // 6. Unmapped key -> returns BOOL(0)
                        let kd_unmapped = key_sink.OnKeyDown(Some(ctx), WPARAM(VK_F1.0 as usize), LPARAM(0));
                        assert_eq!(kd_unmapped.unwrap(), BOOL(0));

                        // 7. Ignored key when buffer is empty -> returns BOOL(0)
                        let kd_ignored = key_sink.OnKeyDown(Some(ctx), WPARAM(VK_SPACE.0 as usize), LPARAM(0));
                        assert_eq!(kd_ignored.unwrap(), BOOL(0));

                        // 7b. Direct commit of punctuation (VK_OEM_5 -> '、')
                        let kd_punc = key_sink.OnKeyDown(Some(ctx), WPARAM(VK_OEM_5.0 as usize), LPARAM(0));
                        assert_eq!(kd_punc.unwrap(), BOOL(1));

                        // 7c. Candidate UI mouse finalization callback test
                        let _ = key_sink.OnKeyDown(Some(ctx), WPARAM(b'1' as usize), LPARAM(0));
                        let _ = key_sink.OnKeyDown(Some(ctx), WPARAM(b'J' as usize), LPARAM(0));
                        let _ = key_sink.OnKeyDown(Some(ctx), WPARAM(b'4' as usize), LPARAM(0));
                        let behavior: ITfCandidateListUIElementBehavior = cand_ui.ui_element.cast().unwrap();
                        behavior.SetSelection(0).unwrap();
                        behavior.Finalize().unwrap();

                        // 7d. Punctuation while candidate list is active -> cancels candidates and commits
                        let _ = key_sink.OnKeyDown(Some(ctx), WPARAM(b'1' as usize), LPARAM(0));
                        let _ = key_sink.OnKeyDown(Some(ctx), WPARAM(b'J' as usize), LPARAM(0));
                        let _ = key_sink.OnKeyDown(Some(ctx), WPARAM(b'4' as usize), LPARAM(0));
                        let kd_punc_cands = key_sink.OnKeyDown(Some(ctx), WPARAM(VK_OEM_5.0 as usize), LPARAM(0));
                        assert_eq!(kd_punc_cands.unwrap(), BOOL(1));

                        // 8. Test OnCompositionTerminated with active thread_mgr
                        let comp_sink: ITfCompositionSink = processor.cast().unwrap();
                        assert!(comp_sink.OnCompositionTerminated(0, None).is_ok());

                        let comp_cell = Rc::new(RefCell::new(None));
                        let sess = KeyKeyEditSession::new(
                            ctx.clone(),
                            EditAction::UpdateComposition("ㄅ".into()),
                            comp_cell.clone(),
                            None,
                        );
                        let es: ITfEditSession = sess.into();
                        let _ = ctx.RequestEditSession(client_id, &es, TF_ES_READWRITE | TF_ES_SYNC);
                        let _ = es.DoEditSession(edit_cookie);

                        let deact_res = processor.Deactivate();
                        assert!(deact_res.is_ok());
                    }
                }

                let _ = mgr.Deactivate();
            }
        }
    }

    #[test]
    fn test_shift_mode_toggle_and_compartments() {
        let default_cin = CinTable::load_from_reader(DEFAULT_BPMF_CIN).unwrap();
        let engine = Rc::new(RefCell::new(KeyKeyEngine::new(default_cin)));
        let tip = KeyKeyTip::with_engine(engine.clone());
        let key_sink: ITfKeyEventSink = tip.into();

        assert_eq!(engine.borrow().mode, keykey_core::engine::InputMode::Bopomofo);

        // 1. Press Shift alone -> triggers mode toggle to English on key up
        assert_eq!(unsafe { key_sink.OnKeyDown(None, WPARAM(VK_SHIFT.0 as usize), LPARAM(0)) }.unwrap(), BOOL(0));
        assert_eq!(unsafe { key_sink.OnKeyUp(None, WPARAM(VK_SHIFT.0 as usize), LPARAM(0)) }.unwrap(), BOOL(0));
        assert_eq!(engine.borrow().mode, keykey_core::engine::InputMode::English);

        // While in English mode, OnTestKeyDown and OnKeyDown pass through (BOOL 0)
        assert_eq!(unsafe { key_sink.OnTestKeyDown(None, WPARAM(b'1' as usize), LPARAM(0)) }.unwrap(), BOOL(0));
        assert_eq!(unsafe { key_sink.OnKeyDown(None, WPARAM(b'1' as usize), LPARAM(0)) }.unwrap(), BOOL(0));

        // 2. Press Shift again -> toggles back to Bopomofo
        assert_eq!(unsafe { key_sink.OnKeyDown(None, WPARAM(VK_LSHIFT.0 as usize), LPARAM(0)) }.unwrap(), BOOL(0));
        assert_eq!(unsafe { key_sink.OnKeyUp(None, WPARAM(VK_LSHIFT.0 as usize), LPARAM(0)) }.unwrap(), BOOL(0));
        assert_eq!(engine.borrow().mode, keykey_core::engine::InputMode::Bopomofo);

        // 3. Shift combined with another key -> should NOT toggle mode
        assert_eq!(unsafe { key_sink.OnKeyDown(None, WPARAM(VK_RSHIFT.0 as usize), LPARAM(0)) }.unwrap(), BOOL(0));
        assert_eq!(unsafe { key_sink.OnKeyDown(None, WPARAM(b'1' as usize), LPARAM(0)) }.unwrap(), BOOL(0));
        assert_eq!(unsafe { key_sink.OnKeyUp(None, WPARAM(VK_RSHIFT.0 as usize), LPARAM(0)) }.unwrap(), BOOL(0));
        assert_eq!(engine.borrow().mode, keykey_core::engine::InputMode::Bopomofo);

        // 4. Compartment sink OnChange
        let comp_sink: ITfCompartmentEventSink = key_sink.cast().unwrap();
        assert!(unsafe { comp_sink.OnChange(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE) }.is_ok());
        let bad_guid = GUID::from_u128(0x12345678_0000_0000_0000_000000000000);
        assert!(unsafe { comp_sink.OnChange(&bad_guid) }.is_ok());
        assert!(unsafe { comp_sink.OnChange(std::ptr::null()) }.is_ok());
    }

    #[test]
    fn test_tip_lifecycle() {
        let tip = KeyKeyTip::new();
        let processor: ITfTextInputProcessorEx = tip.into();
        assert!(unsafe { processor.Activate(None, 42) }.is_ok());
        assert!(unsafe { processor.ActivateEx(None, 42, 0) }.is_ok());
        assert!(unsafe { processor.Deactivate() }.is_ok());

        // Test DllMain
        assert_eq!(DllMain(HINSTANCE(std::ptr::null_mut()), DLL_PROCESS_ATTACH, std::ptr::null_mut()), BOOL(1));
        assert_eq!(DllMain(HINSTANCE(std::ptr::null_mut()), DLL_PROCESS_DETACH, std::ptr::null_mut()), BOOL(1));
    }

    #[test]
    fn test_class_factory_and_com_exports() {
        let _ = unsafe { CoInitialize(None) };
        assert_eq!(DllCanUnloadNow(), S_FALSE);
        assert_eq!(dll_can_unload_now(), S_FALSE);

        // Call DllRegisterServer and DllUnregisterServer
        assert_eq!(DllRegisterServer(), S_OK);
        assert_eq!(DllUnregisterServer(), S_OK);
        assert_eq!(dll_register_server(), S_OK);
        assert_eq!(dll_unregister_server(), S_OK);

        // Test DllGetClassObject null arguments
        unsafe {
            assert_eq!(DllGetClassObject(std::ptr::null(), std::ptr::null(), std::ptr::null_mut()), E_INVALIDARG);
            assert_eq!(dll_get_class_object(std::ptr::null(), std::ptr::null(), std::ptr::null_mut()), E_INVALIDARG);

            let bad_guid = GUID::from_u128(0x11111111_2222_3333_4444_555555555555);
            let mut ppv = std::ptr::null_mut();
            assert_eq!(
                DllGetClassObject(&bad_guid, &IClassFactory::IID, &mut ppv),
                CLASS_E_CLASSNOTAVAILABLE
            );
            assert_eq!(
                dll_get_class_object(&bad_guid, &IClassFactory::IID, &mut ppv),
                CLASS_E_CLASSNOTAVAILABLE
            );

            assert_eq!(
                DllGetClassObject(&CLSID_KEYKEY_TEXT_SERVICE, &IClassFactory::IID, &mut ppv),
                S_OK
            );
            assert_eq!(
                dll_get_class_object(&CLSID_KEYKEY_TEXT_SERVICE, &IClassFactory::IID, &mut ppv),
                S_OK
            );
            assert!(!ppv.is_null());

            let factory: IClassFactory = IClassFactory::from_raw(ppv);
            assert_eq!(factory.LockServer(BOOL(1)).unwrap(), ());

            // CreateInstance with punkouter != null
            let dummy_outer: windows::core::IUnknown = factory.cast().unwrap();
            let res: Result<ITfTextInputProcessorEx> = factory.CreateInstance(Some(&dummy_outer));
            assert!(res.is_err());

            // CreateInstance valid
            let res: Result<ITfTextInputProcessorEx> = factory.CreateInstance(None);
            assert!(res.is_ok());
        }
    }

    #[test]
    fn test_anti_hang_reentrancy_resilience() {
        let default_cin = CinTable::load_from_reader(DEFAULT_BPMF_CIN).unwrap();
        let engine = Rc::new(RefCell::new(KeyKeyEngine::new(default_cin)));
        let tip = KeyKeyTip::with_engine(engine.clone());
        let key_sink: ITfKeyEventSink = tip.into();

        // 1. Simulate re-entrant keystroke when engine is already mutably borrowed
        {
            let _engine_guard = engine.borrow_mut();
            unsafe {
                // Must return Ok(BOOL(0)) to pass-through without panic or hanging
                let res_test = key_sink.OnTestKeyDown(None, WPARAM(b'1' as usize), LPARAM(0));
                assert_eq!(res_test.unwrap(), BOOL(0));

                let res_down = key_sink.OnKeyDown(None, WPARAM(b'1' as usize), LPARAM(0));
                assert_eq!(res_down.unwrap(), BOOL(0));

                let res_up = key_sink.OnKeyUp(None, WPARAM(VK_SHIFT.0 as usize), LPARAM(0));
                assert_eq!(res_up.unwrap(), BOOL(0));
            }
        }

        // 2. Simulate re-entrant activation / termination when thread_mgr or composition is mutably borrowed
        let tip2 = KeyKeyTip::new();
        let tm_cell = tip2.thread_mgr.clone();
        let comp_cell = tip2.active_composition.clone();
        assert_eq!(tip2.get_openclose_compartment(), None);
        tip2.set_openclose_compartment(true);

        let proc: ITfTextInputProcessorEx = tip2.into();
        {
            let _tm_guard = tm_cell.borrow_mut();
            let _comp_guard = comp_cell.borrow_mut();

            unsafe {
                assert!(proc.Activate(None, 99).is_ok());
                assert!(proc.Deactivate().is_ok());

                let comp_sink: ITfCompositionSink = proc.cast().unwrap();
                assert!(comp_sink.OnCompositionTerminated(0, None).is_ok());
            }
        }
    }
}
