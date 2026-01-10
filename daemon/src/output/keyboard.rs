use anyhow::Result;
use enigo::{Enigo, Keyboard, Settings};
use tracing::info;

pub struct VirtualKeyboard {
    enigo: Enigo,
}

impl VirtualKeyboard {
    pub fn new() -> Result<Self> {
        info!("Creating VirtualKeyboard using enigo");
        let enigo = Enigo::new(&Settings::default())
            .map_err(|e| anyhow::anyhow!("Failed to create Enigo instance: {}", e))?;

        info!("VirtualKeyboard created successfully");
        Ok(Self { enigo })
    }

    pub fn type_text(&mut self, text: &str) -> Result<()> {
        info!("Typing text: '{}'", text);

        match self.enigo.text(text) {
            Ok(_) => {
                info!("Successfully typed {} characters", text.chars().count());
                Ok(())
            }
            Err(e) => {
                info!("Error: {:?}", e);
                Err(anyhow::anyhow!("Failed to type text: {:?}", e))
            }
        }
    }
}
