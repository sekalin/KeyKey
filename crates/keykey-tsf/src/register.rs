use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::Ordering;
use windows::core::{HSTRING, PCWSTR, GUID, Result};
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::System::Registry::*;
use windows::Win32::UI::TextServices::*;

use crate::{CLSID_KEYKEY_TEXT_SERVICE, GUID_KEYKEY_PROFILE, DLL_INSTANCE};

pub const TEXT_SERVICE_NAME: &str = "KeyKey Bopomofo Input Method (琦琦注音)";
pub const TEXT_SERVICE_DESC: &str = "中文 (繁體) - 琦琦注音";
pub const LANG_ID_TRADITIONAL_CHINESE: u16 = 0x0404;

pub fn get_module_path() -> Result<Vec<u16>> {
    let hinst = HINSTANCE(DLL_INSTANCE.load(Ordering::SeqCst) as _);
    let mut buffer = vec![0u16; 1024];
    let len = unsafe { GetModuleFileNameW(hinst, &mut buffer) };
    if len == 0 {
        return Err(windows::core::Error::from_win32());
    }
    buffer.truncate(len as usize);

    // If running from an exe (e.g. keykey-tools.exe), resolve keykey_tsf.dll in the same directory
    let path_str = String::from_utf16_lossy(&buffer);
    let path = std::path::Path::new(&path_str);
    if path.extension().map_or(false, |ext| ext.eq_ignore_ascii_case("exe")) {
        let dll_candidate = path.with_file_name("keykey_tsf.dll");
        if dll_candidate.exists() {
            if let Some(dll_str) = dll_candidate.to_str() {
                return Ok(to_wide(dll_str));
            }
        }
    }

    buffer.push(0); // null terminator
    Ok(buffer)
}

pub fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

pub fn register_com_keys(root_key: HKEY, prefix: &str, dll_path: &[u16]) -> Result<()> {
    register_com_keys_with_clsid(root_key, prefix, &CLSID_KEYKEY_TEXT_SERVICE, dll_path)
}

