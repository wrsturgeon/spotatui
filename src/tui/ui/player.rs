use crate::core::app::{ActiveBlock, App};
use ratatui::{
  layout::{Alignment, Constraint, Direction, Layout, Rect},
  style::{Color, Modifier, Style},
  text::{Line, Span, Text},
  widgets::{
    canvas::Canvas, Block, BorderType, Borders, LineGauge, List, ListItem, ListState, Paragraph,
    Wrap,
  },
  Frame,
};
use rspotify::model::enums::RepeatState;
use rspotify::model::PlayableItem;
use rspotify::prelude::Id;

use super::util::{
  create_artist_string, display_track_progress, get_color, get_track_progress_percentage,
  BASIC_VIEW_HEIGHT,
};

pub fn draw_basic_view(f: &mut Frame<'_>, app: &App) {
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
      Constraint::Min(0), // Lyrics Area taking all available space above
      Constraint::Length(BASIC_VIEW_HEIGHT), // Playbar at the bottom
    ])
    .split(f.area());

  draw_lyrics(f, app, chunks[0]);
  draw_playbar(f, app, chunks[1]);
}

fn draw_lyrics(f: &mut Frame<'_>, app: &App, area: Rect) {
  use crate::core::app::LyricsStatus;

  // Draw bordered block first
  let block = Block::default()
    .borders(Borders::ALL)
    .title(" Lyrics ")
    .style(Style::default().fg(Color::Rgb(100, 100, 100))); // RGB for cross-terminal compat
  f.render_widget(block.clone(), area);

  let inner_area = block.inner(area);

  if app.lyrics_status != LyricsStatus::Found {
    let text = match app.lyrics_status {
      LyricsStatus::Loading => "Loading lyrics...",
      LyricsStatus::NotFound => "No lyrics found for this track.",
      LyricsStatus::NotStarted => "Waiting for track update...",
      LyricsStatus::Found => "",
    };

    if !text.is_empty() {
      let p = Paragraph::new(text)
        .style(Style::default().fg(Color::Rgb(100, 100, 100))) // RGB for cross-terminal compat
        .alignment(Alignment::Center);

      // Center vertically in inner area
      let vertical_center = inner_area.y + inner_area.height / 2;
      let top_area = Rect {
        x: inner_area.x,
        y: vertical_center.saturating_sub(0), // Just one line centered
        width: inner_area.width,
        height: 1,
      };
      f.render_widget(p, top_area);
    }
    return;
  }

  if let Some(lyrics) = &app.lyrics {
    if lyrics.is_empty() {
      return;
    }

    let current_time = app.song_progress_ms;
    let mut active_idx = 0;
    for (i, (time, _)) in lyrics.iter().enumerate() {
      if *time <= current_time {
        active_idx = i;
      } else {
        break;
      }
    }

    // Target position for active line: Vertical center of inner_area
    let target_row = inner_area.y + (inner_area.height / 2);

    let area_height = inner_area.height as i32;
    let area_y = inner_area.y as i32;

    // Loop through all visible rows of the screen area
    for row in 0..area_height {
      let screen_y = area_y + row;

      // screen_y = target_row + (line_idx - active_idx)
      // line_idx = screen_y - target_row + active_idx

      let offset_from_target = screen_y - (target_row as i32);
      let line_idx = active_idx as i32 + offset_from_target;

      if line_idx >= 0 && line_idx < lyrics.len() as i32 {
        let (_, text) = &lyrics[line_idx as usize];
        let is_active = line_idx == active_idx as i32;

        // Use explicit RGB colors for cross-terminal compatibility
        // Some terminals (like Kitty with custom themes) remap ANSI colors
        let style = if is_active {
          Style::default()
            .fg(app.user_config.theme.highlighted_lyrics) // Use theme color for highlighted lyrics
            .add_modifier(Modifier::BOLD)
        } else {
          Style::default().fg(Color::Rgb(100, 100, 100)) // Dim gray for inactive lines
        };

        let p = Paragraph::new(text.clone())
          .style(style)
          .alignment(Alignment::Center);

        let line_rect = Rect {
          x: inner_area.x,
          y: screen_y as u16,
          width: inner_area.width,
          height: 1,
        };
        f.render_widget(p, line_rect);
      }
    }
  }
}

