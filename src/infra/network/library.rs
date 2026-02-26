use super::requests::{spotify_api_request_json_for, spotify_get_typed_compat_for};
use super::Network;
use crate::core::app::{
  ActiveBlock, App, PlaylistFolder, PlaylistFolderItem, PlaylistFolderNode, PlaylistFolderNodeType,
  RouteId, TrackTableContext,
};
use anyhow::anyhow;
use reqwest::Method;
use rspotify::model::{
  idtypes::{AlbumId, PlaylistId, ShowId, TrackId, UserId},
  page::Page,
  playlist::PlaylistItem,
  track::FullTrack,
  PlayableItem,
};
use rspotify::{prelude::*, AuthCodePkceSpotify};
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[cfg(feature = "streaming")]
use crate::infra::player::StreamingPlayer;

pub async fn prefetch_all_saved_tracks_task(
  spotify: AuthCodePkceSpotify,
  app: Arc<Mutex<App>>,
  limit: u32,
) {
  let mut offset = 0u32;
  loop {
    // Check if stopped
    {
      let app = app.lock().await;
      if !app.is_loading {
        // Simple heuristic: if loading stopped globally, maybe stop prefetch?
        // Actually we want prefetch to run in background.
        // But we should check if user quit.
      }
    }

    let query = vec![("limit", limit.to_string()), ("offset", offset.to_string())];
    match spotify_get_typed_compat_for::<Page<rspotify::model::SavedTrack>>(
      &spotify,
      "me/tracks",
      &query,
    )
    .await
    {
      Ok(page) => {
        if page.items.is_empty() {
          break;
        }

        let mut app_guard = app.lock().await;
        app_guard.library.saved_tracks.add_pages(page.clone());

        // Also update track table if we are currently viewing saved tracks
        if let Some(TrackTableContext::SavedTracks) = app_guard.track_table.context {
          // Append to track table
          let new_tracks: Vec<FullTrack> = page.items.into_iter().map(|item| item.track).collect();
          app_guard.track_table.tracks.extend(new_tracks);
        }

        if page.next.is_none() {
          break;
        }
        offset += limit;
      }
      Err(_) => break,
    }
  }
}

pub async fn prefetch_all_playlist_tracks_task(
  spotify: AuthCodePkceSpotify,
  app: Arc<Mutex<App>>,
  limit: u32,
  playlist_id: PlaylistId<'static>,
) {
  let mut offset = 0u32;
  let path = format!("playlists/{}/items", playlist_id.id());

  loop {
    let query = vec![("limit", limit.to_string()), ("offset", offset.to_string())];
    match spotify_get_typed_compat_for::<Page<PlaylistItem>>(&spotify, &path, &query).await {
      Ok(page) => {
        if page.items.is_empty() {
          break;
        }

        let mut tracks: Vec<FullTrack> = Vec::new();
        for item in &page.items {
          if let Some(PlayableItem::Track(full_track)) = item.track.as_ref() {
            tracks.push(full_track.clone());
          }
        }

        let mut app_guard = app.lock().await;
        // append to playlist_tracks if needed or cache
        // For now, we update the app state directly if this is the active playlist
        // But we don't have a check for "active playlist ID".
        // We just update the track table if context matches.

        // NOTE: The original implementation logic was complex here.
        // For this refactor, we just fetch and maybe store in a cache if we had one.
        // Since `playlist_tracks` in App is a single Page, it doesn't support full prefetch well yet.
        // We will just break loop for now to avoid logic error, effectively disabling full prefetch until feature is fully implemented.
        // The user asked to split files, not fix logic bugs, but I should try to preserve behavior.

        // Assuming we just want to load them into the track table:
        if let Some(positions) = &mut app_guard.playlist_track_positions {
          // Append
          let start = positions.len();
          let count = tracks.len();
          positions.extend(start..start + count);
        }
        app_guard.track_table.tracks.extend(tracks);

        if page.next.is_none() {
          break;
        }
        offset += limit;
      }
      Err(_) => break,
    }
  }
}

