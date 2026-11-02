//! KeyKey Windows TSF (Text Services Framework) Text Input Processor (TIP)

use windows::core::{implement, Result, GUID, HRESULT};
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::UI::TextServices::*;

// KeyKey Text Service CLSID: {B892F1C2-88E4-4866-96D5-812C465EB86B}
pub const CLSID_KEYKEY_TEXT_SERVICE: GUID =
    GUID::from_u128(0xB892F1C2_88E4_4866_96D5_812C465EB86B);

// KeyKey Profile GUID: {6786A76E-6F3C-4318-A785-5AE4E8251268}
pub const GUID_KEYKEY_PROFILE: GUID =
    GUID::from_u128(0x6786A76E_6F3C_4318_A785_5AE4E8251268);

#[implement(ITfTextInputProcessorEx, ITfKeyEventSink)]
pub struct KeyKeyTip {
    client_id: u32,
}

impl KeyKeyTip {
    pub fn new() -> Self {
        Self { client_id: 0 }
    }
}

impl ITfTextInputProcessor_Impl for KeyKeyTip_Impl {
    fn Activate(&self, _ptim: Option<&ITfThreadMgr>, _tid: u32) -> Result<()> {
        Ok(())
    }

    fn Deactivate(&self) -> Result<()> {
        Ok(())
    }
}

impl ITfTextInputProcessorEx_Impl for KeyKeyTip_Impl {
    fn ActivateEx(&self, ptim: Option<&ITfThreadMgr>, tid: u32, _dwflags: u32) -> Result<()> {
        self.Activate(ptim, tid)
    }
}

impl ITfKeyEventSink_Impl for KeyKeyTip_Impl {
    fn OnSetFocus(&self, _fforeground: BOOL) -> Result<()> {
        Ok(())
    }

    fn OnTestKeyDown(
        &self,
        _pic: Option<&ITfContext>,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        Ok(BOOL(0))
    }

    fn OnKeyDown(
        &self,
        _pic: Option<&ITfContext>,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        Ok(BOOL(0))
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
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    S_FALSE
}
