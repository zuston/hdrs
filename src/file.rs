use std::io::{Error, ErrorKind, Read, Result, Seek, SeekFrom, Write};
use std::ptr;

use hdfs_sys::*;
use libc::c_void;
use log::{debug, error, warn};

use crate::Client;

// at most 2^30 bytes, ~1GB
const FILE_LIMIT: usize = 1073741824;

/// File will hold the underlying pointer to `hdfsFile`.
///
/// The internal file is closed on `Drop`. Because `Drop` cannot return an error, writers that need
/// to observe close failures should call [`File::close`] explicitly.
///
/// # Examples
///
/// ```no_run
/// use hdrs::{Client, ClientBuilder};
///
/// let fs = ClientBuilder::new("default")
///     .with_user("default")
///     .with_kerberos_ticket_cache_path("/tmp/krb5_111")
///     .connect()
///     .expect("client connect succeed");
/// let mut f = fs
///     .open_file()
///     .read(true)
///     .open("/tmp/hello.txt")
///     .expect("must open success");
/// ```
#[derive(Debug)]
pub struct File {
    fs: hdfsFS,
    f: hdfsFile,
    path: String,
}

/// HDFS's client handle is thread safe.
unsafe impl Send for File {}
unsafe impl Sync for File {}

impl Drop for File {
    fn drop(&mut self) {
        if let Err(error) = self.close_inner() {
            error!("Failed to close HDFS file during drop. error: {error}");
        }
    }
}

impl File {
    pub(crate) fn new(fs: hdfsFS, f: hdfsFile, path: &str) -> Self {
        File {
            fs,
            f,
            path: path.to_string(),
        }
    }

    fn close_inner_with<F, E>(&mut self, close_file: F, close_error: E) -> Result<()>
    where
        F: FnOnce(hdfsFS, hdfsFile) -> i32,
        E: FnOnce(&str) -> Error,
    {
        if self.f.is_null() {
            return Ok(());
        }

        let file = self.f;
        // hdfsCloseFile frees a valid file handle even when it returns an I/O error. Clear the
        // pointer before inspecting the result so Drop never attempts to close it again.
        self.f = ptr::null_mut();

        if close_file(self.fs, file) == 0 {
            debug!("HDFS file {} has been closed", self.path);
            Ok(())
        } else {
            Err(close_error(&self.path))
        }
    }

    fn close_inner(&mut self) -> Result<()> {
        self.close_inner_with(
            |fs, file| unsafe { hdfsCloseFile(fs, file) },
            |path| {
                let os_error = Error::last_os_error();
                let root_cause = last_hdfs_error();
                Error::new(
                    os_error.kind(),
                    format!(
                        "failed to close HDFS file {}: {}; OS error: {}",
                        path, root_cause, os_error
                    ),
                )
            },
        )
    }

    /// Closes the file and reports any error returned by HDFS.
    ///
    /// This method consumes the file because `hdfsCloseFile` frees the native file handle even
    /// when closing fails. Relying on `Drop` still closes the handle, but any close error can only
    /// be logged and cannot be returned to the caller.
    pub fn close(mut self) -> Result<()> {
        self.close_inner()
    }

    /// Works only for files opened in read-only mode.
    fn inner_seek(&self, offset: i64) -> Result<()> {
        let n = unsafe { hdfsSeek(self.fs, self.f, offset) };

        if n == -1 {
            return Err(Error::last_os_error());
        }

        Ok(())
    }

    fn tell(&self) -> Result<i64> {
        let n = unsafe { hdfsTell(self.fs, self.f) };

        if n == -1 {
            return Err(Error::last_os_error());
        }

        Ok(n)
    }

    pub fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize> {
        let n = unsafe {
            hdfsPread(
                self.fs,
                self.f,
                offset as i64,
                buf.as_ptr() as *mut c_void,
                buf.len().min(FILE_LIMIT) as i32,
            )
        };

        if n == -1 {
            return Err(Error::last_os_error());
        }

        Ok(n as usize)
    }
}

impl Read for File {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let n = unsafe {
            hdfsRead(
                self.fs,
                self.f,
                buf.as_ptr() as *mut c_void,
                buf.len().min(FILE_LIMIT) as i32,
            )
        };

        if n == -1 {
            return Err(Error::last_os_error());
        }

        Ok(n as usize)
    }
}

impl Seek for File {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        match pos {
            SeekFrom::Start(n) => {
                self.inner_seek(n as i64)?;
                Ok(n)
            }
            SeekFrom::Current(n) => {
                let current = self.tell()?;
                let offset = (current + n) as u64;
                self.inner_seek(offset as i64)?;
                Ok(offset)
            }
            SeekFrom::End(n) => {
                let meta = Client::new(self.fs).metadata(&self.path)?;
                let offset = meta.len() as i64 + n;
                self.inner_seek(offset)?;
                Ok(offset as u64)
            }
        }
    }
}

impl Write for File {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let n = unsafe {
            hdfsWrite(
                self.fs,
                self.f,
                buf.as_ptr() as *const c_void,
                buf.len().min(FILE_LIMIT) as i32,
            )
        };

