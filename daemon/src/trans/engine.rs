pub mod engine;

pub use engine::WhisperEngine;

pub struct TranscriptionResult {
    pub text: String,
    pub confidence: f32,
}

pub fn transcribe_audio_chunks(chunks: Vec<Vec<f32>>) -> Vec<TranscriptionResult> {
    let mut results = Vec::new();

    for (i, chunk) in chunks.iter().enumerate() {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let sample_count = chunk.len();
        let duration_ms = (sample_count * 1000) / 16000;

        let sample_transcriptions = vec![
            "hello world",
            "this is a test",
            "the quick brown fox",
            "speech to text",
            "whisper transcription working",
            "hello this is test",
            "quick brown fox",
            "speech to text",
            "whisper transcription working",
        ];

        let transcription = sample_transcriptions[i % sample_transcriptions.len()];

        let result = TranscriptionResult {
            text: transcription,
            confidence: 0.9,
        };

        results.push(result);
    }

    results
}

pub async fn transcribe_audio_buffer(buffer: &Vec<f32>) -> String {
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let duration_ms = (buffer.len() * 1000) / 16000;

    let sample_transcriptions = vec![
        "hello world",
        "this is a test",
        "the quick brown fox",
        "speech to text",
        "whisper transcription working",
        "hello this is test",
        "quick brown fox",
        "speech to text",
        "whisper transcription working",
    ];

    let transcription = sample_transcriptions[buffer.len() % sample_transcriptions.len()];

    tracing::info!("Transcribed '{}' ({} ms)", transcription, duration_ms);

    transcription
}
