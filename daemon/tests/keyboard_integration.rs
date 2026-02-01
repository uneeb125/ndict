mod common;

#[cfg(test)]
mod tests {
    use crate::common::confirm_action;
    use crate::common::print_error;
    use crate::common::print_header;
    use crate::common::print_info;
    use crate::common::print_success;
    use crate::common::wait_for_user;
    use ndictd::output::keyboard::VirtualKeyboard;
    #[tokio::test]
    #[ignore = "Requires Wayland display and active window"]
    async fn test_keyboard_typing_simple() {
        print_header("Virtual Keyboard Typing Test");

        print_info("This test verifies virtual keyboard can type to active Wayland window.");
        print_info("Prerequisites:");
        print_info("  - Running Wayland session");
        print_info("  - ndictd daemon has CAP_SYS_INPUT capability");
        print_info("  - An active text input window (terminal, editor, etc.)");

        if !confirm_action("Ready to test keyboard typing? (y/n)") {
            return;
        }

        print_info("Creating virtual keyboard...");

        let mut keyboard = VirtualKeyboard::new()
            .expect("Failed to create virtual keyboard. Check Wayland session.");

        let test_text = "Hello, ndict! This is a test message.";

        print_info(&format!("Typing: '{}'", test_text));
        print_info("Please ensure you have an active text input window focused.");

        wait_for_user("Press Enter to type the message...");

        let result = keyboard.type_text(test_text).await;

        match result {
            Ok(_) => {
                print_success("Message typed successfully");
                print_info("Please verify the text appeared in your active window.");
            }
            Err(e) => {
                print_error(&format!("Failed to type message: {}", e));
                print_info("Possible causes:");
                print_info("  - No active Wayland session");
                print_info("  - No focused text input window");
                print_info("  - Missing CAP_SYS_INPUT capability");
                panic!("Test failed: {}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore = "Requires Wayland display and active window"]
    async fn test_keyboard_special_characters() {
        print_header("Special Characters Typing Test");

        print_info("This test verifies special characters and symbols work correctly.");

        if !confirm_action("Ready to test special characters? (y/n)") {
            return;
        }

        let mut keyboard = VirtualKeyboard::new().expect("Failed to create virtual keyboard");

        let test_text = "Test: @#$%^&*()_+-=[]{}|\\:;\"'<>,.?/";

        print_info(&format!("Typing: '{}'", test_text));
        print_info("Please ensure you have an active text input window focused.");

        wait_for_user("Press Enter to type special characters...");

        let result = keyboard.type_text(test_text).await;

        match result {
            Ok(_) => {
                print_success("Special characters typed successfully");
                print_info("Please verify all characters appeared correctly.");
            }
            Err(e) => {
                print_error(&format!("Failed to type special characters: {}", e));
                panic!("Test failed: {}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore = "Requires Wayland display and active window"]
    async fn test_keyboard_unicode() {
        print_header("Unicode Characters Typing Test");

        print_info("This test verifies Unicode characters (non-ASCII) work correctly.");

        if !confirm_action("Ready to test Unicode characters? (y/n)") {
            return;
        }

        let mut keyboard = VirtualKeyboard::new().expect("Failed to create virtual keyboard");

        let test_text = "Unicode test: 你好世界 🌍 ñ é ü";

        print_info(&format!("Typing: '{}'", test_text));
        print_info("Please ensure you have an active text input window focused.");
        print_info("Note: Some applications may not support all Unicode characters.");

        wait_for_user("Press Enter to type Unicode characters...");

        let result = keyboard.type_text(test_text).await;

        match result {
            Ok(_) => {
                print_success("Unicode characters typed successfully");
                print_info("Please verify characters appeared correctly in your window.");
                print_info(
                    "Note: Missing or garbled characters may indicate application limitation.",
                );
            }
            Err(e) => {
                print_error(&format!("Failed to type Unicode characters: {}", e));
                panic!("Test failed: {}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore = "Requires Wayland display and active window"]
    async fn test_keyboard_typing_speed() {
        print_header("Keyboard Typing Speed Test");

        print_info("This test measures typing speed for a longer message.");

        if !confirm_action("Ready to test typing speed? (y/n)") {
            return;
        }

        let mut keyboard = VirtualKeyboard::new().expect("Failed to create virtual keyboard");

        let test_text = "This is a longer test message to measure how quickly the virtual keyboard can type text. It contains multiple words and sentences to simulate realistic usage. ";

        print_info(&format!("Typing {} characters...", test_text.len()));
        print_info("Please ensure you have an active text input window focused.");

        wait_for_user("Press Enter to start typing...");

        let start = std::time::Instant::now();
        let result = keyboard.type_text(test_text).await;
        let elapsed = start.elapsed();

        match result {
            Ok(_) => {
                let chars_per_second = test_text.len() as f64 / elapsed.as_secs_f64();
                print_success(&format!("Message typed in {:.2}s", elapsed.as_secs_f64()));
                print_info(&format!(
                    "Typing speed: {:.1} chars/second",
                    chars_per_second
                ));

                if chars_per_second > 10.0 {
                    print_success("Typing speed is good");
                } else if chars_per_second > 5.0 {
                    print_info("Typing speed is acceptable");
                } else {
                    print_info("Typing speed may be slow");
                }
            }
            Err(e) => {
                print_error(&format!("Failed to type message: {}", e));
                panic!("Test failed: {}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore = "Requires Wayland display and active window"]
    async fn test_keyboard_empty_text() {
        print_header("Empty Text Typing Test");

        print_info("This test verifies keyboard handles empty text gracefully.");

        if !confirm_action("Ready to test empty text? (y/n)") {
            return;
        }

        let mut keyboard = VirtualKeyboard::new().expect("Failed to create virtual keyboard");

        let test_text = "";

        print_info("Typing empty string...");

        let result = keyboard.type_text(test_text).await;

        match result {
            Ok(_) => {
                print_success("Empty text handled correctly (no error)");
            }
            Err(e) => {
                print_error(&format!("Failed with empty text: {}", e));
                panic!("Test failed: {}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore = "Requires Wayland display and active window"]
    async fn test_keyboard_very_long_text() {
        print_header("Very Long Text Typing Test");

        print_info("This test verifies keyboard can handle very long messages.");

        if !confirm_action("Ready to test very long text? (y/n)") {
            return;
        }

        let mut keyboard = VirtualKeyboard::new().expect("Failed to create virtual keyboard");

        let test_text = "A".repeat(500);

        print_info(&format!("Typing {} characters...", test_text.len()));
        print_info("Please ensure you have an active text input window focused.");
        print_info("This test uses 5 second timeout.");

        wait_for_user("Press Enter to start typing...");

        print_info(&format!("Typing {} characters with 5 second timeout...", test_text.len()));

        let start = std::time::Instant::now();

        let typing_result = tokio::time::timeout(
            tokio::time::Duration::from_secs(5),
            keyboard.type_text(&test_text),
        )
        .await;

        let elapsed = start.elapsed();

        match typing_result {
            Ok(Ok(_)) => {
                print_success(&format!(
                    "Very long text test completed in {:.2}s",
                    elapsed.as_secs_f64()
                ));
                print_info("Check if text appeared in your window.");
            }
            Ok(Err(e)) => {
                print_error(&format!("Failed to type very long text: {}", e));
                print_info("This may indicate:");
                print_info("  - Application limitations");
                print_info("  - Wayland connection issues");
                panic!("Test failed: {}", e);
            }
            Err(_) => {
                print_error(&format!("Timeout exceeded: {:.2}s", elapsed.as_secs_f64()));
                print_info("This is expected for very long text (500 characters)");
                print_info("The 5-second timeout is working as designed.");
                panic!("Test failed: Typing exceeded 5 second timeout");
            }
        }
    }
}
