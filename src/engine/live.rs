//! Live cpal output stream — the P0 implementation of [`EqEngine`].

use std::sync::{Arc, RwLock};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, Stream, StreamConfig};

use crate::engine::EqEngine;
use crate::engine::render::Renderer;
use crate::state::SharedState;

/// A running output stream on the default device, driven by [`SharedState`].
pub struct LiveEngine {
    stream: Stream,
    device_name: String,
    sample_rate: u32,
    channels: u16,
    sample_format: SampleFormat,
    last_error: Arc<RwLock<Option<String>>>,
}

impl Drop for LiveEngine {
    fn drop(&mut self) {
        let _ = self.stream.pause();
    }
}

impl EqEngine for LiveEngine {
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn device_label(&self) -> String {
        format!(
            "{} · {} Hz · {:?} · {} ch",
            self.device_name, self.sample_rate, self.sample_format, self.channels
        )
    }

    fn take_error(&self) -> Option<String> {
        self.last_error
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }
}

impl LiveEngine {
    /// Open the default output device and start a live stream driven by
    /// `state`. Errors come back as strings for the status bar — never a
    /// panic.
    pub fn start(state: Arc<SharedState>) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| "no default output device found".to_string())?;
        let device_name = device
            .description()
            .map(|desc| desc.name().to_string())
            .unwrap_or_else(|_| "unknown device".to_string());
        let supported = device
            .default_output_config()
            .map_err(|e| format!("default output config: {e}"))?;
        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.into();
        if config.channels == 0 {
            return Err("device reports zero output channels".to_string());
        }

        let last_error = Arc::new(RwLock::new(None::<String>));
        let stream = match sample_format {
            SampleFormat::F32 => build_stream::<f32>(&device, &config, state, last_error.clone())?,
            SampleFormat::I16 => build_stream::<i16>(&device, &config, state, last_error.clone())?,
            SampleFormat::U16 => build_stream::<u16>(&device, &config, state, last_error.clone())?,
            other => return Err(format!("unsupported sample format: {other:?}")),
        };
        stream.play().map_err(|e| format!("stream.play(): {e}"))?;

        Ok(Self {
            stream,
            device_name,
            sample_rate: config.sample_rate,
            channels: config.channels,
            sample_format,
            last_error,
        })
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    state: Arc<SharedState>,
    last_error: Arc<RwLock<Option<String>>>,
) -> Result<Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    let mut renderer = Renderer::new(config.sample_rate as f32);
    let err_fn = move |err: cpal::Error| {
        *last_error.write().unwrap_or_else(|e| e.into_inner()) = Some(err.to_string());
    };
    device
        .build_output_stream(
            *config,
            move |data: &mut [T], _| render(data, channels, &mut renderer, &state),
            err_fn,
            None,
        )
        .map_err(|e| format!("build_output_stream: {e}"))
}

fn render<T>(out: &mut [T], channels: usize, renderer: &mut Renderer, state: &SharedState)
where
    T: SizedSample + FromSample<f32>,
{
    if channels == 0 || out.is_empty() {
        return;
    }
    let params = state.snapshot();
    renderer.sync(&params);

    let mut written = 0;
    while written < out.len() {
        let sample = renderer.next_sample();
        let end = (written + channels).min(out.len());
        for word in &mut out[written..end] {
            *word = sample.to_sample::<T>();
        }
        written += channels;
    }

    let (rms, peak) = renderer.take_levels();
    state.set_meter(rms, peak);
}
