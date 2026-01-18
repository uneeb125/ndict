mod common;

// Changed to pub(crate) so it is visible to the child 'tests' module
pub(crate) fn calculate_rms_level(audio_data: &[f32]) -> f32 {
    if audio_data.is_empty() {
        return 0.0;
    }
    let sum: f32 = audio_data.iter().map(|&x| x * x).sum();
    (sum / audio_data.len() as f32).sqrt()
}

#[cfg(test)]
mod tests {
    use super::calculate_rms_level; // Correct import for parent module function
    use crate::common::confirm_action;
    use crate::common::print_error;
    use crate::common::print_header;
    use crate::common::print_info;
    use crate::common::print_success;
    use ndictd::audio::capture::AudioCapture;
    use tokio::sync::broadcast;
    use std::time::Duration;

    #[tokio::test]
    #[ignore = "Requires microphone and user interaction"]
    async fn test_microphone_silence_detection() {
        print_header("Microphone Silence Detection");
        print_info("This test verifies microphone can capture silence.");
        print_info("Please ensure your microphone is connected and the environment is quiet.");

        if !confirm_action("Ready to test silence detection? (y/n)") {
            print_info("Test skipped by user.");
            return;
        }

        tokio::time::sleep(Duration::from_millis(500)).await;

        print_info("Starting audio capture for 3 seconds...");
        print_info("Please remain silent.");

        let (tx, mut rx) = broadcast::channel(100);
        let mut capture = AudioCapture::new()
            .expect("Failed to create audio capture. Check permissions.");
        
        capture.start(tx).expect("Failed to start audio capture");

        let mut max_level_detected = 0.0;
        let mut audio_detected = false;
        let mut last_output = std::time::Instant::now();
        
        // Run capture loop for 3 seconds
        let _ = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Ok(audio_data) = rx.recv().await {
                    let level = calculate_rms_level(&audio_data);
                    
                    if level > max_level_detected {
                        max_level_detected = level;
                    }

                    // Log status every 500ms
                    if last_output.elapsed() >= Duration::from_millis(500) {
                        print_info(&format!("Current Level: {:.4}", level));
                        last_output = std::time::Instant::now();
                    }

                    // Threshold check (0.02 is arbitrary, adjust based on hardware)
                    if level > 0.02 {
                        audio_detected = true;
                    }
                }
            }
        }).await; // Timeout error is expected here, we just ignore it to stop the loop

        capture.stop().await.expect("Failed to stop capture");

        if audio_detected {
            print_error(&format!("FAILURE: Detected audio level {:.4} during silence", max_level_detected));
            print_info("Possible causes: Background noise, high gain, or faulty hardware.");
            panic!("Test failed: Silence verification failed.");
        } else {
            print_success(&format!("Success. Max level detected: {:.4}", max_level_detected));
        }
    }

    #[tokio::test]
    #[ignore = "Requires microphone and user interaction"]
    async fn test_microphone_speech_detection() {
        print_header("Microphone Speech Detection");
        print_info("This test verifies microphone can detect speech.");

        if !confirm_action("Ready to test speech detection? (y/n)") {
            print_info("Test skipped by user.");
            return;
        }

        tokio::time::sleep(Duration::from_millis(500)).await;

        print_info("Starting audio capture for 3 seconds...");
        print_info("Please speak clearly now.");

        let (tx, mut rx) = broadcast::channel(100);
        let mut capture = AudioCapture::new()
            .expect("Failed to create audio capture");
        
        capture.start(tx).expect("Failed to start capture");

        let mut speech_detected = false;
        let mut last_output = std::time::Instant::now();

        let _ = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Ok(audio_data) = rx.recv().await {
                    let level = calculate_rms_level(&audio_data);

                    if last_output.elapsed() >= Duration::from_millis(500) {
                        print_info(&format!("Current Level: {:.4}", level));
                        last_output = std::time::Instant::now();
                    }

                    if level > 0.02 {
                        speech_detected = true;
                    }
                }
            }
        }).await;

        capture.stop().await.expect("Failed to stop capture");

        if !speech_detected {
            print_error("FAILURE: No significant audio detected during speech.");
            print_info("Check microphone input volume or connection.");
            panic!("Test failed: Speech not detected.");
        } else {
            print_success("Speech detected successfully.");
        }
    }

    #[tokio::test]
    #[ignore = "Requires microphone and user interaction"]
    async fn test_microphone_continuous_capture() {
        print_header("Microphone Continuous Capture");
        print_info("This test verifies microphone can capture a stream of chunks.");

        if !confirm_action("Ready to test continuous capture? (y/n)") {
            return;
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
        print_info("Capturing for 5 seconds...");

        let (tx, mut rx) = broadcast::channel(100);
        let mut capture = AudioCapture::new()
            .expect("Failed to create capture");
        
        capture.start(tx).expect("Failed to start capture");

        let mut chunk_count = 0;
        let mut last_output = std::time::Instant::now();

        // Use timeout to run the loop for exactly 5 seconds
        // This avoids spawning separate threads and race conditions
        let _ = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Ok(audio_data) = rx.recv().await {
                    chunk_count += 1;
                    let level = calculate_rms_level(&audio_data);

                    if last_output.elapsed() >= Duration::from_millis(500) {
                        print_info(&format!("Chunk: {}, Level: {:.4}", chunk_count, level));
                        last_output = std::time::Instant::now();
                    }
                }
            }
        }).await;

        capture.stop().await.expect("Failed to stop capture");

        if chunk_count == 0 {
            print_error("FAILURE: No audio chunks received.");
            panic!("Test failed: Stream was empty.");
        } else {
            print_success(&format!("Success. Received {} chunks in 5 seconds.", chunk_count));
        }
    }
}
