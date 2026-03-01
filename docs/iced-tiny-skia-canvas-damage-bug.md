# iced tiny_skia Canvas Damage Tracking Bug

## Summary

A coordinate space mismatch in iced's tiny_skia software renderer causes
`Canvas` widgets to stop visually updating after the first few frames. The
application logic (update, view, draw) continues running correctly, but the
renderer computes zero damage rectangles for Canvas geometry, so nothing new
is painted to the screen.

This affects any COSMIC application that uses `Canvas` widgets with the
tiny_skia (software) renderer — i.e., applets without the `wgpu` feature.

**Status:** Fixed locally via fork. Upstream PR pending against `pop-os/iced`.

## The Bug

**File:** `iced/tiny_skia/src/layer.rs`, in the `damage` function  
**Location:** The `bounds` closure passed to `damage::list` for `Item::Group`

### Original Code

```rust
Item::Group(primitives, group_bounds, transformation) => {
    primitives
        .as_slice()
        .iter()
        .map(Primitive::visible_bounds)       // LOCAL coordinates (e.g., 0,0,36,18)
        .map(|bounds| bounds * *transformation) // GLOBAL coordinates (e.g., 100,50,36,18)
        .filter_map(|bounds| bounds.intersection(group_bounds)) // group_bounds is LOCAL!
        .collect()
}
```

### What Goes Wrong

1. `Primitive::visible_bounds()` returns bounds in the Canvas Frame's **local**
   coordinate space (origin at 0,0).
2. `bounds * *transformation` transforms these to **global** (viewport)
   coordinates — e.g., `(100, 50, 36, 18)`.
3. `bounds.intersection(group_bounds)` intersects the **global** bounds with
   `group_bounds`, which is still in **local** coordinates — e.g., `(0, 0, 36, 18)`.
4. Since the global-coordinate rectangle `(100, 50, 36, 18)` does not overlap
   with the local-coordinate rectangle `(0, 0, 36, 18)`, the intersection
   returns `None`.
5. Result: **zero damage rectangles**, even though the Canvas content changed.

### Why It Works on First Frame

On the very first frame there are no previous layers to diff against, so
`damage` falls back to the full viewport rectangle. After that, the
`damage::list` function diffs old vs. new layers and relies on the `bounds`
closure to compute what regions changed — which returns empty due to the
mismatch.

### Why wgpu Doesn't Have This Bug

With wgpu enabled, `Renderer = fallback::Renderer<iced_wgpu::Renderer,
iced_tiny_skia::Renderer>`. The wgpu renderer has its own damage/presentation
pipeline that doesn't use this tiny_skia layer damage code path.

## The Fix

Replace the per-primitive damage computation with the transformed group bounds:

```rust
Item::Group(_, group_bounds, transformation) => {
    vec![*group_bounds * *transformation]
}
```

This returns the entire group's clip region (in global coordinates) as the
damage rectangle. It's slightly less granular than per-primitive damage, but:

- It's **correct** — the coordinates are in the right space.
- It's **simpler** — no per-primitive iteration or intersection.
- For small Canvas widgets (like sparkline charts), the overhead of repainting
  the full group area vs. individual primitives is negligible.
- It eliminates **edge artifacts** that appeared with the per-primitive approach
  (likely due to `visible_bounds()` not accounting for stroke width or
  antialiasing overshoot).

## Reproduction

Any COSMIC applet using `Canvas` widgets without the `wgpu` feature will
exhibit this bug:

1. Build without wgpu (just `features = ["applet"]` in libcosmic dependency)
2. The Canvas renders correctly on the first frame
3. After 2-3 frames, the Canvas visually freezes
4. `update()`, `view()`, and `Program::draw()` all continue firing (confirmed
   via debug logging)
5. `compositor.present()` is called every frame
6. `softbuffer` commits the buffer every frame
7. But the buffer contains stale pixel data because damage regions are empty

## Files

| File | Description |
|------|-------------|
| `iced/tiny_skia/src/layer.rs` | **The bug** — `bounds` closure for `Item::Group` |
| `iced/graphics/src/damage.rs` | `damage::list` and `damage::diff` — consumes the `bounds` closure |
| `iced/tiny_skia/src/lib.rs` | `draw_geometry()` — stores Canvas output as `Item::Group` |
| `iced/tiny_skia/src/geometry.rs` | `Frame::into_geometry()` — produces `Geometry::Live` |
| `iced/widget/src/canvas.rs` | Canvas widget — applies `with_translation(bounds.x, bounds.y)` |

## Temporary Workaround

This project uses a fork of libcosmic with the fix applied:

- `bgub/iced` branch `fix/tiny-skia-canvas-damage`
- `bgub/libcosmic` branch `fix/tiny-skia-canvas-damage` (submodule points to above)

In `Cargo.toml`:

```toml
[dependencies.libcosmic]
git = "https://github.com/bgub/libcosmic.git"
branch = "fix/tiny-skia-canvas-damage"
```

Once the fix is upstreamed, switch back to:

```toml
[dependencies.libcosmic]
git = "https://github.com/pop-os/libcosmic.git"
```
