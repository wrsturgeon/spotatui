# Ratatui 0.30.0 Conversion Plan

## Overview

| Current | Target |
|---------|--------|
| ratatui 0.26 | ratatui 0.30.0 |
| crossterm 0.27 | crossterm 0.29 |
| Rust 1.90.0 ✓ | MSRV 1.86.0 ✓ |

---

## Codebase Analysis Summary

| Breaking Change | Files Affected | Count |
|-----------------|----------------|-------|
| `f.size()` → `f.area()` | 4 files | ~11 occurrences |
| `block::Title` removed | 0 files | Not used ✓ |
| `Alignment` glob imports | 0 files | Not used ✓ |
| `Marker::` exhaustive match | 0 files | Not used ✓ |

---

## Phase 1: Dependency Updates

### Cargo.toml

```diff
-ratatui = { version = "0.26", features = ["crossterm"], default-features = false }
-crossterm = "0.27"
+ratatui = { version = "0.30", features = ["crossterm"], default-features = false }
+crossterm = "0.29"

# Add visualization crates (now compatible)
+tui-equalizer = "0.2.0-alpha"
+tui-bar-graph = "0.3"
+colorgrad = "0.8"
```

Update audio-viz features:
```diff
-audio-viz = ["realfft", "pipewire"]
-audio-viz-cpal = ["realfft", "cpal"]
+audio-viz = ["realfft", "pipewire", "tui-equalizer", "tui-bar-graph", "colorgrad"]
+audio-viz-cpal = ["realfft", "cpal", "tui-equalizer", "tui-bar-graph", "colorgrad"]
```

---

## Phase 2: Breaking Change Fixes

### `f.size()` → `f.area()`

The `Frame::size()` method was deprecated in 0.28 and renamed to `Frame::area()`.

#### src/ui/mod.rs - 9 occurrences

| Line | Change |
|------|--------|
| 76 | `.split(f.size())` → `.split(f.area())` |
| 178 | `.split(f.size())` → `.split(f.area())` |
| 197 | `.split(f.size())` → `.split(f.area())` |
| 919 | `.split(f.size())` → `.split(f.area())` |
| 1185 | `.split(f.size())` → `.split(f.area())` |
| 1492 | `.split(f.size())` → `.split(f.area())` |
| 2006 | `let bounds = f.size()` → `let bounds = f.area()` |
| 2202 | `let bounds = f.size()` → `let bounds = f.area()` |
| 2268 | `let bounds = f.size()` → `let bounds = f.area()` |

#### src/ui/settings.rs - 1 occurrence

| Line | Change |
|------|--------|
| 19 | `.split(f.size())` → `.split(f.area())` |

#### src/ui/audio_analysis.rs - 1 occurrence

| Line | Change |
|------|--------|
| 25 | `.split(f.size())` → `.split(f.area())` |

#### src/main.rs - 1 occurrence

| Line | Change |
|------|--------|
| 1576 | `f.size()` → `f.area()` |

> **Note**: Lines 1313 and 1547 use `terminal.backend().size()` which is NOT deprecated - this is the Backend method, not Frame.

---

## Phase 3: Visualizer Integration (Josh McKinney's Widgets)

