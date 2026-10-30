# Wukong Imager

A desktop app for Windows and Linux that writes OS images to SD cards for the Wukong Pi Zero, in the spirit of Raspberry Pi Imager. It opens on the board straight away, checks the target disk before writing, and verifies the card after the write.

Wukong Imager is a fork of [Armbian Imager](https://github.com/armbian/imager) by Daniele Briguglio and the Armbian community. It is not an official Armbian product and does not talk to Armbian servers.

## Features

- Wukong Pi Zero is selected at startup, with no vendor or board steps
- Writes `.img`, `.iso` and compressed images (`.img.xz`, `.img.gz`, `.img.zst`, `.img.bz2`) from your disk
- Disk safety checks and byte-by-byte verification after the write
- Works fully offline; no updater, no log upload, no external fonts
- English and Russian interface texts are written for this fork; other languages fall back to the upstream translations

## How It Works

1. **Choose an image.** Pick a file from your disk.
2. **Choose the SD card.** System disks are hidden.
3. **Confirm and flash.** The app decompresses if needed, writes, and verifies.

## Building

Node.js 20.19 or newer and Rust 1.85 or newer are required.

```bash
npm ci
npm run tauri:dev     # development build
npm run tauri:build   # installer, no signing keys needed
```

On Windows the app asks for administrator rights at startup because it writes to raw disks, so `tauri:dev` needs an elevated terminal.

For Linux, `scripts/build/linux-docker.sh` builds the `.deb` and `.AppImage` in `ubuntu:22.04` and puts them in `releases/linux/`. It works from Linux or from Git Bash on Windows.

## License

GPL-2.0-or-later, see [LICENSE](LICENSE). Copyright (c) 2025-2026 Daniele Briguglio and contributors. Flag graphics by Twemoji, CC-BY 4.0.
