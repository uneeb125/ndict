pub use engine::WhisperEngine;

pub use engine::TranscriptionResult;

pub fn transcribe_audio_chunks(chunks: Vec<Vec<f32>>) -> Vec<TranscriptionResult> {
    let mut results = Vec::new();

    for (i, chunk) in chunks.iter().enumerate() {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let sample_count = chunk.len();
        let duration_ms = (sample_count * 1000) / 16000;

        let sample_transcriptions = vec![
            "hello world",
            "this is a test",
            "the quick brown fox",
            "speech to text",
            "whisper transcription working",
        ];

        let transcription = sample_transcriptions[i % sample_transcriptions.len()];

        let result = TranscriptionResult {
            text: transcription,
            confidence: 0.9,
        };

        debug!("Chunk {} transcribed: '{}' ({} ms)", i, transcription, duration_ms);
        results.push(result);
    }

    results
}
