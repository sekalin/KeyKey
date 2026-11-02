use std::cell::RefCell;
use std::rc::Rc;
use windows::core::{implement, Result};
use windows_core::Interface;
use windows::Win32::UI::TextServices::*;

#[derive(Debug, Clone)]
pub enum EditAction {
    UpdateComposition(String),
    Commit(String),
    Cancel,
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
        match &self.action {
            EditAction::UpdateComposition(text) => {
                let text_wide: Vec<u16> = text.encode_utf16().collect();
                let mut comp_lock = self.active_composition.borrow_mut();

                if comp_lock.is_none() {
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
                let mut comp_lock = self.active_composition.borrow_mut();

                if let Some(comp) = comp_lock.take() {
                    unsafe {
                        let range = comp.GetRange()?;
                        range.SetText(ec, 0, &text_wide)?;
                        comp.EndComposition(ec)?;
                    }
                } else {
                    let mut selections = [TF_SELECTION::default(); 1];
                    let mut fetched = 0;
                    unsafe {
                        self.context.GetSelection(ec, TF_DEFAULT_SELECTION, &mut selections, &mut fetched)?;
                        if fetched > 0 {
                            if let Some(range) = selections[0].range.as_ref() {
                                range.SetText(ec, 0, &text_wide)?;
                            }
                        }
                    }
                }
            }
            EditAction::Cancel => {
                let mut comp_lock = self.active_composition.borrow_mut();
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
