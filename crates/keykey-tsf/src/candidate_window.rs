use std::cell::RefCell;
use std::rc::Rc;
use windows::core::{w, PCWSTR, Result};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::TextServices::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const CLASS_NAME: PCWSTR = w!("KeyKey_CandidateWindow");

#[derive(Default, Clone)]
pub struct CandidateItemRect {
    pub index: u32,
    pub rect: RECT,
}

pub struct CandidateWindowData {
    pub candidates: Vec<String>,
    pub current_page: u32,
    pub page_size: usize,
    pub selection_index: u32,
    pub item_rects: Vec<CandidateItemRect>,
    pub on_select: Option<Rc<dyn Fn(u32)>>,
}

impl Default for CandidateWindowData {
    fn default() -> Self {
        Self {
            candidates: Vec::new(),
            current_page: 0,
            page_size: 9,
            selection_index: 0,
            item_rects: Vec::new(),
            on_select: None,
        }
    }
}

pub struct CustomCandidateWindow {
    hwnd: HWND,
    data: Rc<RefCell<CandidateWindowData>>,
}

pub fn clamp_to_monitor(mut target_x: i32, mut target_y: i32, win_width: i32, win_height: i32) -> (i32, i32) {
    unsafe {
        let hmon = MonitorFromPoint(
            POINT {
                x: target_x,
                y: target_y,
            },
            MONITOR_DEFAULTTONEAREST,
        );
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(hmon, &mut mi).as_bool() {
            if target_x + win_width > mi.rcWork.right {
                target_x = mi.rcWork.right - win_width - 8;
            }
            if target_x < mi.rcWork.left {
                target_x = mi.rcWork.left + 8;
            }
            if target_y + win_height > mi.rcWork.bottom {
                // Flip vertically above caret
                target_y = (target_y - win_height - 30).max(mi.rcWork.top + 8);
            }
            if target_y < mi.rcWork.top {
                target_y = mi.rcWork.top + 8;
            }
        }
        (target_x, target_y)
    }
}

impl CustomCandidateWindow {
    pub fn new() -> Result<Self> {
        let data = Rc::new(RefCell::new(CandidateWindowData::default()));
        let hwnd = Self::create_window(data.clone())?;
        Ok(Self { hwnd, data })
    }

