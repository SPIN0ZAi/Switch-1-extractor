// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(windows)]
    unsafe {
        unsafe extern "system" {
            fn SetDefaultDllDirectories(directory_flags: u32) -> i32;
            fn SetDllDirectoryW(lp_path_name: *const u16) -> i32;
            fn LoadLibraryW(lp_lib_file_name: *const u16) -> *mut std::ffi::c_void;
            fn GetSystemDirectoryW(lp_buffer: *mut u16, u_size: u32) -> u32;
        }

        // 1. Remove current working directory from DLL search order
        let empty: [u16; 1] = [0];
        SetDllDirectoryW(empty.as_ptr());

        // 2. Restrict DLL search exclusively to System32 (LOAD_LIBRARY_SEARCH_SYSTEM32 = 0x00000800)
        SetDefaultDllDirectories(0x00000800);

        // 3. Preload core Windows system DLLs directly from System32 using absolute paths
        // This immunizes against rogue hook DLLs (e.g. dwmapi.dll, version.dll, xinput1_4.dll)
        // present in user folders like Downloads.
        let mut sys_dir = [0u16; 260];
        let len = GetSystemDirectoryW(sys_dir.as_mut_ptr(), 260);
        let base_path = if len > 0 && (len as usize) < sys_dir.len() {
            String::from_utf16_lossy(&sys_dir[..len as usize])
        } else {
            "C:\\Windows\\System32".to_string()
        };

        let system_dlls = [
            "dwmapi.dll",
            "version.dll",
            "uxtheme.dll",
            "dxgi.dll",
            "d3d11.dll",
            "xinput1_4.dll",
        ];

        for dll in &system_dlls {
            let full_path = format!("{}\\{}\0", base_path, dll);
            let wide: Vec<u16> = full_path.encode_utf16().collect();
            LoadLibraryW(wide.as_ptr());
        }
    }

    switch_extractor_desktop_lib::run()
}
