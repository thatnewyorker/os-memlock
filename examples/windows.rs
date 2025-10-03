/*!
Windows-specific example demonstrating usage of the `os-memlock` crate:
- Locking and unlocking a buffer via `VirtualLock`/`VirtualUnlock` (exposed as `mlock`/`munlock`)
- Suppressing common Windows error dialogs process-wide via `SetErrorMode` helpers

Build & run (on Windows):

    cargo run -p os-memlock --example windows

Notes:
- The memory lock/unlock functions are unsafe; callers must uphold the safety contract as
  documented by the crate (valid pointer/length, region remains mapped, etc).
- The error-mode helpers are process-wide and affect global error dialog behavior; they do not
  provide per-region dump exclusion and are unrelated to `madvise(MADV_DONTDUMP)` semantics.
*/

#[cfg(windows)]
fn main() -> std::io::Result<()> {
    use std::io;
    use std::thread;
    use std::time::Duration;

    // Best-effort: suppress common Windows error dialogs for this process.
    // This is process-wide and inherited by children. Not security-related; purely UX/ops.
    let previous_error_mode: io::Result<u32> =
        os_memlock::suppress_windows_error_dialogs_for_process();
    match &previous_error_mode {
        Ok(prev) => println!("Windows error mode adjusted; previous mode: 0x{prev:08x}"),
        Err(e) if e.kind() == io::ErrorKind::Unsupported => {
            println!(
                "suppress_windows_error_dialogs_for_process is unsupported on this platform/build"
            )
        }
        Err(e) => eprintln!("Failed to set Windows error mode: {e}"),
    }

    // Allocate a buffer representing secret data. Use a page-sized allocation for clarity.
    // On most Windows systems a page is 4096 bytes; this example uses that common size.
    const PAGE_LEN: usize = 4096;
    let mut secret = vec![0u8; PAGE_LEN];

    // Put some dummy secret bytes (for demo only).
    secret[..16].copy_from_slice(b"super-secret-data");

    let ptr = secret.as_ptr() as *const std::os::raw::c_void;
    let len = secret.len();

    println!("Attempting to mlock {} bytes at {:p}", len, ptr);

    // Try to mlock the buffer. This is unsafe and can fail due to OS policies/limits.
    match unsafe { os_memlock::mlock(ptr, len) } {
        Ok(()) => println!("mlock (VirtualLock) succeeded"),
        Err(e) if e.kind() == io::ErrorKind::Unsupported => {
            println!("mlock is unsupported on this platform/build; continuing without page-lock")
        }
        Err(e) => return Err(e),
    }

    // Do some work while memory is (hopefully) locked.
    println!("Working with secret data (simulated)...");
    thread::sleep(Duration::from_millis(250));

    // Before dropping or unmapping the buffer, munlock.
    match unsafe { os_memlock::munlock(ptr, len) } {
        Ok(()) => println!("munlock (VirtualUnlock) succeeded"),
        Err(e) if e.kind() == io::ErrorKind::Unsupported => {
            println!("munlock unsupported (no-op for this platform/build)")
        }
        Err(e) => return Err(e),
    }

    // Zeroize secret before drop as good hygiene (example only; use a proper zeroize crate in production).
    for b in &mut secret {
        *b = 0;
    }
    println!("Secret zeroized.");

    // Restore the previous Windows error mode if we changed it.
    if let Ok(prev) = previous_error_mode {
        match os_memlock::set_windows_error_mode(prev) {
            Ok(_old) => println!("Windows error mode restored to 0x{prev:08x}"),
            Err(e) => eprintln!("Failed to restore Windows error mode: {e}"),
        }
    }

    println!("Windows example complete.");
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    println!("This example is Windows-specific. Build and run it on Windows.");
}
