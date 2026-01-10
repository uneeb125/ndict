use anyhow::Result;
use tokio::sync::broadcast;

pub struct AudioCapture;

impl AudioCapture {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    pub fn start(&mut self, audio_tx: broadcast::Sender<Vec<f32>>) -> Result<()> {
        tokio::spawn(async move {
            let mut sample_counter = 0u32;
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(16)).await;

                sample_counter += 1;

                let samples_per_chunk = 256usize;
                let samples: Vec<f32> = (0..samples_per_chunk)
                    .map(|_| if sample_counter % 200 > 100 { 0.1 } else { 0.0 })
                    .collect();

                let _ = audio_tx.send(samples);
            }
        });

        tracing::info!("Audio capture started");
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        tracing::info!("Audio capture stopped");
        Ok(())
    }
}