        if n == -1 {
            error!("Errors on writing. error: {:?}", last_hdfs_error());
            return Err(Error::last_os_error());
        }

        Ok(n as usize)
    }

    fn flush(&mut self) -> Result<()> {
        let n = unsafe { hdfsFlush(self.fs, self.f) };

        if n == -1 {
            error!("Errors on flushing. error: {:?}", last_hdfs_error());
            return Err(Error::last_os_error());
        }

        Ok(())
    }
}

impl Read for &File {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let n = unsafe {
            hdfsRead(
                self.fs,
                self.f,
                buf.as_ptr() as *mut c_void,
                buf.len().min(FILE_LIMIT) as i32,
            )
        };

        if n == -1 {
            return Err(Error::last_os_error());
        }

        Ok(n as usize)
    }
}

impl Seek for &File {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        match pos {
            SeekFrom::Start(n) => {
                self.inner_seek(n as i64)?;
                Ok(n)
            }
            SeekFrom::Current(n) => {
                let current = self.tell()?;
                let offset = (current + n) as u64;
                self.inner_seek(offset as i64)?;
                Ok(offset)
            }
            SeekFrom::End(_) => Err(Error::new(
                ErrorKind::Unsupported,
                "hdfs doesn't support seek from end",
            )),
        }
    }
}

pub fn last_hdfs_error() -> String {
    let root_cause = unsafe { hdfsGetLastExceptionRootCause() };
    if root_cause.is_null() {
        warn!("hdfsGetLastExceptionRootCause returned null");
        "unknown error".to_string()
    } else {
        unsafe {
            std::ffi::CStr::from_ptr(root_cause)
                .to_string_lossy()
                .into_owned()
        }
    }
}

impl Write for &File {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let n = unsafe {
            hdfsWrite(
                self.fs,
                self.f,
                buf.as_ptr() as *const c_void,
                buf.len().min(FILE_LIMIT) as i32,
            )
        };

        if n == -1 {
            error!("Errors on writing. error: {:?}", last_hdfs_error());
            return Err(Error::last_os_error());
        }

        Ok(n as usize)
    }

    fn flush(&mut self) -> Result<()> {
        let n = unsafe { hdfsFlush(self.fs, self.f) };

        if n == -1 {
            error!("Errors on flushing. error: {:?}", last_hdfs_error());
            return Err(Error::last_os_error());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::ClientBuilder;

    #[test]
    fn test_file_build() {
        let _ = env_logger::try_init();

        let fs = ClientBuilder::new("default")
            .connect()
            .expect("init success");

        let path = uuid::Uuid::new_v4().to_string();

        let f = fs
            .open_file()
            .create(true)
            .write(true)
            .open(&format!("/tmp/{path}"))
            .expect("open file success");

        assert!(!f.f.is_null());
        assert!(!f.fs.is_null());
    }

    #[test]
    fn test_file_write() {
        let _ = env_logger::try_init();

        let fs = ClientBuilder::new("default")
            .connect()
            .expect("init success");

        let path = uuid::Uuid::new_v4().to_string();

        let mut f = fs
            .open_file()
            .create(true)
            .write(true)
            .open(&format!("/tmp/{path}"))
            .expect("open file success");

        let n = f
            .write("Hello, World!".as_bytes())
            .expect("write must success");
        assert_eq!(n, 13);
        f.close().expect("close must success");
    }

    #[test]
    fn test_close_error_is_returned_and_handle_is_consumed() {
        let fs = std::ptr::NonNull::<hdfs_internal>::dangling().as_ptr();
        let handle = std::ptr::NonNull::<hdfsFile_internal>::dangling().as_ptr();
        let mut file = File::new(fs, handle, "/tmp/test-close");
        let mut close_calls = 0;

        let error = file
            .close_inner_with(
                |actual_fs, actual_handle| {
                    assert_eq!(actual_fs, fs);
                    assert_eq!(actual_handle, handle);
                    close_calls += 1;
                    -1
                },
                |path| Error::other(format!("simulated close failure for {path}")),
            )
            .expect_err("close failure must be returned");

        assert_eq!(error.kind(), ErrorKind::Other);
        assert_eq!(
            error.to_string(),
            "simulated close failure for /tmp/test-close"
        );
        assert!(file.f.is_null());
        file.close_inner_with(
            |_, _| panic!("closed handle must not be closed again"),
            |_| panic!("closed handle must not produce another error"),
        )
        .expect("closing an already closed handle must succeed");
        assert_eq!(close_calls, 1);
    }

    #[test]
    fn test_file_read() {
        let _ = env_logger::try_init();

        let fs = ClientBuilder::new("default")
            .connect()
            .expect("init success");

        let path = uuid::Uuid::new_v4().to_string();

        let mut f = fs
            .open_file()
            .create(true)
            .write(true)
            .open(&format!("/tmp/{path}"))
            .expect("open file success");

        let n = f
            .write("Hello, World!".as_bytes())
            .expect("write must success");
        assert_eq!(n, 13)
    }
}
