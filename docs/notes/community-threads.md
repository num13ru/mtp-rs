# Community threads and known device quirks

Catch-up reading for any agent that picks up issue or PR work, so you don't have to reverse-engineer history from a cold
start. Read this first when triaging a new issue or PR, and update it after work that affects community-facing context
(see [Updating this doc](#updating-this-doc) at the bottom).

Last updated: 2026-06-06.

## Intentionally / continuously open threads

### #6 — Tested devices feedback tracker (open since 2026-04-13)

Reporter: [@juleskers](https://github.com/juleskers). Long-running tracker for
"device XYZ works" reports, intentionally kept open as a single thread instead of one issue per device.

Confirmed working from this thread:

- **Fairphone 5** (Android 13, e/OS 3.0.4, LineageOS-derived) — full integration suite passed on 2026-04-13. Added to
  README's tested devices table.
- **Garmin Forerunner 955** (reported by [@dasJ](https://github.com/dasJ) on 2026-04-26, PR
  [#10](https://github.com/vdavid/mtp-rs/pull/10) merged 2026-05-02) — works in production with the
  `set_split_header_data(true)` quirk auto-applied for `manufacturer == "Garmin"` (in
  `MtpDeviceBuilder::open`). Same workaround as the Zune-era hardware from #3/#4, but auto-applied. The
  manufacturer-string match violates the "no device knowledge baked in" philosophy and is on the cleanup list once we
  have evidence it can be removed. The `test_ptp_device` integration failure was a separate protocol bug
  (session-less `GetDeviceInfo` didn't accumulate split USB transfers) and was fixed independently of #10. Awaiting
  @dasJ's re-test confirmation that the protocol fix resolves the test on real hardware.

## Active threads

### #12 — Test failures: Panasonic Lumix DMC-TZ61 (opened 2026-06-03)

Reporter: [@juleskers](https://github.com/juleskers) (the #6 tracker maintainer), testing their 2014 PictBridge/PTP
camera as promised in #6. Two failure classes, both root-caused via the reporter's `diagnose` + `ptp_diagnose` logs
(attached to the issue, 2026-06-05):

1. **Write tests panicked** with `Protocol { code: InvalidObjectFormatCode, operation: SendObjectInfo }`. The camera is
   firmware-filtered read-only (only `/DCIM/1nn_PANA/nnnmmmm.jpg`-named files show; even camera-made photos copied to
   root or `MISC` stay hidden). Fixed 2026-06-04 (commit `2bc64bf`): destructive integration tests now check the new
   `supports_upload()` (advertises both `SendObjectInfo` and `SendObject`) and skip cleanly, mirroring the
   `supports_rename()` pattern. **Confirmed fixed by the reporter**: the camera's 21 advertised operations include
   neither send op, so the gate triggers ("no more panics 🎉").
2. **Recursive file search found nothing** despite hundreds of 4–9 MB photos in `/DCIM/1nn_PANA/`. Root cause: the
   camera reports `20480000T000000` (year 2048, month 0, day 0 — its "no date" sentinel) in
   `DateCreated`/`DateModified`. `unpack_datetime` raised a hard error on it, which failed the whole `ObjectInfo`
   parse, which failed the entire listing — and the old test helper swallowed the error into "no suitable file found".
   Fixed 2026-06-06: receive-side datetime parsing is lenient (unparseable → `None`); send-side packing stays strict.
   (An earlier hypothesis blamed `ParentFilter::Exact` dropping objects on mismatched parent handles — that was
   wrong, don't chase it.)

Also surfaced, **not actionable in the library**: aborting (Ctrl+C) a connected session mid-listing froze the camera
completely (no power-button response; "Transaction ID mismatch" / "expected Response container type (3), got 2" on
reconnect) until a battery pull. Camera firmware bug. The camera does advertise `ResetDevice` (0x1010), which might
un-stick it without a battery pull — untested.

Note: the reporter's retest cycles can take weeks ("days/weeks/months"), so bundle asks into single well-aimed
comments.

## Closed and merged

Chronological order, oldest first.

### #1 — 11th gen Kindle not detected (closed 2026-03-24)

Reporter: [@jannikac](https://github.com/jannikac). The Kindle exposed a vendor-class interface (`class=ff subclass=ff`)
instead of a standard MTP class, so `is_mtp_device` returned false. Fixed in v0.4.1 with a broader detection heuristic.
The "permissive interface scan" pattern that grew out of this was later formalized in #4. Case study:
[veszelovszki.com/a/mtp-rs-bugfix](https://www.veszelovszki.com/a/mtp-rs-bugfix/).

### #2 — OpenSession transaction-id fix (merged 2026-04-01)

Reporter and fixer: [@num13ru](https://github.com/num13ru). OpenSession is a session-less PTP operation and must be sent
with `transaction_id = 0`. The old code routed it through the general `execute()` path, which assigned the first
in-session transaction id. Spec-correct fix, but the symptom was Kindle rejecting OpenSession with a non-zero TID —
Android tolerated it. Tested on @num13ru's Kindle Paperwhite. Their Kindle test screenshot lives in
[this comment](https://github.com/vdavid/mtp-rs/pull/2#issuecomment-4264713119), linked from the README's tested devices
row.

### #3 / #4 — Low-level primitives for non-standard MTP devices (merged 2026-04-09)

Reporter and contributor: [@kelchm](https://github.com/kelchm) (Matthew Kelch). Started as a design discussion in #3
about whether the crate should grow a device-quirks registry. Outcome: no registry, but expose enough primitives so
consumers can drive odd devices out-of-tree. Shipped in #4:

- `PtpSession::execute` / `execute_with_receive` / `execute_with_send` made public, reachable via
  `MtpDevice::session()`.
- `PtpSession::set_split_header_data(bool)`: sends the 12-byte PTP container header and payload as separate bulk
  transfers, for devices that won't accept the combined form. Originally for Zune-era hardware, now also relevant for
  Garmin Forerunner 955 (#6 comment).
- `MtpDevice::list_devices_with_known(&[(VID, PID)])` and
  `MtpDeviceBuilder::known_devices(...)`: enumerate devices whose USB descriptors don't advertise standard MTP class
  codes.
- `MtpDeviceBuilder::open_nusb_device(nusb::Device)`: escape hatch for callers doing their own enumeration.
- macOS `SetConfiguration(1)` retry on `claim_interface` failure for vendor-class devices — IOKit doesn't publish
  interfaces until configuration is set.

The original PR proposal also had a `DeviceQuirks` struct with `manual_traversal`
and `split_header_data` flags plus a `detect_quirks()` callback. We deliberately turned that down: only two quirks
existed, and both could be set directly on the session. Premature abstraction. The current expose-the-primitive approach
keeps device knowledge out of the crate.

### #5 — RUSTSEC-2026-0097: rand unsoundness (closed 2026-04-13)

Source: github-actions advisory. Not affected — `rand` is pulled in only transitively via `proptest` (dev-dependency),
so it never reaches downstream consumers. The trigger conditions (custom logger calling `rand::rng()` during reseed)
don't apply to our test builds either. Closed as not-affected.

### #7 — Python bindings request (closed 2026-04-14)

Requester: [@dragon-Elec](https://github.com/dragon-Elec). Out of scope for this crate. The crate is MIT-licensed, so
anyone can build a `mtp-py` wrapper on top. Pointed to [mtp-mount](https://www.veszelovszki.com/a/mtp-mount/) as a
working consumer that exercises the full API.

### #8 / #9 — Slow root listing on Kindle and other non-Android devices (merged 2026-04-26, shipped in v0.13.2 on 2026-04-27)

Reporter and fixer: [@num13ru](https://github.com/num13ru).
`GetObjectHandles(parent=0)` on the Kindle Paperwhite (12th gen) returned all 2541 objects on the storage instead of the
23 root-level items, and the post-hoc `ParentFilter::Exact` filter then triggered 2541 individual
`GetObjectInfo` round-trips. The `parent=0xFFFFFFFF` workaround that avoided this on Android was gated behind
`is_android()`, so the Kindle never reached it.

Fix: try `parent=0xFFFFFFFF` first for all devices, fall back to `parent=0`
only when the device rejects it with an error. An empty `Ok(_)` from the fast path is treated as a legit empty storage,
not a fallback trigger. The
`is_android()` gate inside `Storage::list_objects_stream` is gone; the
`is_android()` check inside `list_objects_recursive_auto` remains and gates a different workaround. 110× reduction in
USB round-trips for root listing on the Kindle (2541 → 23). Tested end-to-end on the Kindle and on Pixel 9 Pro XL.

### #10 — Garmin Forerunner 955 quirk and split-receive bug fix (merged 2026-05-02)

Reporter and contributor: [@dasJ](https://github.com/dasJ). PR added two things:

- README row for FR955 in the tested devices table.
- Auto-applies `set_split_header_data(true)` in `MtpDeviceBuilder::open` when `manufacturer == "Garmin"`. This is the
  same primitive Zune-era hardware needs (#3 / #4); without it, Garmin uploads fail. Manufacturer-string match
  violates the "no device knowledge baked in" philosophy and is on the cleanup list, but kept for now to ship a
  working device today.

The PR's failing `test_ptp_device` (`data container length mismatch: header says 281, have 12`) turned out to be a
separate protocol bug: `PtpDevice::get_device_info()` (the session-less variant) didn't accumulate split USB bulk
transfers the way in-session `execute_with_receive` already did. Garmin sends the 12-byte container header in one
transfer and the payload in a follow-up transfer (allowed by the PTP spec). Fixed by mirroring the in-session
multi-transfer loop and added two regression tests using the mock transport. Side-effect refactor:
`PtpDevice::transport` is now `Arc<dyn Transport>` instead of `Arc<NusbTransport>` so the mock can be plugged in.
@dasJ confirmed `test_ptp_device` passes on FR955 with the fix.

Follow-up: @dasJ also surfaced that the destructive integration tests assumed a `Download` folder, which is too
Android-specific. Refactored `tests/integration.rs` to walk a priority list of writable folder names covering
Android, Kindle, and Garmin, with `MTP_TEST_FOLDER` env var as override. Tests now skip cleanly with a helpful log
when no match is found.

## Device quirks reference

Cross-cutting summary of every quirk currently handled or known. Sorted by device family.

| Device                       | Quirk                                                                 | Workaround                                        | First spotted in       |
|------------------------------|-----------------------------------------------------------------------|---------------------------------------------------|------------------------|
| Android (general)            | `parent=0` returns all objects on storage                             | `parent=0xFFFFFFFF` for root listing              | Pre-public             |
| Android (general)            | Uploads to storage root rejected with `InvalidObjectHandle`           | Upload into a folder, for example `Download`      | Pre-public             |
| Android (general)            | `ObjectHandle::ALL` recursive listing broken                          | Manual traversal in `list_objects_recursive_auto` | Pre-public             |
| Samsung Galaxy               | `InvalidObjectHandle` on root listing with handle 0                   | Recursive traversal with filtering                | Pre-public             |
| Kindle Paperwhite (12th gen) | `parent=0` returns all objects (same as Android, no `android.com` ID) | Universal fast path with fallback (post v0.13.2)  | #8 / #9                |
| Kindle (11th gen)            | Vendor-class USB interface, missed by class-code match                | Permissive interface scan + custom VID/PID list   | #1, generalized in #4  |
| Kindle (general)             | Rejects OpenSession with non-zero transaction id                      | Send OpenSession with TID=0                       | #2                     |
| Fuji cameras                 | Returns all objects for root listing                                  | Filter by exact parent handle                     | Pre-public             |
| Fuji cameras                 | Reports `AccessCapability::ReadWrite` but errors on writes            | Trust the per-operation `StoreReadOnly` response  | Pre-public             |
| Zune-era hardware (MTPZ)     | Won't accept combined header+payload bulk transfers                   | `set_split_header_data(true)`                     | #3 / #4                |
| Garmin Forerunner 955        | Same as Zune on send (uploads need split mode)                        | Auto-applied via manufacturer-string match (#10)  | #6 (@dasJ, 2026-04-26) |
| Garmin Forerunner 955        | Sends container header and payload as separate bulk transfers on receive | Multi-transfer accumulation in session-less `GetDeviceInfo` | #10 (protocol bug, fixed 2026-05-02) |
| Vendor-class macOS devices   | IOKit doesn't publish interfaces until config is set                  | `SetConfiguration(1)` retry on `claim_interface`  | #4                     |
| Panasonic Lumix DMC-TZ61     | Firmware-filtered read-only PTP view; doesn't advertise `SendObjectInfo`/`SendObject`; rejects `SendObjectInfo` with `InvalidObjectFormatCode` | Gate writes on `supports_upload()` (confirmed working) | #12 (@juleskers, 2026-06-03) |
| Panasonic Lumix DMC-TZ61     | Reports `20480000T000000` (month 0, day 0) as "no date" in ObjectInfo datetimes | Lenient receive-side datetime parsing (unparseable → `None`) | #12 (root-caused 2026-06-05) |
| Panasonic Lumix DMC-TZ61     | Freezes hard (battery-pull-level) if the host aborts mid-listing | None (camera firmware bug); `ResetDevice` (0x1010) advertised but untested | #12 |

## Recurring contributors

- [@num13ru](https://github.com/num13ru) — Kindle Paperwhite owner. Reported and/or shipped fixes across #2, #8, #9.
  High-quality diagnostics with side-by-side comparisons. Tests on real hardware before submitting.
- [@kelchm](https://github.com/kelchm) — Designed and contributed the low-level primitives in #3 / #4. Good
  architectural taste, willing to drop premature abstractions.
- [@juleskers](https://github.com/juleskers) — Maintains the tested-devices tracker (#6). Fairphone 5 confirmed working
  with full integration suite.
- [@jannikac](https://github.com/jannikac) — First external bug report (#1), Kindle-detection fix.
- [@dasJ](https://github.com/dasJ) — Garmin Forerunner 955 confirmed working in production (#6 comment, 2026-04-26).
- [@dragon-Elec](https://github.com/dragon-Elec) — Python bindings request
  (#7).

## Updating this doc

Update when your work touches community-facing context. That includes:

- A new device quirk or workaround discovered, even if not fixed yet.
- A community bug resolved with a notable fix or release.
- A new external contributor lands a PR.
- A merged feature triggered by an external request, where future agents should know the motivation.

For each entry, add the issue or PR number, link the GitHub user with `@`, use ISO dates (YYYY-MM-DD), and place it
chronologically in the right section. Move closed/merged threads from "Active threads" to "Closed and merged" when they
wrap up.

Skip routine internal refactors, dependency bumps, and anything that doesn't affect how a future agent should triage a
new issue.
