mod common;

#[cfg(test)]
mod tests {
    use crate::common::confirm_action;
    use crate::common::print_error;
    use crate::common::print_header;
    use crate::common::print_info;
    use crate::common::print_success;
    use ndictd::audio::capture::AudioCapture;
    use ndictd::vad::detector::VoiceActivityDetector;
    use ndictd::vad::speech_detector::SpeechDetector;
    use tokio::sync::broadcast;
    #[tokio::test]
    #[ignore = "Requires microphone and user interaction"]
    async fn test_vad_threshold_tuning() {
        print_header("VAD Threshold Tuning");

        print_info("This test helps you find optimal VAD thresholds for your environment.");
        print_info("threshold_start: Audio level to start recording (default: 0.02)");
        print_info("threshold_stop: Audio level to stop recording (default: 0.01)");
        print_info("  - Higher = more sensitive, may catch background noise");
        print_info("  - Lower = less sensitive, may miss quiet speech");

        if !confirm_action("Ready to start threshold tuning? (y/n)") {
            return;
        }

        print_info("Collecting 3 samples at current settings...");

        let threshold_start = 0.02;
        let threshold_stop = 0.01;
        let silence_duration_ms = 1000;

        for sample_num in 1..=3 {
            print_info(&format!("Sample {} of 3", sample_num));

            crate::common::wait_for_user("Press Enter and speak, then remain silent...");

            let (tx, mut rx): (broadcast::Sender<Vec<f32>>, broadcast::Receiver<Vec<f32>>) =
                broadcast::channel(100);
            let mut capture = AudioCapture::new().expect("Failed to create audio capture");
            capture.start(tx).expect("Failed to start audio capture");

            let vad = VoiceActivityDetector::new(threshold_start, threshold_stop)
                .expect("Failed to create VAD detector");
            let speech_detector =
                SpeechDetector::new(threshold_start, threshold_stop, silence_duration_ms, 1.0)
                    .expect("Failed to create speech detector");

            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

            capture.stop().await.expect("Failed to stop audio capture");

            let detected_speech = tokio::select! {
                _ = rx.recv() => true,
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => false,
            };

            if detected_speech {
                print_success(&format!("Sample {}: Speech detected", sample_num));
            } else {
                print_error(&format!("Sample {}: No speech detected", sample_num));
            }
        }

        print_info("If speech was not detected consistently, try:");
        print_info("  - Speaking louder");
        print_info("  - Moving closer to microphone");
        print_info(&format!(
            "  - Adjusting threshold_start in config (current: {})",
            threshold_start,
        ));

        print_info("If speech was detected in silence, try:");
        print_info("  - Reducing background noise");
        print_info(&format!(
            "  - Adjusting threshold_start in config (current: {})",
            threshold_start,
        ));
        print_info(&format!(
            "  - Threshold should be 50-80% of threshold_start: {}",
            threshold_stop,
        ));

        print_info("If speech was detected in silence, try:");
        print_info("  - Reducing background noise");
        print_info(&format!(
            "  - Adjusting threshold_start in config (current: {})",
            threshold_start,
        ));
        print_info(&format!(
            "  - Threshold should be 50-80% of threshold_start: {}",
            threshold_stop,
        ));
    }

