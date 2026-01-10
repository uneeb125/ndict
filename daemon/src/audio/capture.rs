use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Host, SampleFormat, Stream, StreamConfig};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

const SAMPLE_RATE: u32 = 16000;
const CHANNELS: u16 = 1;

pub struct AudioCapture {
    host: Host,
    device: Option<Device>,
    stream: Option<Box<Stream>>,
    audio_tx: Arc<Mutex<Option<broadcast::Sender<Vec<f32>>>>>,
    is_running: Arc<Mutex<bool>>,
}

impl AudioCapture {
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No default input device found"))?;

        tracing::info!("Audio capture initialized");
        tracing::info!("Using input device: {}", device.name()?);

        Ok(Self {
            host,
            device: Some(device),
            stream: None,
            audio_tx: Arc::new(Mutex::new(None)),
            is_running: Arc::new(Mutex::new(false)),
        })
    }

    pub fn start(&mut self, audio_tx: broadcast::Sender<Vec<f32>>) -> Result<()> {
        *self.audio_tx.lock().unwrap() = Some(audio_tx);
        *self.is_running.lock().unwrap() = true;

        let device = self
            .device
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No audio device available"))?;

        tracing::info!(
            "Configuring audio stream: {}Hz, {} channel(s)",
            SAMPLE_RATE,
            CHANNELS
        );

        let supported_configs = device.supported_input_configs()?;
        let mut config: Option<StreamConfig> = None;

        for supported in supported_configs {
            tracing::debug!("Supported config: {:?}", supported);
            if supported.channels() == CHANNELS
                && supported.min_sample_rate().0 <= SAMPLE_RATE
                && supported.max_sample_rate().0 >= SAMPLE_RATE
            {
                config = Some(
                    supported
                        .with_sample_rate(cpal::SampleRate(SAMPLE_RATE))
                        .into(),
                );
                break;
            }
        }

        let final_config =
            config.ok_or_else(|| anyhow::anyhow!("No suitable audio configuration found"))?;

        let audio_tx = Arc::clone(&self.audio_tx);
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
                        Self::process_audio_chunk(data, &audio_tx, &is_running);
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
                        Self::process_audio_chunk(&converted, &audio_tx, &is_running);
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
                        Self::process_audio_chunk(&converted, &audio_tx, &is_running);
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
        audio_tx: &Arc<Mutex<Option<broadcast::Sender<Vec<f32>>>>>,
        is_running: &Arc<Mutex<bool>>,
    ) {
        if is_running.try_lock().map(|g| *g).unwrap_or(false) {
            if let Ok(tx) = audio_tx.try_lock() {
                if let Some(sender) = tx.as_ref() {
                    let _ = sender.send(data.to_vec());
                }
            }
        }
    }

    pub async fn stop(&mut self) -> anyhow::Result<()> {
        *self.is_running.lock().unwrap() = false;
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }
        *self.audio_tx.lock().unwrap() = None;

        tracing::info!("Audio capture stopped");
        Ok(())
    }
}

unsafe impl Send for AudioCapture {}
