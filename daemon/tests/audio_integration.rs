mod common;

#[cfg(test)]
mod tests {
    use crate::common::confirm_action;
    use crate::common::print_error;
    use crate::common::print_header;
    use crate::common::print_info;
    use crate::common::print_success;
    use ndictd::audio::capture::AudioCapture;
    use tokio::sync::broadcast;
    #[tokio::test]
    #[ignore = "Requires microphone and user interaction"]
    async fn test_microphone_silence_detection() {
        print_header("Microphone Silence Detection");

        print_info("This test verifies microphone can capture silence.");
        print_info("Please ensure your microphone is connected and the environment is quiet.");

        if !confirm_action("Ready to test silence detection? (y/n)") {
            return;
        }

        print_info("Starting audio capture for 3 seconds...");
        print_info("Please remain silent during this time.");

        let (tx, mut rx): (broadcast::Sender<Vec<f32>>, broadcast::Receiver<Vec<f32>>) =
            broadcast::channel(100);
        let mut capture = AudioCapture::new()
            .expect("Failed to create audio capture. Check microphone permissions.");
        capture.start(tx).expect("Failed to start audio capture");

        let mut received_audio = false;
        let duration = tokio::time::Duration::from_secs(3);

        tokio::select! {
            _ = rx.recv() => {
                received_audio = true;
            }
            _ = tokio::time::sleep(duration) => {}
        }

        capture.stop().await.expect("Failed to stop audio capture");

        if received_audio {
            print_error("Detected audio during silence period");
            print_info("Consider checking:");
            print_info("- Microphone connection");
            print_info("- Background noise levels");
            print_info("- VAD threshold_start setting (current: 0.02)");
        } else {
            print_success("Silence detected correctly");
        }
    }

    #[tokio::test]
    #[ignore = "Requires microphone and user interaction"]
    async fn test_microphone_speech_detection() {
        print_header("Microphone Speech Detection");

        print_info("This test verifies microphone can detect speech.");
        print_info("Please ensure your microphone is connected.");

        if !confirm_action("Ready to test speech detection? (y/n)") {
            return;
        }

        print_info("Starting audio capture for 3 seconds...");
        print_info("Please speak clearly during this time.");

        let (tx, mut rx): (broadcast::Sender<Vec<f32>>, broadcast::Receiver<Vec<f32>>) =
            broadcast::channel(100);
        let mut capture = AudioCapture::new()
            .expect("Failed to create audio capture. Check microphone permissions.");
        capture.start(tx).expect("Failed to start audio capture");

        let mut received_audio = false;
        let duration = tokio::time::Duration::from_secs(3);

        tokio::select! {
            _ = rx.recv() => {
                received_audio = true;
            }
            _ = tokio::time::sleep(duration) => {}
        }

        capture.stop().await.expect("Failed to stop audio capture");

        if !received_audio {
            print_error("No audio detected during speech period");
            print_info("Consider checking:");
            print_info("- Microphone connection");
            print_info("- Microphone volume levels");
            print_info("- VAD threshold_start setting (current: 0.02)");
        } else {
            print_success("Speech detected correctly");
        }
    }

    #[tokio::test]
    #[ignore = "Requires microphone and user interaction"]
    async fn test_microphone_continuous_capture() {
        print_header("Microphone Continuous Capture");

        print_info("This test verifies microphone can capture continuously.");

        if !confirm_action("Ready to test continuous capture? (y/n)") {
            return;
        }

        print_info("Starting audio capture for 5 seconds...");
        print_info("Audio levels will be monitored.");

        let (tx, mut rx): (broadcast::Sender<Vec<f32>>, broadcast::Receiver<Vec<f32>>) =
            broadcast::channel(100);
        let mut capture = AudioCapture::new()
            .expect("Failed to create audio capture. Check microphone permissions.");
        capture.start(tx).expect("Failed to start audio capture");

        let mut chunk_count = 0;
        let duration = tokio::time::Duration::from_secs(5);

        let capture_task = tokio::spawn(async move {
            let start = std::time::Instant::now();
            loop {
                match tokio::time::timeout(tokio::time::Duration::from_millis(100), rx.recv()).await
                {
                    Ok(Ok(_)) => {
                        chunk_count += 1;
                        let elapsed = start.elapsed().as_secs();
                        if elapsed >= 5 {
                            break;
                        }
                    }
                    Ok(Err(_)) | Err(_) => {
                        let elapsed = start.elapsed().as_secs();
                        if elapsed >= 5 {
                            break;
                        }
                    }
                }
            }
        });

        tokio::time::sleep(duration).await;

        capture.stop().await.expect("Failed to stop audio capture");

        let _ = capture_task.await;

        if chunk_count > 0 {
            print_success(&format!(
                "Received {} audio chunks in 5 seconds",
                chunk_count
            ));
        } else {
            print_error("No audio chunks received");
        }
    }
}