    #[tokio::test]
    #[ignore = "Requires microphone and user interaction"]
    async fn test_vad_hysteresis_behavior() {
        print_header("VAD Hysteresis Verification");

        print_info("This test verifies hysteresis prevents rapid toggling.");
        print_info("Audio levels between threshold_stop and threshold_start maintain state.");

        if !confirm_action("Ready to test hysteresis? (y/n)") {
            return;
        }

        let threshold_start = 0.03;
        let threshold_stop = 0.015;
        let silence_duration_ms = 1000;

        print_info(&format!(
            "Settings: threshold_start={}, threshold_stop={}",
            threshold_start, threshold_stop
        ));

        crate::common::wait_for_user("Press Enter and speak at medium volume...");

        let (tx, mut rx): (broadcast::Sender<Vec<f32>>, broadcast::Receiver<Vec<f32>>) =
            broadcast::channel(100);
        let mut capture = AudioCapture::new().expect("Failed to create audio capture");
        capture.start(tx).expect("Failed to start audio capture");

        let vad = VoiceActivityDetector::new(threshold_start, threshold_stop)
            .expect("Failed to create VAD detector");
        let speech_detector =
            SpeechDetector::new(threshold_start, threshold_stop, silence_duration_ms, 1.0)
                .expect("Failed to create speech detector");

        let mut state_changes = 0;
        let duration = tokio::time::Duration::from_secs(5);

        let capture_task = tokio::spawn(async move {
            loop {
                match tokio::time::timeout(tokio::time::Duration::from_millis(100), rx.recv()).await
                {
                    Ok(Ok(_)) => {
                        state_changes += 1;
                    }
                    Ok(Err(_)) | Err(_) => {
                        break;
                    }
                }
            }
        });

        tokio::time::sleep(duration).await;

        capture.stop().await.expect("Failed to stop audio capture");

        let _ = capture_task.await;

        if state_changes < 10 {
            print_success(&format!(
                "Good hysteresis: Only {} state changes in 5s",
                state_changes
            ));
            print_info("Hysteresis prevents rapid toggling");
        } else {
            print_error(&format!("Too many state changes: {} in 5s", state_changes));
            print_info("This may indicate:");
            print_info("  - Background noise near threshold");
            print_info("  - Thresholds too close together");
            print_info("  - Try increasing hysteresis gap");
        }
    }

    #[tokio::test]
    #[ignore = "Requires microphone and user interaction"]
    async fn test_vad_silence_duration() {
        print_header("VAD Silence Duration Test");

        print_info("This test verifies silence duration detection.");
        print_info(&format!("Current min_silence_duration_ms: {}ms", 1000));

        if !confirm_action("Ready to test silence duration? (y/n)") {
            return;
        }

        print_info("Speak clearly for 2 seconds, then remain silent for 2 seconds...");

        let (tx, mut rx): (broadcast::Sender<Vec<f32>>, broadcast::Receiver<Vec<f32>>) =
            broadcast::channel(100);
        let mut capture = AudioCapture::new().expect("Failed to create audio capture");
        capture.start(tx).expect("Failed to start audio capture");

        let threshold_start = 0.02;
        let threshold_stop = 0.01;
        let silence_duration_ms = 1000;

        let vad = VoiceActivityDetector::new(threshold_start, threshold_stop)
            .expect("Failed to create VAD detector");
        let speech_detector =
            SpeechDetector::new(threshold_start, threshold_stop, silence_duration_ms, 1.0)
                .expect("Failed to create speech detector");

        let speech_end_time = tokio::spawn(async move {
            let mut speaking = false;
            let mut silence_start = None;
            let mut final_silence_duration = None;

            loop {
                match tokio::time::timeout(tokio::time::Duration::from_millis(100), rx.recv()).await
                {
                    Ok(Ok(_)) => {
                        let vad_result = vad.detect(vad.calculate_audio_level(&[]), speaking);
                        if vad_result.is_speech {
                            speaking = true;
                            silence_start = None;
                        } else if speaking && silence_start.is_none() {
                            silence_start = Some(std::time::Instant::now());
                        } else if speaking && silence_start.is_some() {
                            let elapsed = silence_start.unwrap().elapsed();
                            final_silence_duration = Some(elapsed.as_millis());
                            break;
                        }
                    }
                    Ok(Err(_)) | Err(_) => {
                        break;
                    }
                }
            }

            final_silence_duration
        });

        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;

        capture.stop().await.expect("Failed to stop audio capture");

        let detected_silence = speech_end_time.await;

        if let Ok(Some(duration)) = detected_silence {
            print_success(&format!("Detected silence duration: {}ms", duration));

            if duration >= 900 && duration <= 1100 {
                print_success("Silence duration within expected range (1000ms ±100ms)");
            } else {
                print_info(&format!(
                    "Silence duration differs from 1000ms threshold by {}ms",
                    (duration as i32 - 1000).abs()
                ));
            }
        } else {
            print_error("Could not determine silence duration");
        }
    }
}
