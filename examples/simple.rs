/*!
Simple example demonstrating usage of the `os-memlock` crate.

Build & run (from workspace root):

    cargo run -p os-memlock --example simple --features locked-memory

Notes:
- This example shows the minimal unsafe calls and handles the "unsupported" case
  gracefully (the crate intentionally returns `io::ErrorKind::Unsupported` when
  the platform or build configuration does not provide the underlying syscalls).
- The functions here are unsafe; callers must uphold the safety contract documented
  in the crate README (valid pointer/length, region remains mapped, etc).
*/

use std::io;
use std::thread;
use std::time::Duration;

fn main() -> io::Result<()> {
    // Simple buffer representing secret data. Use a page-sized allocation for clarity.
    // On many systems a page is 4096 bytes; this example uses that common size.
    const PAGE_LEN: usize = 4096;
    let mut secret = vec![0u8; PAGE_LEN];

    // Put some dummy secret bytes (for demo only).
    secret[..16].copy_from_slice(b"super-secret-data");

    let ptr = secret.as_ptr() as *const std::os::raw::c_void;
    let _mut_ptr = secret.as_mut_ptr() as *mut std::os::raw::c_void;
    let len = secret.len();

    println!("Attempting to lock {} bytes at {:p}", len, ptr);

    // Try to mlock the buffer. This is unsafe and may return Unsupported on some builds/platforms.
    match unsafe { os_memlock::mlock(ptr, len) } {
        Ok(()) => println!("mlock succeeded"),
        Err(e) if e.kind() == io::ErrorKind::Unsupported => {
            println!("mlock is unsupported on this platform/build; continuing without page-lock")
        }
        Err(e) => return Err(e),
    }

    // Best-effort: on Linux, advise the kernel not to include the mapping in core dumps.
    #[cfg(target_os = "linux")]
    match unsafe { os_memlock::madvise_dontdump(mut_ptr, len) } {
        Ok(()) => println!("madvise(MADV_DONTDUMP) applied"),
        Err(e) if e.kind() == io::ErrorKind::Unsupported => {
            println!("madvise(MADV_DONTDUMP) unsupported on this platform/build")
        }
        Err(e) => eprintln!("madvise failed: {:#}", e),
    }

    // Do some work while memory is (hopefully) locked.
    println!("Working with secret data (simulated)...");
    // Sleep briefly to simulate lifetime of locked secret. (In real code avoid sleeping.)
    thread::sleep(Duration::from_millis(250));

    // Before dropping or unmapping the buffer, munlock.
    match unsafe { os_memlock::munlock(ptr, len) } {
        Ok(()) => println!("munlock succeeded"),
        Err(e) if e.kind() == io::ErrorKind::Unsupported => {
            println!("munlock unsupported (no-op for this platform/build)")
        }
        Err(e) => return Err(e),
    }

    // Zeroize secret before drop as a good hygiene (example only; use a proper zeroize crate in production).
    for b in &mut secret {
        *b = 0;
    }

    println!("Secret zeroized and example complete.");
    Ok(())
}