pub fn draw_playbar(f: &mut Frame<'_>, app: &App, layout_chunk: Rect) {
  #[cfg(feature = "cover-art")]
  let (artist_area, progress_area, cover_art) = {
    // first create margins
    let [other] = layout_chunk.layout(&Layout::horizontal([Constraint::Fill(1)]).margin(1));

    let (other, album_art) = if app.cover_art.available() {
      let height = other.height;
      // we need to allocate a square portion of layout_chunk, but terminal characters aren't
      // square!

      // totally arbitrary
      let ratio = 1.9;
      // we ceil rather than simply casting for using the full height of the area
      let width = ((height as f32) * ratio).ceil() as u16;
      let [cover_art, _, other] = other.layout(&Layout::horizontal([
        Constraint::Length(width),
        Constraint::Length(1),
        Constraint::Percentage(100),
      ]));

      (other, Some(cover_art))
    } else {
      (other, None)
    };

    let [artist_area, _, progress_area] = other.layout(&Layout::vertical([
      Constraint::Percentage(50),
      Constraint::Percentage(25),
      Constraint::Percentage(25),
    ]));

    (artist_area, progress_area, album_art)
  };

  #[cfg(not(feature = "cover-art"))]
  let [artist_area, _, progress_area] = layout_chunk.layout(
    &Layout::vertical([
      Constraint::Percentage(50),
      Constraint::Percentage(25),
      Constraint::Percentage(25),
    ])
    .margin(1),
  );

  let mut drew_playbar = false;

  // If no track is playing, render paragraph showing which device is selected, if no selected
  // give hint to choose a device
  if let Some(current_playback_context) = &app.current_playback_context {
    if let Some(track_item) = &current_playback_context.item {
      // Use native playing state when streaming is active (more reliable for MPRIS controls)
      let is_playing = app
        .native_is_playing
        .filter(|_| app.is_streaming_active)
        .unwrap_or(current_playback_context.is_playing);

      let play_title = if is_playing { "Playing" } else { "Paused" };

      let shuffle_text = if current_playback_context.shuffle_state {
        "On"
      } else {
        "Off"
      };

      let repeat_text = match current_playback_context.repeat_state {
        RepeatState::Off => "Off",
        RepeatState::Track => "Track",
        RepeatState::Context => "All",
      };

      let mut title = format!(
        "{:-7} ({} | Shuffle: {:-3} | Repeat: {:-5} | Volume: {:-2}%)",
        play_title,
        current_playback_context.device.name,
        shuffle_text,
        repeat_text,
        current_playback_context.device.volume_percent.unwrap_or(0)
      );

      if let Some(message) = app.status_message.as_ref() {
        title = format!("{} | {}", title, message);
      }

      let current_route = app.get_current_route();
      let highlight_state = (
        current_route.active_block == ActiveBlock::PlayBar,
        current_route.hovered_block == ActiveBlock::PlayBar,
      );

      let title_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(app.user_config.theme.playbar_background))
        .title(Span::styled(
          &title,
          get_color(highlight_state, app.user_config.theme),
        ))
        .border_style(get_color(highlight_state, app.user_config.theme));

      f.render_widget(title_block, layout_chunk);

      let (item_id, name, duration) = match track_item {
        PlayableItem::Track(track) => (
          track
            .id
            .as_ref()
            .map(|id| id.id().to_string())
            .unwrap_or_default(),
          track.name.to_owned(),
          track.duration,
        ),
        PlayableItem::Episode(episode) => (
          episode.id.id().to_string(),
          episode.name.to_owned(),
          episode.duration,
        ),
      };

      // Use native track info for instant display when available (e.g., after skipping tracks)
      // Falls back to API data when native info is not available
      let (display_name, display_artists, display_duration_ms) =
        if let Some(ref native_info) = app.native_track_info {
          (
            native_info.name.clone(),
            native_info.artists_display.clone(),
            native_info.duration_ms as u64,
          )
        } else {
          let artists_str = match track_item {
            PlayableItem::Track(track) => create_artist_string(&track.artists),
            PlayableItem::Episode(episode) => format!("{} - {}", episode.name, episode.show.name),
          };
          (
            name.clone(),
            artists_str,
            duration.num_milliseconds() as u64,
          )
        };

      let track_name = if app.liked_song_ids_set.contains(&item_id) {
        format!("{}{}", &app.user_config.padded_liked_icon(), display_name)
      } else {
        display_name
      };

      let lines = Text::from(Span::styled(
        display_artists,
        Style::default().fg(app.user_config.theme.playbar_text),
      ));

      let artist = Paragraph::new(lines)
        .style(Style::default().fg(app.user_config.theme.playbar_text))
        .block(
          Block::default().title(Span::styled(
            &track_name,
            Style::default()
              .fg(app.user_config.theme.selected)
              .add_modifier(Modifier::BOLD),
          )),
        );
      f.render_widget(artist, artist_area);

      let progress_ms = match app.seek_ms {
        Some(seek_ms) => seek_ms,
        None => app.song_progress_ms,
      };

      let duration_std = std::time::Duration::from_millis(display_duration_ms);
      let perc = get_track_progress_percentage(progress_ms, duration_std);

      let song_progress_label = display_track_progress(progress_ms, duration_std);
      let modifier = if app.user_config.behavior.enable_text_emphasis {
        Modifier::ITALIC | Modifier::BOLD
      } else {
        Modifier::empty()
      };
      let song_progress = LineGauge::default()
        .filled_style(
          Style::default()
            .fg(app.user_config.theme.playbar_progress)
            .add_modifier(modifier),
        )
        .unfilled_style(
          Style::default()
            .fg(app.user_config.theme.playbar_background)
            .add_modifier(modifier),
        )
        .ratio(perc as f64 / 100.0)
        .filled_symbol("⣿")
        .unfilled_symbol("⣉")
        .label(Span::styled(
          &song_progress_label,
          Style::default().fg(app.user_config.theme.playbar_progress_text),
        ));
      f.render_widget(song_progress, progress_area);

      // Draw "Like" animation (heart burst) if active
      if let Some(frame) = app.liked_song_animation_frame {
        let progress = (10 - frame) as f64;
        let y_base = 20.0 + progress * 5.0; // Rise up

        let canvas = Canvas::default()
          .block(Block::default()) // No border, transparent
          .x_bounds([0.0, 100.0])
          .y_bounds([0.0, 100.0])
          .paint(|ctx| {
            let color = app.user_config.theme.selected;
            // Center heart
            ctx.print(50.0, y_base, Span::styled("♥", Style::default().fg(color)));
            // Left particle (lagging slightly)
            ctx.print(
              48.0,
              y_base - 3.0,
              Span::styled("♥", Style::default().fg(color)),
            );
            // Right particle (lagging slightly)
            ctx.print(
              52.0,
              y_base - 3.0,
              Span::styled("♥", Style::default().fg(color)),
            );
          });

        f.render_widget(canvas, layout_chunk);
      }

      #[cfg(feature = "cover-art")]
      if let Some(cover_art) = cover_art {
        app.cover_art.render(f, cover_art);
      }

      drew_playbar = true;
    }
  }

  if !drew_playbar {
    if let Some(message) = app.status_message.as_ref() {
      let title_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(app.user_config.theme.playbar_background))
        .title(Span::styled(
          format!("Status: {}", message),
          Style::default().fg(app.user_config.theme.playbar_text),
        ))
        .border_style(Style::default().fg(app.user_config.theme.inactive));
      f.render_widget(title_block, layout_chunk);
    }
  }
}

