pub mod engine;


pub fn post_process_transcription(text: &str) -> String {
    let mut text = text.trim().to_string();

    let words: Vec<&str> = text.split_whitespace().collect();
    let mut deduped_words = Vec::new();
    for word in words {
        if !deduped_words.last().map_or(false, |last| *last == word) {
            deduped_words.push(word);
        }
    }
    text = deduped_words.join(" ");

    text = text.replace("  ", " ");
    text = text.trim().to_string();

    let re = regex::Regex::new(r"\[.*?\]|\{.*?\}|\(.*?\)").unwrap();
    text = re.replace_all(&text, "").to_string();
    text = text.replace("  ", " ");
    text = text.trim().to_string();

    tracing::debug!("Post-processed: '{}' -> '{}'", text.trim(), text);

    text
}
