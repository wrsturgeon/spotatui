use super::util;
use crate::app::App;
use crate::user_config::VisualizerStyle;
use ratatui::{
  layout::{Constraint, Direction, Layout, Rect},
  style::{Color, Style},
  symbols,
  text::{Line, Span},
  widgets::{Bar, BarChart, BarGroup, Block, Borders, Paragraph},
  Frame,
};

/// Frequency band labels (low to high frequency)
const BAND_LABELS: [&str; 12] = [
  "Sub", "Bass", "Low", "LMid", "Mid", "UMid", "High", "HiMd", "Pres", "Bril", "Air", "Ultra",
];

pub fn draw(f: &mut Frame<'_>, app: &App) {
  let margin = util::get_main_layout_margin(app);

  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Length(3), Constraint::Min(10)].as_ref())
    .margin(margin)
    .split(f.size());

  let white = Style::default().fg(app.user_config.theme.text);
  let gray = Style::default().fg(app.user_config.theme.inactive);
  let tick_rate = app.user_config.behavior.tick_rate_milliseconds;
  let visualizer_style = app.user_config.behavior.visualizer_style;

  let info_block = Block::default()
    .title(Span::styled(
      format!("Audio Visualization ({})", visualizer_style.name()),
      Style::default().fg(app.user_config.theme.inactive),
    ))
    .borders(Borders::ALL)
    .border_style(Style::default().fg(app.user_config.theme.inactive));

  let bar_chart_title = &format!("Spectrum | {} FPS | Press q to exit", 1000 / tick_rate);

  let bar_chart_block = Block::default()
    .borders(Borders::ALL)
    .style(white)
    .title(Span::styled(bar_chart_title, gray))
    .border_style(gray);

  // Check if we have spectrum data from local audio capture
  if let Some(ref spectrum) = app.spectrum_data {
    // Info panel with status
    // Use ASCII-safe symbols instead of emojis for Windows compatibility
    let status_text = if app.audio_capture_active {
      "[>] Capturing audio"
    } else {
      "[||] Paused"
    };

    let peak_text = format!("Peak: {:.0}%", spectrum.peak * 100.0);
    let style_hint = "Press 'V' to cycle visualizer style";

    let texts = vec![Line::from(vec![
      Span::styled(status_text, Style::default().fg(app.user_config.theme.text)),
      Span::raw("  "),
      Span::styled(
        peak_text,
        Style::default().fg(app.user_config.theme.inactive),
      ),
      Span::raw("  |  "),
      Span::styled(style_hint, Style::default().fg(app.user_config.theme.hint)),
    ])];

    let p = Paragraph::new(texts)
      .block(info_block)
      .style(Style::default().fg(app.user_config.theme.text));
    f.render_widget(p, chunks[0]);

    // Calculate inner area for visualizer (within the block borders)
    let inner_area = bar_chart_block.inner(chunks[1]);

    // Render the appropriate visualizer based on user setting
    match visualizer_style {
      VisualizerStyle::Classic => {
        render_classic(f, app, &spectrum.bands, chunks[1], bar_chart_block);
      }
      VisualizerStyle::Equalizer => {
        f.render_widget(bar_chart_block, chunks[1]);
        render_equalizer(f, &spectrum.bands, inner_area);
      }
      VisualizerStyle::BarGraph => {
        f.render_widget(bar_chart_block, chunks[1]);
        render_bar_graph(f, &spectrum.bands, inner_area);
      }
    }
  } else {
    // No audio capture available
    let no_capture_text = vec![
      Line::from("No audio capture available"),
      Line::from(""),
      #[cfg(target_os = "linux")]
      Line::from("Hint: Ensure PipeWire or PulseAudio is running with a monitor device"),
      #[cfg(target_os = "windows")]
      Line::from("Hint: Audio loopback should work automatically on Windows"),
      #[cfg(target_os = "macos")]
      Line::from("Hint: macOS requires a virtual audio device like BlackHole"),
      #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
      Line::from("Hint: Audio capture may not be supported on this platform"),
    ];

    let p = Paragraph::new(no_capture_text)
      .block(info_block)
      .style(Style::default().fg(app.user_config.theme.text));
    f.render_widget(p, chunks[0]);

    // Empty bar chart
    let empty_p = Paragraph::new("Waiting for audio input...")
      .block(bar_chart_block)
      .style(Style::default().fg(app.user_config.theme.text));
    f.render_widget(empty_p, chunks[1]);
  }
}

