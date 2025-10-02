use std::io;
use std::thread;
use std::time::Duration;

/// A minimal safe wrapper around a locked Vec<u8>.
///
/// Design notes:
/// - Owns its buffer and never changes capacity after lock (no push/reserve API exposed).
/// - Attempts `mlock` on construction; gracefully handles Unsupported platforms.
/// - Zeroizes the content before drop, then calls `munlock` if it was locked.
/// - On Linux, it also attempts `madvise(MADV_DONTDUMP)` as a best-effort hint.
///
/// This is an example for demonstration and is not a hardened secret container.
/// For production, prefer a battle-tested type from a dedicated crate and enforce
/// stricter invariants (pinning, poison on failure, etc).
struct LockedVec {
    buf: Vec<u8>,
    locked: bool,
}

impl LockedVec {
    /// Construct a new LockedVec with a fixed length.
    ///
    /// Returns Ok even when `mlock` is Unsupported on this platform/build.
    /// Returns Err for other OS errors (e.g., resource limits).
    pub fn new(len: usize) -> io::Result<Self> {
        // Allocate a zeroed buffer. We won't change capacity after locking.
        let buf = vec![0u8; len];

        // Attempt to lock pages. Treat Unsupported as a non-fatal condition.
        let ptr = buf.as_ptr() as *const std::os::raw::c_void;
        let locked = match unsafe { os_memlock::mlock(ptr, buf.len()) } {
            Ok(()) => true,
            Err(e) if e.kind() == io::ErrorKind::Unsupported => {
                eprintln!(
                    "os-memlock: mlock unsupported on this platform/build; continuing unlocked"
                );
                false
            }
            Err(e) => return Err(e),
        };

        // Best-effort: on Linux, advise dump exclusion.
        #[cfg(target_os = "linux")]
        {
            let mut_ptr = buf.as_mut_ptr() as *mut std::os::raw::c_void;
            match unsafe { os_memlock::madvise_dontdump(mut_ptr, buf.len()) } {
                Ok(()) => (),
                Err(e) if e.kind() == io::ErrorKind::Unsupported => {
                    // Not supported on this target; ignore gracefully.
                }
                Err(e) => {
                    // Non-fatal: just log the error.
                    eprintln!("os-memlock: madvise(MADV_DONTDUMP) failed: {e}");
                }
            }
        }

        Ok(Self { buf, locked })
    }

    /// Length in bytes.
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Get a mutable slice view of the buffer.
    ///
    /// Caller may write secrets here. The buffer will be zeroized on Drop
    /// and `munlock` will be called if the buffer was locked.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.buf
    }

    /// Whether `mlock` succeeded for this buffer.
    pub fn is_locked(&self) -> bool {
        self.locked
    }
}

impl Drop for LockedVec {
    fn drop(&mut self) {
        // Zeroize contents while still locked (if locked).
        for b in &mut self.buf {
            *b = 0;
        }

        if self.locked {
            let ptr = self.buf.as_ptr() as *const std::os::raw::c_void;
            let len = self.buf.len();
            match unsafe { os_memlock::munlock(ptr, len) } {
                Ok(()) => (),
                Err(e) if e.kind() == io::ErrorKind::Unsupported => {
                    // Not supported on this target; ignore.
                }
                Err(e) => {
                    // We can't do much in Drop; log error.
                    eprintln!("os-memlock: munlock failed: {e}");
                }
            }
        }
    }
}

fn main() -> io::Result<()> {
    const LEN: usize = 4096;

    let mut secrets = LockedVec::new(LEN)?;
    println!(
        "LockedVec allocated: {} bytes, locked: {}",
        secrets.len(),
        secrets.is_locked()
    );

    // Write a demo secret (for example purposes).
    // In real usage, manage keys carefully and avoid unnecessary copies.
    let s = secrets.as_mut_slice();
    let demo = b"super-secret-demo";
    if s.len() >= demo.len() {
        s[..demo.len()].copy_from_slice(demo);
    }

    // Simulate work while the buffer is (ideally) locked in memory.
    println!("Working with secret data (simulated)...");
    thread::sleep(Duration::from_millis(200));

    // Drop occurs at end of scope: zeroize then munlock (if locked).
    println!("Example complete; dropping LockedVec (zeroize + munlock).");
    Ok(())
}
