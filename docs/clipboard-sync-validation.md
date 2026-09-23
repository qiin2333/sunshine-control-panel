# Clipboard compound / large-text regression validation

Validated on 2026-09-23 with the Windows user-session agent built from this change, the installed Sunshine Core `v2026.921.161858.杂鱼`, and an Ubuntu 26.04.1 Hyper-V VM running Qt 6.10.2 / SDL 2.32.10. The Linux Moonlight client included its companion clipboard fixes and ran in an isolated X11 desktop. These results do not establish native Wayland support.

## Automated checks

```powershell
npm run build:renderer
cargo test --locked --manifest-path src-tauri/Cargo.toml clipboard::
cargo test --locked --manifest-path src-tauri/Cargo.toml
cargo build --locked --manifest-path src-tauri/Cargo.toml
```

Results: renderer and Windows MSVC debug build passed; clipboard tests 5 passed; complete Rust tests 223 passed, 3 ignored, 0 failed. New regressions cover A → B → A, changing either half of a compound clipboard, adding/removing a flavor, and echo suppression across PNG re-encoding.

Review follow-up added the empty-caption normalization and standalone-clear frame regressions. The complete Rust suite was rerun: **225 passed, 3 ignored, 0 failed** (7 clipboard tests). This follow-up documents/tests the existing compatibility boundaries and does not change the agent's runtime behavior.

## Real Sunshine / Moonlight round trip

### Supported compound content

The current-state identity describes supported sync content, not every OS MIME flavor. As in the existing sender and Windows compound writer, an empty text flavor beside a PNG is normalized to an absent caption; PNG-only and PNG + empty text are intentionally equivalent. A standalone inbound empty text frame is still accepted to clear text, and its resulting empty clipboard is suppressed as an echo. Regression tests explicitly cover both cases.

Compound captions must be nonempty and at most `BURST_TEXT_MAX_INLINE` (65,490 UTF-8 bytes). Larger captions retain the existing image-only fallback. Standalone large text is supported through REF. Sending a text REF before a PNG does not guarantee apply order: each peer fetches the blob asynchronously, and an old peer can finish the text fetch after applying the image, leaving text-only. A shared token cannot fix a receiver that does not implement reordering/aggregation. Removing this fallback requires a separate negotiated receiver capability, rather than changing the v1 compatibility contract in this fix.

### Procedure and results

Pair the VM with Core and open an actual Desktop stream. Run the new panel agent in the Windows interactive session, ensuring the installed panel is not also watching the clipboard. Check the executable path: the service can respawn the installed panel after an abnormal exit, invalidating an apparent test of the new build.

Independent Qt processes write and read the two system clipboards. Compare SHA-256 of UTF-8 text after CRLF/LF normalization and SHA-256 of decoded RGBA pixels. After a match, wait 800 ms and check again to detect a subsequent overwrite. Preserve the original Windows clipboard in memory and restore it after disconnecting the test stream.

| Case | Windows → Ubuntu | Ubuntu → Windows |
| --- | --- | --- |
| Text A, B, A (3 checks) | Pass | Pass |
| Chinese, emoji and newline | Pass | Pass |
| Small PNG | Pass | Pass |
| Same PNG with newly added caption | Pass | Pass |
| Same PNG with changed caption | Pass | Pass |
| Large text through HTTPS REF (290 KB normalized) | Pass | Pass |
| Large PNG through HTTPS REF | Pass | Pass |

**18/18 passed**, compared with 15/18 on the diagnostic run with the installed agent. A transparent helper relay recorded only frame kind, token, length, content hash and REF metadata, not pairing private keys or the original clipboard:

- Before: host compound changes arrived as token=0 text-only or PNG-only frames. After: both flavors arrived with one shared nonzero token.
- Before: host large-text upload used `text/plain; charset=utf-8`; Core returned `400 {"error":"bad_mime"}`. A control upload using `text/plain` returned 200. After: a `text/plain` REF reached Moonlight and the downloaded text matched.
- The receive side also retains the full successful write as its current identity, preventing its watcher from splitting the just-applied compound clipboard into an echo.

Five supplemental checks passed: 65,501 and 65,502 byte standalone client text; new client text superseding an in-flight large image; preserving a pending Ubuntu file paste against host text; and resuming normal sync after the file clipboard is cleared. The standalone text cases use the existing blob threshold and are not tests of the compound inline ceiling.

The temporary agent, stream and isolated test desktop were stopped, and the installed panel and original Windows clipboard restored. Core binaries and service settings were not replaced.
