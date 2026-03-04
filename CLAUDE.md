# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

A system monitor applet for the COSMIC desktop showing CPU, RAM, Swap, Network, Disk, and GPU metrics as configurable run charts and bar charts in the panel. Configuration is persisted via cosmic-config (no popup UI yet — edit config entries directly or via `cosmic-config`). Ported from / inspired by `cosmic-ext-applet-system-monitor`.

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
- `LD_LIBRARY_PATH` is needed at runtime for those same libraries, plus `/run/opengl-driver/lib` for NVIDIA's `libnvidia-ml.so` (NVML)
- `~/.local/bin` must be in PATH (`environment.localBinInPath = true` in NixOS config)
- On NixOS, `/usr/share` is NOT in `XDG_DATA_DIRS` — use `install-user` or `dev-install` instead of `just install`

## Project Structure

```
src/
  main.rs          — entry point, config init, launches cosmic::applet::run::<SystemMonitorApplet>()
  applet.rs        — core: SystemMonitorApplet state, Message enum, Application impl (init/view/update/subscription)
  views.rs         — view helper functions: cpu_view, mem_view, net_view, disk_view, gpu_view
  config.rs        — Config/SamplingConfig/ComponentConfig structs, CosmicConfigEntry impl, config subscription
  color.rs         — Color enum mapping cosmic palette colors + arbitrary RGB for chart theming
  history.rs       — History<T> generic circular buffer for time-series data
  localization.rs  — fluent localization via i18n-embed, fl!("key") macro
  components/
    run.rs         — Canvas-based run charts (HistoryChart, SimpleHistoryChart, SuperimposedHistoryChart)
    bar.rs         — PercentageBar custom widget for CPU core bars, sorted bar charts
    gpu.rs         — GPU detection: NVIDIA via NVML, AMD/others via sysfs (gpu_busy_percent, mem_info_vram_*)
resources/
  icon.svg         — pulse/chart SVG icon using currentColor + stroke
  app.desktop      — desktop entry with X-CosmicApplet keys
  app.metainfo.xml
i18n/en/
  cosmic_ext_applet_sysmon.ftl — English fluent strings
```

## Architecture

COSMIC applets follow an Elm-like architecture via `cosmic::Application`.

### Panel View (`view()`)

- `view()` iterates `config.components` and calls per-metric view helpers in `views.rs`
- Each component (CPU, Mem, Net, Disk, GPU) returns a collection of chart widgets
- Charts: `Canvas`-based run charts (`components/run.rs`) and custom `PercentageBar` widgets (`components/bar.rs`)
- Empty groups are skipped (e.g. GPU when no GPUs detected)
- Layout respects `config.layout` settings (padding, spacing, inner_spacing)
- Uses `self.core.applet.autosize_window(items)` for panel sizing

### Subscriptions

- Separate tick messages per component: `TickCpu`, `TickMem`, `TickNet`, `TickDisk`, `TickGpu`
- Each uses `cosmic::iced::time::every(Duration)` with per-component intervals from `config.sampling`
- Config watcher subscription via `config_subscription()` watches for external changes

### System Metrics

- **CPU**: `sysinfo` with `CpuRefreshKind::nothing().with_cpu_usage()` — `global_cpu_usage()` as f32
- **RAM/Swap**: `sysinfo` with `MemoryRefreshKind::everything()` — stored as raw u64 bytes
- **Network**: `sysinfo::Networks` — upload/download bytes between refreshes
- **Disk**: `sysinfo::Disks` with `DiskRefreshKind::nothing().with_io_usage()` — read/write bytes between refreshes
- **GPU**: `components/gpu.rs` — NVIDIA via `nvml-wrapper` crate (NVML), AMD via sysfs (`gpu_busy_percent`, `mem_info_vram_*`). Intel iGPUs (i915) lack these sysfs files and are skipped with a diagnostic log
- All values stored in `History<T>` circular buffers (capacity from `config.sampling.*.sampling_window`)

### Config Persistence

- `Config` implements `CosmicConfigEntry` manually (not derived) with `VERSION = 2`
- Top-level fields: `sampling` (`SamplingConfig`), `components` (`Box<[ComponentConfig]>`), `layout` (`LayoutConfig`), `tooltip_enabled` (bool)
- `ComponentConfig` enum variants: `Cpu`, `Mem`, `Net`, `Disk`, `Gpu` — each with view configuration (run charts, bar charts, colors, aspect ratios)
- `SamplingConfig` has per-component `Sampling { update_interval, sampling_window }`
- Read in `main.rs` via `cosmic_config::Config::new(ID, CONFIG_VERSION)` then `Config::get_entry(&handler)`
- Config changes watched via `config_subscription()` → `Message::Config`

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

Minimal libcosmic — just `applet` feature (includes wayland, winit, multi-window, etc.):

```toml
[dependencies.libcosmic]
git = "https://github.com/bgub/libcosmic.git"  # fork with tiny_skia Canvas damage fix
default-features = false
features = ["applet"]
```

Additional dependencies: `sysinfo` (CPU/RAM/Swap/Net/Disk), `nvml-wrapper` (NVIDIA GPU via NVML), `serde` (config serialization).

## Publishing

### GitHub

- **Repo**: https://github.com/bgub/cosmic-ext-applet-sysmon
- Releases use `just tag <version>` then `git push origin main --tags`

### NixOS (nixpkgs)

- Package at `pkgs/by-name/co/cosmic-ext-applet-sysmon/package.nix` in nixpkgs
- A local copy is at `package.nix` in this repo for reference
- Uses `rustPlatform.buildRustPackage` + `libcosmicAppHook` + `just`
- Key nix build details: `dontUseJustBuild = true`, `dontUseJustCheck = true` — cargo builds, just only installs
- `justFlags` must override `prefix` and `bin-src` (with cross-compilation-aware target triple)
