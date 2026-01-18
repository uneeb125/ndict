mod common;

#[cfg(test)]
mod tests {
    use crate::common::{
        confirm_action, print_error, print_header, print_info, print_success, wait_for_user,
    };
    use ndictd::audio::capture::AudioCapture;
    use ndictd::vad::detector::VoiceActivityDetector;
    use std::io::{self, Write};
    use std::sync::Mutex;
    use tokio::sync::broadcast;

    // --- CONSTANTS (Initial Defaults) ---
    const DEFAULT_THRESHOLD_START: f32 = 0.02;
    const DEFAULT_THRESHOLD_STOP: f32 = 0.01;
    const DEFAULT_SILENCE_MS: u64 = 1000;
    const LOG_UPDATE_RATE_MS: u128 = 200; // Controls how often audio levels are printed

    // --- GLOBAL SHARED STATE ---
    struct TestConfig {
        threshold_start: f32,
        threshold_stop: f32,
        silence_duration_ms: u64,
    }

    // Thread-safe storage to persist values between tests
    static SHARED_CONFIG: Mutex<TestConfig> = Mutex::new(TestConfig {
        threshold_start: DEFAULT_THRESHOLD_START,
        threshold_stop: DEFAULT_THRESHOLD_STOP,
        silence_duration_ms: DEFAULT_SILENCE_MS,
    });

    // --- Helper Functions ---

    fn get_current_config() -> (f32, f32, u64) {
        let config = SHARED_CONFIG.lock().unwrap();
        (
            config.threshold_start,
            config.threshold_stop,
            config.silence_duration_ms,
        )
    }

    fn update_config(start: f32, stop: f32, silence: u64) {
        let mut config = SHARED_CONFIG.lock().unwrap();
        config.threshold_start = start;
        config.threshold_stop = stop;
        config.silence_duration_ms = silence;
    }

    fn prompt_f32(label: &str, default: f32) -> f32 {
        print!("> Enter {} (default: {}): ", label, default);
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();
        if input.is_empty() {
            default
        } else {
            input.parse::<f32>().unwrap_or_else(|_| {
                println!("Invalid number, using default.");
                default
            })
        }
    }

    fn prompt_u64(label: &str, default: u64) -> u64 {
        print!("> Enter {} (default: {}): ", label, default);
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();
        if input.is_empty() {
            default
        } else {
            input.parse::<u64>().unwrap_or_else(|_| {
                println!("Invalid number, using default.");
                default
            })
        }
    }

    fn prompt_rerun() -> bool {
        print!("\n> Do you want to adjust settings and RERUN this specific test? (y/n): ");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim().to_lowercase();
        input == "y" || input == "yes"
    }

    // --- Interactive Tests ---

