use std::cell::{Cell, RefCell};
use std::rc::Rc;
use windows::core::{implement, BSTR, Result, GUID};
use windows_core::Interface;
use windows::Win32::Foundation::*;
use windows::Win32::UI::TextServices::*;

pub const GUID_KEYKEY_CANDIDATE_UI_ELEMENT: GUID =
    GUID::from_u128(0xC58E3406_5C4F_4C23_8123_09DA3008E123);

#[derive(Default)]
pub struct CandidateData {
    pub candidates: RefCell<Vec<String>>,
    pub current_page: Cell<u32>,
    pub page_size: usize,
    pub selection_index: Cell<u32>,
    pub updated_flags: Cell<u32>,
    pub is_shown: Cell<bool>,
}

#[implement(ITfUIElement, ITfCandidateListUIElement, ITfCandidateListUIElementBehavior)]
pub struct CandidateUIElement {
    guid: GUID,
    data: Rc<CandidateData>,
}

impl CandidateUIElement {
    pub fn new(data: Rc<CandidateData>) -> Self {
        Self {
            guid: GUID_KEYKEY_CANDIDATE_UI_ELEMENT,
            data,
        }
    }
}

impl ITfUIElement_Impl for CandidateUIElement_Impl {
    fn GetDescription(&self) -> Result<BSTR> {
        Ok(BSTR::from("KeyKey Candidate List"))
    }

    fn GetGUID(&self) -> Result<GUID> {
        Ok(self.guid)
    }

    fn Show(&self, bshow: BOOL) -> Result<()> {
        self.data.is_shown.set(bshow.as_bool());
        Ok(())
    }

    fn IsShown(&self) -> Result<BOOL> {
        Ok(BOOL(self.data.is_shown.get() as i32))
    }
}

impl ITfCandidateListUIElement_Impl for CandidateUIElement_Impl {
    fn GetUpdatedFlags(&self) -> Result<u32> {
        Ok(self.data.updated_flags.get())
    }

    fn GetDocumentMgr(&self) -> Result<ITfDocumentMgr> {
        Err(E_NOTIMPL.into())
    }

    fn GetCount(&self) -> Result<u32> {
        Ok(self.data.candidates.borrow().len() as u32)
    }

    fn GetSelection(&self) -> Result<u32> {
        Ok(self.data.selection_index.get())
    }

    fn GetString(&self, uindex: u32) -> Result<BSTR> {
        let list = self.data.candidates.borrow();
        if let Some(cand) = list.get(uindex as usize) {
            Ok(BSTR::from(cand.as_str()))
        } else {
            Ok(BSTR::default())
        }
    }

    fn GetPageIndex(
        &self,
        pindex: *mut u32,
        usize_count: u32,
        pupagesize: *mut u32,
    ) -> Result<()> {
        let page_size = self.data.page_size as u32;
        if !pupagesize.is_null() {
            unsafe {
                *pupagesize = page_size;
            }
        }
        if !pindex.is_null() {
            for i in 0..usize_count {
                unsafe {
                    *pindex.add(i as usize) = i * page_size;
                }
            }
        }
        Ok(())
    }

    fn SetPageIndex(&self, _pindex: *const u32, _usize: u32) -> Result<()> {
        Ok(())
    }

    fn GetCurrentPage(&self) -> Result<u32> {
        Ok(self.data.current_page.get())
    }
}

impl ITfCandidateListUIElementBehavior_Impl for CandidateUIElement_Impl {
    fn SetSelection(&self, nindex: u32) -> Result<()> {
        self.data.selection_index.set(nindex);
        Ok(())
    }

    fn Finalize(&self) -> Result<()> {
        Ok(())
    }

    fn Abort(&self) -> Result<()> {
        Ok(())
    }
}

pub struct CandidateUIManager {
    data: Rc<CandidateData>,
    ui_element: ITfCandidateListUIElement,
    ui_element_id: Cell<Option<u32>>,
}

impl CandidateUIManager {
    pub fn new() -> Self {
        let data = Rc::new(CandidateData {
            candidates: RefCell::new(Vec::new()),
            current_page: Cell::new(0),
            page_size: 9,
            selection_index: Cell::new(0),
            updated_flags: Cell::new(0),
            is_shown: Cell::new(false),
        });

        let element = CandidateUIElement::new(data.clone());
        let ui_element: ITfCandidateListUIElement = element.into();

        Self {
            data,
            ui_element,
            ui_element_id: Cell::new(None),
        }
    }

    pub fn is_visible(&self) -> bool {
        self.ui_element_id.get().is_some()
    }

    pub fn show(
        &self,
        thread_mgr: &ITfThreadMgr,
        candidates: Vec<String>,
        page: usize,
    ) -> Result<()> {
        *self.data.candidates.borrow_mut() = candidates;
        self.data.current_page.set(page as u32);
        self.data.selection_index.set(0);
        self.data.updated_flags.set(
            TF_CLUIE_COUNT
                | TF_CLUIE_SELECTION
                | TF_CLUIE_STRING
                | TF_CLUIE_PAGEINDEX
                | TF_CLUIE_CURRENTPAGE,
        );

        let ui_mgr: ITfUIElementMgr = thread_mgr.cast()?;
        let ui_element: ITfUIElement = self.ui_element.cast()?;

        if let Some(id) = self.ui_element_id.get() {
            unsafe {
                ui_mgr.UpdateUIElement(id)?;
            }
        } else {
            let mut show = BOOL(1);
            let mut id = 0u32;
            unsafe {
                ui_mgr.BeginUIElement(&ui_element, &mut show, &mut id)?;
            }
            self.ui_element_id.set(Some(id));
        }
        Ok(())
    }

