use crate::core::app::App;
use crate::infra::network::IoEvent;
use crate::tui::event::Key;
use rspotify::model::{context::CurrentPlaybackContext, PlayableId, PlayableItem};

pub fn handler(key: Key, app: &mut App) {
  if let Key::Char('s') = key {
    if let Some(CurrentPlaybackContext {
      item: Some(item), ..
    }) = app.current_playback_context.to_owned()
    {
      match item {
        PlayableItem::Track(track) => {
          if let Some(track_id) = track.id {
            app.dispatch(IoEvent::ToggleSaveTrack(PlayableId::Track(
              track_id.into_static(),
            )));
          }
        }
        PlayableItem::Episode(episode) => {
          app.dispatch(IoEvent::ToggleSaveTrack(PlayableId::Episode(
            episode.id.into_static(),
          )));
        }
      };
    };
  }
}
