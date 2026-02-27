use std::{
  io::prelude::*,
  net::{TcpListener, TcpStream},
};

pub fn redirect_uri_web_server(port: u16) -> Result<String, ()> {
  let listener = TcpListener::bind(format!("127.0.0.1:{}", port));

  match listener {
    Ok(listener) => {
      for stream in listener.incoming() {
        match stream {
          Ok(stream) => {
            if let Some(url) = handle_connection(stream) {
              return Ok(url);
            }
          }
          Err(e) => {
            println!("Error: {}", e);
          }
        };
      }
    }
    Err(e) => {
      println!("Error: {}", e);
    }
  }

  Err(())
}

fn handle_connection(mut stream: TcpStream) -> Option<String> {
  // The request will be quite large (> 512) so just assign plenty just in case
  let mut buffer = [0; 1000];
  let _ = stream.read(&mut buffer).unwrap();

  // convert buffer into string and 'parse' the URL
  match String::from_utf8(buffer.to_vec()) {
    Ok(request) => {
      let split: Vec<&str> = request.split_whitespace().collect();

      if split.len() > 1 {
        // Extract the path from the HTTP request (e.g., "/callback?code=...&state=...")
        let path = split[1];

        // Parse the host header to build the full URL
        let host = request
          .lines()
          .find(|line| line.to_lowercase().starts_with("host:"))
          .and_then(|line| line.split(':').nth(1))
          .map(|h| h.trim())
          .unwrap_or("127.0.0.1:8888");

        // Construct the full URL
        let full_url = format!("http://{}{}", host, path);

        respond_with_success(stream);
        return Some(full_url);
      }

      respond_with_error("Malformed request".to_string(), stream);
    }
    Err(e) => {
      respond_with_error(format!("Invalid UTF-8 sequence: {}", e), stream);
    }
  };

  None
}

fn respond_with_success(mut stream: TcpStream) {
  let contents = include_str!("redirect_uri.html");

  let response = format!(
    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
    contents.len(),
    contents
  );

  stream.write_all(response.as_bytes()).unwrap();
  stream.flush().unwrap();
  // Give the browser time to receive the response before closing
  std::thread::sleep(std::time::Duration::from_millis(100));
}

fn respond_with_error(error_message: String, mut stream: TcpStream) {
  println!("Error: {}", error_message);
  let body = format!("400 - Bad Request - {}", error_message);
  let response = format!(
    "HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
    body.len(),
    body
  );

  stream.write_all(response.as_bytes()).unwrap();
  stream.flush().unwrap();
  std::thread::sleep(std::time::Duration::from_millis(100));
}
