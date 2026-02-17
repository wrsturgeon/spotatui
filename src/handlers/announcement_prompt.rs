use super::super::app::App;
use crate::event::Key;

pub fn handler(key: Key, app: &mut App) {
  match key {
    Key::Enter | Key::Esc | Key::Char('q') | Key::Char(' ') => {
      if let Some(dismissed_id) = app.dismiss_active_announcement() {
        app.user_config.mark_announcement_seen(dismissed_id);
        if let Err(error) = app.user_config.save_config() {
          app.handle_error(anyhow::anyhow!(
            "Failed to persist dismissed announcement: {}",
            error
          ));
        }
      }

      if app.active_announcement.is_none() {
        app.pop_navigation_stack();
      }
    }
    _ => {}
  }
}
