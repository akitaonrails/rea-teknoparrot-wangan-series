//! Windows: stop what a game session left running from its runtime folder.
//!
//! OpenParrot's loader starts the auth daemon (`AMCUS\AMAuthd.exe`) as its
//! own child, so it is not one of the launcher's children; on Linux
//! `wineserver -k` ends it with the prefix. Here every process whose program
//! lives in the runtime folder is stopped once the game has ended. Nothing
//! outside that folder is touched.

#[cfg(windows)]
use std::path::Path;

/// Whether `exe` (a full program path) lies inside `dir`. Case-insensitive,
/// either separator, with or without the `\\?\` prefix.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn under_dir(exe: &str, dir: &str) -> bool {
    fn norm(s: &str) -> String {
        let s = s.replace('/', "\\");
        let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
        s.trim_end_matches('\\').to_lowercase()
    }
    let (exe, dir) = (norm(exe), norm(dir));
    !dir.is_empty()
        && exe.len() > dir.len() + 1
        && exe.starts_with(&dir)
        && exe[dir.len()..].starts_with('\\')
}

/// Stop every process whose program is inside `dir`; returns how many.
#[cfg(windows)]
pub fn kill_under(dir: &Path) -> usize {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcessId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        PROCESS_TERMINATE, QueryFullProcessImageNameW, TerminateProcess,
    };

    let dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let dir = dir.to_string_lossy().into_owned();
    let me = unsafe { GetCurrentProcessId() };
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snap == INVALID_HANDLE_VALUE {
        return 0;
    }
    let mut pids = Vec::new();
    let mut e: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    let mut ok = unsafe { Process32FirstW(snap, &mut e) } != 0;
    while ok {
        if e.th32ProcessID != 0 && e.th32ProcessID != me {
            pids.push(e.th32ProcessID);
        }
        ok = unsafe { Process32NextW(snap, &mut e) } != 0;
    }
    unsafe { CloseHandle(snap) };

    let mut killed = 0;
    for pid in pids {
        let h = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE,
                0,
                pid,
            )
        };
        if h.is_null() {
            continue;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let named = unsafe {
            QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len)
        } != 0;
        if named {
            let exe = String::from_utf16_lossy(&buf[..len as usize]);
            if under_dir(&exe, &dir) && unsafe { TerminateProcess(h, 1) } != 0 {
                killed += 1;
            }
        }
        unsafe { CloseHandle(h) };
    }
    killed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_inside_the_runtime_only() {
        let rt = r"C:\Users\p\AppData\Local\rea-wangan\runtimes\mt6";
        assert!(under_dir(
            r"C:\Users\p\AppData\Local\rea-wangan\runtimes\mt6\AMCUS\AMAuthd.exe",
            rt
        ));
        assert!(under_dir(
            r"c:\users\P\appdata\local\rea-wangan\runtimes\MT6\wangan-io.exe",
            &format!(r"\\?\{rt}\")
        ));
        assert!(under_dir(
            "Z:/mnt/x/runtimes/mt6/wmn6r.exe",
            r"Z:\mnt\x\runtimes\mt6"
        ));
        // a sibling runtime with a longer name, the folder itself, elsewhere
        assert!(!under_dir(
            r"C:\Users\p\AppData\Local\rea-wangan\runtimes\mt6rr\wmn6r.exe",
            rt
        ));
        assert!(!under_dir(rt, rt));
        assert!(!under_dir(r"C:\Windows\explorer.exe", rt));
        assert!(!under_dir(r"C:\Windows\explorer.exe", ""));
    }
}
