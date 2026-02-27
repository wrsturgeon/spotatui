use anyhow::anyhow;
use log::{debug, info};
use ratatui::{layout::Rect, Frame};
use ratatui_image::{
  picker::{Picker, ProtocolType},
  protocol::StatefulProtocol,
  Resize, StatefulImage,
};
use rspotify::model::Image;
use std::sync::Mutex;
use tokio::io::AsyncWriteExt;

pub struct CoverArt {
  pub state: Mutex<Option<CoverArtState>>,
  picker: Picker,
}

pub struct CoverArtState {
  url: String,
  image: StatefulProtocol,
}

impl CoverArtState {
  fn new(url: String, image: StatefulProtocol) -> Self {
    Self { url, image }
  }
}

impl CoverArt {
  pub fn new() -> Self {
    let picker = Picker::from_query_stdio().unwrap();

    info!(
      "cover art rendered detected a {:?} backend",
      picker.protocol_type()
    );
    Self {
      picker,
      state: Mutex::new(None),
    }
  }

  pub fn full_image_support(&self) -> bool {
    match self.picker.protocol_type() {
      ProtocolType::Kitty | ProtocolType::Iterm2 | ProtocolType::Sixel => true,
      ProtocolType::Halfblocks => false,
    }
  }

  pub fn get_url(&self) -> Option<String> {
    self.state.lock().unwrap().as_ref().map(|s| s.url.clone())
  }

  pub fn set_state(&self, state: CoverArtState) {
    let mut lock = self.state.lock().unwrap();
    *lock = Some(state);
  }

  pub async fn refresh(&self, image: &Image) -> anyhow::Result<()> {
    if self.get_url().as_ref() != Some(&image.url) {
      info!("getting new cover art image...");

      let res = match reqwest::get(&image.url).await {
        Ok(r) => r.error_for_status(),
        Err(e) => Result::Err(e),
      };

      let file = match res {
        Ok(res) => {
          // Allocate Vec "file" with capacity if content_length is provided
          let mut file = match res.content_length() {
            Some(s) => Vec::with_capacity(s as usize),
            None => Vec::new(),
          };

          file
            .write_all(&res.bytes().await.unwrap())
            .await
            .expect("error while downloading album art");

          debug!("finished reading response: {} bytes", file.len());
          file
        }
        Err(e) => return Err(anyhow!(e)),
      };

      let image_protocol = self
        .picker
        .new_resize_protocol(image::load_from_memory(&file).unwrap());

      self.set_state(CoverArtState::new(image.url.clone(), image_protocol));
      info!("got new cover art: {}", image.url);
    } else {
      debug!("skipping image refresh: cover art already downloaded");
    }

    Ok(())
  }

  pub fn available(&self) -> bool {
    self.state.lock().unwrap().is_some()
  }

  pub fn render(&self, f: &mut Frame, area: Rect) {
    let mut lock = self.state.lock().unwrap();
    if let Some(sp) = lock.as_mut() {
      f.render_stateful_widget(
        StatefulImage::new().resize(Resize::Fit(None)),
        area,
        &mut sp.image,
      );
    }
  }
}