/// Render the classic Ratatui BarChart with gradient colors
fn render_classic(f: &mut Frame<'_>, app: &App, bands: &[f32], area: Rect, block: Block<'_>) {
  let width = (area.width as f32 / (1 + BAND_LABELS.len()) as f32).max(3.0);

  // Create bars with gradient colors based on height
  let bars: Vec<Bar> = bands
    .iter()
    .enumerate()
    .map(|(index, &value)| {
      let label = BAND_LABELS.get(index).unwrap_or(&"?");
      // Scale value to u64 for display (0.0-1.0 -> 0-1000)
      // Cap at 800 so bars never hit the top (max is 1000)
      let bar_value = ((value * 1000.0) as u64).min(800);

      // Gradient color based on bar height: green -> yellow -> orange -> red
      let color = value_to_gradient_color(value);

      Bar::default()
        .value(bar_value)
        .label(Line::from(*label))
        .style(Style::default().fg(color))
        .value_style(Style::default().fg(Color::Rgb(255, 255, 255)).bg(color))
    })
    .collect();

  let spectrum_bar = BarChart::default()
    .block(block)
    .data(BarGroup::default().bars(&bars))
    .bar_width(width as u16)
    .max(1000);
  f.render_widget(spectrum_bar, area);
}

/// Render equalizer-style visualization using half-block characters
/// Inspired by tui-equalizer but implemented natively for compatibility
fn render_equalizer(f: &mut Frame<'_>, bands: &[f32], area: Rect) {
  if area.width == 0 || area.height == 0 || bands.is_empty() {
    return;
  }

  let buf = f.buffer_mut();

  // Calculate bar width (2 chars per band like tui-equalizer)
  let bar_width = 2u16;
  let total_width = (bands.len() as u16) * bar_width;
  let start_x = area.x + (area.width.saturating_sub(total_width)) / 2;

  for (i, &value) in bands.iter().enumerate() {
    let x = start_x + (i as u16) * bar_width;
    if x >= area.x + area.width {
      break;
    }

    let value = value.clamp(0.0, 1.0);
    let height = ((value * area.height as f32) as u16).min(area.height);

    // Render each segment with color gradient
    for row in 0..height {
      let y = area.y + area.height - 1 - row;
      if y < area.y {
        break;
      }

      // Calculate color based on position (green at bottom, red at top)
      let position = row as f32 / area.height as f32;
      let color = position_to_equalizer_color(position);

      // Use half-block for smoother appearance
      if x < area.x + area.width {
        buf
          .get_mut(x, y)
          .set_symbol(symbols::bar::HALF)
          .set_fg(color);
      }
      if x + 1 < area.x + area.width {
        buf
          .get_mut(x + 1, y)
          .set_symbol(symbols::bar::HALF)
          .set_fg(color);
      }
    }
  }
}

