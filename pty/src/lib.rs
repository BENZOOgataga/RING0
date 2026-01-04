use std::io::{Read, Write};

#[derive(Debug, thiserror::Error)]
pub enum PtyError {
    #[error("unsupported platform")]
    UnsupportedPlatform,
    #[error("invalid size: cols={cols}, rows={rows}")]
    InvalidSize { cols: u16, rows: u16 },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[cfg(windows)]
    #[error("windows api error: {0}")]
    Windows(#[from] windows::core::Error),
}

#[derive(Debug, Copy, Clone)]
pub struct PtySize {
    pub cols: u16,
    pub rows: u16,
}

impl PtySize {
    fn validate(self) -> Result<(), PtyError> {
        if self.cols == 0 || self.rows == 0 {
            return Err(PtyError::InvalidSize {
                cols: self.cols,
                rows: self.rows,
            });
        }
        Ok(())
    }
}

pub struct Pty {
    inner: PtyInner,
}

impl Pty {
    pub fn spawn(command: &str, size: PtySize) -> Result<Self, PtyError> {
        size.validate()?;
        let inner = PtyInner::spawn(command, size)?;
        Ok(Self { inner })
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, PtyError> {
        self.inner.read(buf)
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<usize, PtyError> {
        self.inner.write(buf)
    }

    pub fn resize(&mut self, size: PtySize) -> Result<(), PtyError> {
        size.validate()?;
        self.inner.resize(size)
    }

    pub fn is_running(&self) -> Result<bool, PtyError> {
        self.inner.is_running()
    }

    pub fn reader(&self) -> Result<PtyReader, PtyError> {
        Ok(PtyReader {
            inner: self.inner.clone_reader()?,
        })
    }

    pub fn writer(&self) -> Result<PtyWriter, PtyError> {
        Ok(PtyWriter {
            inner: self.inner.clone_writer()?,
        })
    }

    pub fn bytes_available(&self) -> Result<u32, PtyError> {
        self.inner.bytes_available()
    }
}

pub struct PtyReader {
    inner: std::fs::File,
}

impl PtyReader {
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, PtyError> {
        Ok(self.inner.read(buf)?)
    }
}

pub struct PtyWriter {
    inner: std::fs::File,
}

impl PtyWriter {
    pub fn write_all(&mut self, buf: &[u8]) -> Result<(), PtyError> {
        self.inner.write_all(buf)?;
        Ok(())
    }
}

#[cfg(windows)]
mod platform {
    use super::{PtyError, PtySize};
    use std::ffi::{c_void, OsStr};
    use std::fs::File;
    use std::io::{Read, Write};
    use std::mem::{size_of, zeroed};
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle};
    use windows::core::{Error, PCWSTR, PWSTR};
    use windows::Win32::Foundation::{
        CloseHandle, SetHandleInformation, BOOL, HANDLE, HANDLE_FLAG_INHERIT,
    };
    use windows::Win32::Security::SECURITY_ATTRIBUTES;
    use windows::Win32::System::Console::{
        ClosePseudoConsole, CreatePseudoConsole, ResizePseudoConsole, COORD, HPCON,
    };
    use windows::Win32::System::Pipes::{CreatePipe, PeekNamedPipe};
    use windows::Win32::System::Threading::{
        CreateProcessW, DeleteProcThreadAttributeList, InitializeProcThreadAttributeList,
        UpdateProcThreadAttribute, CREATE_NO_WINDOW, EXTENDED_STARTUPINFO_PRESENT,
        LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
        STARTF_USESTDHANDLES, STARTUPINFOEXW,
    };

    pub(super) struct PtyInner {
        hpc: HPCON,
        input_write: File,
        output_read: File,
        conpty_input: HANDLE,
        conpty_output: HANDLE,
        process_handle: HANDLE,
        thread_handle: HANDLE,
    }

    impl PtyInner {
        pub(super) fn spawn(command: &str, size: PtySize) -> Result<Self, PtyError> {
            let (input_read, input_write) = create_pipe()?;
            let (output_read, output_write) = create_pipe()?;

            set_handle_inherit(input_read, true)?;
            set_handle_inherit(output_write, true)?;
            set_handle_inherit(input_write, false)?;
            set_handle_inherit(output_read, false)?;

            let input_read_guard = HandleGuard::new(input_read);
            let input_write_guard = HandleGuard::new(input_write);
            let output_read_guard = HandleGuard::new(output_read);
            let output_write_guard = HandleGuard::new(output_write);

            let hpc = unsafe {
                CreatePseudoConsole(
                    COORD {
                        X: size.cols as i16,
                        Y: size.rows as i16,
                    },
                    input_read_guard.handle,
                    output_write_guard.handle,
                    0,
                )?
            };
            let hpc_guard = PseudoConsoleGuard::new(hpc);

            let input_write_handle = input_write_guard.into_inner();
            let output_read_handle = output_read_guard.into_inner();
            let conpty_input = input_read_guard.into_inner();
            let conpty_output = output_write_guard.into_inner();

            let input_write = unsafe { File::from_raw_handle(raw_handle(input_write_handle)) };
            let output_read = unsafe { File::from_raw_handle(raw_handle(output_read_handle)) };

            let mut attr_list_size: usize = 0;
            unsafe {
                let _ = InitializeProcThreadAttributeList(
                    LPPROC_THREAD_ATTRIBUTE_LIST::default(),
                    1,
                    0,
                    &mut attr_list_size,
                );
            }
            if attr_list_size == 0 {
                return Err(PtyError::Windows(Error::from_win32()));
            }

            let mut attr_list_buffer = vec![0u8; attr_list_size];
            let attr_list_ptr =
                LPPROC_THREAD_ATTRIBUTE_LIST(attr_list_buffer.as_mut_ptr() as *mut c_void);
            unsafe {
                InitializeProcThreadAttributeList(attr_list_ptr, 1, 0, &mut attr_list_size)?;
            }
            let attr_list_guard = AttrListGuard::new(attr_list_ptr, attr_list_buffer);

            let mut hpc_copy = hpc_guard.handle;
            unsafe {
                UpdateProcThreadAttribute(
                    attr_list_guard.ptr,
                    0,
                    PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                    Some(&mut hpc_copy as *mut _ as *const c_void),
                    size_of::<HPCON>(),
                    None,
                    None,
                )?;
            }

            let mut startup_info: STARTUPINFOEXW = unsafe { zeroed() };
            startup_info.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
            startup_info.lpAttributeList = attr_list_guard.ptr;
            startup_info.StartupInfo.dwFlags |= STARTF_USESTDHANDLES;
            startup_info.StartupInfo.hStdInput = conpty_input;
            startup_info.StartupInfo.hStdOutput = conpty_output;
            startup_info.StartupInfo.hStdError = conpty_output;

            let mut proc_info: PROCESS_INFORMATION = unsafe { zeroed() };
            let mut command_line = wide_command_line(command);

            let inherit_handles = true;
            unsafe {
                CreateProcessW(
                    PCWSTR::null(),
                    PWSTR(command_line.as_mut_ptr()),
                    None,
                    None,
                    inherit_handles,
                    EXTENDED_STARTUPINFO_PRESENT | CREATE_NO_WINDOW,
                    None,
                    PCWSTR::null(),
                    &startup_info.StartupInfo,
                    &mut proc_info,
                )?;
            }

            let process_handle = proc_info.hProcess;
            let thread_handle = proc_info.hThread;

            Ok(Self {
                hpc: hpc_guard.into_inner(),
                input_write,
                output_read,
                conpty_input,
                conpty_output,
                process_handle,
                thread_handle,
            })
        }

        pub(super) fn read(&mut self, buf: &mut [u8]) -> Result<usize, PtyError> {
            Ok(self.output_read.read(buf)?)
        }

        pub(super) fn write(&mut self, buf: &[u8]) -> Result<usize, PtyError> {
            Ok(self.input_write.write(buf)?)
        }

        pub(super) fn resize(&mut self, size: PtySize) -> Result<(), PtyError> {
            unsafe {
                ResizePseudoConsole(
                    self.hpc,
                    COORD {
                        X: size.cols as i16,
                        Y: size.rows as i16,
                    },
                )?;
            }
            Ok(())
        }

        pub(super) fn clone_reader(&self) -> Result<File, PtyError> {
            Ok(self.output_read.try_clone()?)
        }

        pub(super) fn clone_writer(&self) -> Result<File, PtyError> {
            Ok(self.input_write.try_clone()?)
        }

        pub(super) fn is_running(&self) -> Result<bool, PtyError> {
            use windows::Win32::System::Threading::GetExitCodeProcess;
            const STILL_ACTIVE: u32 = 259;
            let mut exit_code = 0u32;
            unsafe {
                GetExitCodeProcess(self.process_handle, &mut exit_code)?;
            }
            Ok(exit_code == STILL_ACTIVE)
        }

        pub(super) fn bytes_available(&self) -> Result<u32, PtyError> {
            let mut available = 0u32;
            unsafe {
                PeekNamedPipe(
                    HANDLE(self.output_read.as_raw_handle() as isize),
                    None,
                    0,
                    None,
                    Some(&mut available),
                    None,
                )?;
            }
            Ok(available)
        }
    }

    impl Drop for PtyInner {
        fn drop(&mut self) {
            unsafe {
                ClosePseudoConsole(self.hpc);
                close_handle(self.conpty_input);
                close_handle(self.conpty_output);
                close_handle(self.process_handle);
                close_handle(self.thread_handle);
            }
        }
    }

    struct HandleGuard {
        handle: HANDLE,
    }

    impl HandleGuard {
        fn new(handle: HANDLE) -> Self {
            Self { handle }
        }

        fn into_inner(self) -> HANDLE {
            let handle = self.handle;
            std::mem::forget(self);
            handle
        }
    }

    impl Drop for HandleGuard {
        fn drop(&mut self) {
            close_handle(self.handle);
        }
    }

    struct PseudoConsoleGuard {
        handle: HPCON,
    }

    impl PseudoConsoleGuard {
        fn new(handle: HPCON) -> Self {
            Self { handle }
        }

        fn into_inner(self) -> HPCON {
            let handle = self.handle;
            std::mem::forget(self);
            handle
        }
    }

    impl Drop for PseudoConsoleGuard {
        fn drop(&mut self) {
            unsafe {
                ClosePseudoConsole(self.handle);
            }
        }
    }

    struct AttrListGuard {
        ptr: LPPROC_THREAD_ATTRIBUTE_LIST,
        _buffer: Vec<u8>,
    }

    impl AttrListGuard {
        fn new(ptr: LPPROC_THREAD_ATTRIBUTE_LIST, buffer: Vec<u8>) -> Self {
            Self {
                ptr,
                _buffer: buffer,
            }
        }
    }

    impl Drop for AttrListGuard {
        fn drop(&mut self) {
            unsafe {
                DeleteProcThreadAttributeList(self.ptr);
            }
        }
    }

    fn create_pipe() -> Result<(HANDLE, HANDLE), PtyError> {
        let mut read_pipe = HANDLE::default();
        let mut write_pipe = HANDLE::default();
        let sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: std::ptr::null_mut(),
            bInheritHandle: BOOL(1),
        };
        unsafe {
            CreatePipe(&mut read_pipe, &mut write_pipe, Some(&sa), 0)?;
        }
        Ok((read_pipe, write_pipe))
    }