    #[tokio::test]
    #[ignore = "Requires microphone and user interaction"]
    async fn test_01_vad_threshold_tuning() {
        print_header("Step 1: VAD Threshold Tuning");
        print_info("This tool helps you find the 'Sweet Spot' for your microphone.");

        if !confirm_action("Ready to start threshold tuning? (y/n)") {
            return;
        }

        // Load defaults from Global State (or Constants if first run)
        let (mut current_start, mut current_stop, current_silence) = get_current_config();

        loop {
            println!("\n==========================================");
            println!("CONFIGURATION FOR THIS RUN");
            println!("==========================================");
            
            // Interactive Configuration
            current_start = prompt_f32("threshold_start (Sensitivity)", current_start);
            current_stop = prompt_f32("threshold_stop (Silence detection)", current_stop);

            if current_stop >= current_start {
                print_error("WARNING: threshold_stop should usually be lower than threshold_start.");
            }

            println!("\n--- Collecting 3 samples ---");
            
            for sample_num in 1..=3 {
                println!("\nSample {} of 3:", sample_num);
                wait_for_user("Press Enter and speak, then remain silent...");

                // Slight delay to let keyboard noise settle
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                let (tx, mut rx) = broadcast::channel(100);
                let mut capture = AudioCapture::new().expect("Failed audio capture");
                capture.start(tx).expect("Failed start capture");

                let mut vad = VoiceActivityDetector::new(current_start, current_stop)
                    .expect("Failed VAD init");

                let collect_task = tokio::spawn(async move {
                    let mut max_level = 0.0;
                    let mut speech_frames = 0;
                    let mut total_frames = 0;
                    let mut is_speaking = false;
                    let mut last_output = std::time::Instant::now();

                    // Run for 3 seconds
                    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(3));
                    tokio::pin!(timeout);

                    loop {
                        tokio::select! {
                            res = rx.recv() => {
                                if let Ok(chunk) = res {
                                    let level = vad.calculate_audio_level(&chunk);
                                    if level > max_level { max_level = level; }

                                    // Print live feedback based on LOG_UPDATE_RATE_MS
                                    if last_output.elapsed().as_millis() > LOG_UPDATE_RATE_MS {
                                        println!("   -> Level: {:.5} | Speech: {}", level, if is_speaking { "YES" } else { "NO" });
                                        last_output = std::time::Instant::now();
                                    }

                                    let res = vad.detect(level, is_speaking);
                                    is_speaking = res.is_speech;
                                    
                                    if is_speaking { speech_frames += 1; }
                                    total_frames += 1;
                                } else { break; }
                            }
                            _ = &mut timeout => break,
                        }
                    }
                    (max_level, speech_frames, total_frames)
                });

                let (max_lvl, speech, total) = collect_task.await.unwrap();
                capture.stop().await.unwrap();

                // Analysis of this sample
                print_info(&format!("   Peak Level: {:.5}", max_lvl));
                if speech > 0 {
                    print_success(&format!("   Speech Detected: {:.1}% of time", (speech as f32 / total as f32) * 100.0));
                } else {
                    print_error("   No speech detected.");
                    if max_lvl < current_start {
                        print_info("   Reason: Peak level was below threshold_start.");
                    }
                }
            }

            // End of run interaction
            if !prompt_rerun() {
                // Save the successful values to Global State for next tests
                update_config(current_start, current_stop, current_silence);
                print_success("Settings saved for subsequent tests.");
                break;
            }
        }
    }

    #[tokio::test]
    #[ignore = "Requires microphone and user interaction"]
    async fn test_02_vad_hysteresis_behavior() {
        print_header("Step 2: VAD Hysteresis Verification");
        print_info("Verifies that audio hovering between thresholds doesn't cause rapid toggling.");

        if !confirm_action("Ready? (y/n)") { return; }

        // Grab defaults from Step 1
        let (mut current_start, mut current_stop, current_silence) = get_current_config();

        loop {
            println!("\n==========================================");
            println!("CONFIGURATION FOR THIS RUN");
            println!("==========================================");
            
            current_start = prompt_f32("threshold_start", current_start);
            current_stop = prompt_f32("threshold_stop", current_stop);

            wait_for_user("Press Enter and make a CONSTANT noise (humming/fan)...");
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            let (tx, mut rx) = broadcast::channel(100);
            let mut capture = AudioCapture::new().expect("Failed capture");
            capture.start(tx).expect("Failed start");

            let mut vad = VoiceActivityDetector::new(current_start, current_stop).unwrap();
            
            let duration = tokio::time::Duration::from_secs(5);
            
            let analysis_task = tokio::spawn(async move {
                let mut flips = 0;
                let mut speaking = false;
                let mut last_print = std::time::Instant::now();
                
                let timeout = tokio::time::sleep(duration);
                tokio::pin!(timeout);

                loop {
                    tokio::select! {
                        res = rx.recv() => {
                            if let Ok(data) = res {
                                let level = vad.calculate_audio_level(&data);
                                let result = vad.detect(level, speaking);
                                
                                if last_print.elapsed().as_millis() > LOG_UPDATE_RATE_MS {
                                    println!("   -> Level: {:.5} | State: {}", level, if result.is_speech {"ON"} else {"OFF"});
                                    last_print = std::time::Instant::now();
                                }

                                if result.is_speech != speaking {
                                    flips += 1;
                                    speaking = result.is_speech;
                                    println!("      [!FLIP!] State changed to {}", if speaking {"ON"} else {"OFF"});
                                }
                            } else { break; }
                        }
                        _ = &mut timeout => break,
                    }
                }
                flips
            });

            let flips = analysis_task.await.unwrap();
            capture.stop().await.unwrap();

            println!("\n------------------------------------------");
            println!("RESULTS: {} state changes in 5 seconds", flips);
            
            if flips < 10 {
                print_success("STABLE. Hysteresis is working well.");
            } else {
                print_error("UNSTABLE. Too much flickering.");
                print_info("Suggestion: Increase the gap between start and stop thresholds.");
            }

            if !prompt_rerun() { 
                update_config(current_start, current_stop, current_silence);
                break; 
            }
        }
    }

    #[tokio::test]
    #[ignore = "Requires microphone and user interaction"]
    async fn test_03_vad_silence_duration() {
        print_header("Step 3: Silence Duration (Trailing) Test");
        print_info("Tests how long the VAD 'holds' ON state after you stop speaking.");

        if !confirm_action("Ready? (y/n)") { return; }

        // Grab defaults from previous steps
        let (mut current_start, mut current_stop, mut target_silence_ms) = get_current_config();

        loop {
            println!("\n==========================================");
            println!("CONFIGURATION FOR THIS RUN");
            println!("==========================================");

            current_start = prompt_f32("threshold_start", current_start);
            current_stop = prompt_f32("threshold_stop", current_stop);
            target_silence_ms = prompt_u64("Target Silence Duration (ms)", target_silence_ms);

            println!("\nINSTRUCTIONS:");
            println!("1. Recording starts.");
            println!("2. Say 'TEST'.");
            println!("3. Immediately go SILENT.");
            
            wait_for_user("Press Enter to start...");
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            let (tx, mut rx) = broadcast::channel(100);
            let mut capture = AudioCapture::new().expect("Failed capture");
            capture.start(tx).expect("Failed start");

            let mut vad = VoiceActivityDetector::new(current_start, current_stop).unwrap();
            let silence_target = target_silence_ms as u128;

            let task = tokio::spawn(async move {
                let mut speaking = false;
                let mut silence_start: Option<std::time::Instant> = None;
                let mut measured = None;
                let mut speech_seen = false;
                let mut last_print = std::time::Instant::now();

                let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(6));
                tokio::pin!(timeout);

                loop {
                    tokio::select! {
                        res = rx.recv() => {
                            if let Ok(data) = res {
                                let level = vad.calculate_audio_level(&data);
                                let res = vad.detect(level, speaking);
                                
                                if last_print.elapsed().as_millis() > LOG_UPDATE_RATE_MS {
                                     println!("   -> Level: {:.5} | State: {}", level, if res.is_speech {"ON"} else {"OFF"});
                                     last_print = std::time::Instant::now();
                                }

                                if res.is_speech {
                                    speaking = true;
                                    speech_seen = true;
                                    silence_start = None; // Reset silence
                                } else if speaking {
                                    // We were speaking, now raw VAD says silent. 
                                    // Start counting silence duration.
                                    let now = std::time::Instant::now();
                                    if silence_start.is_none() {
                                        silence_start = Some(now);
                                    }
                                    
                                    let elapsed = silence_start.unwrap().elapsed().as_millis();
                                    if elapsed >= silence_target {
                                        measured = Some(elapsed);
                                        break; 
                                    }
                                }
                            } else { break; }
                        }
                        _ = &mut timeout => break,
                    }
                }
                (speech_seen, measured)
            });

            let (speech_seen, measured) = task.await.unwrap();
            capture.stop().await.unwrap();

            println!("\n------------------------------------------");
            if !speech_seen {
                print_error("No speech detected. Threshold start too high?");
            } else if let Some(ms) = measured {
                print_success(&format!("Silence threshold triggered after: ~{}ms", ms));
                println!("(This includes processing latency overhead)");
            } else {
                print_error("Timeout. Silence threshold never triggered.");
                print_info("Maybe background noise is keeping VAD active (level > threshold_stop)?");
            }

            if !prompt_rerun() { 
                update_config(current_start, current_stop, target_silence_ms);
                break; 
            }
        }
    }
}
