# iced tiny_skia Canvas Bugs: Damage Tracking and Clipping

## Overview

Three related bugs in iced's `tiny_skia` software renderer cause `Canvas` widgets
to either never repaint or render with incorrect/missing clipping. All three affect
any COSMIC application that uses `Canvas` widgets without the `wgpu` feature (i.e.,
applets using only the software renderer).

**Status:** Fixed in `bgub/iced` branch `fix/tiny-skia-canvas-damage`. Upstream PR
filed at https://github.com/pop-os/iced/pull/270.

---

## Bug 1 — Damage tracking returns empty rectangles

**File:** `iced/tiny_skia/src/layer.rs` — `damage()` function, `Item::Group` branch

**Symptom:** `Canvas` widgets never repaint after the initial render. Application
logic (`update`, `view`, `Program::draw`) continues running, but the compositor
receives empty damage rectangles so stale pixels are never overwritten.

### Root cause

```rust
// BEFORE (broken)
Item::Group(primitives, group_bounds, transformation) => {
    primitives
        .as_slice()
        .iter()
        .map(Primitive::visible_bounds)         // LOCAL coordinates (e.g. 0,0,36,18)
        .map(|bounds| bounds * *transformation) // GLOBAL coordinates (e.g. 100,50,36,18)
        .filter_map(|bounds| bounds.intersection(group_bounds)) // group_bounds is LOCAL!
        .collect()
}
```

1. `Primitive::visible_bounds()` returns bounds in the Canvas Frame's **local**
   coordinate space (origin at 0,0).
2. `bounds * *transformation` converts to **global** (viewport) coordinates —
   e.g., `(100, 50, 36, 18)`.
3. `bounds.intersection(group_bounds)` intersects the **global** bounds with
   `group_bounds`, which is still in **local** space — e.g., `(0, 0, 36, 18)`.
4. The two rectangles don't overlap → intersection returns `None`.
5. Result: zero damage rectangles every frame after the first.

### Why it works on the first frame

On the very first frame there are no previous layers to diff, so `damage` falls back
to the full viewport rectangle. From frame 2 onward, `damage::list` diffs old vs.
new layers using the `bounds` closure, which always returns empty.

### Fix

```rust
// AFTER (fixed)
Item::Group(_, group_bounds, transformation) => {
    vec![*group_bounds * *transformation]
}
```

Returns the entire group clip region in global coordinates as the damage rectangle.
It's slightly less granular than per-primitive damage but correct, simpler, and
adequate for small Canvas widgets.

---

## Bug 2 — Canvas clip region not translated to screen space

**File:** `iced/tiny_skia/src/lib.rs` — `draw()` method, group processing loop

**Symptom:** Canvas content is clipped to a region anchored at `(0, 0)` in screen
space rather than the widget's actual position. For a Canvas widget positioned away
from the top-left corner, nothing (or the wrong portion) is visible.

### Root cause

```rust
// BEFORE (broken)
let Some(new_clip_bounds) = (group.clip_bounds()
    * scale_factor)          // only scales — no translation!
    .intersection(&clip_bounds)
else { continue; };
```

`group.clip_bounds()` returns the Frame bounds in **local** coordinates
(e.g., `{x:0, y:0, w:36, h:18}`). The code scaled these but never applied
`group.transformation()`, which carries the translation to screen position.
So the clip region always started at `(0, 0)` scaled, not at the widget's
actual screen position.

### Companion issue — wrong bounds passed to `draw_primitive`

Even after computing the correct `new_clip_bounds`, the old code passed the outer
`clip_bounds` (full layer bounds) to `draw_primitive`. Inside `draw_primitive`,
whether the clip mask is applied is decided by comparing the primitive's physical
bounds to the passed-in clip bounds. Passing the wide layer bounds meant the mask
was skipped for primitives that extended to the canvas edge.

### Fix

```rust
// AFTER (fixed)
let Some(new_clip_bounds) = (group.clip_bounds()
    * group.transformation()             // apply translation first
    * Transformation::scale(scale_factor))
.intersection(&clip_bounds) else { continue; };

// ...

self.engine.draw_primitive(
    primitive,
    ...,
    pixels,
    clip_mask,
    new_clip_bounds,   // was: clip_bounds
);
```