> **Credits**: The visualization crates [`tui-equalizer`](https://github.com/joshka/tui-equalizer) and 
> [`tui-bar-graph`](https://github.com/joshka/tui-widgets/tree/main/tui-bar-graph) were suggested by 
> [**Josh McKinney**](https://github.com/joshka) — Ratatui core maintainer and OpenAI engineer.
> Thank you Josh for the recommendation!

### src/ui/audio_analysis.rs

Replace the native visualizer implementations with the external crates:

```rust
#[cfg(any(feature = "audio-viz", feature = "audio-viz-cpal"))]
use tui_bar_graph::{BarGraph, BarStyle, ColorMode};
#[cfg(any(feature = "audio-viz", feature = "audio-viz-cpal"))]
use tui_equalizer::{Band, Equalizer};

// For Equalizer style:
fn render_equalizer(f: &mut Frame<'_>, bands: &[f32], area: Rect) {
  let eq_bands: Vec<Band> = bands.iter().map(|&v| Band::from(v as f64)).collect();
  let equalizer = Equalizer { bands: eq_bands, brightness: 1.0 };
  f.render_widget(equalizer, area);
}

// For BarGraph style:
fn render_bar_graph(f: &mut Frame<'_>, bands: &[f32], area: Rect) {
  let data: Vec<f64> = bands.iter().map(|&v| v as f64).collect();
  let bar_graph = BarGraph::new(data)
    .with_gradient(colorgrad::preset::turbo())
    .with_bar_style(BarStyle::Braille)
    .with_color_mode(ColorMode::VerticalGradient)
    .with_max(1.0);
  f.render_widget(bar_graph, area);
}
```

---

## Phase 4: New Features to Adopt 🚀

### 1. `ratatui::run()` - New Simplified Execution API

The new `ratatui::run()` handles terminal initialization and restoration automatically:

```rust
// Before (current)
fn main() -> Result<()> {
  let terminal = ratatui::init();
  let result = run(terminal);
  ratatui::restore();
  result
}

// After (0.30.0)
fn main() -> Result<()> {
  ratatui::run(|terminal| run(terminal))
}
```

**Recommended**: Consider refactoring `main.rs` to use this pattern.

---

### 2. Border Merging 🧩

Overlapping borders now automatically merge into clean corners:

```
Before: ┘┏  →  After: ╆
```

Useful for adjacent panels. Enable with `MergeStrategy`.

---

### 3. New BorderTypes 🎨

Choose from new dashed border styles for UI variety:

| Type | Example |
|------|---------|
| `LightDoubleDashed` | `┌╌╌╌╌┐` |
| `HeavyDoubleDashed` | `┏╍╍╍╍┓` |
| `LightTripleDashed` | `┌┄┄┄┄┐` |
| `HeavyTripleDashed` | `┏┅┅┅┅┓` |
| `LightQuadrupleDashed` | `┌┈┈┈┈┐` |
| `HeavyQuadrupleDashed` | `┏┉┉┉┉┓` |

**Idea**: Use for loading states or disabled UI elements.

---

### 4. Canvas Markers (High-Resolution Graphics)

New marker types for Canvas widget:

| Marker | Resolution | Use Case |
|--------|------------|----------|
| `Marker::Quadrant` | 2x2 per char | Compact graphics |
| `Marker::Sextant` | 2x3 per char | Medium detail |
| `Marker::Octant` | 2x4 per char | Alternative to Braille |

**Idea**: Upgrade album art rendering or waveforms.

---

### 5. Styled List Highlight Symbols

`List::highlight_symbol` now accepts styled `Line`:

```rust
// Before
List::new(items).highlight_symbol(">> ")

// After (0.30.0) - with color!
List::new(items).highlight_symbol(
  Line::from("▶ ").green().bold()
)
```

**Recommended**: Update all List widgets with themed highlight symbols.

---

### 6. Const Style Shortcuts

Create const styles using shorthand methods:

```rust
const ACTIVE_STYLE: Style = Style::new().green().bold();
const ERROR_STYLE: Style = Style::new().red().on_black();
```

**Recommended**: Add to `user_config.rs` for theme constants.

---

### 7. Color Tuple Conversions

Create colors from tuples:

```rust
Color::from((255, 0, 0))      // RGB tuple
Color::from([0, 255, 0])      // RGB array
Color::from((0, 0, 255, 128)) // RGBA with alpha
```

---

### 8. LineGauge with Custom Symbols (Playbar Upgrade!) 🎵

**Current**: `Gauge::default()` at `src/ui/mod.rs:1163`

**New**: `LineGauge` now supports custom filled/unfilled symbols:

```rust
use ratatui::widgets::LineGauge;

// Current playbar (Gauge)
let song_progress = Gauge::default()
  .gauge_style(style)
  .percent(perc)
  .label(label);

// Upgraded playbar (LineGauge with custom symbols)
let song_progress = LineGauge::default()
  .filled_symbol("█")     // Solid block for elapsed
  .unfilled_symbol("░")   // Light shade for remaining
  .ratio(perc as f64 / 100.0)
  .label(label)
  .gauge_style(style);
```

**Result**:
```
80% ████████████░░░░
```

**Alternative symbols**:
| Style | Filled | Unfilled | Example |
|-------|--------|----------|---------|
| Blocks | `█` | `░` | `████░░░░` |
| Dots | `●` | `○` | `●●●●○○○○` |
| Arrows | `▶` | `▷` | `▶▶▶▶▷▷▷▷` |
| Bars | `━` | `─` | `━━━━────` |

**Recommended**: Update the playbar for a modern look!

---

### 9. Rect Centering Helpers 🎯

New ergonomic methods for centering content:

```rust
// Center a popup in the screen
let popup_area = frame.area()
  .centered(Constraint::Percent(50), Constraint::Percent(30));

// Center vertically only
let area = frame.area().centered_vertically(Constraint::Length(5));

// Center horizontally only  
let area = frame.area().centered_horizontally(Constraint::Percent(80));
```

**Idea**: Simplify dialog/popup centering logic.

---

### 10. Ergonomic Layout with Array Destructuring

```rust
// Before
let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(0)])
  .split(area);
let header = chunks[0];
let body = chunks[1];

// After (0.30.0) - Compile-time checked!
let [header, body] = area.layout(&Layout::vertical([
  Constraint::Length(3),
  Constraint::Min(0),
]));
```

---

### 11. Layout Cache Feature Flag ⚡

If you disable `default-features`, add `layout-cache` for performance:

```toml
ratatui = { version = "0.30", default-features = false, features = ["crossterm", "layout-cache"] }
```

---

### 12. `Direction::perpendicular()` 🔄

Easily get the perpendicular direction for responsive layouts:

```rust
let perpendicular = Direction::Vertical.perpendicular();   // → Horizontal
let perpendicular = Direction::Horizontal.perpendicular(); // → Vertical
```

**Idea**: Dynamically switch layouts based on terminal orientation.

---

### 13. CSS-like Flex Layout Options 📐

New `Flex` variants that match CSS flexbox behavior:

| Variant | Behavior |
|---------|----------|
| `Flex::SpaceEvenly` | Equal space between items *and* edges |
| `Flex::SpaceAround` | Space between items is 2× edge spacing |
| `Flex::SpaceBetween` | No space at edges, only between items |
| `Flex::Center` | Centers items with equal space on both sides |
| `Flex::Start` / `Flex::End` | Align to start or end |

> **⚠️ Breaking**: Old `Flex::SpaceAround` behavior is now `Flex::SpaceEvenly`

---

### 14. BarChart Improvements 📊

- **`BarChart::grouped()`** - New constructor for grouped bar charts
- **`Bar` implements `Styled`** - Apply styles directly to bars
- **Simplified API** - Less verbose syntax:

```rust
// Before
Bar::default().label("foo".into());

// After (0.30.0)
Bar::default().label("foo");
```

---

### 15. Layered Chart/Canvas Rendering 🎨

Braille characters now render **over** block symbols, enabling:
- Stacked charts with text overlays
- Text on top of visualizations while showing background symbols

**Idea**: Overlay track info on the audio visualizer!

---

### 16. `Style::has_modifier()` Method ✅

Check if a style has a specific modifier:

```rust
let style = Style::default().bold();
if style.has_modifier(Modifier::BOLD) {
    // Style is bold
}
```

---

### 17. Multiple Crossterm Versions Support 🔀

Feature flags allow library authors to depend on specific crossterm versions:

```toml
# Explicitly use crossterm 0.29
ratatui = { version = "0.30", features = ["crossterm_0_29"] }

# Or use crossterm 0.28 for compatibility
ratatui = { version = "0.30", features = ["crossterm_0_28"] }
```

---

### 18. `Offset::new()` Constructor 📍

New ergonomic constructor:

```rust
// Before
let offset = Offset { x: 10, y: 5 };

// After (0.30.0)
let offset = Offset::new(10, 5);
```

---

### 19. Calendar Widget Improvements 📅

New `width()` and `height()` functions to query calendar dimensions:

```rust
let calendar = Monthly::new(date, CalendarEventStore::default());
let width = calendar.width();   // Returns expected width
let height = calendar.height(); // Returns expected height
```

---

## Priority Summary for Spotatui 🎯

| Feature | Priority | Reason |
|---------|----------|--------|
| LineGauge custom symbols | ⭐⭐⭐ | Modern playbar look |
| Styled List highlights | ⭐⭐⭐ | Colored `▶` in playlists |
| Layered Braille rendering | ⭐⭐⭐ | Text over visualizers |
| Flex::SpaceEvenly | ⭐⭐ | Better panel layouts |
| Octant markers | ⭐⭐ | Sharper visualizer graphics |
| Direction::perpendicular() | ⭐ | Responsive layouts |
| Bar improvements | ⭐ | Cleaner code |

---

## Verification Checklist

- [ ] `cargo build --features audio-viz-cpal`
- [ ] `cargo clippy --features audio-viz-cpal`
- [ ] Run app, navigate all screens
- [ ] Test visualizer: press `v`, then `V` to cycle styles

---

## Rollback Plan

If issues arise, revert Cargo.toml:
```toml
ratatui = { version = "0.26", features = ["crossterm"], default-features = false }
crossterm = "0.27"
```
