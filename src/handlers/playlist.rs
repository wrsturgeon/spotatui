use super::{
  super::app::{App, DialogContext, PlaylistFolderItem, TrackTableContext},
  common_key_events,
};
use crate::app::{ActiveBlock, RouteId};
use crate::event::Key;
use crate::network::IoEvent;

pub fn handler(key: Key, app: &mut App) {
  match key {
    k if common_key_events::right_event(k) => common_key_events::handle_right_event(app),
    k if common_key_events::down_event(k) => {
      let count = app.get_playlist_display_count();
      if count > 0 {
        let current = app.selected_playlist_index.unwrap_or(0);
        app.selected_playlist_index = Some((current + 1) % count);
      }
    }
    k if common_key_events::up_event(k) => {
      let count = app.get_playlist_display_count();
      if count > 0 {
        let current = app.selected_playlist_index.unwrap_or(0);
        app.selected_playlist_index = Some(if current == 0 { count - 1 } else { current - 1 });
      }
    }
    k if common_key_events::high_event(k) => {
      if app.get_playlist_display_count() > 0 {
        app.selected_playlist_index = Some(0);
      }
    }
    k if common_key_events::middle_event(k) => {
      let count = app.get_playlist_display_count();
      if count > 0 {
        let next_index = if count.is_multiple_of(2) {
          count.saturating_sub(1) / 2
        } else {
          count / 2
        };
        app.selected_playlist_index = Some(next_index);
      }
    }
    k if common_key_events::low_event(k) => {
      let count = app.get_playlist_display_count();
      if count > 0 {
        app.selected_playlist_index = Some(count - 1);
      }
    }
    Key::Enter => {
      if let Some(selected_idx) = app.selected_playlist_index {
        if let Some(item) = app.get_playlist_display_item_at(selected_idx) {
          match item {
            PlaylistFolderItem::Folder(folder) => {
              // Navigate into/out of folder
              app.current_playlist_folder_id = folder.target_id;
              app.selected_playlist_index = Some(0);
            }
            PlaylistFolderItem::Playlist { index, .. } => {
              // Open the playlist tracks
              if let Some(playlist) = app.all_playlists.get(*index) {
                app.active_playlist_index = Some(*index);
                app.track_table.context = Some(TrackTableContext::MyPlaylists);
                app.playlist_offset = 0;
                let playlist_id = playlist.id.clone().into_static();
                app.dispatch(IoEvent::GetPlaylistItems(
                  playlist_id.clone(),
                  app.playlist_offset,
                ));
                // Pre-fetch more pages in background for seamless playback
                app.dispatch(IoEvent::PreFetchAllPlaylistTracks(playlist_id));
              }
            }
          }
        }
      }
    }
    Key::Char('D') => {
      if let Some(selected_idx) = app.selected_playlist_index {
        if let Some(PlaylistFolderItem::Playlist { index, .. }) =
          app.get_playlist_display_item_at(selected_idx)
        {
          if let Some(playlist) = app.all_playlists.get(*index) {
            let selected_playlist = &playlist.name;
            app.dialog = Some(selected_playlist.clone());
            app.confirm = false;

            app.push_navigation_stack(
              RouteId::Dialog,
              ActiveBlock::Dialog(DialogContext::PlaylistWindow),
            );
          }
        }
      }
    }
    _ => {}
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn test() {}
}