pub fn draw_device_list(f: &mut Frame<'_>, app: &App) {
  let [instructions_area, list_area] = f
    .area()
    .layout(&Layout::vertical([Constraint::Percentage(20), Constraint::Percentage(80)]).margin(5));

  let device_instructions: Vec<Line> = vec![
        "To play tracks, please select a device. ",
        "Use `j/k` or up/down arrow keys to move up and down and <Enter> to select. ",
        "Your choice here will be cached so you can jump straight back in when you next open `spotatui`. ",
        "You can change the playback device at any time by pressing `d`.",
    ].into_iter().map(|instruction| Line::from(Span::raw(instruction))).collect();

  let instructions = Paragraph::new(device_instructions)
    .style(Style::default().fg(app.user_config.theme.text))
    .wrap(Wrap { trim: true })
    .block(
      Block::default().borders(Borders::NONE).title(Span::styled(
        "Welcome to spotatui!",
        Style::default()
          .fg(app.user_config.theme.active)
          .add_modifier(Modifier::BOLD),
      )),
    );
  f.render_widget(instructions, instructions_area);

  let no_device_message = Span::raw("No devices found: Make sure a device is active");

  let items = match &app.devices {
    Some(items) => {
      if items.devices.is_empty() {
        vec![ListItem::new(no_device_message)]
      } else {
        items
          .devices
          .iter()
          .map(|device| ListItem::new(Span::raw(&device.name)))
          .collect()
      }
    }
    None => vec![ListItem::new(no_device_message)],
  };

  let mut state = ListState::default();
  state.select(app.selected_device_index);
  let list = List::new(items)
    .block(
      Block::default()
        .title(Span::styled(
          "Devices",
          Style::default().fg(app.user_config.theme.active),
        ))
        .borders(Borders::ALL)
        .style(app.user_config.theme.base_style())
        .border_style(Style::default().fg(app.user_config.theme.inactive)),
    )
    .style(app.user_config.theme.base_style())
    .highlight_style(
      Style::default()
        .fg(app.user_config.theme.active)
        .bg(app.user_config.theme.inactive)
        .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol(Line::from("▶ ").style(Style::default().fg(app.user_config.theme.active)));
  f.render_stateful_widget(list, list_area, &mut state);
}
