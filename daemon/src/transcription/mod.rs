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

    let re = regex::Regex::new(r"\s+").unwrap();
    text = re.replace_all(&text, " ").trim().to_string();

    let re_brackets = regex::Regex::new(r"\[.*?\]|\{.*?\}|\(.*?\)").unwrap();
    text = re_brackets.replace_all(&text, "").to_string();

    let re_final = regex::Regex::new(r"\s+").unwrap();
    text = re_final.replace_all(&text, " ").trim().to_string();

    tracing::debug!("Post-processed: '{}' -> '{}'", text, text);

    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_post_process_empty_string() {
        let input = "";
        let output = post_process_transcription(input);
        assert_eq!(output, "");
    }

    #[test]
    fn test_post_process_simple_text() {
        let input = "hello world";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello world");
    }

    #[test]
    fn test_post_process_remove_duplicate_words() {
        let input = "hello hello world world test";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello world test");
    }

    #[test]
    fn test_post_process_remove_bracketed_square() {
        let input = "hello [world] test";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello test");
    }

    #[test]
    fn test_post_process_remove_bracketed_curly() {
        let input = "hello {world} test";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello test");
    }

    #[test]
    fn test_post_process_remove_bracketed_paren() {
        let input = "hello (world) test";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello test");
    }

    #[test]
    fn test_post_process_remove_multiple_bracket_types() {
        let input = "hello [one] {two} (three) test";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello test");
    }

    #[test]
    fn test_post_process_normalize_whitespace() {
        let input = "hello  world   test";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello world test");
    }

    #[test]
    fn test_post_process_trim_whitespace() {
        let input = "  hello world test  ";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello world test");
    }

    #[test]
    fn test_post_process_combined() {
        let input = "  hello hello [noise] world {test} (skip)  world  ";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello world world");
    }

    #[test]
    fn test_post_process_triple_spaces() {
        let input = "hello   world   test";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello world test");
    }

    #[test]
    fn test_post_process_only_brackets() {
        let input = "[hello]";
        let output = post_process_transcription(input);
        assert_eq!(output, "");
    }

    #[test]
    fn test_post_process_brackets_with_spaces() {
        let input = "hello [ world ] test";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello test");
    }

    #[test]
    fn test_post_process_realistic_whisper_output() {
        let input = " hello [laughs] world (um) [clears throat]  test  test  ";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello world test");
    }

    #[test]
    fn test_post_process_unicode_characters() {
        let input = "hello 世界 🌍 world";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello 世界 🌍 world");
    }

    #[test]
    fn test_post_process_numbers_and_punctuation() {
        let input = "hello,  world!  test. 123";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello, world! test. 123");
    }

    #[test]
    fn test_post_process_multiple_consecutive_duplicates() {
        let input = "hello hello hello world world world";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello world");
    }

    #[test]
    fn test_post_process_no_duplicates() {
        let input = "hello world test";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello world test");
    }

    #[test]
    fn test_post_process_single_word() {
        let input = "hello";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello");
    }
}
