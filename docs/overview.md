# os-memlock: Overview and Deep-Dive

This document provides a comprehensive overview of the `os-memlock` crate: its goals, API, platform behavior, safety model, usage patterns, testing guidance, and operational considerations. It is intended for engineers integrating low-level memory locking primitives into security-conscious applications and libraries.

- Crate: `os-memlock`
- Purpose: Thin, unsafe wrappers around OS memory locking syscalls and adjacent hints
- Public API: `unsafe fn mlock`, `unsafe fn munlock`, `unsafe fn madvise_dontdump` (Linux and FreeBSD), `fn disable_core_dumps_for_process` (macOS; Unsupported elsewhere), and `fn disable_core_dumps_with_guard() -> CoreDumpsDisabledGuard` (macOS; Unsupported elsewhere)
- License: MIT OR Apache-2.0
- Docs: https://docs.rs/os-memlock
- Repository: https://github.com/thatnewyorker/Conflux

--------------------------------------------------------------------------------

## TL;DR

- `os-memlock` centralizes the minimal unsafe FFI surface to lock and unlock memory pages (`mlock`/`munlock`) and to exclude mappings from core dumps on Linux (`madvise(MADV_DONTDUMP)`).
- All functions are `unsafe` and require strict caller-side guarantees about pointer validity, lifetime, and concurrency.
- The crate makes no policy decisions (ownership, alignment, lifetime management, zeroization). Build safe abstractions on top (RAII wrappers, guards) to enforce those in your application.

--------------------------------------------------------------------------------

## Design Goals

- Minimal, audit-friendly FFI: Keep unsafe code small and explicit.
- Predictable cross-platform behavior: On unsupported platforms, return `io::ErrorKind::Unsupported`.
- Honest signatures: Use raw pointers and lengths to reflect kernel semantics; do not pretend safety where none exists.
- Ergonomics in small ways only: Zero-length regions are a no-op that return `Ok(())`.

### Non-Goals

- This crate does not provide a “safe” secret container or memory allocator.
- It does not attempt to align, pin, or page-bound buffers for you.
- It does not manage lifetimes or guarantee timely unlock during panics (you must design for that at a higher level).

--------------------------------------------------------------------------------

## Public API (Summary)

- `unsafe fn mlock(addr: *const c_void, len: usize) -> io::Result<()>`
  - Locks pages covering the memory region, preventing swap.
  - Returns `Unsupported` when unavailable on the target.

- `unsafe fn munlock(addr: *const c_void, len: usize) -> io::Result<()>`
  - Unlocks previously locked pages for the region.
  - Returns `Unsupported` when unavailable on the target.

- `unsafe fn madvise_dontdump(addr: *mut c_void, len: usize) -> io::Result<()>` (Linux and FreeBSD)
  - Advises the kernel not to include the mapping in core dumps (Linux: `MADV_DONTDUMP`, FreeBSD: `MADV_NOCORE`).
  - Returns `Unsupported` on targets other than Linux and FreeBSD.

- `fn disable_core_dumps_for_process() -> io::Result<()>` (macOS)
  - Disables core dumps for the current process by setting the `RLIMIT_CORE` soft limit to 0 (process-wide).
  - On non-macOS targets, returns `Unsupported`.
  - Process-wide effect and inherited by child processes; lowering is typically permitted, raising back may require privileges or be disallowed by policy.

Notes
- Zero-length `len == 0` is treated as a no-op success for ergonomics.
- Pointers are never dereferenced by this crate; however, invalid pointers may still cause kernel errors or undefined behavior at the OS boundary.

--------------------------------------------------------------------------------

## Safety Model and Caller Obligations

All functions are `unsafe`. Callers must uphold the following:

1) Valid region and ownership
- `(addr, len)` must denote a valid memory region owned by the process for the call’s duration.
- The region must be mapped and addressable. Passing invalid or stale pointers is undefined behavior at the OS boundary.

2) No concurrent unmapping or reallocation
- The region must not be concurrently deallocated, unmapped, or remapped while the syscall is in-flight.

