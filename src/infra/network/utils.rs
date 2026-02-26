use super::Network;
use crate::core::app::{Announcement, AnnouncementLevel, LyricsStatus};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::time::{Duration, Instant};

#[derive(Deserialize, Debug)]
#[allow(non_snake_case)]
struct LrcResponse {
  syncedLyrics: Option<String>,
  plainLyrics: Option<String>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct GlobalSongCountResponse {
  count: u64,
}

#[derive(Deserialize, Debug)]
struct AnnouncementFeedResponse {
  #[allow(dead_code)]
  version: Option<u8>,
  #[serde(default)]
  announcements: Vec<AnnouncementRecord>,
}

#[derive(Deserialize, Debug)]
struct AnnouncementRecord {
  id: String,
  title: Option<String>,
  body: String,
  level: Option<String>,
  url: Option<String>,
  starts_at: Option<String>,
  ends_at: Option<String>,
}

pub trait UtilsNetwork {
  async fn get_lyrics(&mut self, track: String, artist: String, duration: f64);
  async fn increment_global_song_count(&mut self);
  async fn fetch_global_song_count(&mut self);
  async fn fetch_announcements(&mut self);
}

impl UtilsNetwork for Network {
  async fn get_lyrics(&mut self, track: String, artist: String, duration: f64) {
    let client = reqwest::Client::new();
    let query = vec![
      ("track_name", track.clone()),
      ("artist_name", artist.clone()),
      ("duration", duration.to_string()),
    ];

    // Update state to loading
    {
      let mut app = self.app.lock().await;
      app.lyrics_status = LyricsStatus::Loading;
      app.lyrics = None;
    }

    match client
      .get("https://lrclib.net/api/get")
      .query(&query)
      .send()
      .await
    {
      Ok(resp) => {
        if resp.status().is_success() {
          if let Ok(lrc_resp) = resp.json::<LrcResponse>().await {
            let lyrics_text = lrc_resp
              .syncedLyrics
              .or(lrc_resp.plainLyrics)
              .unwrap_or_default();

            if !lyrics_text.is_empty() {
              let mut app = self.app.lock().await;
              // Simple LRC parser
              let parsed: Vec<(u128, String)> = lyrics_text
                .lines()
                .filter_map(|line| {
                  // [mm:ss.xx] text
                  if let Some(idx) = line.find(']') {
                    if idx > 1 && line.starts_with('[') {
                      let timestamp = &line[1..idx];
                      let content = line[idx + 1..].trim().to_string();

                      // Parse timestamp
                      let parts: Vec<&str> = timestamp.split(':').collect();
                      if parts.len() == 2 {
                        let mins = parts[0].parse::<u64>().unwrap_or(0);
                        let secs_parts: Vec<&str> = parts[1].split('.').collect();
                        let secs = secs_parts[0].parse::<u64>().unwrap_or(0);
                        let ms = if secs_parts.len() > 1 {
                          // Handle 2 or 3 digit ms
                          let ms_str = secs_parts[1];
                          let ms_val = ms_str.parse::<u64>().unwrap_or(0);
                          if ms_str.len() == 2 {
                            ms_val * 10
                          } else {
                            ms_val
                          }
                        } else {
                          0
                        };

                        let total_ms = (mins * 60 * 1000) + (secs * 1000) + ms;
                        return Some((total_ms as u128, content));
                      }
                    }
                  }
                  None
                })
                .collect();

              if !parsed.is_empty() {
                app.lyrics = Some(parsed);
                app.lyrics_status = LyricsStatus::Found;
              } else {
                app.lyrics_status = LyricsStatus::NotFound;
              }
            } else {
              let mut app = self.app.lock().await;
              app.lyrics_status = LyricsStatus::NotFound;
            }
          }
        } else {
          let mut app = self.app.lock().await;
          app.lyrics_status = LyricsStatus::NotFound;
        }
      }
      Err(_) => {
        let mut app = self.app.lock().await;
        app.lyrics_status = LyricsStatus::NotFound;
      }
    }
  }

  async fn increment_global_song_count(&mut self) {
    let client = reqwest::Client::new();
    // Fire and forget
    let _ = client
      .post("https://api.spotatui.com/count/inc")
      .timeout(Duration::from_secs(5))
      .send()
      .await;
  }

  async fn fetch_global_song_count(&mut self) {
    let client = reqwest::Client::new();
    match client
      .get("https://api.spotatui.com/count")
      .timeout(Duration::from_secs(5))
      .send()
      .await
    {
      Ok(resp) => {
        if let Ok(data) = resp.json::<GlobalSongCountResponse>().await {
          let mut app = self.app.lock().await;
          app.global_song_count = Some(data.count);
          app.global_song_count_failed = false;
        } else {
          let mut app = self.app.lock().await;
          app.global_song_count_failed = true;
        }
      }
      Err(_) => {
        let mut app = self.app.lock().await;
        app.global_song_count_failed = true;
      }
    }
  }

  async fn fetch_announcements(&mut self) {
    let client = reqwest::Client::new();
    if let Ok(resp) = client
      .get("https://api.spotatui.com/announcements")
      .timeout(Duration::from_secs(5))
      .send()
      .await
    {
      if let Ok(feed) = resp.json::<AnnouncementFeedResponse>().await {
        let now = Utc::now();
        let mut active_announcement: Option<Announcement> = None;

        for record in feed.announcements {
          // Check dates
          if let Some(start_str) = record.starts_at {
            if let Ok(start) = DateTime::parse_from_rfc3339(&start_str) {
              if start.with_timezone(&Utc) > now {
                continue; // Not started yet
              }
            }
          }

          if let Some(end_str) = record.ends_at {
            if let Ok(end) = DateTime::parse_from_rfc3339(&end_str) {
              if end.with_timezone(&Utc) < now {
                continue; // Already ended
              }
            }
          }

          // Check if user has dismissed this specific announcement
          // We need to check read lock first to avoid deadlock or re-entrancy issues if we used a single lock
          let is_dismissed = {
            let app = self.app.lock().await;
            app
              .user_config
              .behavior
              .dismissed_announcements
              .contains(&record.id)
          };

          if is_dismissed {
            continue;
          }

          let level = match record.level.as_deref() {
            Some("critical") => AnnouncementLevel::Critical,
            Some("warning") => AnnouncementLevel::Warning,
            _ => AnnouncementLevel::Info,
          };

          active_announcement = Some(Announcement {
            id: record.id,
            title: record.title.unwrap_or_else(|| "Announcement".to_string()),
            body: record.body,
            level,
            url: record.url,
            received_at: Instant::now(),
          });
          break; // Show only the most relevant/first active one
        }

        if let Some(announcement) = active_announcement {
          let mut app = self.app.lock().await;
          app.active_announcement = Some(announcement);
          app.push_navigation_stack(
            crate::core::app::RouteId::AnnouncementPrompt,
            crate::core::app::ActiveBlock::AnnouncementPrompt,
          );
        }
      }
    }
  }
}
