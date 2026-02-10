//! Sort menu handler for context sorting
//!
//! Handles keyboard input for the sort menu popup

use super::super::app::{ActiveBlock, App};
use crate::event::Key;
use crate::sort::{SortContext, SortField};

/// Handle input when the sort menu is active
pub fn handler(key: Key, app: &mut App) {
  let available_fields = match app.sort_context {
    Some(ctx) => ctx.available_fields(),
    None => {
      // No context, close menu
      close_sort_menu(app);
      return;
    }
  };

  match key {
    Key::Esc | Key::Char(',') => {
      close_sort_menu(app);
    }
    Key::Up | Key::Char('k') => {
      if app.sort_menu_selected > 0 {
        app.sort_menu_selected -= 1;
      } else {
        app.sort_menu_selected = available_fields.len().saturating_sub(1);
      }
    }
    Key::Down | Key::Char('j') => {
      if app.sort_menu_selected < available_fields.len().saturating_sub(1) {
        app.sort_menu_selected += 1;
      } else {
        app.sort_menu_selected = 0;
      }
    }
    Key::Enter => {
      if let Some(field) = available_fields.get(app.sort_menu_selected) {
        apply_sort(app, *field);
      }
      close_sort_menu(app);
    }
    // Quick select by shortcut character (lowercase = ascending, uppercase = descending)
    Key::Char(c) => {
      // Find field matching this shortcut
      for field in available_fields {
        if let Some(shortcut) = field.shortcut() {
          if c == shortcut || c == shortcut.to_ascii_uppercase() {
            apply_sort(app, *field);
            // Toggle order if uppercase
            if c.is_ascii_uppercase() {
              if let Some(ctx) = app.sort_context {
                let sort_state = get_sort_state_mut(app, ctx);
                sort_state.order = sort_state.order.toggle();
              }
            }
            close_sort_menu(app);
            return;
          }
        }
      }
    }
    _ => {}
  }
}

/// Open the sort menu for a given context
pub fn open_sort_menu(app: &mut App, context: SortContext) {
  app.sort_context = Some(context);
  app.sort_menu_visible = true;
  app.sort_menu_selected = 0;

  // Find current sort field in the available fields to highlight it
  let current_field = match context {
    SortContext::PlaylistTracks => app.playlist_sort.field,
    SortContext::SavedAlbums => app.album_sort.field,
    SortContext::SavedArtists => app.artist_sort.field,
    SortContext::RecentlyPlayed => SortField::Default, // No persistent sort for this
  };

  let available = context.available_fields();
  for (i, field) in available.iter().enumerate() {
    if *field == current_field {
      app.sort_menu_selected = i;
      break;
    }
  }

  app.set_current_route_state(Some(ActiveBlock::SortMenu), None);
}

fn close_sort_menu(app: &mut App) {
  app.sort_menu_visible = false;
  app.sort_context = None;
  app.set_current_route_state(Some(ActiveBlock::Empty), None);
}

fn apply_sort(app: &mut App, field: SortField) {
  if let Some(ctx) = app.sort_context {
    let sort_state = get_sort_state_mut(app, ctx);
    sort_state.apply_field(field);

    // Actually sort the data
    match ctx {
      SortContext::PlaylistTracks => {
        // For playlists, dispatch network event to fetch all tracks and sort
        // Get the current playlist ID
        if let Some(active_playlist_index) = app.active_playlist_index {
          if let Some(playlist) = app.all_playlists.get(active_playlist_index) {
            let playlist_id = playlist.id.clone().into_static();
            app.dispatch(crate::network::IoEvent::FetchAllPlaylistTracksAndSort(
              playlist_id,
            ));
          }
        }
      }
      SortContext::SavedAlbums => sort_saved_albums(app),
      SortContext::SavedArtists => sort_saved_artists(app),
      SortContext::RecentlyPlayed => { /* no persistent sort */ }
    }
  }
}

fn get_sort_state_mut(app: &mut App, ctx: SortContext) -> &mut crate::sort::SortState {
  match ctx {
    SortContext::PlaylistTracks => &mut app.playlist_sort,
    SortContext::SavedAlbums => &mut app.album_sort,
    SortContext::SavedArtists => &mut app.artist_sort,
    SortContext::RecentlyPlayed => &mut app.playlist_sort, // fallback
  }
}

fn sort_saved_albums(app: &mut App) {
  use crate::sort::SortOrder;

  let sort_state = app.album_sort;

  // Sort library.saved_albums pages
  for page in &mut app.library.saved_albums.pages {
    page.items.sort_by(|a, b| {
      let cmp = match sort_state.field {
        SortField::Default => std::cmp::Ordering::Equal,
        SortField::Name => a
          .album
          .name
          .to_lowercase()
          .cmp(&b.album.name.to_lowercase()),
        SortField::Artist => {
          let artist_a = a
            .album
            .artists
            .first()
            .map(|a| a.name.to_lowercase())
            .unwrap_or_default();
          let artist_b = b
            .album
            .artists
            .first()
            .map(|a| a.name.to_lowercase())
            .unwrap_or_default();
          artist_a.cmp(&artist_b)
        }
        SortField::DateAdded => a.added_at.cmp(&b.added_at),
        _ => std::cmp::Ordering::Equal,
      };

      if sort_state.order == SortOrder::Descending {
        cmp.reverse()
      } else {
        cmp
      }
    });
  }
}

fn sort_saved_artists(app: &mut App) {
  use crate::sort::SortOrder;

  let sort_state = app.artist_sort;

  // Sort library.saved_artists pages
  for page in &mut app.library.saved_artists.pages {
    page.items.sort_by(|a, b| {
      let cmp = match sort_state.field {
        SortField::Default => std::cmp::Ordering::Equal,
        SortField::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        _ => std::cmp::Ordering::Equal,
      };

      if sort_state.order == SortOrder::Descending {
        cmp.reverse()
      } else {
        cmp
      }
    });
  }

  // Also sort the app.artists vec
  app.artists.sort_by(|a, b| {
    let cmp = match sort_state.field {
      SortField::Default => std::cmp::Ordering::Equal,
      SortField::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
      _ => std::cmp::Ordering::Equal,
    };

    if sort_state.order == SortOrder::Descending {
      cmp.reverse()
    } else {
      cmp
    }
  });
}