3) Lifetime and policy
- If you lock memory to protect secrets, ensure:
  - The secret’s lifetime is controlled while locked.
  - `munlock` is called before deallocation/unmapping (prefer RAII).
  - The secret is zeroized before unlock/drop (use `zeroize` or equivalent).

4) Page granularity
- Kernels operate at page granularity. The OS may round down the base address and round up the length to page boundaries. Design your wrappers with this in mind.

5) Platform constraints
- Be prepared for `Unsupported` errors on platforms without the syscall(s) or on restricted environments.
- On Linux, `mlock` may fail due to `RLIMIT_MEMLOCK` or missing capabilities.

--------------------------------------------------------------------------------

## Cross-Platform Behavior

- Linux
  - `mlock` and `munlock`: call into `libc::mlock` and `libc::munlock`.
  - `madvise_dontdump`: calls `madvise(..., MADV_DONTDUMP)`.
  - Subject to `RLIMIT_MEMLOCK` and capabilities (e.g., `CAP_IPC_LOCK`).
  - Cgroups and container limits may further constrain locked memory.

- Other Unix (macOS, BSDs)
  - `mlock` and `munlock`: call into libc equivalents.
  - `madvise_dontdump`: on FreeBSD, best-effort via `MADV_NOCORE`; on macOS, returns `Unsupported`. Semantics remain advisory and platform-specific.

- Non-Unix (e.g., Windows)
  - All functions return `Unsupported`.
  - This crate currently does not map to Windows APIs like `VirtualLock`/`VirtualUnlock`.

--------------------------------------------------------------------------------

## Error Model

- `Ok(())` on success.
- `Err(io::ErrorKind::Unsupported)` when the syscall or hint is not available on this platform/target.
- Other `io::Error` values reflect `errno` from the kernel:
  - `EPERM`/`EACCES`: Insufficient privileges or capabilities (e.g., beyond `RLIMIT_MEMLOCK`).
  - `ENOMEM`: Not enough resources; region too large, or limits exceeded.
  - `EINVAL`: Invalid address or length parameters.
  - `EFAULT`: Address points outside accessible address space (invalid pointer).
- Treat `Unsupported` as an operational signal, not necessarily a failure, depending on your policy.

--------------------------------------------------------------------------------

## Usage Patterns

- Minimal unsafe usage
  - Wrap calls in a small unsafe scope and propagate errors with context.
- RAII wrapper (recommended)
  - Encapsulate allocation, lock, zeroization, and unlock in a type that guarantees correct ordering and lifetimes.
- Feature gating in larger applications
  - Provide a feature flag or runtime policy to downgrade gracefully on unsupported platforms. Log or emit metrics when falling back.

### Minimal unsafe example

```/dev/null/example_minimal.rs#L1-40
use std::io;

fn try_lock(buf: &mut [u8]) -> io::Result<()> {
    let ptr = buf.as_ptr() as *const std::os::raw::c_void;
    let len = buf.len();

    // Safety: caller owns `buf` and guarantees it's valid during this call.
    match unsafe { os_memlock::mlock(ptr, len) } {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::Unsupported => {
            // Platform/build does not support mlock; continue without page-lock.
            Ok(())
        }
        Err(e) => Err(e),
    }
}
```

### RAII wrapper sketch

```/dev/null/example_raii.rs#L1-120
use std::io;

pub struct LockedBuf {
    buf: Vec<u8>,
    locked: bool,
}

impl LockedBuf {
    pub fn new(len: usize) -> io::Result<Self> {
        let mut buf = vec![0u8; len];
        let ptr = buf.as_ptr() as *const std::os::raw::c_void;
        let locked = match unsafe { os_memlock::mlock(ptr, buf.len()) } {
            Ok(()) => true,
            Err(e) if e.kind() == io::ErrorKind::Unsupported => false,
            Err(e) => return Err(e),
        };
        Ok(Self { buf, locked })
    }

    pub fn as_mut(&mut self) -> &mut [u8] {
        &mut self.buf
    }
}

impl Drop for LockedBuf {
    fn drop(&mut self) {
        // Zeroize first.
        for b in &mut self.buf {
            *b = 0;
        }
        // Then unlock if we actually locked.
        if self.locked {
            let ptr = self.buf.as_ptr() as *const std::os::raw::c_void;
            let len = self.buf.len();
            let _ = unsafe { os_memlock::munlock(ptr, len) };
        }
    }
}
```

