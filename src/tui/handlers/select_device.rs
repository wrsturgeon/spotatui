use super::common_key_events;
use crate::core::app::{ActiveBlock, App};
use crate::infra::network::IoEvent;
use crate::tui::event::Key;

pub fn handler(key: Key, app: &mut App) {
  match key {
    Key::Esc => {
      app.set_current_route_state(Some(ActiveBlock::Library), None);
    }
    k if common_key_events::down_event(k) => {
      if let Some(p) = &app.devices {
        if let Some(selected_device_index) = app.selected_device_index {
          let next_index =
            common_key_events::on_down_press_handler(&p.devices, Some(selected_device_index));
          app.selected_device_index = Some(next_index);
        }
      };
    }
    k if common_key_events::up_event(k) => {
      if let Some(p) = &app.devices {
        if let Some(selected_device_index) = app.selected_device_index {
          let next_index =
            common_key_events::on_up_press_handler(&p.devices, Some(selected_device_index));
          app.selected_device_index = Some(next_index);
        }
      };
    }
    k if common_key_events::high_event(k) => {
      if let Some(_p) = &app.devices {
        if let Some(_selected_device_index) = app.selected_device_index {
          let next_index = common_key_events::on_high_press_handler();
          app.selected_device_index = Some(next_index);
        }
      };
    }
    k if common_key_events::middle_event(k) => {
      if let Some(p) = &app.devices {
        if let Some(_selected_device_index) = app.selected_device_index {
          let next_index = common_key_events::on_middle_press_handler(&p.devices);
          app.selected_device_index = Some(next_index);
        }
      };
    }
    k if common_key_events::low_event(k) => {
      if let Some(p) = &app.devices {
        if let Some(_selected_device_index) = app.selected_device_index {
          let next_index = common_key_events::on_low_press_handler(&p.devices);
          app.selected_device_index = Some(next_index);
        }
      };
    }
    Key::Enter => {
      if let Some(index) = app.selected_device_index {
        if let Some(devices) = &app.devices {
          if let Some(device) = devices.devices.get(index) {
            if let Some(device_id) = &device.id {
              app.dispatch(IoEvent::TransferPlaybackToDevice(device_id.clone(), true));
            }
          }
        }
      }
    }
    _ => {}
  }
}
