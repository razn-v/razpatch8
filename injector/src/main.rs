use windows::core::{PCSTR, PSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::System::Diagnostics::Debug::WriteProcessMemory;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32First, Process32Next, PROCESSENTRY32, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
use windows::Win32::System::Memory::{
    VirtualAllocEx, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE,
};
use windows::Win32::System::Threading::{
    CreateProcessA, CreateRemoteThread, OpenProcess, WaitForInputIdle, CREATE_NO_WINDOW, INFINITE,
    PROCESS_ALL_ACCESS, PROCESS_INFORMATION, STARTUPINFOA,
};

use std::ffi::{CStr, CString};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut path = PathBuf::from(args.get(1).unwrap());
    path.pop();
    path.push("Polaris");
    path.push("Binaries");
    path.push("Win64");
    path.push("Polaris-Win64-Shipping.exe");

    let mut startup_info: STARTUPINFOA = unsafe { std::mem::zeroed() };
    startup_info.cb = std::mem::size_of::<STARTUPINFOA>() as u32;
    let mut process_info: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };

    println!("Creating process {:?}", path);

    let _ = unsafe {
        let path: CString = CString::new(path.to_str().unwrap()).unwrap();
        CreateProcessA(
            PCSTR::from_raw(path.as_ptr() as _),
            PSTR(std::ptr::null_mut()),
            None,
            None,
            false,
            CREATE_NO_WINDOW,
            None,
            None,
            &mut startup_info,
            &mut process_info,
        )
    }
    .expect("Failed to create process");

    assert_eq!(
        unsafe { WaitForInputIdle(process_info.hProcess, INFINITE) },
        0
    );

    let mut handle: HANDLE = INVALID_HANDLE_VALUE;
    // Polaris-Win64-Shipping.exe
    let mut found_polaris = false;

    while !found_polaris {
        let snapshot = unsafe { 
            CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) 
        }.expect("CreateToolhelp32Snapshot failed");

        let mut entry: PROCESSENTRY32 = Default::default();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32>() as u32;

        if unsafe { Process32First(snapshot, &mut entry).is_ok() } {
            while unsafe { Process32Next(snapshot, &mut entry).is_ok() } {
                let proc_name = unsafe { CStr::from_ptr(entry.szExeFile.as_ptr()) };

                if proc_name.cmp(c"Polaris-Win64-Shipping.exe").is_eq() {
                    handle = unsafe {
                        OpenProcess(
                            PROCESS_ALL_ACCESS, 
                            false, 
                            entry.th32ProcessID
                        ).unwrap()
                    };
                    found_polaris = true;
                    break;
                }
            }
        }

        let _ = unsafe { CloseHandle(snapshot) };

        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    assert_ne!(
        handle, INVALID_HANDLE_VALUE,
        "Polaris-Win64-Shipping.exe not found, is the game running?"
    );

    println!("Found Polaris-Win64-Shipping.exe");

    let mut dll_path = std::env::current_exe()
        .expect("Couldn't get current directory");
    dll_path.pop();
    dll_path.push("patch.dll");
    let dll_path = dll_path.into_os_string();

    let rb = unsafe {
        VirtualAllocEx(
            handle,
            None,
            dll_path.len(),
            MEM_RESERVE | MEM_COMMIT,
            PAGE_EXECUTE_READWRITE,
        )
    };

    unsafe {
        WriteProcessMemory(
            handle,
            rb,
            dll_path.as_encoded_bytes().as_ptr() as _,
            dll_path.len(),
            None,
        )
        .unwrap()
    }

    let ll_addr = unsafe {
        let h_kernel32 = GetModuleHandleA(PCSTR(c"Kernel32".as_ptr() as _))
            .unwrap();
        GetProcAddress(h_kernel32, PCSTR(c"LoadLibraryA".as_ptr() as _))
            .unwrap()
    };

    unsafe {
        CreateRemoteThread(
            handle,
            None,
            0,
            Some(std::mem::transmute(ll_addr)),
            Some(rb),
            0,
            None,
        )
        .unwrap()
    };

    println!("patch.dll injected");

    let _ = unsafe { CloseHandle(handle) };
}
