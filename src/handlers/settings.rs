use super::super::app::{App, SettingValue, SettingsCategory};
use crate::event::Key;
use crate::handlers::common_key_events::{down_event, left_event, right_event, up_event};

pub fn handler(key: Key, app: &mut App) {
  if app.settings_edit_mode {
    handle_edit_mode(key, app);
  } else {
    handle_navigation(key, app);
  }
}

fn handle_navigation(key: Key, app: &mut App) {
  match key {
    // Category switching with left/right (only when not in edit mode)
    key if left_event(key) => switch_category_left(app),
    key if right_event(key) => switch_category_right(app),

    // Item selection with up/down
    key if down_event(key) => select_next_item(app),
    key if up_event(key) => select_previous_item(app),

    // Enter edit mode
    Key::Enter => enter_edit_mode(app),

    // Save settings
    Key::Alt('s') => save_settings(app),

    // Exit settings
    Key::Esc => {
      app.pop_navigation_stack();
    }
    key if key == app.user_config.keys.back => {
      app.pop_navigation_stack();
    }
    _ => {}
  }
}

fn handle_edit_mode(key: Key, app: &mut App) {
  if let Some(setting) = app.settings_items.get(app.settings_selected_index) {
    match &setting.value {
      SettingValue::Bool(_) => handle_bool_edit(key, app),
      SettingValue::Number(_) => handle_number_edit(key, app),
      SettingValue::Preset(_) => handle_preset_edit(key, app),
      SettingValue::String(_) | SettingValue::Key(_) | SettingValue::Color(_) => {
        handle_string_edit(key, app)
      }
    }
  }
}

fn handle_bool_edit(key: Key, app: &mut App) {
  match key {
    Key::Enter | Key::Char(' ') => {
      // Toggle the boolean value
      if let Some(setting) = app.settings_items.get_mut(app.settings_selected_index) {
        if let SettingValue::Bool(v) = setting.value {
          setting.value = SettingValue::Bool(!v);
        }
      }
      app.settings_edit_mode = false;
    }
    Key::Esc => {
      app.settings_edit_mode = false;
    }
    key if left_event(key) || right_event(key) => {
      // Toggle on left/right as well for better UX
      if let Some(setting) = app.settings_items.get_mut(app.settings_selected_index) {
        if let SettingValue::Bool(v) = setting.value {
          setting.value = SettingValue::Bool(!v);
        }
      }
    }
    _ => {}
  }
}

fn handle_number_edit(key: Key, app: &mut App) {
  match key {
    Key::Enter => {
      // Parse and apply the edited number
      if let Ok(num) = app.settings_edit_buffer.parse::<i64>() {
        if let Some(setting) = app.settings_items.get_mut(app.settings_selected_index) {
          setting.value = SettingValue::Number(num);
        }
      }
      app.settings_edit_mode = false;
      app.settings_edit_buffer.clear();
    }
    Key::Esc => {
      app.settings_edit_mode = false;
      app.settings_edit_buffer.clear();
    }
    Key::Char(c) if c.is_ascii_digit() || c == '-' => {
      app.settings_edit_buffer.push(c);
    }
    Key::Backspace => {
      app.settings_edit_buffer.pop();
    }
    key if up_event(key) => {
      // Increment value
      if let Some(setting) = app.settings_items.get_mut(app.settings_selected_index) {
        if let SettingValue::Number(v) = setting.value {
          let new_val = v + 1;
          setting.value = SettingValue::Number(new_val);
          app.settings_edit_buffer = new_val.to_string();
        }
      }
    }
    key if down_event(key) => {
      // Decrement value
      if let Some(setting) = app.settings_items.get_mut(app.settings_selected_index) {
        if let SettingValue::Number(v) = setting.value {
          let new_val = v - 1;
          setting.value = SettingValue::Number(new_val);
          app.settings_edit_buffer = new_val.to_string();
        }
      }
    }
    _ => {}
  }
}

fn handle_string_edit(key: Key, app: &mut App) {
  match key {
    Key::Enter => {
      // Apply the edited string
      if let Some(setting) = app.settings_items.get_mut(app.settings_selected_index) {
        let new_value = app.settings_edit_buffer.clone();
        match &setting.value {
          SettingValue::String(_) => {
            setting.value = SettingValue::String(new_value);
          }
          SettingValue::Key(_) => {
            setting.value = SettingValue::Key(new_value);
          }
          SettingValue::Color(_) => {
            setting.value = SettingValue::Color(new_value);
          }
          _ => {}
        }
      }
      app.settings_edit_mode = false;
      app.settings_edit_buffer.clear();
    }
    Key::Esc => {
      app.settings_edit_mode = false;
      app.settings_edit_buffer.clear();
    }
    Key::Char(c) => {
      app.settings_edit_buffer.push(c);
    }
    Key::Backspace => {
      app.settings_edit_buffer.pop();
    }
    _ => {}
  }
}

