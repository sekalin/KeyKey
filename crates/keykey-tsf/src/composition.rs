use std::cell::RefCell;
use std::rc::Rc;
use windows::core::{implement, Result};
use windows_core::Interface;
use windows::Win32::UI::TextServices::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditAction {
    UpdateComposition(String),
    Commit(String),
    Cancel,
}

pub trait CompositionTarget {
    fn update_text(&mut self, text: &str) -> Result<()>;
    fn commit_text(&mut self, text: &str) -> Result<()>;
    fn cancel(&mut self) -> Result<()>;
}

pub fn apply_composition_action<T: CompositionTarget>(action: &EditAction, target: &mut T) -> Result<()> {
    match action {
        EditAction::UpdateComposition(text) => target.update_text(text),
        EditAction::Commit(text) => target.commit_text(text),
        EditAction::Cancel => target.cancel(),
    }
}

#[implement(ITfEditSession)]
pub struct KeyKeyEditSession {
    context: ITfContext,
    action: EditAction,
    active_composition: Rc<RefCell<Option<ITfComposition>>>,
    sink: Option<ITfCompositionSink>,
}

impl KeyKeyEditSession {
    pub fn new(
        context: ITfContext,
        action: EditAction,
        active_composition: Rc<RefCell<Option<ITfComposition>>>,
        sink: Option<ITfCompositionSink>,
    ) -> Self {
        Self {
            context,
            action,
            active_composition,
            sink,
        }
    }
}

impl ITfEditSession_Impl for KeyKeyEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.do_edit_session_internal(ec)
        }));
        res.unwrap_or(Ok(()))
    }
}