--------------------------------------------------------------------------------

## Operational Notes and Limits

- Page size
  - Typically 4 KiB on many systems; confirm with `libc::sysconf(_SC_PAGESIZE)` or Rust’s `page_size` from a helper crate if you need alignment-sensitive behavior.
- RLIMIT_MEMLOCK (Linux)
  - The maximum amount of memory that unprivileged processes may lock. Exceeding it yields errors. Consider exposing configuration to tune sizes or fallback.
- Capabilities and privileges
  - `mlock` may require `CAP_IPC_LOCK` or elevated limits. In containerized environments, these may be disabled by default.
- Cgroup constraints
  - Memory/cgroup policies may further restrict locking.
- Performance
  - Locking too much memory can degrade system behavior. Favor small, targeted regions containing only the data that must be protected.

--------------------------------------------------------------------------------

## Patterns for Safer Usage

- Minimize the locked footprint
  - Lock only the specific buffers containing secrets. Avoid locking large unrelated data.
- Predictable lifetimes
  - Tie lock and unlock to RAII Drop semantics; avoid manually matching calls across distant code paths.
- Zeroization
  - Explicitly zeroize secrets before unlock and drop.
- Error handling
  - Classify `Unsupported` as a “degraded mode” rather than a fatal error if your product can operate without lock guarantees.
- Observability
  - Emit logs/metrics on lock failures or unsupported paths to monitor drift from intended security posture.

--------------------------------------------------------------------------------

## Platform-Specific Guidance

- Linux
  - `mlock`/`munlock`: available.
  - `madvise_dontdump`: available via `MADV_DONTDUMP` (best-effort advisory).
  - Consider using `madvise_dontdump` for buffers containing secrets to reduce exposure in core dumps; treat as advisory, not a hard guarantee.
- FreeBSD
  - `mlock`/`munlock`: available.
  - `madvise_dontdump`: available via `MADV_NOCORE` (best-effort advisory).
- macOS
  - `mlock`/`munlock`: available.
  - `madvise_dontdump`: returns `Unsupported` (no per-region dump-exclusion advice on Darwin).
  - Process-wide helpers: `disable_core_dumps_for_process()` to disable core dumps for the process, and `disable_core_dumps_with_guard() -> CoreDumpsDisabledGuard` to temporarily disable and automatically restore the previous `RLIMIT_CORE` on Drop. Best-effort and may fail in sandboxed/restricted environments. Effect is inherited by child processes at fork; dropping the guard in the parent does not retroactively change limits of already-forked children. Lowering limits is generally permitted; raising them back may require additional privileges.
- Non-Unix
  - All functions return `Unsupported`.

--------------------------------------------------------------------------------

## Testing and CI

- Unit tests for higher-level wrappers
  - When building safe abstractions, prefer to test the higher-level logic (zeroize-on-drop, lifetime, error mapping).
- Privilege-sensitive tests
  - Avoid tests that require elevated privileges or large locked memory on CI. Use small buffers and be prepared for `Unsupported`.
- Docs tests
  - Use `no_run` in doctests to make them compile on docs.rs without requiring privileged syscalls.

Example doctest-friendly snippet:

```/dev/null/doc_no_run.rs#L1-30
/// ```no_run
/// # use std::io;
/// # fn example() -> io::Result<()> {
/// let mut buf = vec![0u8; 4096];
/// match unsafe { os_memlock::mlock(buf.as_ptr() as *const _, buf.len()) } {
///     Ok(()) => unsafe { os_memlock::munlock(buf.as_ptr() as *const _, buf.len())?; },
///     Err(e) if e.kind() == io::ErrorKind::Unsupported => { /* degrade gracefully */ }
///     Err(e) => return Err(e),
/// }
/// # Ok(()) }
/// ```
```

--------------------------------------------------------------------------------

## docs.rs Integration

- The crate is configured to:
  - Include README as crate-level docs.
  - Display `#[doc(cfg(...))]` badges under docs.rs (gated by `--cfg docsrs`).
  - Build with `all-features = true` for complete coverage of feature-gated APIs (if any are added in the future).

