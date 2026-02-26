use anyhow::anyhow;
use ratatui::{layout::Rect, Frame};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, Resize, StatefulImage};
use rspotify::model::Image;
use std::sync::Mutex;
use tokio::io::AsyncWriteExt;

pub struct CoverArt {
  pub state: Mutex<Option<StatefulProtocol>>,
  url: Option<String>,
}

impl CoverArt {
  pub fn new() -> Self {
    Self {
      state: Mutex::new(None),
      url: None,
    }
  }

  pub async fn refresh(&mut self, image: &Image) -> anyhow::Result<()> {
    if self.url.as_ref() == Some(&image.url) {
      // eprintln!("skipping image refresh: already exists");
      assert!(self.state.lock().unwrap().is_some());
      return Ok(());
    }

    let res = reqwest::get(&image.url).await;
    let res = match res {
      Ok(r) => r.error_for_status(),
      Err(e) => Result::Err(e),
    };
    let file = match res {
      Ok(res) => {
        let mut file = match res.content_length() {
          Some(s) => Vec::with_capacity(s as usize),
          None => Vec::new(),
        };
        file
          .write_all(&res.bytes().await.unwrap())
          .await
          .expect("error while downloading album art");
        file
      }
      Err(e) => return Err(anyhow!(e)),
    };

    let picker = Picker::from_query_stdio().unwrap();
    let image_protocol = picker.new_resize_protocol(image::load_from_memory(&file).unwrap());
    // let ty = match image_protocol.protocol_type() {
    //   StatefulProtocolType::Kitty(_) => "kitty",
    //   StatefulProtocolType::Sixel(_) => "sixel",
    //   StatefulProtocolType::ITerm2(_) => "iterm2",
    //   StatefulProtocolType::Halfblocks(_) => "halfblocks",
    // };
    // eprintln!("USING IMAGE PROTOCOL TYPE: {}", ty);

    self.url = Some(image.url.to_string());
    let mut lock = self.state.lock().unwrap();
    *lock = Some(image_protocol);

    // eprintln!("got new image: url={}, size={}", image.url, file.len());

    Ok(())
  }

  pub fn available(&self) -> bool {
    self.state.lock().unwrap().is_some()
  }

  pub fn render(&self, f: &mut Frame, area: Rect) {
    let mut lock = self.state.lock().unwrap();
    if let Some(sp) = lock.as_mut() {
      f.render_stateful_widget(StatefulImage::new().resize(Resize::Fit(None)), area, sp);
    }
  }
}
