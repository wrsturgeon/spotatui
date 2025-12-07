// Audio capture and analysis module for real-time visualization
// This module provides cross-platform audio capture:
// - Linux: PipeWire native (via pipewire-rs)
// - Windows/macOS: cpal (WASAPI/CoreAudio)

#[cfg(any(feature = "audio-viz", feature = "audio-viz-cpal"))]
mod analyzer;

// Platform-specific capture backends
#[cfg(all(feature = "audio-viz", target_os = "linux"))]
mod pipewire_capture;

#[cfg(feature = "audio-viz-cpal")]
mod capture;

// Re-export the appropriate capture manager based on platform
#[cfg(all(feature = "audio-viz", target_os = "linux"))]
pub use pipewire_capture::PipeWireCapture as AudioCaptureManager;

#[cfg(feature = "audio-viz-cpal")]
pub use capture::AudioCaptureManager;

// Re-export SpectrumData
#[cfg(any(feature = "audio-viz", feature = "audio-viz-cpal"))]
#[allow(unused_imports)]
pub use analyzer::SpectrumData;

// Fallback types when no audio-viz feature is enabled
#[cfg(not(any(
  all(feature = "audio-viz", target_os = "linux"),
  feature = "audio-viz-cpal"
)))]
#[derive(Clone, Default)]
pub struct SpectrumData {
  pub bands: [f32; 12],
  pub peak: f32,
}

#[cfg(not(any(
  all(feature = "audio-viz", target_os = "linux"),
  feature = "audio-viz-cpal"
)))]
pub struct AudioCaptureManager;

#[cfg(not(any(
  all(feature = "audio-viz", target_os = "linux"),
  feature = "audio-viz-cpal"
)))]
impl AudioCaptureManager {
  pub fn new() -> Option<Self> {
    None
  }

  pub fn get_spectrum(&self) -> Option<SpectrumData> {
    None
  }

  pub fn is_active(&self) -> bool {
    false
  }
}
