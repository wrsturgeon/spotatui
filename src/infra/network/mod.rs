pub mod library;
pub mod metadata;
pub mod playback;
pub mod recommend;
pub mod requests;
pub mod search;
pub mod user;
pub mod utils;

use crate::core::app::App;
use crate::core::config::ClientConfig;
use anyhow::anyhow;
use rspotify::clients::BaseClient;
use rspotify::model::{
  album::SimplifiedAlbum,
  artist::FullArtist,
  enums::{Country, RepeatState},
  idtypes::{AlbumId, ArtistId, PlayContextId, PlayableId, PlaylistId, ShowId, TrackId, UserId},
  show::SimplifiedShow,
  track::FullTrack,
};
use rspotify::AuthCodePkceSpotify;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[cfg(feature = "streaming")]
use crate::infra::player::StreamingPlayer;

// Re-export traits
use self::library::LibraryNetwork;
use self::metadata::MetadataNetwork;
use self::playback::PlaybackNetwork;
use self::recommend::RecommendationNetwork;
use self::search::SearchNetwork;
use self::user::UserNetwork;
use self::utils::UtilsNetwork;

pub enum IoEvent {
  GetCurrentPlayback,
  /// After a track transition (e.g., EndOfTrack), ensure we don't end up paused on the next item.
  /// The payload is the previous track identifier (either base62 id or a `spotify:track:` URI).
  #[allow(dead_code)]
  EnsurePlaybackContinues(String),
  RefreshAuthentication,
  GetPlaylists,
  GetDevices,
  GetSearchResults(String, Option<Country>),
  SetTracksToTable(Vec<FullTrack>),
  GetPlaylistItems(PlaylistId<'static>, u32),
  GetCurrentSavedTracks(Option<u32>),
  StartPlayback(
    Option<PlayContextId<'static>>,
    Option<Vec<PlayableId<'static>>>,
    Option<usize>,
  ),
  UpdateSearchLimits(u32, u32),
  Seek(u32),
  NextTrack,
  PreviousTrack,
  Shuffle(bool), // desired shuffle state
  Repeat(RepeatState),
  PausePlayback,
  ChangeVolume(u8),
  GetArtist(ArtistId<'static>, String, Option<Country>),
  GetAlbumTracks(Box<SimplifiedAlbum>),
  GetRecommendationsForSeed(
    Option<Vec<ArtistId<'static>>>,
    Option<Vec<TrackId<'static>>>,
    Box<Option<FullTrack>>,
    Option<Country>,
  ),
  GetCurrentUserSavedAlbums(Option<u32>),
  CurrentUserSavedAlbumsContains(Vec<AlbumId<'static>>),
  CurrentUserSavedAlbumDelete(AlbumId<'static>),
  CurrentUserSavedAlbumAdd(AlbumId<'static>),
  UserUnfollowArtists(Vec<ArtistId<'static>>),
  UserFollowArtists(Vec<ArtistId<'static>>),
  UserFollowPlaylist(UserId<'static>, PlaylistId<'static>, Option<bool>),
  UserUnfollowPlaylist(UserId<'static>, PlaylistId<'static>),
  AddTrackToPlaylist(PlaylistId<'static>, TrackId<'static>),
  RemoveTrackFromPlaylistAtPosition(PlaylistId<'static>, TrackId<'static>, usize),
  GetUser,
  ToggleSaveTrack(PlayableId<'static>),
  GetRecommendationsForTrackId(TrackId<'static>, Option<Country>),
  GetRecentlyPlayed,
  GetFollowedArtists(Option<ArtistId<'static>>),
  SetArtistsToTable(Vec<FullArtist>),
  UserArtistFollowCheck(Vec<ArtistId<'static>>),
  GetAlbum(AlbumId<'static>),
  TransferPlaybackToDevice(String, bool),
  #[allow(dead_code)]
  AutoSelectStreamingDevice(String, bool), // Auto-select a device by name (used for native streaming)
  GetAlbumForTrack(TrackId<'static>),
  CurrentUserSavedTracksContains(Vec<TrackId<'static>>),
  GetCurrentUserSavedShows(Option<u32>),
  CurrentUserSavedShowsContains(Vec<ShowId<'static>>),
  CurrentUserSavedShowDelete(ShowId<'static>),
  CurrentUserSavedShowAdd(ShowId<'static>),
  GetShowEpisodes(Box<SimplifiedShow>),
  GetShow(ShowId<'static>),
  GetCurrentShowEpisodes(ShowId<'static>, Option<u32>),
  AddItemToQueue(PlayableId<'static>),
  IncrementGlobalSongCount,
  FetchGlobalSongCount,
  FetchAnnouncements,
  GetLyrics(String, String, f64),
  /// Start playback from the user's saved tracks collection (Liked Songs)
  /// Takes the absolute position in the collection to start from
  /// NOTE: Currently unused - Spotify Web API doesn't support collection context URI
  /// Keeping for potential future use if Spotify adds support
  #[allow(dead_code)]
  StartCollectionPlayback(usize),
  /// Pre-fetch all saved tracks pages in background for seamless playback
  PreFetchAllSavedTracks,
  /// Pre-fetch all tracks from a playlist in background
  PreFetchAllPlaylistTracks(PlaylistId<'static>),
  /// Get user's top tracks for Discover feature (with time range)
  GetUserTopTracks(crate::core::app::DiscoverTimeRange),
  /// Get Top Artists Mix - fetches top artists and their top tracks
  GetTopArtistsMix,
  /// Fetch all playlist tracks and apply sorting
  FetchAllPlaylistTracksAndSort(PlaylistId<'static>),
}

pub struct Network {
  pub spotify: AuthCodePkceSpotify,
  pub large_search_limit: u32,
  pub small_search_limit: u32,
  pub client_config: ClientConfig,
  pub app: Arc<Mutex<App>>,
  #[cfg(feature = "streaming")]
  pub streaming_player: Option<Arc<StreamingPlayer>>,
}

impl Network {
  #[cfg(feature = "streaming")]
  pub fn new(
    spotify: AuthCodePkceSpotify,
    client_config: ClientConfig,
    app: &Arc<Mutex<App>>,
    streaming_player: Option<Arc<StreamingPlayer>>,
  ) -> Self {
    Network {
      spotify,
      large_search_limit: 50,
      small_search_limit: 4,
      client_config,
      app: Arc::clone(app),
      streaming_player,
    }
  }

  #[cfg(not(feature = "streaming"))]
  pub fn new(
    spotify: AuthCodePkceSpotify,
    client_config: ClientConfig,
    app: &Arc<Mutex<App>>,
  ) -> Self {
    Network {
      spotify,
      large_search_limit: 50,
      small_search_limit: 4,
      client_config,
      app: Arc::clone(app),
    }
  }

  #[allow(clippy::cognitive_complexity)]
  pub async fn handle_network_event(&mut self, io_event: IoEvent) {
    match io_event {
      IoEvent::RefreshAuthentication => {
        self.refresh_authentication().await;
      }
      IoEvent::EnsurePlaybackContinues(previous_track_id) => {
        self.ensure_playback_continues(previous_track_id).await;
      }
      IoEvent::GetPlaylists => {
        self.get_current_user_playlists().await;
      }
      IoEvent::GetUser => {
        self.get_user().await;
      }
      IoEvent::GetDevices => {
        self.get_devices().await;
      }
      IoEvent::GetCurrentPlayback => {
        self.get_current_playback().await;
      }
      IoEvent::SetTracksToTable(full_tracks) => {
        self.set_tracks_to_table(full_tracks).await;
      }
      IoEvent::GetSearchResults(search_term, country) => {
        self.get_search_results(search_term, country).await;
      }

      IoEvent::GetPlaylistItems(playlist_id, playlist_offset) => {
        self.get_playlist_tracks(playlist_id, playlist_offset).await;
      }
      IoEvent::GetCurrentSavedTracks(offset) => {
        self.get_current_user_saved_tracks(offset).await;
      }
      IoEvent::StartPlayback(context_uri, uris, offset) => {
        self.start_playback(context_uri, uris, offset).await;
      }
      IoEvent::UpdateSearchLimits(large_search_limit, small_search_limit) => {
        self.large_search_limit = large_search_limit;
        self.small_search_limit = small_search_limit;
      }
      IoEvent::Seek(position_ms) => {
        self.seek(position_ms).await;
      }
      IoEvent::NextTrack => {
        self.next_track().await;
      }
      IoEvent::PreviousTrack => {
        self.previous_track().await;
      }
      IoEvent::Repeat(repeat_state) => {
        self.repeat(repeat_state).await;
      }
      IoEvent::PausePlayback => {
        self.pause_playback().await;
      }
      IoEvent::ChangeVolume(volume) => {
        self.change_volume(volume).await;
      }
      IoEvent::GetArtist(artist_id, input_artist_name, country) => {
        self.get_artist(artist_id, input_artist_name, country).await;
      }
      IoEvent::GetAlbumTracks(album) => {
        self.get_album_tracks(album).await;
      }
      IoEvent::GetRecommendationsForSeed(seed_artists, seed_tracks, first_track, country) => {
        self
          .get_recommendations_for_seed(seed_artists, seed_tracks, first_track, country)
          .await;
      }
      IoEvent::GetCurrentUserSavedAlbums(offset) => {
        self.get_current_user_saved_albums(offset).await;
      }
      IoEvent::CurrentUserSavedAlbumsContains(album_ids) => {
        self.current_user_saved_albums_contains(album_ids).await;
      }
      IoEvent::CurrentUserSavedAlbumDelete(album_id) => {
        self.current_user_saved_album_delete(album_id).await;
      }
      IoEvent::CurrentUserSavedAlbumAdd(album_id) => {
        self.current_user_saved_album_add(album_id).await;
      }
      IoEvent::UserUnfollowArtists(artist_ids) => {
        self.user_unfollow_artists(artist_ids).await;
      }
      IoEvent::UserFollowArtists(artist_ids) => {
        self.user_follow_artists(artist_ids).await;
      }
      IoEvent::UserFollowPlaylist(playlist_owner_id, playlist_id, is_public) => {
        self
          .user_follow_playlist(playlist_owner_id, playlist_id, is_public)
          .await;
      }
      IoEvent::UserUnfollowPlaylist(user_id, playlist_id) => {
        self.user_unfollow_playlist(user_id, playlist_id).await;
      }
      IoEvent::AddTrackToPlaylist(playlist_id, track_id) => {
        self.add_track_to_playlist(playlist_id, track_id).await;
      }
      IoEvent::RemoveTrackFromPlaylistAtPosition(playlist_id, track_id, position) => {
        self
          .remove_track_from_playlist_at_position(playlist_id, track_id, position)
          .await;
      }

      IoEvent::ToggleSaveTrack(track_id) => {
        self.toggle_save_track(track_id).await;
      }
      IoEvent::GetRecommendationsForTrackId(track_id, country) => {
        self
          .get_recommendations_for_track_id(track_id, country)
          .await;
      }
      IoEvent::GetRecentlyPlayed => {
        self.get_recently_played().await;
      }
      IoEvent::GetFollowedArtists(after) => {
        self.get_followed_artists(after).await;
      }
      IoEvent::SetArtistsToTable(full_artists) => {
        self.set_artists_to_table(full_artists).await;
      }
      IoEvent::UserArtistFollowCheck(artist_ids) => {
        self.user_artist_check_follow(artist_ids).await;
      }
      IoEvent::GetAlbum(album_id) => {
        self.get_album(album_id).await;
      }
      IoEvent::TransferPlaybackToDevice(device_id, persist_device_id) => {
        self
          .transfert_playback_to_device(device_id, persist_device_id)
          .await;
      }
      #[cfg(feature = "streaming")]
      IoEvent::AutoSelectStreamingDevice(device_name, persist_device_id) => {
        self
          .auto_select_streaming_device(device_name, persist_device_id)
          .await;
      }
      #[cfg(not(feature = "streaming"))]
      IoEvent::AutoSelectStreamingDevice(..) => {} // No-op without native streaming
      IoEvent::GetAlbumForTrack(track_id) => {
        self.get_album_for_track(track_id).await;
      }
      IoEvent::Shuffle(shuffle_state) => {
        self.shuffle(shuffle_state).await;
      }
      IoEvent::CurrentUserSavedTracksContains(track_ids) => {
        self.current_user_saved_tracks_contains(track_ids).await;
      }
      IoEvent::GetCurrentUserSavedShows(offset) => {
        self.get_current_user_saved_shows(offset).await;
      }
      IoEvent::CurrentUserSavedShowsContains(show_ids) => {
        self.current_user_saved_shows_contains(show_ids).await;
      }
      IoEvent::CurrentUserSavedShowDelete(show_id) => {
        self.current_user_saved_shows_delete(show_id).await;
      }
      IoEvent::CurrentUserSavedShowAdd(show_id) => {
        self.current_user_saved_shows_add(show_id).await;
      }
      IoEvent::GetShowEpisodes(show) => {
        self.get_show_episodes(show).await;
      }
      IoEvent::GetShow(show_id) => {
        self.get_show(show_id).await;
      }
      IoEvent::GetCurrentShowEpisodes(show_id, offset) => {
        self.get_current_show_episodes(show_id, offset).await;
      }
      IoEvent::AddItemToQueue(item) => {
        self.add_item_to_queue(item).await;
      }
      IoEvent::IncrementGlobalSongCount => {
        self.increment_global_song_count().await;
      }
      IoEvent::FetchGlobalSongCount => {
        self.fetch_global_song_count().await;
      }
      IoEvent::FetchAnnouncements => {
        self.fetch_announcements().await;
      }
      IoEvent::GetLyrics(track, artist, duration) => {
        self.get_lyrics(track, artist, duration).await;
      }
      IoEvent::StartCollectionPlayback(offset) => {
        self.start_collection_playback(offset).await;
      }
      IoEvent::PreFetchAllSavedTracks => {
        // Spawn prefetch as a separate task to avoid blocking playback
        let spotify = self.spotify.clone();
        let app = self.app.clone();
        let large_search_limit = self.large_search_limit;
        tokio::spawn(async move {
          library::prefetch_all_saved_tracks_task(spotify, app, large_search_limit).await;
        });
      }
      IoEvent::PreFetchAllPlaylistTracks(playlist_id) => {
        // Spawn prefetch as a separate task to avoid blocking playback
        let spotify = self.spotify.clone();
        let app = self.app.clone();
        let large_search_limit = self.large_search_limit;
        tokio::spawn(async move {
          library::prefetch_all_playlist_tracks_task(spotify, app, large_search_limit, playlist_id)
            .await;
        });
      }
      IoEvent::GetUserTopTracks(time_range) => {
        self.get_user_top_tracks(time_range).await;
      }
      IoEvent::GetTopArtistsMix => {
        self.get_top_artists_mix().await;
      }
      IoEvent::FetchAllPlaylistTracksAndSort(playlist_id) => {
        self.fetch_all_playlist_tracks_and_sort(playlist_id).await;
      }
    };

    {
      let mut app = self.app.lock().await;
      app.is_loading = false;
    }
  }

  async fn handle_error(&mut self, e: anyhow::Error) {
    let mut app = self.app.lock().await;
    app.handle_error(e);
  }

  async fn show_status_message(&self, message: String, ttl_secs: u64) {
    let mut app = self.app.lock().await;
    app.status_message = Some(message);
    app.status_message_expires_at = Some(Instant::now() + Duration::from_secs(ttl_secs));
  }

  async fn refresh_authentication(&mut self) {
    // Refresh token if needed
    // This is implicitly handled by spotify_api_request_json_for which checks 401
    // But sometimes we might want to force refresh or check validity.
    // The original code called self.spotify.refresh_token().await.
    if let Err(e) = self.spotify.refetch_token().await {
      self.handle_error(anyhow!(e)).await;
    }
  }
}