    fn create_window(data: Rc<RefCell<CandidateWindowData>>) -> Result<HWND> {
        let hinstance = unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None)? };

        unsafe {
            let mut wc = WNDCLASSEXW::default();
            wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
            wc.style = CS_HREDRAW | CS_VREDRAW | CS_DROPSHADOW;
            wc.lpfnWndProc = Some(candidate_wndproc);
            wc.hInstance = hinstance.into();
            wc.hCursor = LoadCursorW(None, IDC_ARROW)?;
            wc.lpszClassName = CLASS_NAME;

            let _ = RegisterClassExW(&wc);

            let raw_data = Box::into_raw(Box::new(data));

            let hwnd = match CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
                CLASS_NAME,
                w!("KeyKey Candidates"),
                WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
                0,
                0,
                360,
                54,
                None,
                None,
                hinstance,
                Some(raw_data as *const _ as _),
            ) {
                Ok(h) if !h.0.is_null() => h,
                _ => {
                    let _ = Box::from_raw(raw_data);
                    return Err(windows::core::Error::from_win32());
                }
            };

            // Apply Windows 11 Modern Rounded Corners
            let preference = DWMWCP_ROUND;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &preference as *const _ as _,
                std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
            );

            Ok(hwnd)
        }
    }

    pub fn set_on_select<F: Fn(u32) + 'static>(&self, f: F) {
        if let Ok(mut d) = self.data.try_borrow_mut() {
            d.on_select = Some(Rc::new(f));
        }
    }

    pub fn show(
        &self,
        candidates: Vec<String>,
        page: usize,
        sel_idx: u32,
        context: Option<&ITfContext>,
        active_comp: Option<&ITfComposition>,
        edit_cookie: u32,
    ) {
        if let Ok(mut d) = self.data.try_borrow_mut() {
            d.candidates = candidates;
            d.current_page = page as u32;
            d.selection_index = sel_idx;
        }

        self.position_and_display(context, active_comp, edit_cookie);
    }

    pub fn update_page(&self, page: usize) {
        if let Ok(mut d) = self.data.try_borrow_mut() {
            d.current_page = page as u32;
            d.selection_index = 0;
        }
        let (win_width, win_height) = self.calculate_dimensions();
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                0,
                0,
                win_width,
                win_height,
                SWP_NOMOVE | SWP_NOACTIVATE | SWP_NOZORDER,
            );
            let _ = InvalidateRect(self.hwnd, None, BOOL(0));
        }
    }

    pub fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn is_visible(&self) -> bool {
        unsafe { IsWindowVisible(self.hwnd).as_bool() }
    }

    fn calculate_dimensions(&self) -> (i32, i32) {
        let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(self.hwnd) };
        let dpi = if dpi == 0 { 96 } else { dpi };
        let scale = dpi as f32 / 96.0;
        let win_height = (54.0 * scale).round() as i32;

        let Ok(data) = self.data.try_borrow() else {
            return (((320.0 * scale).round() as i32), win_height);
        };

        let start = data.current_page as usize * data.page_size;
        let page_cands: Vec<&String> = data.candidates.iter().skip(start).take(data.page_size).collect();

        // Base width calculation: padding + items + page counter
        let mut est_width = (24.0 * scale).round() as i32;
        for (idx, cand) in page_cands.iter().enumerate() {
            // Index number + candidate text + padding
            let char_len = cand.chars().count() as i32;
            let item_w = ((42 + char_len * 22) as f32 * scale).round() as i32;
            est_width += item_w;
            if idx < page_cands.len() - 1 {
                est_width += (8.0 * scale).round() as i32;
            }
        }
        est_width += (64.0 * scale).round() as i32; // page indicator space
        let min_width = (280.0 * scale).round() as i32;
        (est_width.max(min_width), win_height)
    }

    fn position_and_display(
        &self,
        context: Option<&ITfContext>,
        active_comp: Option<&ITfComposition>,
        edit_cookie: u32,
    ) {
        let (win_width, win_height) = self.calculate_dimensions();
        let (mut target_x, mut target_y) = (100i32, 100i32);
        let mut found_pos = false;

        // 1. Try TSF ContextView GetTextExt
        if let (Some(ctx), Some(comp)) = (context, active_comp) {
            unsafe {
                if let Ok(range) = comp.GetRange() {
                    if let Ok(view) = ctx.GetActiveView() {
                        let mut rc = RECT::default();
                        let mut fclipped = BOOL(0);
                        if view.GetTextExt(edit_cookie, &range, &mut rc, &mut fclipped).is_ok() {
                            if rc.right > rc.left || rc.bottom > rc.top {
                                target_x = rc.left;
                                target_y = rc.bottom + 6;
                                found_pos = true;
                            }
                        }
                    }
                }
            }
        }

        // 2. Fallback: Win32 Caret via GetGUIThreadInfo
        if !found_pos {
            unsafe {
                let mut gui = GUITHREADINFO {
                    cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
                    ..Default::default()
                };
                if GetGUIThreadInfo(0, &mut gui).is_ok() && !gui.hwndCaret.0.is_null() {
                    let mut pt = POINT {
                        x: gui.rcCaret.left,
                        y: gui.rcCaret.bottom + 6,
                    };
                    if ClientToScreen(gui.hwndCaret, &mut pt).as_bool() {
                        target_x = pt.x;
                        target_y = pt.y;
                        found_pos = true;
                    }
                }
            }
        }

        // 3. Fallback: GetCursorPos
        if !found_pos {
            unsafe {
                let mut pt = POINT::default();
                if GetCursorPos(&mut pt).is_ok() {
                    target_x = pt.x;
                    target_y = pt.y + 24;
                }
            }
        }

        // Boundary Clamp against monitor work area
        let (clamped_x, clamped_y) = clamp_to_monitor(target_x, target_y, win_width, win_height);

        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                clamped_x,
                clamped_y,
                win_width,
                win_height,
                SWP_SHOWWINDOW | SWP_NOACTIVATE,
            );
            let _ = InvalidateRect(self.hwnd, None, BOOL(0));
        }
    }
}

