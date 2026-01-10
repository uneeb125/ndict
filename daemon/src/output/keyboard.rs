use anyhow::Result;
use tracing::info;
use wrtype::WrtypeClient;

pub struct VirtualKeyboard {
    client: WrtypeClient,
}

impl VirtualKeyboard {
    pub fn new() -> Result<Self> {
        info!("Creating VirtualKeyboard using wrtype");

        // Initialize the Wayland virtual keyboard client
        let client = WrtypeClient::new()
            .map_err(|e| anyhow::anyhow!("Failed to create WrtypeClient: {:?}", e))?;

        info!("VirtualKeyboard created successfully");
        Ok(Self { client })
    }

    pub fn type_text(&mut self, text: &str) -> Result<()> {
        info!("Typing text: '{}'", text);

        // wrtype handles the string parsing and keypress generation internally
        match self.client.type_text(text) {
            Ok(_) => {
                info!("Successfully typed {} characters", text.chars().count());
                Ok(())
            }
            Err(e) => {
                // Log the specific error from wrtype
                info!("Error: {:?}", e);
                Err(anyhow::anyhow!("Failed to type text: {:?}", e))
            }
        }
    }
}
