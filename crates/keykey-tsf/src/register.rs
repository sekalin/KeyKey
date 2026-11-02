use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::Ordering;
use windows::core::{HSTRING, PCWSTR, Result};
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::System::Registry::*;
use windows::Win32::UI::TextServices::*;

use crate::{CLSID_KEYKEY_TEXT_SERVICE, GUID_KEYKEY_PROFILE, DLL_INSTANCE};

const TEXT_SERVICE_NAME: &str = "KeyKey Bopomofo Input Method (琦琦注音)";
const TEXT_SERVICE_DESC: &str = "中文 (繁體) - 琦琦注音";
const LANG_ID_TRADITIONAL_CHINESE: u16 = 0x0404;

pub fn get_module_path() -> Result<Vec<u16>> {
    let hinst = HINSTANCE(DLL_INSTANCE.load(Ordering::SeqCst) as _);
    let mut buffer = vec![0u16; 1024];
    let len = unsafe { GetModuleFileNameW(hinst, &mut buffer) };
    if len == 0 {
        return Err(windows::core::Error::from_win32());
    }
    buffer.truncate(len as usize);
    buffer.push(0); // null terminator
    Ok(buffer)
}

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

pub fn register_server() -> Result<()> {
    let dll_path = get_module_path()?;
    let clsid_str = format!("{{{:?}}}", CLSID_KEYKEY_TEXT_SERVICE);
    let clsid_key_path = format!("CLSID\\{}", clsid_str);

    // 1. Register COM InProcServer32 in HKEY_CLASSES_ROOT
    let mut hkey_clsid = HKEY::default();
    unsafe {
        RegCreateKeyW(
            HKEY_CLASSES_ROOT,
            PCWSTR(to_wide(&clsid_key_path).as_ptr()),
            &mut hkey_clsid,
        ).ok()?;

        let desc_w = to_wide(TEXT_SERVICE_NAME);
        RegSetValueExW(
            hkey_clsid,
            None,
            0,
            REG_SZ,
            Some(std::slice::from_raw_parts(desc_w.as_ptr() as *const u8, desc_w.len() * 2)),
        ).ok()?;

        let mut hkey_inproc = HKEY::default();
        RegCreateKeyW(
            hkey_clsid,
            PCWSTR(to_wide("InprocServer32").as_ptr()),
            &mut hkey_inproc,
        ).ok()?;

        // Set DLL path
        RegSetValueExW(
            hkey_inproc,
            None,
            0,
            REG_SZ,
            Some(std::slice::from_raw_parts(dll_path.as_ptr() as *const u8, dll_path.len() * 2)),
        ).ok()?;

        // Set ThreadingModel = Apartment
        let tm_w = to_wide("Apartment");
        RegSetValueExW(
            hkey_inproc,
            PCWSTR(to_wide("ThreadingModel").as_ptr()),
            0,
            REG_SZ,
            Some(std::slice::from_raw_parts(tm_w.as_ptr() as *const u8, tm_w.len() * 2)),
        ).ok()?;

        let _ = RegCloseKey(hkey_inproc);
        let _ = RegCloseKey(hkey_clsid);
    }

    // 2. Register TSF Language Profile via ITfInputProcessorProfiles
    unsafe {
        let profiles: ITfInputProcessorProfiles =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;

        profiles.Register(&CLSID_KEYKEY_TEXT_SERVICE)?;

        let desc_hstring = HSTRING::from(TEXT_SERVICE_DESC);
        profiles.AddLanguageProfile(
            &CLSID_KEYKEY_TEXT_SERVICE,
            LANG_ID_TRADITIONAL_CHINESE,
            &GUID_KEYKEY_PROFILE,
            desc_hstring.as_wide(),
            &dll_path[..dll_path.len() - 1], // without trailing null
            0,
        )?;

        profiles.EnableLanguageProfile(
            &CLSID_KEYKEY_TEXT_SERVICE,
            LANG_ID_TRADITIONAL_CHINESE,
            &GUID_KEYKEY_PROFILE,
            true,
        )?;
    }

    // 3. Register TSF Categories via ITfCategoryMgr
    unsafe {
        let cat_mgr: ITfCategoryMgr =
            CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;

        let categories = [
            GUID_TFCAT_TIP_KEYBOARD,
            GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
            GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
            GUID_TFCAT_TIPCAP_SECUREMODE,
            GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT, // Critical for Windows 11 AppContainer / Modern apps
            GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
        ];

        for cat in &categories {
            let _ = cat_mgr.RegisterCategory(
                &CLSID_KEYKEY_TEXT_SERVICE,
                cat,
                &CLSID_KEYKEY_TEXT_SERVICE,
            );
        }
    }

    Ok(())
}

pub fn unregister_server() -> Result<()> {
    // 1. Unregister TSF Categories
    unsafe {
        if let Ok(cat_mgr) =
            CoCreateInstance::<_, ITfCategoryMgr>(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)
        {
            let categories = [
                GUID_TFCAT_TIP_KEYBOARD,
                GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
                GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
                GUID_TFCAT_TIPCAP_SECUREMODE,
                GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
                GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
            ];

            for cat in &categories {
                let _ = cat_mgr.UnregisterCategory(
                    &CLSID_KEYKEY_TEXT_SERVICE,
                    cat,
                    &CLSID_KEYKEY_TEXT_SERVICE,
                );
            }
        }
    }

    // 2. Unregister TSF Language Profile
    unsafe {
        if let Ok(profiles) = CoCreateInstance::<_, ITfInputProcessorProfiles>(
            &CLSID_TF_InputProcessorProfiles,
            None,
            CLSCTX_INPROC_SERVER,
        ) {
            let _ = profiles.Unregister(&CLSID_KEYKEY_TEXT_SERVICE);
        }
    }

    // 3. Delete COM Registry Keys
    let clsid_str = format!("{{{:?}}}", CLSID_KEYKEY_TEXT_SERVICE);
    let clsid_key_path = format!("CLSID\\{}", clsid_str);
    let inproc_key_path = format!("CLSID\\{}\\InprocServer32", clsid_str);

    unsafe {
        let _ = RegDeleteKeyW(HKEY_CLASSES_ROOT, PCWSTR(to_wide(&inproc_key_path).as_ptr()));
        let _ = RegDeleteKeyW(HKEY_CLASSES_ROOT, PCWSTR(to_wide(&clsid_key_path).as_ptr()));
    }

    Ok(())
}
