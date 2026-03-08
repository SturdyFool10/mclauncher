# Vertex Launcher

An unofficial Minecraft launcher written in Rust, focused on being fast, capable, and pleasant to use.

---

## Have

What the project already has today:

- A native Rust desktop launcher built with `eframe/egui` + `wgpu`
- Core instance workflow in place: create, manage, and launch instances
- Account authentication flow and launch context wiring
- A library screen, settings screen, console view, legal page, and skin manager screen
- Configurable themes and UI font options
- Performance and quality controls (frame limiter, preview AA modes, runtime/config tuning)
- Modular crates for install/runtime/auth/mod-provider responsibilities
- multiple instances launched at a time
- multiple accounts logged in at a time
- a convenient vanilla minecraft launcher-less experience
---

## Want

What we are actively aiming for next:


- export and import manifests for modpacks
- Modrinth content browsing in-app
- CurseForge content browsing in-app
- RustServerController integration
- export of pre-made server instances for modpacks

---

## Project Direction

The goal is to keep building a launcher that feels reliable and flexible without getting in your way.

- Privacy-respecting by default
- Cross-platform where Minecraft can run
- Fast and lightweight, including on older Vulkan-capable hardware
- Stable enough for daily use
- Friendly to both default users and power users
- Ready to adapt as Minecraft and modding ecosystems evolve

In short: keep the app native, keep it practical, and keep improving the experience one solid feature at a time. do it all while making it from scratch

---
## What we don't do

in general, this launcher is designed to make life simpler for paying minecraft players, it is not an aide for pirates, and it will not allow you to launch the game without a valid Minecraft account and a valid Minecraft license.

# Installation

Currently we are in a pre-alpha state, and as such there is no way to install this without building from source. I however do not like complex builds so just clone the repo and run 
```sh
cargo build --release
``` 
inside the directory you cloned it into.

## Linux caveat

On Linux, you need native development libraries installed before compilation will work.
If these are missing, `cargo build` will fail during `pkg-config` checks for `gtk/glib/webkit`.

For Debian/Ubuntu:
```sh
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  pkg-config \
  libglib2.0-dev \
  libgtk-3-dev \
  libgdk-pixbuf-2.0-dev \
  libpango1.0-dev \
  libatk1.0-dev \
  libcairo2-dev \
  libsoup-3.0-dev \
  libwebkit2gtk-4.1-dev \
  libjavascriptcoregtk-4.1-dev
```

If your distro does not provide `4.1` packages, use:
- `libwebkit2gtk-4.0-dev`
- `libjavascriptcoregtk-4.0-dev`

On Wayland, taskbar/dock icons are resolved via desktop app ID. This app uses `vertexlauncher` as its app ID.
