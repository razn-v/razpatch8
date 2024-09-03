#![allow(non_snake_case)]
#![allow(unused_variables)]
#![allow(unreachable_patterns)]

use windows::Win32::Foundation::{BOOL, HANDLE, HMODULE};
use windows::Win32::System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory};
use windows::Win32::System::ProcessStatus::{
    EnumProcessModulesEx, GetModuleBaseNameA, GetModuleInformation, LIST_MODULES_ALL, MODULEINFO,
};
use windows::Win32::System::Threading::GetCurrentProcess;

use std::error::Error;
use std::ffi::CStr;
use std::os::raw::c_void;

// UGameUserSettings::ApplyNonResolutionSettings
const PATTERN: &[u8] = &[
    0x40, 0x57, 0x48, 0x83, 0xec, 0x60, 0x48, 0x8b, 0x01, 0x48, 0x8b, 0xf9, 0xff, 0x90, 0xf0, 0x02,
    0x00, 0x00
];

static START: std::sync::Once = std::sync::Once::new();

#[no_mangle]
unsafe extern "system" fn DllMain(_hinst: HANDLE, reason: u32, _reserved: *mut c_void) -> BOOL {
    //let _ = windows::Win32::System::Console::AllocConsole();

    match reason {
        DLL_PROCESS_ATTACH => {
            START.call_once(|| main().unwrap());
        }
        _ => {}
    };
    return BOOL::from(true);
}

fn get_module_info(h_process: HANDLE, module_name: &CStr) -> Option<MODULEINFO> {
    let mut modules: [HMODULE; 1024] = unsafe { std::mem::zeroed() };
    let mut cb_needed = 0;
    let res = unsafe {
        EnumProcessModulesEx(
            h_process,
            modules.as_mut_ptr(),
            (modules.len() * std::mem::size_of::<HMODULE>()) as u32,
            &mut cb_needed,
            LIST_MODULES_ALL,
        )
    };

    for module in modules {
        if module.is_invalid() {
            break;
        }

        let mut name: [u8; 128] = unsafe { std::mem::zeroed() };
        unsafe { GetModuleBaseNameA(h_process, module, name.as_mut()) };

        let name = std::ffi::CStr::from_bytes_until_nul(&name).unwrap();
        if name == module_name {
            let mut module_info = MODULEINFO::default();

            let _ = unsafe {
                GetModuleInformation(
                    h_process,
                    module,
                    &mut module_info,
                    std::mem::size_of::<MODULEINFO>() as u32,
                )
            };

            return Some(module_info);
        }
    }

    None
}

fn search_pattern(buf: Vec<u8>, pattern: &[u8]) -> Option<usize> {
    for i in 0..buf.len() {
        let mut j = 0;
        while j < pattern.len() && buf.get(i + j) == Some(&pattern[j]) {
            j += 1;
        }
        if j == pattern.len() {
            return Some(i);
        }
    }
    return None;
}

unsafe fn main() -> Result<(), Box<dyn Error>> {
    let process = unsafe { GetCurrentProcess() };
    let mod_info = get_module_info(process, c"Polaris-Win64-Shipping.exe")
        .expect("Polaris-Win64-Shipping.exe not found, is the game running?");

    let mut buf: Vec<u8> = vec![0; mod_info.SizeOfImage as usize];
    let mut read = 0;

    let _ = unsafe {
        ReadProcessMemory(
            process,
            mod_info.lpBaseOfDll,
            buf.as_mut_ptr() as _,
            mod_info.SizeOfImage as usize,
            Some(&mut read),
        )
    };

    assert_eq!(read, mod_info.SizeOfImage as usize, "Read mismatch.");

    let offset = search_pattern(buf, PATTERN)
        .expect("Couldn't find UGameUserSettings::ApplyNonResolutionSettings");

    let ret = vec![0xc3];
    let mut wrote = 0;
    let _ = unsafe {
        WriteProcessMemory(
            process,
            (mod_info.lpBaseOfDll as usize + offset) as _,
            ret.as_ptr() as _,
            1,
            Some(&mut wrote),
        )
    };
    assert_eq!(
        wrote, 1,
        "Failed to patch UGameUserSettings::ApplyNonResolutionSettings"
    );

    Ok(())
}