pub fn register_com_keys_with_clsid(
    root_key: HKEY,
    prefix: &str,
    clsid: &GUID,
    dll_path: &[u16],
) -> Result<()> {
    let clsid_str = format!("{{{:?}}}", clsid);
    let clsid_key_path = format!("{}{}", prefix, clsid_str);

    let mut hkey_clsid = HKEY::default();
    unsafe {
        RegCreateKeyW(
            root_key,
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

        RegSetValueExW(
            hkey_inproc,
            None,
            0,
            REG_SZ,
            Some(std::slice::from_raw_parts(dll_path.as_ptr() as *const u8, dll_path.len() * 2)),
        ).ok()?;

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
    Ok(())
}

pub fn unregister_com_keys(root_key: HKEY, prefix: &str) -> Result<()> {
    unregister_com_keys_with_clsid(root_key, prefix, &CLSID_KEYKEY_TEXT_SERVICE)
}

pub fn unregister_com_keys_with_clsid(root_key: HKEY, prefix: &str, clsid: &GUID) -> Result<()> {
    let clsid_str = format!("{{{:?}}}", clsid);
    let clsid_key_path = format!("{}{}", prefix, clsid_str);
    let inproc_key_path = format!("{}{}\\InprocServer32", prefix, clsid_str);

    unsafe {
        let _ = RegDeleteKeyW(root_key, PCWSTR(to_wide(&inproc_key_path).as_ptr()));
        let _ = RegDeleteKeyW(root_key, PCWSTR(to_wide(&clsid_key_path).as_ptr()));
    }
    Ok(())
}

pub fn enable_user_profile_tip(enable: bool) {
    let subkey_str = "Control Panel\\International\\User Profile\\zh-Hant-TW";
    let val_name = format!(
        "0404:{{{:?}}}{{{:?}}}",
        CLSID_KEYKEY_TEXT_SERVICE, GUID_KEYKEY_PROFILE
    );
    let mut hkey = HKEY::default();
    unsafe {
        if RegCreateKeyW(
            HKEY_CURRENT_USER,
            PCWSTR(to_wide(subkey_str).as_ptr()),
            &mut hkey,
        ).is_ok() {
            let val_name_w = to_wide(&val_name);
            if enable {
                let one: u32 = 1;
                let _ = RegSetValueExW(
                    hkey,
                    PCWSTR(val_name_w.as_ptr()),
                    0,
                    REG_DWORD,
                    Some(std::slice::from_raw_parts(&one as *const u32 as *const u8, 4)),
                );
            } else {
                let _ = RegDeleteValueW(hkey, PCWSTR(val_name_w.as_ptr()));
            }
            let _ = RegCloseKey(hkey);
        }
    }
}

pub fn register_server() -> Result<()> {
    unsafe {
        let _ = CoInitialize(None);
    }
    let dll_path = get_module_path()?;

    // 1. Register COM InProcServer32 (HKCR if elevated, otherwise fallback to HKCU)
    if register_com_keys(HKEY_CLASSES_ROOT, "CLSID\\", &dll_path).is_err() {
        register_com_keys(HKEY_CURRENT_USER, "Software\\Classes\\CLSID\\", &dll_path)?;
    }

    // 2. Register TSF Language Profile via ITfInputProcessorProfiles
    unsafe {
        let profiles: ITfInputProcessorProfiles =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;

        let _ = profiles.Register(&CLSID_KEYKEY_TEXT_SERVICE);

        let desc_hstring = HSTRING::from(TEXT_SERVICE_DESC);
        let _ = profiles.AddLanguageProfile(
            &CLSID_KEYKEY_TEXT_SERVICE,
            LANG_ID_TRADITIONAL_CHINESE,
            &GUID_KEYKEY_PROFILE,
            desc_hstring.as_wide(),
            &dll_path[..dll_path.len() - 1], // without trailing null
            0,
        );

        let _ = profiles.EnableLanguageProfile(
            &CLSID_KEYKEY_TEXT_SERVICE,
            LANG_ID_TRADITIONAL_CHINESE,
            &GUID_KEYKEY_PROFILE,
            true,
        );
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

    // 4. Configure User Profile for current user
    enable_user_profile_tip(true);

    Ok(())
}

pub fn unregister_server() -> Result<()> {
    unsafe {
        let _ = CoInitialize(None);
    }
    // 0. Remove from User Profile
    enable_user_profile_tip(false);

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

    // 3. Delete COM Registry Keys (try HKCR then HKCU)
    let _ = unregister_com_keys(HKEY_CLASSES_ROOT, "CLSID\\");
    let _ = unregister_com_keys(HKEY_CURRENT_USER, "Software\\Classes\\CLSID\\");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_wide() {
        let wide = to_wide("hello");
        assert_eq!(wide, vec![b'h' as u16, b'e' as u16, b'l' as u16, b'l' as u16, b'o' as u16, 0]);
    }

    #[test]
    fn test_get_module_path() {
        let path = get_module_path();
        assert!(path.is_ok());
        let path_vec = path.unwrap();
        assert!(!path_vec.is_empty());
        assert_eq!(path_vec.last(), Some(&0));
    }

    #[test]
    fn test_register_and_unregister_com_keys_current_user() {
        let test_dll_path = to_wide("C:\\Windows\\System32\\test.dll");
        let prefix = "Software\\Classes\\CLSID\\";
        let test_clsid = GUID::from_u128(0x99998888_7777_6666_5555_444433332222);

        // Register under HKCU
        assert!(register_com_keys_with_clsid(HKEY_CURRENT_USER, prefix, &test_clsid, &test_dll_path).is_ok());

        // Unregister under HKCU
        assert!(unregister_com_keys_with_clsid(HKEY_CURRENT_USER, prefix, &test_clsid).is_ok());
    }
}
