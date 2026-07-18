---
name: Device bug (freeze, wedge, or won't work)
about: A phone, camera, or other MTP/PTP device that hangs, freezes, or misbehaves
title: "[device] "
labels: device
---

This template is NOT here to make your life hard. Skip anything that doesn't apply.

The thing is, MTP bugs are almost always device-specific, and I ([David](https://github.com/vdavid)) don't own a lot of devices, so if you say "Hey, file writes don't work on my Motorola ZX 18 Ultra", well, I either get a bunch of more info from you right away, or a slower back-and-forth starts, because I don't own a Motorola ZX 18 Ultra (also, I just made that up). So I prefer you send me a bunch of info now. Thanks for improving `mtp-rs`!

**What happened** (a line or two):

**Device**: model, plus OS / Android version if it's a phone:

**Diagnostics** (install with `cargo install mtp-rs-cli`; the binary is `mtp-rs`):

1. Identity, capabilities, and a cancel-health check, paste the output:
   ```sh
   mtp-rs doctor --probe-cancel --json
   ```
   (`--probe-cancel` downloads a file and cancels it mid-stream. It's read-only but can briefly wedge the device; the library recovers it.)

2. A protocol trace of the operation that fails, paste the last ~30 lines of stderr, around where it stalls:
   ```sh
   RUST_LOG=mtp_rs=trace mtp-rs <the command that fails>   # e.g. get /DCIM/Camera/IMG_0001.jpg ./IMG_0001.jpg
   ```
   (Ctrl-C if it hangs, then paste what printed)

3. If the device gets stuck (every command times out afterward), does this bring it back without unplugging?
   ```sh
   mtp-rs reset
   ```
   Tell me: recovered, or needed a physical replug.

That's it. Thanks for the help, and for helping improve `mtp-rs` for everyone!
