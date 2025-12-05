# Spotatui Copilot Instructions

A Rust terminal UI Spotify client using ratatui (TUI framework) and rspotify (Spotify Web API).

## Architecture Overview

```
main.rs          → Entry point, auth flow, UI/network thread split
app.rs           → Central state (`App` struct), navigation, dispatch system
network.rs       → Async Spotify API calls via `IoEvent` enum
handlers/        → Keyboard input handlers per UI block
ui/              → Rendering logic using ratatui widgets
event/           → Crossterm event polling, custom `Key` enum
cli/             → Headless command-line interface
```

**Core Pattern**: UI runs on main thread, network operations spawn on a separate tokio task. Communication uses `std::sync::mpsc` channels with `IoEvent` enum for requests, `Arc<Mutex<App>>` for shared state updates.

## Key Conventions

### Handler Pattern (see `handlers/`)

Each `ActiveBlock` has a corresponding handler file with a `handler(key: Key, app: &mut App)` function:

```rust
pub fn handler(key: Key, app: &mut App) {
  match key {
    k if common_key_events::down_event(k) => { /* ... */ }
    k if common_key_events::up_event(k) => { /* ... */ }
    Key::Enter => { /* action */ }
    _ => {}
  }
}
```

Use `common_key_events` for navigation (j/k/h/l + arrows). Reserved keys (h/j/k/l, H/M/L, arrows, Enter, Backspace) cannot be remapped.

### Dispatching Network Requests

Never call Spotify API directly from handlers. Use the dispatch pattern:

```rust
app.dispatch(IoEvent::GetPlaylists);
app.dispatch(IoEvent::StartPlayback(context, uris, offset));
```

Add new operations to `IoEvent` enum in `network.rs`, implement in `handle_network_event()`.

### Navigation Stack

Use `push_navigation_stack()` / `pop_navigation_stack()` for view changes:

```rust
app.push_navigation_stack(RouteId::Artist, ActiveBlock::ArtistBlock);
app.set_current_route_state(Some(ActiveBlock::Input), Some(ActiveBlock::Input));
```

### Configuration

- **Auth config**: `~/.config/spotatui/client.yml` (client_id, client_secret, port)
- **User config**: `~/.config/spotatui/config.yml` (theme, keybindings, behavior)
- Parse configs in `config.rs` (auth) and `user_config.rs` (UI settings)

## Build & Development

```bash
cargo run --release           # Run with optimizations
cargo build                   # Debug build
cargo test                    # Run tests
cargo fmt --all               # Format (2-space indent, see rustfmt.toml)
cargo clippy                  # Lint
```

**CI Requirements**: Code must pass `cargo fmt --check`, `cargo clippy`, and `cargo test`.

## Adding Features

### New UI View

1. Add variant to `ActiveBlock` and `RouteId` enums in `app.rs`
2. Create handler in `handlers/new_view.rs`, register in `handlers/mod.rs`
3. Add rendering in `ui/mod.rs` `draw_main_layout()` or create new draw function
4. Wire navigation via `push_navigation_stack()`

### New Spotify API Call

1. Add variant to `IoEvent` enum in `network.rs`
2. Implement handler in `Network::handle_network_event()`
3. Update `App` state with results using `self.app.lock().await`
4. Dispatch from handler: `app.dispatch(IoEvent::NewOperation(params))`

### New Keybinding

1. Add field to `KeyBindingsString` in `user_config.rs`
2. Add to `KeyBindings` struct with default in `impl Default`
3. Handle in `handlers/mod.rs` `handle_app()` for global keys, or in specific handler

## Rspotify ID Handling

Spotify IDs require static lifetimes for async dispatch. Use conversion pattern:

```rust
let playlist_id = PlaylistId::from_id(id_string).into_static();
app.dispatch(IoEvent::GetPlaylistItems(playlist_id, offset));
```

## UI/UX Notes

- Theme colors support RGB strings (`"255, 255, 255"`) or terminal color names
- Icons (`liked_icon`, `shuffle_icon`) require nerd fonts for proper display
- Tick rate affects audio visualization smoothness vs CPU usage