    fn set_handle_inherit(handle: HANDLE, inherit: bool) -> Result<(), PtyError> {
        let mask = HANDLE_FLAG_INHERIT.0;
        let value = if inherit {
            HANDLE_FLAG_INHERIT
        } else {
            windows::Win32::Foundation::HANDLE_FLAGS(0)
        };
        unsafe {
            SetHandleInformation(handle, mask, value)?;
        }
        Ok(())
    }

    fn close_handle(handle: HANDLE) {
        if handle.is_invalid() {
            return;
        }
        unsafe {
            let _ = CloseHandle(handle);
        }
    }

    fn raw_handle(handle: HANDLE) -> RawHandle {
        handle.0 as RawHandle
    }

    fn wide_command_line(command: &str) -> Vec<u16> {
        OsStr::new(command)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }
}

#[cfg(not(windows))]
mod platform {
    use super::{PtyError, PtySize};
    use std::fs::File;

    pub(super) struct PtyInner;

    impl PtyInner {
        pub(super) fn spawn(_command: &str, _size: PtySize) -> Result<Self, PtyError> {
            Err(PtyError::UnsupportedPlatform)
        }

        pub(super) fn read(&mut self, _buf: &mut [u8]) -> Result<usize, PtyError> {
            Err(PtyError::UnsupportedPlatform)
        }

        pub(super) fn write(&mut self, _buf: &[u8]) -> Result<usize, PtyError> {
            Err(PtyError::UnsupportedPlatform)
        }

        pub(super) fn resize(&mut self, _size: PtySize) -> Result<(), PtyError> {
            Err(PtyError::UnsupportedPlatform)
        }

        pub(super) fn clone_reader(&self) -> Result<File, PtyError> {
            Err(PtyError::UnsupportedPlatform)
        }

        pub(super) fn clone_writer(&self) -> Result<File, PtyError> {
            Err(PtyError::UnsupportedPlatform)
        }

        pub(super) fn is_running(&self) -> Result<bool, PtyError> {
            Err(PtyError::UnsupportedPlatform)
        }

        pub(super) fn bytes_available(&self) -> Result<u32, PtyError> {
            Err(PtyError::UnsupportedPlatform)
        }
    }
}

use platform::PtyInner;
