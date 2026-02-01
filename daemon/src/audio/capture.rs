use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;

pub struct AudioCapture {
    device: Option<Device>,
    stream: Option<Box<Stream>>,
    audio_tx: Option<Arc<broadcast::Sender<Vec<f32>>>>,
    is_running: Arc<AtomicBool>,
    sample_rate: u32,
    channels: u16,
}

impl AudioCapture {
    pub fn new(sample_rate: u32) -> Result<Self> {
        Self::new_with_channels(sample_rate, 1)
    }

    pub fn new_with_channels(sample_rate: u32, channels: u16) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No default input device found"))?;

        tracing::info!("Audio capture initialized with sample rate: {}Hz, channels: {}", sample_rate, channels);
        tracing::info!("Using input device: {}", device.name()?);

        Ok(Self {
            device: Some(device),
            stream: None,
            audio_tx: None,
            is_running: Arc::new(AtomicBool::new(false)),
            sample_rate,
            channels,
        })
    }

    pub fn start(&mut self, audio_tx: broadcast::Sender<Vec<f32>>) -> Result<()> {
        self.audio_tx = Some(Arc::new(audio_tx));
        self.is_running.store(true, Ordering::Release);

        let device = self
            .device
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No audio device available"))?;

        tracing::info!(
            "Configuring audio stream: {}Hz, {} channel(s)",
            self.sample_rate,
            self.channels
        );

        let supported_configs = device.supported_input_configs()?;
        let mut config: Option<StreamConfig> = None;

        for supported in supported_configs {
            tracing::debug!("Supported config: {:?}", supported);
            if supported.channels() == self.channels
                && supported.min_sample_rate().0 <= self.sample_rate
                && supported.max_sample_rate().0 >= self.sample_rate
            {
                config = Some(
                    supported
                        .with_sample_rate(cpal::SampleRate(self.sample_rate))
                        .into(),
                );
                break;
            }
        }

        let final_config =
            config.ok_or_else(|| anyhow::anyhow!("No suitable audio configuration found"))?;

        let audio_tx = self.audio_tx.as_ref().map(Arc::clone);
        let is_running = Arc::clone(&self.is_running);

        let error_callback = |err| {
            tracing::error!("Audio stream error: {}", err);
        };

        let sample_format = device
            .default_input_config()
            .map(|c| c.sample_format())
            .unwrap_or(SampleFormat::F32);

        let stream: Box<Stream> = match sample_format {
            SampleFormat::F32 => {
                let stream = device.build_input_stream(
                    &final_config,
                    move |data: &[f32], _: &_| {
                        Self::process_audio_chunk(data, audio_tx.as_deref(), &is_running);
                    },
                    error_callback,
                    None,
                )?;
                Box::new(stream)
            }
            SampleFormat::I16 => {
                let stream = device.build_input_stream(
                    &final_config,
                    move |data: &[i16], _: &_| {
                        let converted: Vec<f32> =
                            data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                        Self::process_audio_chunk(&converted, audio_tx.as_deref(), &is_running);
                    },
                    error_callback,
                    None,
                )?;
                Box::new(stream)
            }
            SampleFormat::U16 => {
                let stream = device.build_input_stream(
                    &final_config,
                    move |data: &[u16], _: &_| {
                        let converted: Vec<f32> = data
                            .iter()
                            .map(|&s| (s as i16 as f32) / i16::MAX as f32)
                            .collect();
                        Self::process_audio_chunk(&converted, audio_tx.as_deref(), &is_running);
                    },
                    error_callback,
                    None,
                )?;
                Box::new(stream)
            }
            format => {
                return Err(anyhow::anyhow!("Unsupported sample format: {:?}", format));
            }
        };

        stream.play()?;
        self.stream = Some(stream);

        tracing::info!("Audio capture started");
        Ok(())
    }

    fn process_audio_chunk(
        data: &[f32],
        audio_tx: Option<&broadcast::Sender<Vec<f32>>>,
        is_running: &Arc<AtomicBool>,
    ) {
        if is_running.load(Ordering::Acquire) {
            if let Some(sender) = audio_tx {
                let _ = sender.send(data.to_vec());
            }
        }
    }

    pub async fn stop(&mut self) -> anyhow::Result<()> {
        self.is_running.store(false, Ordering::Release);
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }
        self.audio_tx = None;

        tracing::info!("Audio capture stopped");
        Ok(())
    }
}

unsafe impl Send for AudioCapture {}