#[inline]
unsafe fn set_window_userdata(hwnd: HWND, val: isize) {
    #[cfg(target_pointer_width = "64")]
    let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, val);
    #[cfg(target_pointer_width = "32")]
    let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, val as i32);
}

impl Drop for CustomCandidateWindow {
    fn drop(&mut self) {
        unsafe {
            let ptr = GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut Rc<RefCell<CandidateWindowData>>;
            if !ptr.is_null() {
                let _ = Box::from_raw(ptr);
                set_window_userdata(self.hwnd, 0);
            }
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

unsafe extern "system" fn candidate_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        match msg {
            WM_NCCREATE => {
                let cs = lparam.0 as *const CREATESTRUCTW;
                if !cs.is_null() && !(*cs).lpCreateParams.is_null() {
                    let data_ptr = (*cs).lpCreateParams as *mut Rc<RefCell<CandidateWindowData>>;
                    set_window_userdata(hwnd, data_ptr as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_ERASEBKGND => LRESULT(1), // Prevent flicker
            WM_PAINT => {
                paint_candidate_window(hwnd);
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                let x = (lparam.0 & 0xFFFF) as i16 as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                let data_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Rc<RefCell<CandidateWindowData>>;
                if !data_ptr.is_null() {
                    let data_rc = &*data_ptr;
                    if let Ok(d) = data_rc.try_borrow() {
                        for item in &d.item_rects {
                            if x >= item.rect.left && x <= item.rect.right && y >= item.rect.top && y <= item.rect.bottom {
                                if let Some(ref cb) = d.on_select {
                                    cb(item.index);
                                }
                                break;
                            }
                        }
                    }
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }));

    res.unwrap_or_else(|_| DefWindowProcW(hwnd, msg, wparam, lparam))
}

unsafe fn paint_candidate_window(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);
    if hdc.0.is_null() {
        return;
    }

    let mut client_rc = RECT::default();
    let _ = GetClientRect(hwnd, &mut client_rc);
    let width = client_rc.right - client_rc.left;
    let height = client_rc.bottom - client_rc.top;

    // Double buffering
    let mem_dc = CreateCompatibleDC(hdc);
    if mem_dc.0.is_null() {
        let _ = EndPaint(hwnd, &ps);
        return;
    }
    let mem_bmp = CreateCompatibleBitmap(hdc, width, height);
    if mem_bmp.0.is_null() {
        let _ = DeleteDC(mem_dc);
        let _ = EndPaint(hwnd, &ps);
        return;
    }
    let old_bmp = SelectObject(mem_dc, mem_bmp);

    // Modern Windows 11 Fluent Soft Gray / Off-White background
    let bg_brush = CreateSolidBrush(COLORREF(0x00F8F9FA));
    FillRect(mem_dc, &client_rc, bg_brush);
    let _ = DeleteObject(bg_brush);

    // Subtle 1px border
    let border_brush = CreateSolidBrush(COLORREF(0x00E2E8F0));
    FrameRect(mem_dc, &client_rc, border_brush);
    let _ = DeleteObject(border_brush);

    // DPI-scaled font sizes and layout
    let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) };
    let dpi = if dpi == 0 { 96 } else { dpi };
    let scale = dpi as f32 / 96.0;

    let font_height = (22.0 * scale).round() as i32;
    let key_font_height = (15.0 * scale).round() as i32;

    // Font: Microsoft JhengHei UI for Candidate Text (22px * scale)
    let font = CreateFontW(
        font_height,
        0,
        0,
        0,
        FW_NORMAL.0 as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET.0 as u32,
        OUT_DEFAULT_PRECIS.0 as u32,
        CLIP_DEFAULT_PRECIS.0 as u32,
        CLEARTYPE_QUALITY.0 as u32,
        (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
        w!("Microsoft JhengHei UI"),
    );

    // Font: Segoe UI for Selection Key Numbers (15px * scale)
    let key_font = CreateFontW(
        key_font_height,
        0,
        0,
        0,
        FW_SEMIBOLD.0 as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET.0 as u32,
        OUT_DEFAULT_PRECIS.0 as u32,
        CLIP_DEFAULT_PRECIS.0 as u32,
        CLEARTYPE_QUALITY.0 as u32,
        (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
        w!("Segoe UI"),
    );

    let old_font = SelectObject(mem_dc, font);
    SetBkMode(mem_dc, TRANSPARENT);

    let data_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Rc<RefCell<CandidateWindowData>>;
    if !data_ptr.is_null() {
        let data_rc = &*data_ptr;
        if let Ok(mut d) = data_rc.try_borrow_mut() {
            d.item_rects.clear();

            let start = d.current_page as usize * d.page_size;
            let total_items = d.candidates.len();
            let total_pages = if total_items == 0 { 1 } else { (total_items + d.page_size - 1) / d.page_size };
            let page_cands: Vec<(usize, String)> = d
                .candidates
                .iter()
                .skip(start)
                .take(d.page_size)
                .enumerate()
                .map(|(i, s)| (i, s.clone()))
                .collect();

            let mut cur_x = (12.0 * scale).round() as i32;

            for (page_idx, cand) in page_cands {
                let key_num = (page_idx + 1).to_string();
                let key_str: Vec<u16> = format!("{}.", key_num).encode_utf16().collect();
                let cand_wide: Vec<u16> = cand.encode_utf16().collect();

                // Measure key number
                SelectObject(mem_dc, key_font);
                let mut key_size = SIZE::default();
                let _ = GetTextExtentPoint32W(mem_dc, &key_str, &mut key_size);

                // Measure candidate text
                SelectObject(mem_dc, font);
                let mut cand_size = SIZE::default();
                let _ = GetTextExtentPoint32W(mem_dc, &cand_wide, &mut cand_size);

                let pad_h = (8.0 * scale).round() as i32;
                let pad_gap = (6.0 * scale).round() as i32;
                let item_width = key_size.cx + pad_gap + cand_size.cx + (pad_h * 2);
                let item_rc = RECT {
                    left: cur_x,
                    top: (5.0 * scale).round() as i32,
                    right: cur_x + item_width,
                    bottom: height - (5.0 * scale).round() as i32,
                };

                // Highlight selected candidate with active accent background pill
                let global_idx = (start + page_idx) as u32;
                if global_idx == d.selection_index || page_idx as u32 == d.selection_index {
                    let sel_brush = CreateSolidBrush(COLORREF(0x00E2E8F0)); // Win11 subtle gray selection
                    FillRect(mem_dc, &item_rc, sel_brush);
                    let _ = DeleteObject(sel_brush);
                }

                // Render number index in secondary gray
                SelectObject(mem_dc, key_font);
                SetTextColor(mem_dc, COLORREF(0x0064748B));
                let mut key_rc = RECT {
                    left: cur_x + pad_h,
                    top: 0,
                    right: cur_x + pad_h + key_size.cx,
                    bottom: height,
                };
                let mut key_str_z = key_str.clone();
                let _ = DrawTextW(mem_dc, &mut key_str_z, &mut key_rc, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

                // Render candidate text in dark
                SelectObject(mem_dc, font);
                SetTextColor(mem_dc, COLORREF(0x000F172A));
                let mut text_rc = RECT {
                    left: key_rc.right + pad_gap,
                    top: 0,
                    right: key_rc.right + pad_gap + cand_size.cx,
                    bottom: height,
                };
                let mut cand_z = cand_wide.clone();
                let _ = DrawTextW(mem_dc, &mut cand_z, &mut text_rc, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

                d.item_rects.push(CandidateItemRect {
                    index: global_idx,
                    rect: item_rc,
                });

                cur_x += item_width + (6.0 * scale).round() as i32;
            }

            // Draw page counter at right edge
            let page_text = format!("{}/{}", d.current_page + 1, total_pages);
            let mut page_wide: Vec<u16> = page_text.encode_utf16().collect();
            SelectObject(mem_dc, key_font);
            SetTextColor(mem_dc, COLORREF(0x0094A3B8));
            let mut page_rc = RECT {
                left: width - (60.0 * scale).round() as i32,
                top: 0,
                right: width - (10.0 * scale).round() as i32,
                bottom: height,
            };
            let _ = DrawTextW(mem_dc, &mut page_wide, &mut page_rc, DT_RIGHT | DT_VCENTER | DT_SINGLELINE);
        }
    }

    let _ = BitBlt(hdc, 0, 0, width, height, mem_dc, 0, 0, SRCCOPY);

    SelectObject(mem_dc, old_font);
    let _ = DeleteObject(font);
    let _ = DeleteObject(key_font);
    SelectObject(mem_dc, old_bmp);
    let _ = DeleteObject(mem_bmp);
    let _ = DeleteDC(mem_dc);
    let _ = EndPaint(hwnd, &ps);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn test_candidate_window_lifecycle_and_rendering() {
        let win = CustomCandidateWindow::new().expect("Failed to create CustomCandidateWindow");
        assert!(!win.is_visible());

        let selected = Rc::new(Cell::new(None));
        let sel_clone = selected.clone();
        win.set_on_select(move |idx| {
            sel_clone.set(Some(idx));
        });

        // 1. Show with candidates
        let test_cands: Vec<String> = vec![
            "不".to_string(),
            "步".to_string(),
            "部".to_string(),
            "人工智慧".to_string(),
        ];
        win.show(test_cands.clone(), 0, 0, None, None, 0);
        assert!(win.is_visible());

        let (w, h) = win.calculate_dimensions();
        assert!(w >= 260);
        assert_eq!(h, 54);

        // 2. Update page
        win.update_page(0);

        // 3. Trigger manual paint and erasebkgnd
        unsafe {
            paint_candidate_window(win.hwnd);
            let res_erase = candidate_wndproc(win.hwnd, WM_ERASEBKGND, WPARAM(0), LPARAM(0));
            assert_eq!(res_erase, LRESULT(1));
        }

        // 4. Simulate mouse click on the first candidate item
        {
            let data = win.data.borrow();
            assert!(!data.item_rects.is_empty());
            let first_rc = data.item_rects[0].rect;
            let click_x = (first_rc.left + first_rc.right) / 2;
            let click_y = (first_rc.top + first_rc.bottom) / 2;
            let lparam_click = LPARAM(((click_y as isize) << 16) | (click_x as isize & 0xFFFF));

            unsafe {
                let _ = candidate_wndproc(win.hwnd, WM_LBUTTONDOWN, WPARAM(0), lparam_click);
            }
        }
        assert_eq!(selected.get(), Some(0));

        // Click outside any candidate (e.g. bottom corner)
        unsafe {
            let lparam_miss = LPARAM(((1000 as isize) << 16) | (1000 as isize & 0xFFFF));
            let _ = candidate_wndproc(win.hwnd, WM_LBUTTONDOWN, WPARAM(0), lparam_miss);
        }

        // Test without on_select callback
        win.data.borrow_mut().on_select = None;
        {
            let data = win.data.borrow();
            let first_rc = data.item_rects[0].rect;
            let click_x = (first_rc.left + first_rc.right) / 2;
            let click_y = (first_rc.top + first_rc.bottom) / 2;
            let lparam_click = LPARAM(((click_y as isize) << 16) | (click_x as isize & 0xFFFF));
            unsafe {
                let _ = candidate_wndproc(win.hwnd, WM_LBUTTONDOWN, WPARAM(0), lparam_click);
            }
        }

        // 5. Hide window
        win.hide();
        assert!(!win.is_visible());
    }

    #[test]
    fn test_candidate_window_edge_cases() {
        let win = CustomCandidateWindow::new().expect("Failed to create CustomCandidateWindow");

        // Empty candidates list
        win.show(vec![], 0, 0, None, None, 0);
        let (w, h) = win.calculate_dimensions();
        assert!(w >= 260);
        assert_eq!(h, 54);

        // Update page while visible
        win.update_page(1);

        unsafe {
            paint_candidate_window(win.hwnd);

            // Test wndproc with default message
            let res = candidate_wndproc(win.hwnd, WM_USER + 100, WPARAM(0), LPARAM(0));
            assert_eq!(res, LRESULT(0));

            // Test wndproc with invalid/null HWND (panic safety)
            let res_null = candidate_wndproc(HWND(std::ptr::null_mut()), WM_PAINT, WPARAM(0), LPARAM(0));
            assert_eq!(res_null, LRESULT(0));

            // Test paint and lbuttondown with re-entrant borrow failure
            {
                let _lock = win.data.borrow_mut();
                paint_candidate_window(win.hwnd);
                let _ = candidate_wndproc(win.hwnd, WM_LBUTTONDOWN, WPARAM(0), LPARAM(0));
            }
        }

        // Test calculate_dimensions with borrow failure
        {
            let _lock = win.data.borrow_mut();
            assert_eq!(win.calculate_dimensions(), (320, 54));
        }

        win.hide();
        // Hide again when not visible
        win.hide();
    }

    #[test]
    fn test_clamp_to_monitor_all_quadrants() {
        // Normal bounds
        let (cx, cy) = clamp_to_monitor(100, 100, 300, 46);
        assert_eq!(cx, 100);
        assert_eq!(cy, 100);

        // Clamping right & bottom screen limits
        let (rx, by) = clamp_to_monitor(99999, 99999, 300, 46);
        assert!(rx < 99999);
        assert!(by < 99999);

        // Clamping negative left and top
        let (lx, _) = clamp_to_monitor(-50000, 100, 300, 46);
        assert!(lx > -50000);
        let (_, ty) = clamp_to_monitor(100, -50000, 300, 46);
        assert!(ty > -50000);
    }

    #[test]
    fn test_candidate_window_click_multipage_global_index() {
        let win = CustomCandidateWindow::new().expect("Failed to create CustomCandidateWindow");
        let selected = Rc::new(Cell::new(None));
        let sel_clone = selected.clone();
        win.set_on_select(move |idx| {
            sel_clone.set(Some(idx));
        });

        // 15 candidates: page 0 has 0..8, page 1 has 9..14
        let test_cands: Vec<String> = (0..15).map(|i| format!("候選詞{}", i)).collect();
        win.show(test_cands, 0, 0, None, None, 0);

        // Turn to page 1 (second page)
        win.update_page(1);
        unsafe {
            paint_candidate_window(win.hwnd);
        }

        // Click first candidate on page 1
        {
            let data = win.data.borrow();
            assert!(!data.item_rects.is_empty());
            assert_eq!(data.item_rects[0].index, 9); // Must be global index 9, NOT relative 0!

            let first_rc = data.item_rects[0].rect;
            let click_x = (first_rc.left + first_rc.right) / 2;
            let click_y = (first_rc.top + first_rc.bottom) / 2;
            let lparam_click = LPARAM(((click_y as isize) << 16) | (click_x as isize & 0xFFFF));

            unsafe {
                let _ = candidate_wndproc(win.hwnd, WM_LBUTTONDOWN, WPARAM(0), lparam_click);
            }
        }

        assert_eq!(selected.get(), Some(9));
        win.hide();
    }
}