Implications
- Platform availability is clearly annotated in the docs (e.g., “cfg(unix)”).
- Doctests should compile consistently on docs.rs. Use `no_run` to avoid executing syscalls.

--------------------------------------------------------------------------------

## Versioning and Stability

- SemVer policy
  - Prior to 1.0, breaking changes may occur in minor versions (0.x.y). After 1.0, public API changes will follow SemVer strictly.
- API surface is intentionally small
  - Additions will be considered carefully. Platform-specific expansions (e.g., BSD dump-exclusion hints) may be introduced behind cfgs or feature flags.

--------------------------------------------------------------------------------

## Security Considerations

- Threat model
  - Memory locking reduces the risk of secrets being written to swap, but it is not a panacea. Physical attacks, kernel compromise, DMA, or process memory leaks remain in scope.
  - `madvise_dontdump` is an advisory hint and does not guarantee that secrets will never appear in core dumps under all circumstances.
- Defense-in-depth
  - Combine `mlock` with:
    - Minimizing secret lifetimes.
    - Avoiding copies of sensitive data.
    - Zeroization on drop/failure paths.
    - Limiting logs/telemetry exposure.
- Auditing
  - Keep unsafe usage localized (as this crate does) and audit wrapper layers that enforce policy.

--------------------------------------------------------------------------------

## FAQ

- Why “unsafe” functions?
  - The OS interface is inherently unsafe: it operates on raw pointers and regions with no lifetime tracking. The API reflects that reality, pushing safety enforcement to the caller or safe wrappers.

- Why not provide a safe, high-level secret container?
  - This crate intentionally separates the low-level primitives. You can build a safe container on top tailored to your application’s policy (alignment, pinning, zeroization strategy, lifetime guarantees).

- Why not implement `madvise` on non-Linux Unixes?
  - Semantics and availability vary (e.g., `MADV_NOCORE`). This crate opts for an explicit Linux-only implementation; other platforms return `Unsupported`. Future versions may add optional support where a stable, portable contract is possible.

- Why not `mlockall`?
  - `mlockall` locks the entire address space and can have significant system impact. This crate focuses on precise, region-based locking that is safer to integrate incrementally.

--------------------------------------------------------------------------------

## Examples Directory

- `examples/simple.rs`
  - Minimal usage with safe error handling and Linux/FreeBSD dump-exclusion hint.
- `examples/locked_vec.rs`
  - A minimal RAII example that locks on creation, zeroizes on drop, and unlocks before freeing. Demonstrates a pattern for safe wrappers.

--------------------------------------------------------------------------------

## Integration Checklist

- Choose your policy:
  - What data needs to be locked?
  - What to do if `mlock` is unsupported or fails? (fail closed vs degrade gracefully)
- Wrap and enforce:
  - Create a RAII wrapper with zeroization.
  - Ensure unlock always executes (even on error paths).
- Observe:
  - Log/metric on fallback paths and failures to enforce your operational stance.
- Test realistically:
  - Consider platform limits (`RLIMIT_MEMLOCK`), container policies, and bandwidth of locks.

--------------------------------------------------------------------------------

## License

Dual-licensed under MIT OR Apache-2.0.

- MIT: See `LICENSE-MIT`
- Apache-2.0: See `LICENSE-APACHE`

By contributing, you agree that your contributions are licensed under the same terms.

--------------------------------------------------------------------------------

## References and Further Reading

- `mlock(2)`, `munlock(2)`, `madvise(2)` man pages
- `RLIMIT_MEMLOCK`, `CAP_IPC_LOCK` on Linux
- docs.rs for this crate: https://docs.rs/os-memlock
- Repository: https://github.com/thatnewyorker/Conflux