/// Render bar graph-style visualization using Braille characters for high resolution
/// Inspired by tui-bar-graph but implemented natively for compatibility
fn render_bar_graph(f: &mut Frame<'_>, bands: &[f32], area: Rect) {
  if area.width == 0 || area.height == 0 || bands.is_empty() {
    return;
  }

  let buf = f.buffer_mut();

  // Expand bands to fill width for higher resolution
  let target_width = area.width as usize;
  let data = interpolate_bands(bands, target_width);

  // Braille characters have 4 dots vertically per cell
  let dots_per_cell = 4u16;
  let max_dots = area.height * dots_per_cell;

  for (i, &value) in data.iter().enumerate() {
    let x = area.x + i as u16;
    if x >= area.x + area.width {
      break;
    }

    let value = value.clamp(0.0, 1.0);
    let total_dots = ((value * max_dots as f64) as u16).min(max_dots);
    let full_cells = total_dots / dots_per_cell;
    let remaining_dots = total_dots % dots_per_cell;

    // Render full cells from bottom
    for row in 0..full_cells {
      let y = area.y + area.height - 1 - row;
      if y < area.y {
        break;
      }

      // Color gradient based on vertical position (turbo-like gradient)
      let position = row as f32 / area.height as f32;
      let color = position_to_turbo_color(position);

      // Use full block for filled cells
      buf.get_mut(x, y).set_symbol("█").set_fg(color);
    }

    // Render partial cell at top if needed
    if remaining_dots > 0 && full_cells < area.height {
      let y = area.y + area.height - 1 - full_cells;
      if y >= area.y {
        let position = full_cells as f32 / area.height as f32;
        let color = position_to_turbo_color(position);

        // Use fractional block characters
        let symbol = match remaining_dots {
          1 => "▁",
          2 => "▂",
          3 => "▃",
          _ => "▄",
        };
        buf.get_mut(x, y).set_symbol(symbol).set_fg(color);
      }
    }
  }
}

/// Convert value (0.0-1.0) to gradient color (green -> yellow -> orange -> red)
fn value_to_gradient_color(value: f32) -> Color {
  if value < 0.25 {
    Color::Rgb(0, 200, 0) // Green
  } else if value < 0.5 {
    Color::Rgb(180, 200, 0) // Yellow-green
  } else if value < 0.65 {
    Color::Rgb(255, 200, 0) // Yellow
  } else if value < 0.75 {
    Color::Rgb(255, 140, 0) // Orange
  } else {
    Color::Rgb(255, 50, 0) // Red
  }
}

/// Convert vertical position (0.0-1.0) to equalizer color (green at bottom, red at top)
fn position_to_equalizer_color(position: f32) -> Color {
  // Smooth gradient from green to yellow to red
  let r = if position < 0.5 {
    (position * 2.0 * 255.0) as u8
  } else {
    255
  };
  let g = if position < 0.5 {
    255
  } else {
    ((1.0 - (position - 0.5) * 2.0) * 255.0) as u8
  };
  Color::Rgb(r, g, 0)
}

/// Convert vertical position to turbo-like colormap (blue -> cyan -> green -> yellow -> red)
fn position_to_turbo_color(position: f32) -> Color {
  // Simplified turbo colormap approximation
  let t = position.clamp(0.0, 1.0);
  let r: u8;
  let g: u8;
  let b: u8;

  if t < 0.25 {
    // Blue to cyan
    let f = t * 4.0;
    r = 0;
    g = (f * 200.0) as u8;
    b = 255;
  } else if t < 0.5 {
    // Cyan to green
    let f = (t - 0.25) * 4.0;
    r = 0;
    g = 200 + (f * 55.0) as u8;
    b = ((1.0 - f) * 255.0) as u8;
  } else if t < 0.75 {
    // Green to yellow
    let f = (t - 0.5) * 4.0;
    r = (f * 255.0) as u8;
    g = 255;
    b = 0;
  } else {
    // Yellow to red
    let f = (t - 0.75) * 4.0;
    r = 255;
    g = ((1.0 - f) * 255.0) as u8;
    b = 0;
  }

  Color::Rgb(r, g, b)
}

/// Interpolate band values to fill the target width for smoother display
fn interpolate_bands(bands: &[f32], target_width: usize) -> Vec<f64> {
  if bands.is_empty() {
    return vec![0.0; target_width];
  }
  if bands.len() == 1 {
    return vec![bands[0] as f64; target_width];
  }

  let mut result = Vec::with_capacity(target_width);
  let scale = (bands.len() - 1) as f64 / (target_width - 1).max(1) as f64;

  for i in 0..target_width {
    let pos = i as f64 * scale;
    let idx = pos.floor() as usize;
    let frac = pos - idx as f64;

    let value = if idx + 1 < bands.len() {
      bands[idx] as f64 * (1.0 - frac) + bands[idx + 1] as f64 * frac
    } else {
      bands[idx.min(bands.len() - 1)] as f64
    };

    result.push(value);
  }

  result
}
