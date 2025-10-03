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
    /// On Linux, this wraps `madvise(MADV_DONTDUMP)`. On FreeBSD, this wraps
    /// `madvise(MADV_NOCORE)`. On other Unix targets, this returns `Unsupported`.
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

    /// FreeBSD: use MADV_NOCORE to request exclusion from core dumps.
    #[cfg(target_os = "freebsd")]
    /// # Safety
    /// The caller must ensure that `(addr, len)` denotes a valid memory mapping for
    /// this process and that the region is not deallocated or remapped concurrently.
    pub unsafe fn madvise_dontdump(addr: *mut c_void, len: usize) -> io::Result<()> {
        if len == 0 {
            return Ok(());
        }
        let rc = libc::madvise(addr, len, libc::MADV_NOCORE);
        if rc == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    /// See `madvise_dontdump` above. On other Unix targets, this is unsupported.
    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    /// # Safety
    /// This function is marked unsafe for signature consistency. On unsupported Unix
    /// targets it always returns `Unsupported`; callers compiling cross-platform
    /// must still treat `(addr, len)` as potentially unsafe inputs.
    pub unsafe fn madvise_dontdump(_addr: *mut c_void, _len: usize) -> io::Result<()> {
        super::unsupported("madvise-based dump exclusion unsupported on this platform")
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

/// Disable core dumps for the current process on macOS by setting the RLIMIT_CORE soft limit to 0.
///
/// Platform:
/// - macOS only. On other platforms, see the cross-platform stub which returns `Unsupported`.
///
/// Behavior:
/// - This is a process-wide policy and is inherited by child processes.
/// - Lowering the soft limit is typically permitted; raising it back may require extra privileges.
/// - May fail in sandboxed or restricted environments; returns `io::Error` from the OS.
///
/// Returns:
/// - `Ok(())` on success.
/// - `Err(io::Error)` with `last_os_error()` on failure.
#[cfg(target_os = "macos")]
#[cfg_attr(docsrs, doc(cfg(target_os = "macos")))]
pub fn disable_core_dumps_for_process() -> io::Result<()> {
    // Fetch existing limits so we can preserve the hard limit (rlim_max).
    let mut old = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let rc = unsafe { libc::getrlimit(libc::RLIMIT_CORE, &mut old as *mut _) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    // Set the soft limit to 0 to disable core dump generation for the process.
    let new_lim = libc::rlimit {
        rlim_cur: 0,
        rlim_max: old.rlim_max,
    };
    let rc2 = unsafe { libc::setrlimit(libc::RLIMIT_CORE, &new_lim as *const _) };
    if rc2 != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Disable core dumps for the current process.
///
/// Platform:
/// - This stub is compiled on non-macOS targets and always returns `Unsupported`.
///
/// See also:
/// - On macOS, `disable_core_dumps_for_process` attempts to set `RLIMIT_CORE` to 0.
#[cfg(not(target_os = "macos"))]
#[cfg_attr(docsrs, doc(cfg(not(target_os = "macos"))))]
pub fn disable_core_dumps_for_process() -> io::Result<()> {
    unsupported("disable_core_dumps_for_process unsupported on this platform")
}

/// RAII guard that disables core dumps on macOS and restores the previous RLIMIT_CORE on drop.
///
/// On non-macOS platforms, this type is still defined to keep cross-platform signatures
/// consistent, but creating it is not possible via this crate's API.
#[derive(Debug)]
pub struct CoreDumpsDisabledGuard {
    #[cfg(target_os = "macos")]
    old: libc::rlimit,
}

#[cfg(target_os = "macos")]
impl Drop for CoreDumpsDisabledGuard {
    fn drop(&mut self) {
        // Best-effort: restore previous soft/hard core limits.
        let rc = unsafe { libc::setrlimit(libc::RLIMIT_CORE, &self.old as *const _) };
        if rc != 0 {
            // Avoid panicking in Drop; emit a diagnostic.
            eprintln!(
                "os-memlock: failed to restore RLIMIT_CORE: {}",
                io::Error::last_os_error()
            );
        }
    }
}

/// Disable core dumps for the current process and return a guard that restores the previous limit on drop.
///
/// Platform:
/// - macOS only. On other platforms, this function returns `Unsupported`.
///
/// Behavior:
/// - Sets RLIMIT_CORE soft limit to 0; guard restores previous limit on Drop.
#[cfg(target_os = "macos")]
#[cfg_attr(docsrs, doc(cfg(target_os = "macos")))]
pub fn disable_core_dumps_with_guard() -> io::Result<CoreDumpsDisabledGuard> {
    let mut old = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let rc = unsafe { libc::getrlimit(libc::RLIMIT_CORE, &mut old as *mut _) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    let new_lim = libc::rlimit {
        rlim_cur: 0,
        rlim_max: old.rlim_max,
    };
    let rc2 = unsafe { libc::setrlimit(libc::RLIMIT_CORE, &new_lim as *const _) };
    if rc2 != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(CoreDumpsDisabledGuard { old })
}

#[cfg(not(target_os = "macos"))]
#[cfg_attr(docsrs, doc(cfg(not(target_os = "macos"))))]
pub fn disable_core_dumps_with_guard() -> io::Result<CoreDumpsDisabledGuard> {
    unsupported("disable_core_dumps_with_guard unsupported on this platform")
}

// Re-export platform module functions at the crate root for a stable API.
#[cfg(unix)]
#[cfg_attr(docsrs, doc(cfg(unix)))]
pub use unix::{madvise_dontdump, mlock, munlock};

#[cfg(not(unix))]
#[cfg_attr(docsrs, doc(cfg(not(unix))))]
pub use non_unix::{madvise_dontdump, mlock, munlock};

#[cfg(test)]
mod tests {
    #[test]
    fn smoke_disable_core_dumps_for_process() {
        let _ = crate::disable_core_dumps_for_process();
        let _ = crate::disable_core_dumps_with_guard();
    }
}