    pub fn update_page(&self, thread_mgr: &ITfThreadMgr, page: usize) -> Result<()> {
        self.data.current_page.set(page as u32);
        self.data.updated_flags.set(TF_CLUIE_CURRENTPAGE | TF_CLUIE_SELECTION);

        if let Some(id) = self.ui_element_id.get() {
            let ui_mgr: ITfUIElementMgr = thread_mgr.cast()?;
            unsafe {
                ui_mgr.UpdateUIElement(id)?;
            }
        }
        Ok(())
    }

    pub fn close(&self, thread_mgr: &ITfThreadMgr) -> Result<()> {
        if let Some(id) = self.ui_element_id.take() {
            let ui_mgr: ITfUIElementMgr = thread_mgr.cast()?;
            unsafe {
                let _ = ui_mgr.EndUIElement(id);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candidate_ui_data() {
        let mgr = CandidateUIManager::new();
        let test_cands: Vec<String> = (1..=12).map(|i| format!("字_{}", i)).collect();

        // Simulate set candidates
        mgr.data.candidates.borrow_mut().clone_from(&test_cands);
        mgr.data.current_page.set(0);

        unsafe {
            assert_eq!(mgr.ui_element.GetCount().unwrap(), 12);
            assert_eq!(mgr.ui_element.GetUpdatedFlags().unwrap(), 0);
            assert_eq!(mgr.ui_element.GetString(0).unwrap().to_string(), "字_1");
            assert_eq!(mgr.ui_element.GetString(8).unwrap().to_string(), "字_9");
            assert_eq!(mgr.ui_element.GetString(999).unwrap().to_string(), "");
            assert_eq!(mgr.ui_element.GetCurrentPage().unwrap(), 0);

            let desc = mgr.ui_element.cast::<ITfUIElement>().unwrap().GetDescription().unwrap();
            assert_eq!(desc.to_string(), "KeyKey Candidate List");

            let guid = mgr.ui_element.cast::<ITfUIElement>().unwrap().GetGUID().unwrap();
            assert_eq!(guid, GUID_KEYKEY_CANDIDATE_UI_ELEMENT);

            let ui_el = mgr.ui_element.cast::<ITfUIElement>().unwrap();
            ui_el.Show(BOOL(1)).unwrap();
            assert_eq!(ui_el.IsShown().unwrap(), BOOL(1));
            ui_el.Show(BOOL(0)).unwrap();
            assert_eq!(ui_el.IsShown().unwrap(), BOOL(0));

            assert_eq!(mgr.ui_element.GetDocumentMgr().is_err(), true);

            // Behavior interface
            let behavior = mgr.ui_element.cast::<ITfCandidateListUIElementBehavior>().unwrap();
            behavior.SetSelection(3).unwrap();
            assert_eq!(mgr.ui_element.GetSelection().unwrap(), 3);
            behavior.Finalize().unwrap();
            behavior.Abort().unwrap();

            mgr.ui_element.SetPageIndex(&[]).unwrap();

            // Test page index array calculation
            let mut page_indices = [0u32; 2];
            let mut page_size = 0u32;
            mgr.ui_element
                .GetPageIndex(&mut page_indices, &mut page_size)
                .unwrap();

            assert_eq!(page_size, 9);
            assert_eq!(page_indices[0], 0);  // Page 0 starts at index 0
            assert_eq!(page_indices[1], 9);  // Page 1 starts at index 9

            // Test null pointer handling
            mgr.ui_element
                .GetPageIndex(&mut [], std::ptr::null_mut())
                .unwrap();

            assert!(!mgr.is_visible());
        }
    }

    #[test]
    fn test_candidate_ui_lifecycle() {
        use windows::Win32::System::Com::*;
        unsafe {
            let _ = CoInitialize(None);
            if let Ok(tm) = CoCreateInstance::<_, ITfThreadMgr>(&CLSID_TF_ThreadMgr, None, CLSCTX_INPROC_SERVER) {
                let mgr = CandidateUIManager::new();
                assert!(!mgr.is_visible());

                // Test show (first time: BeginUIElement)
                let cands = vec!["候選字1".to_string(), "候選字2".to_string()];
                let show_res = mgr.show(&tm, cands.clone(), 0);
                assert!(show_res.is_ok());

                // Test update_page
                let upd_res = mgr.update_page(&tm, 1);
                assert!(upd_res.is_ok());

                // Test show when already visible (UpdateUIElement)
                let show_again_res = mgr.show(&tm, cands, 1);
                assert!(show_again_res.is_ok());

                // Test close (EndUIElement)
                let close_res = mgr.close(&tm);
                assert!(close_res.is_ok());
                assert!(!mgr.is_visible());

                // Test close when already closed
                let close_again = mgr.close(&tm);
                assert!(close_again.is_ok());
            }
        }
    }
}