fn switch_category_left(app: &mut App) {
  let current_index = app.settings_category.index();
  let new_index = if current_index == 0 {
    SettingsCategory::all().len() - 1
  } else {
    current_index - 1
  };
  app.settings_category = SettingsCategory::from_index(new_index);
  app.settings_selected_index = 0;
  app.load_settings_for_category();
}

fn switch_category_right(app: &mut App) {
  let current_index = app.settings_category.index();
  let new_index = (current_index + 1) % SettingsCategory::all().len();
  app.settings_category = SettingsCategory::from_index(new_index);
  app.settings_selected_index = 0;
  app.load_settings_for_category();
}

fn select_next_item(app: &mut App) {
  if !app.settings_items.is_empty() {
    app.settings_selected_index = (app.settings_selected_index + 1) % app.settings_items.len();
  }
}

fn select_previous_item(app: &mut App) {
  if !app.settings_items.is_empty() {
    if app.settings_selected_index == 0 {
      app.settings_selected_index = app.settings_items.len() - 1;
    } else {
      app.settings_selected_index -= 1;
    }
  }
}

fn enter_edit_mode(app: &mut App) {
  if let Some(setting) = app.settings_items.get(app.settings_selected_index) {
    // For booleans, toggle directly without entering edit mode
    if let SettingValue::Bool(v) = setting.value {
      // Need to get mutable reference
      if let Some(setting_mut) = app.settings_items.get_mut(app.settings_selected_index) {
        setting_mut.value = SettingValue::Bool(!v);
      }
      return;
    }

    // For presets, cycle to next preset directly
    if let SettingValue::Preset(ref preset_name) = setting.value {
      use crate::user_config::ThemePreset;
      let current = ThemePreset::from_name(preset_name);
      let next = current.next();
      if let Some(setting_mut) = app.settings_items.get_mut(app.settings_selected_index) {
        setting_mut.value = SettingValue::Preset(next.name().to_string());
      }
      return;
    }

    // For other types, enter edit mode
    app.settings_edit_mode = true;
    // Pre-populate the edit buffer with current value
    app.settings_edit_buffer = match &setting.value {
      SettingValue::Bool(_) => String::new(), // Shouldn't reach here
      SettingValue::Number(v) => v.to_string(),
      SettingValue::String(v) => v.clone(),
      SettingValue::Key(v) => v.clone(),
      SettingValue::Color(v) => v.clone(),
      SettingValue::Preset(_) => String::new(), // Shouldn't reach here
    };
  }
}

fn handle_preset_edit(key: Key, app: &mut App) {
  use crate::user_config::ThemePreset;

  match key {
    Key::Enter | Key::Char(' ') => {
      // Cycle to next preset
      if let Some(setting) = app.settings_items.get(app.settings_selected_index) {
        if let SettingValue::Preset(ref preset_name) = setting.value {
          let current = ThemePreset::from_name(preset_name);
          let next = current.next();
          if let Some(setting_mut) = app.settings_items.get_mut(app.settings_selected_index) {
            setting_mut.value = SettingValue::Preset(next.name().to_string());
          }
        }
      }
      app.settings_edit_mode = false;
    }
    Key::Esc => {
      app.settings_edit_mode = false;
    }
    key if right_event(key) => {
      // Next preset
      if let Some(setting) = app.settings_items.get(app.settings_selected_index) {
        if let SettingValue::Preset(ref preset_name) = setting.value {
          let current = ThemePreset::from_name(preset_name);
          let next = current.next();
          if let Some(setting_mut) = app.settings_items.get_mut(app.settings_selected_index) {
            setting_mut.value = SettingValue::Preset(next.name().to_string());
          }
        }
      }
    }
    key if left_event(key) => {
      // Previous preset
      if let Some(setting) = app.settings_items.get(app.settings_selected_index) {
        if let SettingValue::Preset(ref preset_name) = setting.value {
          let current = ThemePreset::from_name(preset_name);
          let prev = current.prev();
          if let Some(setting_mut) = app.settings_items.get_mut(app.settings_selected_index) {
            setting_mut.value = SettingValue::Preset(prev.name().to_string());
          }
        }
      }
    }
    _ => {}
  }
}

fn save_settings(app: &mut App) {
  // Apply settings to user_config and save to file
  app.apply_settings_changes();
  if let Err(e) = app.user_config.save_config() {
    app.handle_error(anyhow::anyhow!("Failed to save settings: {}", e));
  }
}
