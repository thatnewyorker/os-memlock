#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, deny(broken_intra_doc_links))]
#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

use std::io;
use std::os::raw::c_void;

#[inline]
fn unsupported(msg: &'static str) -> io::Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, msg))
}

#[cfg(unix)]
mod unix {
    use super::{c_void, io};

    /// Lock the pages containing the specified memory region to prevent swapping.
    ///
    /// Returns:
    /// - `Ok(())` on success
    /// - `Err(...)` with `last_os_error()` on failure
    /// - `Err(Unsupported)` on platforms where this call is not available
    ///
    /// # Safety
    /// The caller must ensure that `(addr, len)` refers to a valid, non-null memory
    /// region owned by this process for the duration of the call, and that the region
    /// is not deallocated, unmapped, or remapped concurrently.
    ///
    /// # Examples
    /// ```no_run
    /// # use std::io;
    /// # fn demo() -> io::Result<()> {
    /// let mut buf = vec![0u8; 4096];
    /// let ptr = buf.as_ptr() as *const std::os::raw::c_void;
    /// let len = buf.len();
    /// match unsafe { os_memlock::mlock(ptr, len) } {
    ///     Ok(()) => {
    ///         // do work...
    ///         unsafe { os_memlock::munlock(ptr, len)?; }
    ///     }
    ///     Err(e) if e.kind() == std::io::ErrorKind::Unsupported => {
    ///         // platform/build doesn't support mlock; proceed without locking
    ///     }
    ///     Err(e) => return Err(e),
    /// }
    /// # Ok(()) }
    /// ```
    pub unsafe fn mlock(addr: *const c_void, len: usize) -> io::Result<()> {
        if len == 0 {
            // Treat zero-length as a no-op success for ergonomic callers.
            return Ok(());
        }
        // Safety:
        // - We do not dereference `addr`.
        // - Caller guarantees `(addr, len)` is a valid region they own during the call.
        let rc = unsafe { libc::mlock(addr, len) };
        if rc == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    /// Unlock the pages containing the specified memory region.
    ///
    /// Returns:
    /// - `Ok(())` on success
    /// - `Err(...)` with `last_os_error()` on failure
    /// - `Err(Unsupported)` on platforms where this call is not available
    ///
    /// # Safety
    /// The caller must ensure that `(addr, len)` refers to a valid, non-null memory
    /// region owned by this process for the duration of the call, and that the region
    /// is not deallocated, unmapped, or remapped concurrently.
    pub unsafe fn munlock(addr: *const c_void, len: usize) -> io::Result<()> {
        if len == 0 {
            // Treat zero-length as a no-op success for ergonomic callers.
            return Ok(());
        }
        // Safety: same preconditions as `mlock`.
        let rc = unsafe { libc::munlock(addr, len) };
        if rc == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    /// Best-effort advisory to exclude the memory region from core dumps.
    ///
    /// On Linux, this wraps `madvise(MADV_DONTDUMP)`. On other Unix targets,
    /// this returns `Unsupported`.
    ///
    /// Returns:
    /// - `Ok(())` when the hint is applied
    /// - `Err(...)` with `last_os_error()` if the call failed
    /// - `Err(Unsupported)` if not supported on this platform
    ///
    /// # Safety
    /// The caller must ensure that `(addr, len)` denotes a valid memory mapping for
    /// this process and that the region is not deallocated or remapped concurrently.
    #[cfg(target_os = "linux")]
    pub unsafe fn madvise_dontdump(addr: *mut c_void, len: usize) -> io::Result<()> {
        if len == 0 {
            return Ok(());
        }
        // Safety:
        // - We do not dereference `addr`.
        // - Caller guarantees `(addr, len)` is a valid region they own during the call.
        let rc = libc::madvise(addr, len, libc::MADV_DONTDUMP);
        if rc == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    /// See `madvise_dontdump` above. On non-Linux Unix targets, this is unsupported.
    #[cfg(not(target_os = "linux"))]
    /// # Safety
    /// This function is marked unsafe for signature consistency. On non-Linux Unix
    /// targets it always returns `Unsupported`; callers compiling cross-platform
    /// must still treat `(addr, len)` as potentially unsafe inputs.
    pub unsafe fn madvise_dontdump(_addr: *mut c_void, _len: usize) -> io::Result<()> {
        super::unsupported("madvise(MADV_DONTDUMP) unsupported on this platform")
    }
}

#[cfg(not(unix))]
mod non_unix {
    use super::{c_void, io};

    /// # Safety
    /// This function is marked unsafe for signature consistency across platforms.
    /// On non-Unix targets it always returns `Unsupported`; callers compiling
    /// cross-platform must still treat `(addr, len)` as potentially unsafe inputs.
    pub unsafe fn mlock(_addr: *const c_void, _len: usize) -> io::Result<()> {
        super::unsupported("mlock unsupported on this platform")
    }

    /// # Safety
    /// This function is marked unsafe for signature consistency across platforms.
    /// On non-Unix targets it always returns `Unsupported`; callers compiling
    /// cross-platform must still treat `(addr, len)` as potentially unsafe inputs.
    pub unsafe fn munlock(_addr: *const c_void, _len: usize) -> io::Result<()> {
        super::unsupported("munlock unsupported on this platform")
    }

    /// # Safety
    /// This function is marked unsafe for signature consistency across platforms.
    /// On non-Unix targets it always returns `Unsupported`; callers compiling
    /// cross-platform must still treat `(addr, len)` as potentially unsafe inputs.
    pub unsafe fn madvise_dontdump(_addr: *mut c_void, _len: usize) -> io::Result<()> {
        super::unsupported("madvise(MADV_DONTDUMP) unsupported on this platform")
    }
}

// Re-export platform module functions at the crate root for a stable API.
#[cfg(unix)]
#[cfg_attr(docsrs, doc(cfg(unix)))]
pub use unix::{madvise_dontdump, mlock, munlock};

#[cfg(not(unix))]
#[cfg_attr(docsrs, doc(cfg(not(unix))))]
pub use non_unix::{madvise_dontdump, mlock, munlock};
