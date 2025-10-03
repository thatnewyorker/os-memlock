# os-memlock

[![Docs](https://docs.rs/os-memlock/badge.svg)](https://docs.rs/os-memlock)

Quick links:
- Detailed guide: docs/overview.md
- API docs (docs.rs): <https://docs.rs/os-memlock>
- Examples:
  - examples/simple.rs
  - examples/locked_vec.rs

## Docs

The detailed guide covers:
- Safety model and caller obligations: docs/overview.md#safety-model-and-caller-obligations
- Platform support and behavior: docs/overview.md#cross-platform-behavior
- Usage patterns and RAII wrappers: docs/overview.md#usage-patterns
- Error model and diagnostics: docs/overview.md#error-model
- Testing and CI: docs/overview.md#testing-and-ci
- Security considerations and threat model: docs/overview.md#security-considerations
- Integration checklist: docs/overview.md#integration-checklist

Small, focused crate providing thin, unsafe wrappers around OS memory-locking syscalls:
- `mlock` / `munlock` (prevent swapping)
- `madvise(MADV_DONTDUMP)` (best-effort exclusion from core dumps on Linux)

This crate isolates the minimal unsafe FFI surface so higher-level modules can remain
`#![forbid(unsafe_code)]`. The public functions are intentionally `unsafe` to make
pointer-safety obligations explicit to callers.

---

## Purpose

- Provide a tiny, audit-friendly layer over platform syscalls used to lock memory pages
  and apply dump-exclusion hints.
- Keep all `unsafe` and FFI details in a single, well-documented crate so the rest of
  the codebase can use a safe abstraction that validates inputs before calling into this
  crate when appropriate.
- Expose a stable, minimal API that is easy to reason about and to wrap in safer helpers.

---

## Crate API (surface)

The crate re-exports the platform-specific implementations at the crate root:

- `unsafe fn mlock(addr: *const std::os::raw::c_void, len: usize) -> std::io::Result<()>`
  - Lock the pages containing the memory region so they are not swapped out.
  - On unsupported platforms, returns `Err(io::ErrorKind::Unsupported)`.

- `unsafe fn munlock(addr: *const std::os::raw::c_void, len: usize) -> std::io::Result<()>`
  - Unlock the pages, reversing `mlock`.
  - On unsupported platforms, returns `Err(io::ErrorKind::Unsupported)`.

- `unsafe fn madvise_dontdump(addr: *mut std::os::raw::c_void, len: usize) -> std::io::Result<()>`
  - Best-effort hint to exclude a mapping from core dumps (Linux: `MADV_DONTDUMP`).
  - On non-Linux or unsupported platforms, returns `Err(io::ErrorKind::Unsupported)`.

Notes on signatures:
- The functions intentionally use raw pointers and `usize` lengths to mirror the OS call
  semantics and to avoid hiding important safety obligations behind false safety.
- Zero-length regions are treated as a no-op and return `Ok(())` for ergonomic callers.

---

## Safety contract

All functions are `unsafe`. Callers must uphold the following preconditions for each call:

1. The `(addr, len)` pair must denote a valid memory region that the caller owns for
   the duration of the call and for as long as the OS considers the lock to be held.
   - The range must be mapped into the process address space and addressable (initialized)
     memory. Passing invalid pointers is undefined behavior at the OS/FFI boundary.

2. The memory region must not be concurrently deallocated, unmapped, or remapped while
   the system call is in-flight. Concurrent unmapping or reallocation may cause the OS
   call to operate on a different mapping and can lead to undefined behavior or kernel
   errors.

3. Callers must ensure alignment and fractional-page concerns are addressed if required
   by their higher-level policy; the OS operates at page granularity, but `mlock` is
   defined on an arbitrary address and length.

4. When using `mlock` to protect secrets, callers must consider:
   - Handling and limiting locked memory lifetime.
   - Zeroizing secrets before `munlock`/drop, where appropriate.
   - Observability: `mlock` failures may be transient or platform-dependent — be prepared
     to treat `Err(Unsupported)` and other error kinds as operational signals.

5. For `madvise_dontdump`:
   - This is advisory and best-effort; the kernel may ignore or reject the hint.
   - Use it as a privacy/operational enhancement, not a strict security boundary.

---

## Platform support & behavior

- Unix (Linux, *BSD, macOS):
  - `mlock` and `munlock` call through to `libc::mlock` and `libc::munlock`.
  - `madvise_dontdump`:
    - On Linux: wraps `madvise(..., MADV_DONTDUMP)`.
    - On non-Linux Unices: returns `Err(io::ErrorKind::Unsupported)`.

- Non-Unix platforms:
  - All functions return `Err(io::ErrorKind::Unsupported)`.
  - The function signatures exist to preserve a consistent cross-platform API; callers
    should handle `Unsupported` gracefully.

---

## Examples (usage guidance)

- Minimal unsafe call (illustrative — not a full safety wrapper):

Use `mlock` to lock a buffer you control. Wrap calls in `unsafe` and uphold the safety contract:

`unsafe { os_memlock::mlock(buf.as_ptr() as *const _, buf.len())?; }`

Later, before drop/unmapping:
`unsafe { os_memlock::munlock(buf.as_ptr() as *const _, buf.len())?; }`

Call `madvise_dontdump` on Linux to reduce chance of core dump exposure:
`unsafe { os_memlock::madvise_dontdump(buf.as_mut_ptr() as *mut _, buf.len())?; }`

- Higher-level recommended pattern:
  - Prefer a safe wrapper in your application that:
    - Accepts owned buffers (e.g., a wrapper type),
    - Ensures the buffer lives for the duration of the lock,
    - Calls `mlock` at allocation or when the secret is installed,
    - Zeroizes the content before `munlock` and ensures `munlock` is called (via Drop).
  - See `src/mem/locked.rs` in this repository for an example of a safe `LockedVec` style wrapper.

---

## Error handling and diagnostics

- `io::ErrorKind::Unsupported` signals platform/build-time unavailability;
  do not treat it as a panic-worthy error unless your feature policy requires it.
- Other OS errors (e.g., resource limits) will be returned as `io::Error` with kernel
  `errno` translated into `std::io::Error`. These must be handled by the caller or
  propagated with context.

---

## Testing notes

- Unit tests in the repository provide behavior verification in environments where
  syscalls are available. Tests exercise both success paths and fallback behavior.
- Where platform syscalls are unavailable or require elevated privileges, tests
  should mock or stub the syscall provider rather than invoking real FFI.

---

## Security & maintenance notes

- This crate keeps unsafe code minimal and concentrated for simpler auditing.
- When adding new functions or platform support:
  - Document safety obligations clearly in the function-level comments.
  - Add unit tests for both supported and unsupported-platform behaviors.
  - Avoid adding higher-level policies here; keep this crate focused on raw syscall
    mapping and let callers implement policy/ownership semantics.

---

## License

This crate is dual-licensed under Apache-2.0 OR MIT; see `Cargo.toml` for details.

---
