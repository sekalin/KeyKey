//! KeyKey Windows TSF (Text Services Framework) Text Input Processor (TIP)

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

#[implement(ITfTextInputProcessorEx, ITfKeyEventSink, ITfCompositionSink)]
pub struct KeyKeyTip {
    client_id: Cell<u32>,
    thread_mgr: RefCell<Option<ITfThreadMgr>>,
    engine: RefCell<KeyKeyEngine>,
    active_composition: Rc<RefCell<Option<ITfComposition>>>,
}

impl KeyKeyTip {
    pub fn new() -> Self {
        // Load default CinTable from embedded bpmf.cin
        let cin_table = CinTable::load_from_reader(DEFAULT_BPMF_CIN)
            .unwrap_or_else(|_| CinTable::new());

        let engine = KeyKeyEngine::new(cin_table);

        Self {
            client_id: Cell::new(0),
            thread_mgr: RefCell::new(None),
            engine: RefCell::new(engine),
            active_composition: Rc::new(RefCell::new(None)),
        }
    }

    fn map_virtual_key(&self, wparam: WPARAM) -> Option<KeyCode> {
        let vk = wparam.0 as u16;
        match VIRTUAL_KEY(vk) {
            VK_BACK => Some(KeyCode::Backspace),
            VK_RETURN => Some(KeyCode::Enter),
            VK_ESCAPE => Some(KeyCode::Escape),
            VK_SPACE => Some(KeyCode::Space),
            VK_DOWN => Some(KeyCode::Down),
            VK_UP => Some(KeyCode::Up),
            VK_PRIOR => Some(KeyCode::PageUp),
            VK_NEXT => Some(KeyCode::PageDown),
            VK_OEM_COMMA => Some(KeyCode::Char(',')),
            VK_OEM_PERIOD => Some(KeyCode::Char('.')),
            VK_OEM_1 => Some(KeyCode::Char(';')),
            VK_OEM_2 => Some(KeyCode::Char('/')),
            VK_OEM_MINUS => Some(KeyCode::Char('-')),
            _ => {
                if (b'0'..=b'9').contains(&(vk as u8)) {
                    Some(KeyCode::Char((vk as u8) as char))
                } else if (b'A'..=b'Z').contains(&(vk as u8)) {
                    // Convert to lowercase
                    let ch = ((vk as u8) + 32) as char;
                    Some(KeyCode::Char(ch))
                } else {
                    None
                }
            }
        }
    }
}

impl ITfTextInputProcessor_Impl for KeyKeyTip_Impl {
    fn Activate(&self, ptim: Option<&ITfThreadMgr>, tid: u32) -> Result<()> {
        self.client_id.set(tid);
        *self.thread_mgr.borrow_mut() = ptim.cloned();

        if let Some(tm) = ptim {
            unsafe {
                let keystroke_mgr: ITfKeystrokeMgr = tm.cast()?;
                let key_sink: ITfKeyEventSink = self.cast()?;
                keystroke_mgr.AdviseKeyEventSink(tid, &key_sink, true)?;
            }
        }
        Ok(())
    }

    fn Deactivate(&self) -> Result<()> {
        let tid = self.client_id.get();
        if let Some(tm) = self.thread_mgr.borrow_mut().take() {
            unsafe {
                if let Ok(keystroke_mgr) = tm.cast::<ITfKeystrokeMgr>() {
                    let _ = keystroke_mgr.UnadviseKeyEventSink(tid);
                }
            }
        }
        self.engine.borrow_mut().reset();
        *self.active_composition.borrow_mut() = None;
        Ok(())
    }
}

impl ITfTextInputProcessorEx_Impl for KeyKeyTip_Impl {
    fn ActivateEx(&self, ptim: Option<&ITfThreadMgr>, tid: u32, _dwflags: u32) -> Result<()> {
        self.Activate(ptim, tid)
    }
}

impl ITfCompositionSink_Impl for KeyKeyTip_Impl {
    fn OnCompositionTerminated(
        &self,
        _ec: u32,
        _pcomposition: Option<&ITfComposition>,
    ) -> Result<()> {
        self.engine.borrow_mut().reset();
        *self.active_composition.borrow_mut() = None;
        Ok(())
    }
}

impl ITfKeyEventSink_Impl for KeyKeyTip_Impl {
    fn OnSetFocus(&self, _fforeground: BOOL) -> Result<()> {
        Ok(())
    }

    fn OnTestKeyDown(
        &self,
        _pic: Option<&ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        let engine = self.engine.borrow();
        if let Some(keycode) = self.map_virtual_key(wparam) {
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

        Ok(BOOL(0))
    }

    fn OnKeyDown(
        &self,
        pic: Option<&ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        let Some(context) = pic else {
            return Ok(BOOL(0));
        };

        let Some(keycode) = self.map_virtual_key(wparam) else {
            return Ok(BOOL(0));
        };

        let actions = self.engine.borrow_mut().handle_key(keycode);
        if actions.is_empty() || actions == vec![EngineAction::Ignored] {
            return Ok(BOOL(0));
        }

        let sink: Option<ITfCompositionSink> = unsafe { self.cast().ok() };
        let tid = self.client_id.get();

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
                EngineAction::HideCandidates => {
                    if self.engine.borrow().syllable.is_empty() {
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

        Ok(BOOL(1))
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
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        Ok(BOOL(0))
    }

    fn OnPreservedKey(
        &self,
        _pic: Option<&ITfContext>,
        _rguid: *const GUID,
    ) -> Result<BOOL> {
        Ok(BOOL(0))
    }
}

#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
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

#[no_mangle]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    S_FALSE
}

#[no_mangle]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    register::register_server().into()
}

#[no_mangle]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    register::unregister_server().into()
}