---

## Bug 3 — Stroke width not accounted for in clip mask decision

**File:** `iced/tiny_skia/src/engine.rs` — `draw_primitive()`, stroke path

**Symptom:** Strokes drawn at the edge of a `Canvas` frame bleed outside the frame
boundary. A 1px stroke along the bottom edge of a 36×18 canvas paints into adjacent
pixels below the canvas.

### Root cause

The code decided whether to apply a clip mask by comparing `physical_bounds`
(computed from `path.bounds()`) to `layer_bounds`:

```rust
// BEFORE (broken)
let clip_mask = (physical_bounds != clip_bounds).then_some(clip_mask as &_);
```

`path.bounds()` is the geometric path boundary — it does not include stroke width.
A 1px stroke exactly at `y = canvas_height` has
`physical_bounds.bottom() == layer_bounds.bottom()`, so the `!=` check is `false`
and the clip mask is not applied, allowing the stroke to bleed.

The same `!=` equality check for fills was also fragile — anti-aliased pixels can
render slightly outside `path.bounds()`.

### Fix

For fills, use `is_within` instead of equality:
```rust
let clip_mask = (!physical_bounds.is_within(&layer_bounds))
    .then_some(clip_mask as &_);
```

For strokes, expand physical bounds by half the stroke width before the check:
```rust
let half_width = stroke.width / 2.0;
let rendered_bounds = Rectangle {
    x: physical_bounds.x - half_width,
    y: physical_bounds.y - half_width,
    width: physical_bounds.width + half_width * 2.0,
    height: physical_bounds.height + half_width * 2.0,
};
let clip_mask = (!rendered_bounds.is_within(&layer_bounds))
    .then_some(clip_mask as &_);
```

---

## Why wgpu Doesn't Have These Bugs

With wgpu enabled, `Renderer = fallback::Renderer<iced_wgpu::Renderer, iced_tiny_skia::Renderer>`.
The wgpu renderer has its own damage/presentation pipeline and does not use the
`tiny_skia` layer damage or clipping code paths.

---

## Reproduction

Any COSMIC applet using `Canvas` without the `wgpu` feature:

1. Build with only `features = ["applet"]` in the libcosmic dependency
2. Place a `Canvas` widget somewhere other than the top-left corner
3. Update canvas data on a timer

**Observed (unfixed):**
- Canvas freezes after 2-3 frames (Bug 1)
- Canvas clip region is anchored at `(0,0)` instead of widget position (Bug 2)
- Strokes at canvas edge bleed outside the frame (Bug 3)

**Confirmed (fixed):** `update()`, `view()`, and `Program::draw()` all fire each
frame; `compositor.present()` and softbuffer commits fire; but damage regions were
empty (Bug 1) or the clip was mispositioned (Bug 2).

---

## File Reference

| File | Bug |
|------|-----|
| `iced/tiny_skia/src/layer.rs` | Bug 1 — damage tracking |
| `iced/tiny_skia/src/lib.rs` | Bug 2 — clip bounds not transformed; wrong bounds to draw_primitive |
| `iced/tiny_skia/src/engine.rs` | Bug 3 — stroke width not in clip decision |
| `iced/graphics/src/damage.rs` | `damage::list` / `damage::diff` — consumes the `bounds` closure |
| `iced/tiny_skia/src/geometry.rs` | `Frame::into_geometry()` — produces `Geometry::Live` |
| `iced/widget/src/canvas.rs` | Canvas widget — applies `with_translation(bounds.x, bounds.y)` |

---

## Fork / Workaround

Until the upstream PR is merged, this project depends on:

- `bgub/iced` branch `fix/tiny-skia-canvas-damage` (commits `0211b55f`, `2003467f`)
- `bgub/libcosmic` branch `fix/tiny-skia-canvas-damage` (submodule points to above)

```toml
# Cargo.toml
[dependencies.libcosmic]
git = "https://github.com/bgub/libcosmic.git"
branch = "fix/tiny-skia-canvas-damage"
```

Once the fixes are upstreamed, switch back to:

```toml
[dependencies.libcosmic]
git = "https://github.com/pop-os/libcosmic.git"
```
