// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(windows)]
    unsafe {
        unsafe extern "system" {
            fn SetDefaultDllDirectories(directory_flags: u32) -> i32;
            fn SetDllDirectoryW(lp_path_name: *const u16) -> i32;
        }
        // Remove current directory from the search path
        let empty: [u16; 1] = [0];
        SetDllDirectoryW(empty.as_ptr());
        // LOAD_LIBRARY_SEARCH_SYSTEM32 (0x00000800) | LOAD_LIBRARY_SEARCH_APPLICATION_DIR (0x00000200)
        SetDefaultDllDirectories(0x00000800 | 0x00000200);
    }

    switch_extractor_desktop_lib::run()
}
