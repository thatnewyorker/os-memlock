# Changelog

All notable changes to this project will be documented in this file.

This project adheres to Semantic Versioning. Entries are grouped by Added, Changed, Fixed, etc. Dates are in YYYY-MM-DD format.

## [0.2.0] - 2025-10-03

A feature release that adds Windows support, process-wide helpers for macOS and Windows, and FreeBSD support for dump-exclusion hints. No breaking API changes; all additions are backward compatible.

### Added

- Windows platform support for region locking:
  - `mlock` implemented via Win32 `VirtualLock`.
  - `munlock` implemented via Win32 `VirtualUnlock`.
  - Errors map to `std::io::Error::last_os_error()` (GetLastError).
- FreeBSD support for dump-exclusion hints:
  - `madvise_dontdump` now uses `MADV_NOCORE` on FreeBSD.
- macOS process-wide core-dump helpers:
  - `disable_core_dumps_for_process()` sets `RLIMIT_CORE` soft limit to 0 for the current process.
  - `disable_core_dumps_with_guard() -> CoreDumpsDisabledGuard` temporarily disables core dumps and restores prior limits on `Drop`.
- Windows process-level error-mode helpers (operational/UX controls):
  - `set_windows_error_mode(new_mode: u32) -> io::Result<u32>` calls `SetErrorMode`, returning the previous mode.
  - `suppress_windows_error_dialogs_for_process() -> io::Result<u32>` sets a common combination of `SEM_*` flags (`SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX`) and returns the previous mode.
  - Exposed `SEM_*` constants for convenience (documented as process-wide, not security controls).
- Examples:
  - `examples/windows.rs` demonstrating Windows usage (locking/unlocking and error-mode helpers).
  - Updated `examples/simple.rs` to apply dump-exclusion on Linux and FreeBSD.
- CI:
  - Added GitHub Actions Windows job that builds and runs tests with `cargo nextest`.
- Tests:
  - Added a small cross-platform smoke test for the macOS process-wide helper path.

### Changed

- Documentation:
  - README and `docs/overview.md` updated with platform matrix and new APIs:
    - Linux: `madvise_dontdump` via `MADV_DONTDUMP`.
    - FreeBSD: `madvise_dontdump` via `MADV_NOCORE`.
    - macOS: `madvise_dontdump` remains unsupported (Darwin provides no per-region dump-exclusion advice); process-wide helpers documented.
    - Windows: `mlock`/`munlock` supported; `madvise_dontdump` remains unsupported; process-wide error-mode helpers documented.

### Notes and caveats

- Security model unchanged:
  - `mlock`/`munlock` remain `unsafe` and require callers to guarantee pointer validity, ownership, and no concurrent unmapping; zero-length regions are a no-op that return `Ok(())`.
- Dump exclusion:
  - Linux: best-effort advisory via `MADV_DONTDUMP`.
  - FreeBSD: best-effort advisory via `MADV_NOCORE`.
  - macOS: no per-region equivalent; use process-wide `RLIMIT_CORE` helpers if appropriate.
  - Windows: no per-region equivalent; the Windows error-mode helpers are operational/UX controls and do not alter dump contents.
- Cross-platform behavior:
  - Functions unavailable on a platform continue to return `io::ErrorKind::Unsupported` (e.g., `madvise_dontdump` on macOS/Windows).
  - New process-level helpers return `Unsupported` on platforms where they are not applicable.

### Dependency updates

- Added `windows-sys` dependency (Windows only) for bindings to `VirtualLock`, `VirtualUnlock`, and `SetErrorMode`.

### Migration

- No code changes required for existing users.
- If you rely on dump exclusion:
  - On FreeBSD, you may now call `madvise_dontdump` where you previously handled `Unsupported`.
  - On macOS/Windows, continue to handle `Unsupported`; consider the new process-wide helpers if they fit your policy (understanding their scope and limitations).
- If you target Windows and want page locking, you can start using `mlock`/`munlock` with the same safety contract as on Unix.

## [0.1.11] - 2025-09-xx

- Maintenance release before 0.2.0. No functional changes worth highlighting here.