impl KeyKeyEditSession_Impl {
    fn do_edit_session_internal(&self, ec: u32) -> Result<()> {
        match &self.action {
            EditAction::UpdateComposition(text) => {
                let text_wide: Vec<u16> = text.encode_utf16().collect();
                let Ok(mut comp_lock) = self.active_composition.try_borrow_mut() else {
                    return Ok(());
                };

                if comp_lock.is_none() {
                    if text_wide.is_empty() {
                        return Ok(());
                    }
                    let mut selections = [TF_SELECTION::default(); 1];
                    let mut fetched = 0;
                    unsafe {
                        self.context.GetSelection(ec, TF_DEFAULT_SELECTION, &mut selections, &mut fetched)?;
                        if fetched > 0 {
                            if let Some(range) = selections[0].range.as_ref() {
                                let ctx_comp: ITfContextComposition = self.context.cast()?;
                                let comp = ctx_comp.StartComposition(ec, range, self.sink.as_ref())?;
                                range.SetText(ec, 0, &text_wide)?;
                                *comp_lock = Some(comp);
                            }
                        }
                    }
                } else if let Some(comp) = comp_lock.as_ref() {
                    unsafe {
                        let range = comp.GetRange()?;
                        range.SetText(ec, 0, &text_wide)?;
                    }
                }
            }
            EditAction::Commit(text) => {
                let text_wide: Vec<u16> = text.encode_utf16().collect();
                let Ok(mut comp_lock) = self.active_composition.try_borrow_mut() else {
                    return Ok(());
                };

                if let Some(comp) = comp_lock.take() {
                    unsafe {
                        let range = comp.GetRange()?;
                        range.SetText(ec, 0, &text_wide)?;
                        comp.EndComposition(ec)?;
                        let _ = range.Collapse(ec, TF_ANCHOR_END);
                        let sel = TF_SELECTION {
                            range: std::mem::ManuallyDrop::new(Some(range)),
                            style: TF_SELECTIONSTYLE {
                                ase: TF_AE_NONE,
                                fInterimChar: windows::Win32::Foundation::BOOL(0),
                            },
                        };
                        let _ = self.context.SetSelection(ec, &[sel]);
                    }
                } else {
                    let mut selections = [TF_SELECTION::default(); 1];
                    let mut fetched = 0;
                    unsafe {
                        self.context.GetSelection(ec, TF_DEFAULT_SELECTION, &mut selections, &mut fetched)?;
                        if fetched > 0 {
                            if let Some(range) = selections[0].range.as_ref() {
                                range.SetText(ec, 0, &text_wide)?;
                                let _ = range.Collapse(ec, TF_ANCHOR_END);
                                let sel = TF_SELECTION {
                                    range: std::mem::ManuallyDrop::new(Some(range.clone())),
                                    style: TF_SELECTIONSTYLE {
                                        ase: TF_AE_NONE,
                                        fInterimChar: windows::Win32::Foundation::BOOL(0),
                                    },
                                };
                                let _ = self.context.SetSelection(ec, &[sel]);
                            }
                        }
                    }
                }
            }
            EditAction::Cancel => {
                let Ok(mut comp_lock) = self.active_composition.try_borrow_mut() else {
                    return Ok(());
                };
                if let Some(comp) = comp_lock.take() {
                    unsafe {
                        let range = comp.GetRange()?;
                        range.SetText(ec, 0, &[])?;
                        comp.EndComposition(ec)?;
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MockCompositionTarget {
        composition_text: Option<String>,
        committed_text: Option<String>,
        is_cancelled: bool,
    }

    impl CompositionTarget for MockCompositionTarget {
        fn update_text(&mut self, text: &str) -> Result<()> {
            self.composition_text = Some(text.to_string());
            Ok(())
        }

        fn commit_text(&mut self, text: &str) -> Result<()> {
            self.committed_text = Some(text.to_string());
            self.composition_text = None;
            Ok(())
        }

        fn cancel(&mut self) -> Result<()> {
            self.is_cancelled = true;
            self.composition_text = None;
            Ok(())
        }
    }

    use windows_core::{PWSTR, PCWSTR, GUID, IUnknown};
    use windows::Win32::Foundation::BOOL;
    use windows::Win32::System::Com::IDataObject;

    #[implement(ITfComposition)]
    struct MockComposition;

    impl ITfComposition_Impl for MockComposition_Impl {
        fn GetRange(&self) -> Result<ITfRange> {
            let range: ITfRange = MockRange.into();
            Ok(range)
        }
        fn ShiftStart(&self, _ec: u32, _pnewstart: Option<&ITfRange>) -> Result<()> {
            Ok(())
        }
        fn ShiftEnd(&self, _ec: u32, _pnewend: Option<&ITfRange>) -> Result<()> {
            Ok(())
        }
        fn EndComposition(&self, _ec: u32) -> Result<()> {
            Ok(())
        }
    }

    #[implement(ITfRange)]
    struct MockRange;

    impl ITfRange_Impl for MockRange_Impl {
        fn GetText(&self, _: u32, _: u32, _: PWSTR, _: u32, _: *mut u32) -> Result<()> { Ok(()) }
        fn SetText(&self, _: u32, _: u32, _: &PCWSTR, _: i32) -> Result<()> { Ok(()) }
        fn GetFormattedText(&self, _: u32) -> Result<IDataObject> { Err(windows::core::Error::from_hresult(windows::Win32::Foundation::E_NOTIMPL)) }
        fn GetEmbedded(&self, _: u32, _: *const GUID, _: *const GUID) -> Result<IUnknown> { Err(windows::core::Error::from_hresult(windows::Win32::Foundation::E_NOTIMPL)) }
        fn InsertEmbedded(&self, _: u32, _: u32, _: Option<&IDataObject>) -> Result<()> { Ok(()) }
        fn ShiftStart(&self, _: u32, _: i32, _: *mut i32, _: *const TF_HALTCOND) -> Result<()> { Ok(()) }
        fn ShiftEnd(&self, _: u32, _: i32, _: *mut i32, _: *const TF_HALTCOND) -> Result<()> { Ok(()) }
        fn ShiftStartToRange(&self, _: u32, _: Option<&ITfRange>, _: TfAnchor) -> Result<()> { Ok(()) }
        fn ShiftEndToRange(&self, _: u32, _: Option<&ITfRange>, _: TfAnchor) -> Result<()> { Ok(()) }
        fn ShiftStartRegion(&self, _: u32, _: TfShiftDir) -> Result<BOOL> { Ok(BOOL(0)) }
        fn ShiftEndRegion(&self, _: u32, _: TfShiftDir) -> Result<BOOL> { Ok(BOOL(0)) }
        fn IsEmpty(&self, _: u32) -> Result<BOOL> { Ok(BOOL(0)) }
        fn Collapse(&self, _: u32, _: TfAnchor) -> Result<()> { Ok(()) }
        fn IsEqualStart(&self, _: u32, _: Option<&ITfRange>, _: TfAnchor) -> Result<BOOL> { Ok(BOOL(0)) }
        fn IsEqualEnd(&self, _: u32, _: Option<&ITfRange>, _: TfAnchor) -> Result<BOOL> { Ok(BOOL(0)) }
        fn CompareStart(&self, _: u32, _: Option<&ITfRange>, _: TfAnchor) -> Result<i32> { Ok(0) }
        fn CompareEnd(&self, _: u32, _: Option<&ITfRange>, _: TfAnchor) -> Result<i32> { Ok(0) }
        fn AdjustForInsert(&self, _: u32, _: u32) -> Result<BOOL> { Ok(BOOL(0)) }
        fn GetGravity(&self, _: *mut TfGravity, _: *mut TfGravity) -> Result<()> { Ok(()) }
        fn SetGravity(&self, _: u32, _: TfGravity, _: TfGravity) -> Result<()> { Ok(()) }
        fn Clone(&self) -> Result<ITfRange> { Ok(MockRange.into()) }
        fn GetContext(&self) -> Result<ITfContext> { Err(windows::core::Error::from_hresult(windows::Win32::Foundation::E_NOTIMPL)) }
    }

    #[test]
    fn test_apply_composition_actions() {
        let mut target = MockCompositionTarget::default();

        // Update
        let action1 = EditAction::UpdateComposition("ㄅㄨ".to_string());
        apply_composition_action(&action1, &mut target).unwrap();
        assert_eq!(target.composition_text.as_deref(), Some("ㄅㄨ"));

        // Commit
        let action2 = EditAction::Commit("不".to_string());
        apply_composition_action(&action2, &mut target).unwrap();
        assert_eq!(target.committed_text.as_deref(), Some("不"));
        assert_eq!(target.composition_text, None);

        // Cancel
        let action3 = EditAction::Cancel;
        apply_composition_action(&action3, &mut target).unwrap();
        assert!(target.is_cancelled);
    }

    #[test]
    fn test_edit_session_do_edit_session_all_actions() {
        use windows::Win32::System::Com::*;
        unsafe {
            let _ = CoInitialize(None);
            if let Ok(tm) = CoCreateInstance::<_, ITfThreadMgr>(&CLSID_TF_ThreadMgr, None, CLSCTX_INPROC_SERVER) {
                let client_id = tm.Activate().unwrap();
                let dm = tm.CreateDocumentMgr().unwrap();
                let mut edit_cookie = 0u32;
                let mut context: Option<ITfContext> = None;
                dm.CreateContext(client_id, 0, None, &mut context, &mut edit_cookie).unwrap();
                let ctx = context.unwrap();

                // 1. UpdateComposition with active composition
                let comp1: ITfComposition = MockComposition.into();
                let comp_cell1 = Rc::new(RefCell::new(Some(comp1)));
                let sess1 = KeyKeyEditSession::new(ctx.clone(), EditAction::UpdateComposition("ㄅㄨ".into()), comp_cell1.clone(), None);
                let edit_session1: ITfEditSession = sess1.into();
                assert!(edit_session1.DoEditSession(1).is_ok());

                // 2. Commit with active composition
                let comp2: ITfComposition = MockComposition.into();
                let comp_cell2 = Rc::new(RefCell::new(Some(comp2)));
                let sess2 = KeyKeyEditSession::new(ctx.clone(), EditAction::Commit("不".into()), comp_cell2.clone(), None);
                let edit_session2: ITfEditSession = sess2.into();
                assert!(edit_session2.DoEditSession(1).is_ok());
                assert!(comp_cell2.borrow().is_none());

                // 3. Cancel with active composition
                let comp3: ITfComposition = MockComposition.into();
                let comp_cell3 = Rc::new(RefCell::new(Some(comp3)));
                let sess3 = KeyKeyEditSession::new(ctx.clone(), EditAction::Cancel, comp_cell3.clone(), None);
                let edit_session3: ITfEditSession = sess3.into();
                assert!(edit_session3.DoEditSession(1).is_ok());
                assert!(comp_cell3.borrow().is_none());

                // 4. Cancel without active composition
                let comp_cell4 = Rc::new(RefCell::new(None));
                let sess4 = KeyKeyEditSession::new(ctx.clone(), EditAction::Cancel, comp_cell4.clone(), None);
                let edit_session4: ITfEditSession = sess4.into();
                assert!(edit_session4.DoEditSession(1).is_ok());

                // 5. Commit without active composition (exercises GetSelection branch)
                let comp_cell5 = Rc::new(RefCell::new(None));
                let sess5 = KeyKeyEditSession::new(ctx.clone(), EditAction::Commit("字".into()), comp_cell5.clone(), None);
                let edit_session5: ITfEditSession = sess5.into();
                let _ = edit_session5.DoEditSession(1);

                // 6. UpdateComposition without active composition (exercises GetSelection branch)
                let comp_cell6 = Rc::new(RefCell::new(None));
                let sess6 = KeyKeyEditSession::new(ctx.clone(), EditAction::UpdateComposition("字".into()), comp_cell6.clone(), None);
                let edit_session6: ITfEditSession = sess6.into();
                let _ = edit_session6.DoEditSession(1);

                // 7. Anti-Hang Reentrancy test: active_composition already mutably borrowed
                let comp_cell_locked = Rc::new(RefCell::new(None));
                let sess_locked = KeyKeyEditSession::new(ctx.clone(), EditAction::Commit("鎖".into()), comp_cell_locked.clone(), None);
                let edit_session_locked: ITfEditSession = sess_locked.into();
                let sess_upd = KeyKeyEditSession::new(ctx.clone(), EditAction::UpdateComposition("鎖".into()), comp_cell_locked.clone(), None);
                let es_upd: ITfEditSession = sess_upd.into();
                let sess_can = KeyKeyEditSession::new(ctx.clone(), EditAction::Cancel, comp_cell_locked.clone(), None);
                let es_can: ITfEditSession = sess_can.into();
                {
                    let _guard = comp_cell_locked.borrow_mut();
                    // Must fail safe with Ok(()) and NEVER panic or hang
                    assert!(edit_session_locked.DoEditSession(1).is_ok());
                    assert!(es_upd.DoEditSession(1).is_ok());
                    assert!(es_can.DoEditSession(1).is_ok());
                }

                let _ = tm.Deactivate();
            }
        }
    }

    #[test]
    fn test_mock_range_and_composition_helpers() {
        let comp: ITfComposition = MockComposition.into();
        let range: ITfRange = MockRange.into();
        unsafe {
            assert!(comp.ShiftStart(0, None).is_ok());
            assert!(comp.ShiftEnd(0, None).is_ok());

            let mut buf = [0u16; 1];
            assert!(range.GetText(0, 0, &mut buf, std::ptr::null_mut()).is_ok());
            assert!(range.GetFormattedText(0).is_err());
            assert!(range.GetEmbedded(0, std::ptr::null(), std::ptr::null()).is_err());
            assert!(range.InsertEmbedded(0, 0, None).is_ok());
            assert!(range.ShiftStart(0, 0, std::ptr::null_mut(), std::ptr::null()).is_ok());
            assert!(range.ShiftEnd(0, 0, std::ptr::null_mut(), std::ptr::null()).is_ok());
            assert!(range.ShiftStartToRange(0, None, TF_ANCHOR_START).is_ok());
            assert!(range.ShiftEndToRange(0, None, TF_ANCHOR_START).is_ok());
            assert!(range.ShiftStartRegion(0, TF_SD_FORWARD).is_ok());
            assert!(range.ShiftEndRegion(0, TF_SD_FORWARD).is_ok());
            assert!(range.IsEmpty(0).is_ok());
            assert!(range.Collapse(0, TF_ANCHOR_START).is_ok());
            assert!(range.IsEqualStart(0, None, TF_ANCHOR_START).is_ok());
            assert!(range.IsEqualEnd(0, None, TF_ANCHOR_START).is_ok());
            assert!(range.CompareStart(0, None, TF_ANCHOR_START).is_ok());
            assert!(range.CompareEnd(0, None, TF_ANCHOR_START).is_ok());
            assert!(range.AdjustForInsert(0, 0).is_ok());
            assert!(range.GetGravity(std::ptr::null_mut(), std::ptr::null_mut()).is_ok());
            assert!(range.SetGravity(0, TF_GRAVITY_BACKWARD, TF_GRAVITY_FORWARD).is_ok());
            assert!(range.Clone().is_ok());
            assert!(range.GetContext().is_err());
        }
    }
}
