use anyhow::Result;
use mouse_keyboard_input::{
    key_codes::{
        EV_KEY, EV_SYN, KEY_1, KEY_10, KEY_2, KEY_3, KEY_4, KEY_5, KEY_6, KEY_7, KEY_8, KEY_9,
        KEY_A, KEY_APOSTROPHE, KEY_B, KEY_C, KEY_COMMA, KEY_D, KEY_DOT, KEY_E, KEY_ENTER, KEY_F,
        KEY_G, KEY_H, KEY_I, KEY_J, KEY_K, KEY_L, KEY_LEFTSHIFT, KEY_M, KEY_N, KEY_O, KEY_P, KEY_Q,
        KEY_R, KEY_S, KEY_SLASH, KEY_SPACE, KEY_T, KEY_U, KEY_V, KEY_W, KEY_X, KEY_Y, KEY_Z,
        SYN_REPORT,
    },
    VirtualDevice,
};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

pub struct VirtualKeyboard {
    device: VirtualDevice,
}

impl VirtualKeyboard {
    pub fn new() -> Result<Self> {
        info!("Creating VirtualKeyboard for Wayland");
        let device = VirtualDevice::new(std::time::Duration::from_millis(1), 100).map_err(|e| {
            anyhow::anyhow!(
                "Failed to create virtual device: {}. Are you running with CAP_SYS_INPUT?",
                e
            )
        })?;

        info!("VirtualKeyboard created successfully");
        Ok(Self { device })
    }

    pub fn type_text(&mut self, text: &str) -> Result<()> {
        info!("Typing text: '{}'", text);

        for ch in text.chars() {
            match self.char_to_events(ch) {
                Ok(events) => {
                    for (kind, code, value) in events {
                        if let Err(e) = self.device.sender.send((kind, code, value)) {
                            warn!("Failed to send key event: {}", e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Cannot type character '{}': {}", ch, e);
                }
            }
        }

        info!("Successfully typed {} characters", text.chars().count());
        Ok(())
    }

    fn char_to_events(&self, ch: char) -> Result<Vec<(u16, u16, i32)>> {
        let mut events = Vec::new();

        match ch {
            'a'..='z' | 'A'..='Z' => {
                let is_shifted = ch.is_uppercase();
                let keycode = self.letter_to_keycode(ch.to_ascii_lowercase());
                if is_shifted {
                    events.push((EV_KEY, KEY_LEFTSHIFT, 1));
                }
                events.push((EV_KEY, keycode, 1));
                events.push((EV_KEY, keycode, 0));
                if is_shifted {
                    events.push((EV_KEY, KEY_LEFTSHIFT, 0));
                }
            }
            '0'..='9' => {
                let keycode = self.digit_to_keycode(ch);
                events.push((EV_KEY, keycode, 1));
                events.push((EV_KEY, keycode, 0));
            }
            ' ' => {
                events.push((EV_KEY, KEY_SPACE, 1));
                events.push((EV_KEY, KEY_SPACE, 0));
            }
            '.' => {
                events.push((EV_KEY, KEY_DOT, 1));
                events.push((EV_KEY, KEY_DOT, 0));
            }
            ',' => {
                events.push((EV_KEY, KEY_COMMA, 1));
                events.push((EV_KEY, KEY_COMMA, 0));
            }
            '!' => {
                events.push((EV_KEY, KEY_LEFTSHIFT, 1));
                events.push((EV_KEY, KEY_1, 1));
                events.push((EV_KEY, KEY_1, 0));
                events.push((EV_KEY, KEY_LEFTSHIFT, 0));
            }
            '?' => {
                events.push((EV_KEY, KEY_LEFTSHIFT, 1));
                events.push((EV_KEY, KEY_SLASH, 1));
                events.push((EV_KEY, KEY_SLASH, 0));
                events.push((EV_KEY, KEY_LEFTSHIFT, 0));
            }
            '\n' => {
                events.push((EV_KEY, KEY_ENTER, 1));
                events.push((EV_KEY, KEY_ENTER, 0));
            }
            '\'' => {
                events.push((EV_KEY, KEY_APOSTROPHE, 1));
                events.push((EV_KEY, KEY_APOSTROPHE, 0));
            }
            _ => {
                return Err(anyhow::anyhow!("Character '{}' not supported yet", ch));
            }
        }

        Ok(events)
    }

    fn letter_to_keycode(&self, c: char) -> u16 {
        match c {
            'a' => KEY_A,
            'b' => KEY_B,
            'c' => KEY_C,
            'd' => KEY_D,
            'e' => KEY_E,
            'f' => KEY_F,
            'g' => KEY_G,
            'h' => KEY_H,
            'i' => KEY_I,
            'j' => KEY_J,
            'k' => KEY_K,
            'l' => KEY_L,
            'm' => KEY_M,
            'n' => KEY_N,
            'o' => KEY_O,
            'p' => KEY_P,
            'q' => KEY_Q,
            'r' => KEY_R,
            's' => KEY_S,
            't' => KEY_T,
            'u' => KEY_U,
            'v' => KEY_V,
            'w' => KEY_W,
            'x' => KEY_X,
            'y' => KEY_Y,
            'z' => KEY_Z,
            _ => KEY_A,
        }
    }

    fn digit_to_keycode(&self, d: char) -> u16 {
        match d {
            '0' => KEY_10,
            '1' => KEY_1,
            '2' => KEY_2,
            '3' => KEY_3,
            '4' => KEY_4,
            '5' => KEY_5,
            '6' => KEY_6,
            '7' => KEY_7,
            '8' => KEY_8,
            '9' => KEY_9,
            _ => KEY_10,
        }
    }
}
