# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

A system vitals applet for the COSMIC desktop showing CPU, RAM, and Swap usage as sparkline charts in the panel. Right-click opens a settings popup with toggles and refresh interval control. This repo also serves as a **reference implementation** for building COSMIC applets.

## Build Commands

All commands use `just`. On NixOS, prefix with `direnv exec .` (or enter the direnv shell) since the toolchain comes from nix:

```sh
direnv exec . just          # build release
direnv exec . just check    # clippy with pedantic warnings
direnv exec . just dev-reload  # rebuild + restart cosmic-panel
```

Other targets:
- `just run` — build and run standalone (Wayland errors are normal — applets need the panel)
- `just dev-install` — one-time setup: symlinks binary to `~/.local/bin`, copies desktop/icon/metadata
- `just install-user` — copies binary to `~/.local` (no root needed)
- `just tag <version>` — bump version, commit, and git tag

## NixOS Development

A shared nix-shell at `~/.config/nix/cosmic-shell.nix` provides native deps and linker flags. The `.envrc` activates it:

```
use nix ~/.config/nix/cosmic-shell.nix
```

Key details:
- **Always use `direnv exec .`** when running build/check/run commands outside the direnv shell — without it, `cc` linker and native libs won't be found
- `RUSTFLAGS` must force-link dlopen'd libraries (EGL, wayland, vulkan, xkbcommon, X11) — same approach as `libcosmicAppHook` in nixos-cosmic
- `LD_LIBRARY_PATH` is needed at runtime for those same libraries
- `~/.local/bin` must be in PATH (`environment.localBinInPath = true` in NixOS config)
- On NixOS, `/usr/share` is NOT in `XDG_DATA_DIRS` — use `install-user` or `dev-install` instead of `just install`

## Project Structure

```
src/
  main.rs       — entry point, i18n init, launches cosmic::applet::run::<AppModel>()
  app.rs        — core: AppModel state, Message enum, view/update/subscription
  chart.rs      — History circular buffer + SparklineChart Canvas Program
  config.rs     — Config struct with #[derive(CosmicConfigEntry)], persisted via cosmic-config
  i18n.rs       — fluent localization via i18n-embed, fl!("key") macro
resources/
  icon.svg      — pulse/chart SVG icon using currentColor + stroke
  app.desktop   — desktop entry with applet keys
  app.metainfo.xml
i18n/en/
  cosmic_ext_applet_vitals.ftl  — English fluent strings
```

## Architecture

COSMIC applets follow an Elm-like architecture via `cosmic::Application`.

### Panel Button (`view()`)

- `view()` renders the panel button showing sparkline charts for enabled metrics (CPU/RAM/Swap)
- Each chart is a `Canvas` widget with `SparklineChart` program, sized 36x18 pixels
- Uses `widget::button::custom(autosize_window(row)).class(cosmic::theme::Button::AppletIcon)`
- Both left-click and right-click open the popup

### Popup (`view_window()`)

- `view_window()` renders the settings popup with:
  - "Vitals" heading
  - Togglers for CPU/RAM/Swap (showing current percentage)
  - Refresh interval spin button (200-5000ms, step 100)
- Wrapped with `self.core.applet.popup_container(content)`
- Popups use `get_popup()` / `destroy_popup()` from `cosmic::iced_winit::commands::popup`

### Subscriptions

Uses `cosmic::iced::time::every(Duration)` for periodic system sampling — simpler than channel-based subscriptions. The interval is configurable via `config.refresh_interval_ms`. Config watcher subscription watches for external changes.

### System Metrics

- Uses `sysinfo` crate with selective refresh: `RefreshKind::nothing().with_cpu(...).with_memory(...)`
- On each tick: `sys.refresh_cpu_usage()` + `sys.refresh_memory()`
- CPU: `sys.global_cpu_usage()` (already a percentage)
- RAM/Swap: computed as `used / total * 100.0`
- All values stored as `f32` 0-100 in `History` circular buffers (capacity 60)

### Config Persistence

- `Config` struct derives `CosmicConfigEntry` with `#[version = 1]`
- Fields: `show_cpu`, `show_ram`, `show_swap` (booleans), `refresh_interval_ms` (u32)
- Store `config_handler: Option<cosmic_config::Config>` in the model to write changes back
- Read in `init()` via `cosmic_config::Config::new(APP_ID, Config::VERSION)` then `Config::get_entry(&handler)`
- Write with `self.config.write_entry(&handler)`
- Watch for external changes via `self.core().watch_config::<Config>(APP_ID)` in `subscription()`

### Desktop Entry

The `.desktop` file must have these keys for COSMIC to recognize it as an applet:
- `X-CosmicApplet=true`
- `X-CosmicHoverPopup=Auto`
- `NoDisplay=true`
- `Categories=COSMIC`

## Widget Patterns

### Custom SVG Icons

Use outline-only SVGs with `currentColor` so they adapt to the theme:

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none"
     stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
  ...
</svg>
```

### Spin Buttons

For numeric settings, use `widget::spin_button`. Note: with `a11y` feature (enabled by default), it takes 7 args — the second is an accessibility name:

```rust
widget::spin_button(
    format!("{value}"),    // display label
    format!("{value}"),    // a11y name
    value,                 // current value
    step,                  // step
    min,                   // min
    max,                   // max
    Message::SetValue,     // on_change (receives new value directly)
)
```

## Cargo Features

Minimal libcosmic features for an applet (no wgpu — uses software renderer):

```toml
[dependencies.libcosmic]
git = "https://github.com/pop-os/libcosmic.git"
features = ["applet", "applet-token", "dbus-config", "multi-window", "tokio", "wayland", "winit"]
```

## Publishing

### GitHub

- **Repo**: https://github.com/bgub/cosmic-ext-applet-vitals
- Releases use `just tag <version>` then `git push origin main --tags`

### NixOS (nixpkgs)

- Package at `pkgs/by-name/co/cosmic-ext-applet-vitals/package.nix` in nixpkgs
- A local copy is at `package.nix` in this repo for reference
- Uses `rustPlatform.buildRustPackage` + `libcosmicAppHook` + `just`
- Key nix build details: `dontUseJustBuild = true`, `dontUseJustCheck = true` — cargo builds, just only installs
- `justFlags` must override `prefix` and `bin-src` (with cross-compilation-aware target triple)