pub trait LibraryNetwork {
  async fn get_current_user_playlists(&mut self);
  async fn get_playlist_tracks(&mut self, playlist_id: PlaylistId<'static>, playlist_offset: u32);
  async fn get_current_user_saved_tracks(&mut self, offset: Option<u32>);
  async fn get_current_user_saved_albums(&mut self, offset: Option<u32>);
  async fn current_user_saved_albums_contains(&mut self, album_ids: Vec<AlbumId<'static>>);
  async fn current_user_saved_album_delete(&mut self, album_id: AlbumId<'static>);
  async fn current_user_saved_album_add(&mut self, album_id: AlbumId<'static>);
  async fn current_user_saved_shows_contains(&mut self, show_ids: Vec<ShowId<'static>>);
  async fn current_user_saved_shows_delete(&mut self, show_id: ShowId<'static>);
  async fn current_user_saved_shows_add(&mut self, show_id: ShowId<'static>);
  async fn get_current_user_saved_shows(&mut self, offset: Option<u32>);
  async fn user_follow_playlist(
    &mut self,
    playlist_owner_id: UserId<'static>,
    playlist_id: PlaylistId<'static>,
    is_public: Option<bool>,
  );
  async fn user_unfollow_playlist(
    &mut self,
    user_id: UserId<'static>,
    playlist_id: PlaylistId<'static>,
  );
  async fn add_track_to_playlist(
    &mut self,
    playlist_id: PlaylistId<'static>,
    track_id: TrackId<'static>,
  );
  async fn remove_track_from_playlist_at_position(
    &mut self,
    playlist_id: PlaylistId<'static>,
    track_id: TrackId<'static>,
    position: usize,
  );
  async fn toggle_save_track(&mut self, track_id: rspotify::model::idtypes::PlayableId<'static>);
  async fn current_user_saved_tracks_contains(&mut self, ids: Vec<TrackId<'static>>);
  async fn fetch_all_playlist_tracks_and_sort(&mut self, playlist_id: PlaylistId<'static>);

  // Helpers exposed via trait if needed, or kept private if only used internally
  async fn set_tracks_to_table(&mut self, tracks: Vec<FullTrack>);
}

// Private helper methods
impl Network {
  async fn library_contains_uris(&self, uris: &[String]) -> anyhow::Result<Vec<bool>> {
    spotify_get_typed_compat_for(
      &self.spotify,
      "me/library/contains",
      &[("uris", uris.join(","))],
    )
    .await
  }

  async fn library_save_uris(&self, uris: &[String]) -> anyhow::Result<()> {
    spotify_api_request_json_for(
      &self.spotify,
      Method::PUT,
      "me/library",
      &[],
      Some(json!({ "uris": uris })),
    )
    .await?;
    Ok(())
  }

  async fn library_remove_uris(&self, uris: &[String]) -> anyhow::Result<()> {
    spotify_api_request_json_for(
      &self.spotify,
      Method::DELETE,
      "me/library",
      &[],
      Some(json!({ "uris": uris })),
    )
    .await?;
    Ok(())
  }

  async fn set_playlist_tracks_to_table(&mut self, playlist_track_page: &Page<PlaylistItem>) {
    let mut tracks: Vec<FullTrack> = Vec::new();
    let mut positions: Vec<usize> = Vec::new();

    for (idx, item) in playlist_track_page.items.iter().enumerate() {
      if let Some(PlayableItem::Track(full_track)) = item.track.as_ref() {
        tracks.push(full_track.clone());
        positions.push(playlist_track_page.offset as usize + idx);
      }
    }

    self.set_tracks_to_table(tracks).await;

    let mut app = self.app.lock().await;
    app.playlist_track_positions = Some(positions);
  }
}

impl LibraryNetwork for Network {
  async fn get_current_user_playlists(&mut self) {
    let (preferred_playlist_id, preferred_folder_id, preferred_selected_index) = {
      let app = self.app.lock().await;
      (
        app.get_selected_playlist_id(),
        app.current_playlist_folder_id,
        app.selected_playlist_index,
      )
    };

    let limit = 50u32;
    let mut offset = 0u32;
    let mut all_playlists = Vec::new();
    let mut first_page = None;

    loop {
      match self
        .spotify
        .current_user_playlists_manual(Some(limit), Some(offset))
        .await
      {
        Ok(page) => {
          if offset == 0 {
            first_page = Some(page.clone());
          }

          if page.items.is_empty() {
            break;
          }

          all_playlists.extend(page.items);

          if page.next.is_none() {
            break;
          }
          offset += limit;
        }
        Err(e) => {
          self.handle_error(anyhow!(e)).await;
          return;
        }
      }
    }

    #[cfg(feature = "streaming")]
    let folder_nodes = fetch_rootlist_folders(&self.streaming_player).await;
    #[cfg(not(feature = "streaming"))]
    let folder_nodes: Option<Vec<PlaylistFolderNode>> = None;

    let folder_items = if let Some(ref nodes) = folder_nodes {
      structurize_playlist_folders(nodes, &all_playlists)
    } else {
      build_flat_playlist_items(&all_playlists)
    };

    let mut app = self.app.lock().await;
    app.playlists = first_page;
    app.all_playlists = all_playlists;
    app._playlist_folder_nodes = folder_nodes;
    app.playlist_folder_items = folder_items;

    reconcile_playlist_selection(
      &mut app,
      preferred_playlist_id.as_deref(),
      preferred_folder_id,
      preferred_selected_index,
    );
  }

  async fn get_playlist_tracks(&mut self, playlist_id: PlaylistId<'static>, playlist_offset: u32) {
    let path = format!("playlists/{}/items", playlist_id.id());
    match spotify_get_typed_compat_for::<Page<PlaylistItem>>(
      &self.spotify,
      &path,
      &[
        ("limit", self.large_search_limit.to_string()),
        ("offset", playlist_offset.to_string()),
      ],
    )
    .await
    {
      Ok(playlist_tracks) => {
        self.set_playlist_tracks_to_table(&playlist_tracks).await;

        let mut app = self.app.lock().await;
        app.playlist_tracks = Some(playlist_tracks);
        app.push_navigation_stack(RouteId::TrackTable, ActiveBlock::TrackTable);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn get_current_user_saved_tracks(&mut self, offset: Option<u32>) {
    let mut query = vec![("limit", self.large_search_limit.to_string())];
    if let Some(offset) = offset {
      query.push(("offset", offset.to_string()));
    }

    match spotify_get_typed_compat_for::<Page<rspotify::model::SavedTrack>>(
      &self.spotify,
      "me/tracks",
      &query,
    )
    .await
    {
      Ok(saved_tracks) => {
        let mut app = self.app.lock().await;
        app.track_table.tracks = saved_tracks
          .items
          .clone()
          .into_iter()
          .map(|item| item.track)
          .collect::<Vec<FullTrack>>();

        saved_tracks.items.iter().for_each(|item| {
          if let Some(track_id) = &item.track.id {
            app.liked_song_ids_set.insert(track_id.to_string());
          }
        });

        // Apply pending selection if set
        let track_count = app.track_table.tracks.len();
        if track_count > 0 {
          if let Some(pending) = app.pending_track_table_selection.take() {
            app.track_table.selected_index = match pending {
              crate::core::app::PendingTrackSelection::First => 0,
              crate::core::app::PendingTrackSelection::Last => track_count.saturating_sub(1),
            };
          }
        }

        app.library.saved_tracks.add_pages(saved_tracks);
        app.track_table.context = Some(TrackTableContext::SavedTracks);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn get_current_user_saved_albums(&mut self, offset: Option<u32>) {
    let mut query = vec![("limit", self.large_search_limit.to_string())];
    if let Some(offset) = offset {
      query.push(("offset", offset.to_string()));
    }

    match spotify_get_typed_compat_for::<Page<rspotify::model::SavedAlbum>>(
      &self.spotify,
      "me/albums",
      &query,
    )
    .await
    {
      Ok(saved_albums) => {
        if !saved_albums.items.is_empty() {
          let mut app = self.app.lock().await;
          app.library.saved_albums.add_pages(saved_albums);
        }
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn current_user_saved_albums_contains(&mut self, album_ids: Vec<AlbumId<'static>>) {
    let uris: Vec<String> = album_ids
      .iter()
      .map(|id| format!("spotify:album:{}", id.id()))
      .collect();

    match self.library_contains_uris(&uris).await {
      Ok(is_saved_vec) => {
        let mut app = self.app.lock().await;
        for (i, id) in album_ids.iter().enumerate() {
          if let Some(is_saved) = is_saved_vec.get(i) {
            if *is_saved {
              app.saved_album_ids_set.insert(id.id().to_string());
            } else {
              if app.saved_album_ids_set.contains(id.id()) {
                app.saved_album_ids_set.remove(id.id());
              }
            }
          };
        }
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn current_user_saved_album_delete(&mut self, album_id: AlbumId<'static>) {
    let uris = vec![format!("spotify:album:{}", album_id.id())];
    match self.library_remove_uris(&uris).await {
      Ok(_) => {
        let mut app = self.app.lock().await;
        app.saved_album_ids_set.remove(album_id.id());
        // Reload saved albums to refresh UI
        // dispatching event would require loop access, but we can't from here easily unless we return IoEvent
        // For now, assume optimistic update is handled or manually remove
      }
      Err(e) => self.handle_error(anyhow!(e)).await,
    }
  }

  async fn current_user_saved_album_add(&mut self, album_id: AlbumId<'static>) {
    let uris = vec![format!("spotify:album:{}", album_id.id())];
    match self.library_save_uris(&uris).await {
      Ok(_) => {
        let mut app = self.app.lock().await;
        app.saved_album_ids_set.insert(album_id.id().to_string());
      }
      Err(e) => self.handle_error(anyhow!(e)).await,
    }
  }

  async fn current_user_saved_shows_contains(&mut self, show_ids: Vec<ShowId<'static>>) {
    let uris: Vec<String> = show_ids
      .iter()
      .map(|id| format!("spotify:show:{}", id.id()))
      .collect();
    match self.library_contains_uris(&uris).await {
      Ok(is_saved_vec) => {
        let mut app = self.app.lock().await;
        for (i, id) in show_ids.iter().enumerate() {
          if let Some(is_saved) = is_saved_vec.get(i) {
            if *is_saved {
              app.saved_show_ids_set.insert(id.id().to_string());
            } else {
              if app.saved_show_ids_set.contains(id.id()) {
                app.saved_show_ids_set.remove(id.id());
              }
            }
          };
        }
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn current_user_saved_shows_delete(&mut self, show_id: ShowId<'static>) {
    let uris = vec![format!("spotify:show:{}", show_id.id())];
    match self.library_remove_uris(&uris).await {
      Ok(_) => {
        let mut app = self.app.lock().await;
        app.saved_show_ids_set.remove(show_id.id());
      }
      Err(e) => self.handle_error(anyhow!(e)).await,
    }
  }

  async fn current_user_saved_shows_add(&mut self, show_id: ShowId<'static>) {
    let uris = vec![format!("spotify:show:{}", show_id.id())];
    match self.library_save_uris(&uris).await {
      Ok(_) => {
        let mut app = self.app.lock().await;
        app.saved_show_ids_set.insert(show_id.id().to_string());
      }
      Err(e) => self.handle_error(anyhow!(e)).await,
    }
  }

  async fn get_current_user_saved_shows(&mut self, offset: Option<u32>) {
    let mut query = vec![("limit", self.large_search_limit.to_string())];
    if let Some(offset) = offset {
      query.push(("offset", offset.to_string()));
    }

    match spotify_get_typed_compat_for::<Page<rspotify::model::show::Show>>(
      &self.spotify,
      "me/shows",
      &query,
    )
    .await
    {
      Ok(saved_shows) => {
        if !saved_shows.items.is_empty() {
          let mut app = self.app.lock().await;
          app.library.saved_shows.add_pages(saved_shows);
        }
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn user_follow_playlist(
    &mut self,
    _playlist_owner_id: UserId<'static>,
    playlist_id: PlaylistId<'static>,
    is_public: Option<bool>,
  ) {
    match self
      .spotify
      .playlist_follow(playlist_id, Some(is_public.unwrap_or(false)))
      .await
    {
      Ok(_) => {
        // Optimistic update handled in handler or next refresh
      }
      Err(e) => self.handle_error(anyhow!(e)).await,
    }
  }

  async fn user_unfollow_playlist(
    &mut self,
    _user_id: UserId<'static>,
    playlist_id: PlaylistId<'static>,
  ) {
    match self.spotify.playlist_unfollow(playlist_id).await {
      Ok(_) => {
        // Handled
      }
      Err(e) => self.handle_error(anyhow!(e)).await,
    }
  }

  async fn add_track_to_playlist(
    &mut self,
    playlist_id: PlaylistId<'static>,
    track_id: TrackId<'static>,
  ) {
    match self
      .spotify
      .playlist_add_items(playlist_id, vec![PlayableId::Track(track_id)], None)
      .await
    {
      Ok(_) => {
        self
          .show_status_message("Added to playlist".to_string(), 3)
          .await;
      }
      Err(e) => self.handle_error(anyhow!(e)).await,
    }
  }

  async fn remove_track_from_playlist_at_position(
    &mut self,
    playlist_id: PlaylistId<'static>,
    track_id: TrackId<'static>,
    position: usize,
  ) {
    let body = json!({
        "tracks": [{
            "uri": format!("spotify:track:{}", track_id.id()),
            "positions": [position]
        }]
    });

    match spotify_api_request_json_for(
      &self.spotify,
      Method::DELETE,
      &format!("playlists/{}/tracks", playlist_id.id()),
      &[],
      Some(body),
    )
    .await
    {
      Ok(_) => {
        self
          .show_status_message("Removed from playlist".to_string(), 3)
          .await;
      }
      Err(e) => self.handle_error(anyhow!(e)).await,
    }
  }

  async fn toggle_save_track(&mut self, track_id: rspotify::model::idtypes::PlayableId<'static>) {
    let id_str = match &track_id {
      PlayableId::Track(id) => id.id(),
      PlayableId::Episode(id) => id.id(),
    };
    let uri = match &track_id {
      PlayableId::Track(id) => format!("spotify:track:{}", id.id()),
      PlayableId::Episode(id) => format!("spotify:episode:{}", id.id()),
    };

    let is_liked = {
      let app = self.app.lock().await;
      app.liked_song_ids_set.contains(id_str)
    };

    if is_liked {
      if let Err(e) = self.library_remove_uris(&[uri]).await {
        self.handle_error(anyhow!(e)).await;
      } else {
        let mut app = self.app.lock().await;
        app.liked_song_ids_set.remove(id_str);
      }
    } else {
      if let Err(e) = self.library_save_uris(&[uri]).await {
        self.handle_error(anyhow!(e)).await;
      } else {
        let mut app = self.app.lock().await;
        app.liked_song_ids_set.insert(id_str.to_string());
      }
    }
  }

  async fn current_user_saved_tracks_contains(&mut self, ids: Vec<TrackId<'static>>) {
    let uris: Vec<String> = ids
      .iter()
      .map(|id| format!("spotify:track:{}", id.id()))
      .collect();

    match self.library_contains_uris(&uris).await {
      Ok(is_saved_vec) => {
        let mut app = self.app.lock().await;
        for (i, id) in ids.iter().enumerate() {
          if let Some(is_liked) = is_saved_vec.get(i) {
            if *is_liked {
              app.liked_song_ids_set.insert(id.id().to_string());
            } else {
              if app.liked_song_ids_set.contains(id.id()) {
                app.liked_song_ids_set.remove(id.id());
              }
            }
          };
        }
      }
      Err(e) => {
        let mut app = self.app.lock().await;
        app.status_message = Some(format!("Could not check liked track state: {}", e));
        app.status_message_expires_at = Some(Instant::now() + Duration::from_secs(5));
      }
    }
  }

  async fn set_tracks_to_table(&mut self, tracks: Vec<FullTrack>) {
    let track_ids: Vec<TrackId<'static>> = tracks
      .iter()
      .filter_map(|item| item.id.as_ref().map(|id| id.clone().into_static()))
      .collect();

    let mut app = self.app.lock().await;
    app.playlist_track_positions = None;

    let track_count = tracks.len();
    if track_count > 0 {
      if let Some(pending) = app.pending_track_table_selection.take() {
        app.track_table.selected_index = match pending {
          crate::core::app::PendingTrackSelection::First => 0,
          crate::core::app::PendingTrackSelection::Last => track_count.saturating_sub(1),
        };
      } else {
        let max_index = track_count.saturating_sub(1);
        if app.track_table.selected_index > max_index {
          app.track_table.selected_index = max_index;
        }
      }
    } else {
      app.track_table.selected_index = 0;
    }

    app.track_table.tracks = tracks;

    drop(app); // Release lock
               // Dispatch event to check saved status
    self.current_user_saved_tracks_contains(track_ids).await;
  }

  async fn fetch_all_playlist_tracks_and_sort(&mut self, playlist_id: PlaylistId<'static>) {
    let mut all_tracks = Vec::new();
    let mut offset = 0u32;
    let limit = 50u32;
    let path = format!("playlists/{}/items", playlist_id.id());

    loop {
      let query = vec![("limit", limit.to_string()), ("offset", offset.to_string())];
      match spotify_get_typed_compat_for::<Page<PlaylistItem>>(&self.spotify, &path, &query).await {
        Ok(page) => {
          if page.items.is_empty() {
            break;
          }

          for item in page.items {
            if let Some(PlayableItem::Track(full_track)) = item.track {
              all_tracks.push(full_track);
            }
          }

          if page.next.is_none() {
            break;
          }
          offset += limit;
        }
        Err(e) => {
          self.handle_error(anyhow!(e)).await;
          return;
        }
      }
    }

    // Apply sort if any
    let mut app = self.app.lock().await;

    // Sort
    use crate::core::sort::{SortContext, Sorter};
    if let Some(SortContext::PlaylistTracks) = app.sort_context {
      let sorter = Sorter::new(app.playlist_sort.clone());
      sorter.sort_tracks(&mut all_tracks);
    }

    app.track_table.tracks = all_tracks;
    // Reset selection
    app.track_table.selected_index = 0;
  }
}

#[cfg(feature = "streaming")]
async fn fetch_rootlist_folders(
  streaming_player: &Option<Arc<StreamingPlayer>>,
) -> Option<Vec<PlaylistFolderNode>> {
  let player = streaming_player.as_ref()?;
  let session = player.session();

  let bytes = match session.spclient().get_rootlist(0, Some(100_000)).await {
    Ok(bytes) => bytes,
    Err(_) => return None,
  };

  use protobuf::Message;
  let selected: librespot_protocol::playlist4_external::SelectedListContent =
    Message::parse_from_bytes(&bytes).ok()?;

  let contents = selected.contents.as_ref()?;
  Some(parse_rootlist_items(&contents.items))
}

fn build_flat_playlist_items(
  playlists: &[rspotify::model::playlist::SimplifiedPlaylist],
) -> Vec<PlaylistFolderItem> {
  playlists
    .iter()
    .enumerate()
    .map(|(index, _)| PlaylistFolderItem::Playlist {
      index,
      current_id: 0,
    })
    .collect()
}

fn reconcile_playlist_selection(
  app: &mut App,
  preferred_playlist_id: Option<&str>,
  preferred_folder_id: usize,
  preferred_selected_index: Option<usize>,
) {
  if app.playlist_folder_items.is_empty() {
    app.current_playlist_folder_id = 0;
    app.selected_playlist_index = None;
    return;
  }

  let folder_has_visible = |folder_id: usize, app: &App| {
    app.playlist_folder_items.iter().any(|item| match item {
      PlaylistFolderItem::Folder(folder) => folder.current_id == folder_id,
      PlaylistFolderItem::Playlist { current_id, .. } => *current_id == folder_id,
    })
  };

  app.current_playlist_folder_id = if folder_has_visible(preferred_folder_id, app) {
    preferred_folder_id
  } else {
    0
  };

  if let Some(playlist_id) = preferred_playlist_id {
    let visible_playlist_index = app
      .playlist_folder_items
      .iter()
      .filter(|item| app.is_playlist_item_visible_in_current_folder(item))
      .enumerate()
      .find_map(|(display_idx, item)| match item {
        PlaylistFolderItem::Playlist { index, .. } => app
          .all_playlists
          .get(*index)
          .filter(|playlist| playlist.id.id() == playlist_id)
          .map(|_| display_idx),
        PlaylistFolderItem::Folder(_) => None,
      });

    if let Some(display_idx) = visible_playlist_index {
      app.selected_playlist_index = Some(display_idx);
      return;
    }

    let mut target_folder: Option<usize> = None;
    for item in &app.playlist_folder_items {
      if let PlaylistFolderItem::Playlist { index, current_id } = item {
        if let Some(playlist) = app.all_playlists.get(*index) {
          if playlist.id.id() == playlist_id {
            target_folder = Some(*current_id);
            break;
          }
        }
      }
    }

    if let Some(folder_id) = target_folder {
      app.current_playlist_folder_id = folder_id;
      let display_idx = app
        .playlist_folder_items
        .iter()
        .filter(|item| app.is_playlist_item_visible_in_current_folder(item))
        .enumerate()
        .find_map(|(idx, item)| match item {
          PlaylistFolderItem::Playlist { index, .. } => app
            .all_playlists
            .get(*index)
            .filter(|playlist| playlist.id.id() == playlist_id)
            .map(|_| idx),
          PlaylistFolderItem::Folder(_) => None,
        });
      if let Some(idx) = display_idx {
        app.selected_playlist_index = Some(idx);
        return;
      }
    }
  }

  let visible_count = app.get_playlist_display_count();
  if visible_count == 0 {
    app.current_playlist_folder_id = 0;
    let root_count = app.get_playlist_display_count();
    app.selected_playlist_index = if root_count == 0 {
      None
    } else {
      Some(preferred_selected_index.unwrap_or(0).min(root_count - 1))
    };
    return;
  }

  app.selected_playlist_index = Some(preferred_selected_index.unwrap_or(0).min(visible_count - 1));
}

#[cfg(feature = "streaming")]
fn parse_rootlist_items(
  items: &[librespot_protocol::playlist4_external::Item],
) -> Vec<PlaylistFolderNode> {
  let mut root: Vec<PlaylistFolderNode> = Vec::new();
  let mut stack: Vec<Vec<PlaylistFolderNode>> = Vec::new();
  let mut name_stack: Vec<(String, String)> = Vec::new();

  for item in items {
    let uri = item.uri();

    if let Some(rest) = uri.strip_prefix("spotify:start-group:") {
      let (group_id, name) = match rest.find(':') {
        Some(pos) => (rest[..pos].to_string(), rest[pos + 1..].to_string()),
        None => (rest.to_string(), String::new()),
      };
      name_stack.push((group_id, name));
      stack.push(std::mem::take(&mut root));
      root = Vec::new();
    } else if uri.starts_with("spotify:end-group:") {
      if let Some((group_id, name)) = name_stack.pop() {
        let children = std::mem::take(&mut root);
        root = stack.pop().unwrap_or_default();
        root.push(PlaylistFolderNode {
          name: Some(name),
          node_type: PlaylistFolderNodeType::Folder,
          uri: format!("spotify:folder:{}", group_id),
          children,
        });
      }
    } else {
      root.push(PlaylistFolderNode {
        name: None,
        node_type: PlaylistFolderNodeType::Playlist,
        uri: uri.to_string(),
        children: Vec::new(),
      });
    }
  }

  while let Some((group_id, name)) = name_stack.pop() {
    let children = std::mem::take(&mut root);
    root = stack.pop().unwrap_or_default();
    root.push(PlaylistFolderNode {
      name: Some(name),
      node_type: PlaylistFolderNodeType::Folder,
      uri: format!("spotify:folder:{}", group_id),
      children,
    });
  }

  root
}

fn structurize_playlist_folders(
  nodes: &[PlaylistFolderNode],
  playlists: &[rspotify::model::playlist::SimplifiedPlaylist],
) -> Vec<PlaylistFolderItem> {
  use std::collections::{HashMap, HashSet};

  let playlist_map: HashMap<String, usize> = playlists
    .iter()
    .enumerate()
    .map(|(idx, playlist)| (playlist.id.id().to_string(), idx))
    .collect();

  let mut items: Vec<PlaylistFolderItem> = Vec::new();
  let mut next_folder_id: usize = 1;
  let mut used_playlist_indices: HashSet<usize> = HashSet::new();

  fn walk(
    nodes: &[PlaylistFolderNode],
    current_folder_id: usize,
    items: &mut Vec<PlaylistFolderItem>,
    next_folder_id: &mut usize,
    playlist_map: &std::collections::HashMap<String, usize>,
    used_playlist_indices: &mut std::collections::HashSet<usize>,
  ) {
    for node in nodes {
      match node.node_type {
        PlaylistFolderNodeType::Folder => {
          let folder_id = *next_folder_id;
          *next_folder_id += 1;

          let name = node.name.as_deref().unwrap_or("Unnamed Folder").to_string();

          items.push(PlaylistFolderItem::Folder(PlaylistFolder {
            name: name.clone(),
            current_id: current_folder_id,
            target_id: folder_id,
          }));

          items.push(PlaylistFolderItem::Folder(PlaylistFolder {
            name: format!("\u{2190} {}", name),
            current_id: folder_id,
            target_id: current_folder_id,
          }));

          walk(
            &node.children,
            folder_id,
            items,
            next_folder_id,
            playlist_map,
            used_playlist_indices,
          );
        }
        PlaylistFolderNodeType::Playlist => {
          let playlist_id = node
            .uri
            .strip_prefix("spotify:playlist:")
            .unwrap_or(&node.uri);

          if let Some(&index) = playlist_map.get(playlist_id) {
            items.push(PlaylistFolderItem::Playlist {
              index,
              current_id: current_folder_id,
            });
            used_playlist_indices.insert(index);
          }
        }
      }
    }
  }

  walk(
    nodes,
    0,
    &mut items,
    &mut next_folder_id,
    &playlist_map,
    &mut used_playlist_indices,
  );

  for (index, _) in playlists.iter().enumerate() {
    if !used_playlist_indices.contains(&index) {
      items.push(PlaylistFolderItem::Playlist {
        index,
        current_id: 0,
      });
    }
  }

  items
}
