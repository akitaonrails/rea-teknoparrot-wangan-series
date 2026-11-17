//! The only `unsafe` in wangan-io: a named pipe, a named shared section and
//! a read-only file mapping, each behind a small safe type.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr::{null, null_mut};

use wangan_jvs::shared::{SIZE, Snapshot};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_PIPE_CONNECTED, GENERIC_READ, GetLastError, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    PIPE_ACCESS_DUPLEX, ReadFile, WriteFile,
};
use windows_sys::Win32::System::Memory::{
    CreateFileMappingW, FILE_MAP_ALL_ACCESS, FILE_MAP_READ, MEMORY_MAPPED_VIEW_ADDRESS,
    MapViewOfFile, PAGE_READONLY, PAGE_READWRITE, UnmapViewOfFile,
};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE,
    PIPE_WAIT,
};

fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn last_error() -> String {
    format!("Win32 error {}", unsafe { GetLastError() })
}

/// `TeknoParrot_JvsState`: 64 bytes, first int = "the board was addressed".
pub struct JvsState {
    section: HANDLE,
    view: MEMORY_MAPPED_VIEW_ADDRESS,
}

// SAFETY: the view is plain shared memory written with volatile stores.
unsafe impl Send for JvsState {}
unsafe impl Sync for JvsState {}

impl JvsState {
    pub fn create() -> Result<Self, String> {
        let name = wide("TeknoParrot_JvsState");
        // SAFETY: arguments are valid; the name is NUL-terminated.
        let section = unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                null(),
                PAGE_READWRITE,
                0,
                64,
                name.as_ptr(),
            )
        };
        if section.is_null() {
            return Err(last_error());
        }
        // SAFETY: section is a valid mapping handle of 64 bytes.
        let view = unsafe { MapViewOfFile(section, FILE_MAP_ALL_ACCESS, 0, 0, 64) };
        if view.Value.is_null() {
            let e = last_error();
            unsafe { CloseHandle(section) };
            return Err(e);
        }
        Ok(Self { section, view })
    }

    pub fn set_addressed(&self, on: bool) {
        // SAFETY: the view is at least 64 bytes and aligned to a page.
        unsafe { std::ptr::write_volatile(self.view.Value as *mut i32, on as i32) };
    }

    /// Stores a little-endian u32 at a 4-byte aligned `offset` (< 61).
    pub fn put_u32(&self, offset: usize, v: u32) {
        assert!(offset.is_multiple_of(4) && offset + 4 <= 64);
        // SAFETY: in bounds and aligned (the view is page-aligned).
        unsafe {
            std::ptr::write_volatile((self.view.Value as *mut u8).add(offset) as *mut u32, v)
        };
    }
}

impl Drop for JvsState {
    fn drop(&mut self) {
        unsafe {
            UnmapViewOfFile(self.view);
            CloseHandle(self.section);
        }
    }
}

/// The launcher's input block, mapped read-only from its file.
pub struct InputBlock {
    file: HANDLE,
    section: HANDLE,
    view: MEMORY_MAPPED_VIEW_ADDRESS,
}

unsafe impl Send for InputBlock {}
unsafe impl Sync for InputBlock {}

impl InputBlock {
    pub fn open(path: &str) -> Result<Self, String> {
        let p = wide(path);
        // SAFETY: valid NUL-terminated path, plain read access.
        let file = unsafe {
            CreateFileW(
                p.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                null(),
                OPEN_EXISTING,
                0,
                null_mut(),
            )
        };
        if file == INVALID_HANDLE_VALUE {
            return Err(last_error());
        }
        let section =
            unsafe { CreateFileMappingW(file, null(), PAGE_READONLY, 0, SIZE as u32, null()) };
        if section.is_null() {
            let e = last_error();
            unsafe { CloseHandle(file) };
            return Err(e);
        }
        let view = unsafe { MapViewOfFile(section, FILE_MAP_READ, 0, 0, SIZE) };
        if view.Value.is_null() {
            let e = last_error();
            unsafe {
                CloseHandle(section);
                CloseHandle(file);
            }
            return Err(e);
        }
        Ok(Self {
            file,
            section,
            view,
        })
    }

    pub fn snapshot(&self) -> Option<Snapshot> {
        let mut b = [0u8; SIZE];
        // SAFETY: the view maps SIZE readable bytes; volatile because the
        // launcher rewrites them from another process.
        for (i, x) in b.iter_mut().enumerate() {
            *x = unsafe { std::ptr::read_volatile((self.view.Value as *const u8).add(i)) };
        }
        Snapshot::decode(&b)
    }
}

impl Drop for InputBlock {
    fn drop(&mut self) {
        unsafe {
            UnmapViewOfFile(self.view);
            CloseHandle(self.section);
            CloseHandle(self.file);
        }
    }
}

/// One instance of a byte-mode duplex named pipe.
pub struct Pipe {
    handle: HANDLE,
}

impl Pipe {
    pub fn create(name: &str) -> Result<Self, String> {
        let n = wide(name);
        // SAFETY: valid name; one instance, byte mode, blocking.
        let handle = unsafe {
            CreateNamedPipeW(
                n.as_ptr(),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                1,
                4096,
                4096,
                0,
                null(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(last_error());
        }
        Ok(Self { handle })
    }

    pub fn connect(&self) -> Result<(), String> {
        // SAFETY: valid pipe handle, synchronous connect.
        let ok = unsafe { ConnectNamedPipe(self.handle, null_mut()) };
        if ok == 0 && unsafe { GetLastError() } != ERROR_PIPE_CONNECTED {
            return Err(last_error());
        }
        Ok(())
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<usize, String> {
        let mut got = 0u32;
        // SAFETY: buf is valid for buf.len() bytes.
        let ok = unsafe {
            ReadFile(
                self.handle,
                buf.as_mut_ptr(),
                buf.len() as u32,
                &mut got,
                null_mut(),
            )
        };
        if ok == 0 {
            return Err(last_error());
        }
        Ok(got as usize)
    }

    pub fn write(&self, data: &[u8]) -> Result<(), String> {
        let mut put = 0u32;
        // SAFETY: data is valid for data.len() bytes.
        let ok = unsafe {
            WriteFile(
                self.handle,
                data.as_ptr(),
                data.len() as u32,
                &mut put,
                null_mut(),
            )
        };
        if ok == 0 || put as usize != data.len() {
            return Err(last_error());
        }
        Ok(())
    }
}

impl Drop for Pipe {
    fn drop(&mut self) {
        unsafe {
            DisconnectNamedPipe(self.handle);
            CloseHandle(self.handle);
        }
    }
}
