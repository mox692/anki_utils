use csv::{Reader, Writer};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::Duration;

const MODEL: &str = "gemma3:12b";
const OLLAMA_URL: &str = "http://localhost:11434/api/chat";
const SYSTEM_PROMPT: &str = r#"
You are an English learning assistant.

Your task:
Create ONE short English sentence using the given English word.
Also create the Japanese translation of that English sentence.

Rules:
- The English sentence MUST contain the exact given word.
- The English output MUST be a full sentence, not a single word.
- The Japanese output MUST be a natural Japanese translation of the English sentence.
- Do NOT output definitions.
- Do NOT output only the word.
- Do NOT output explanations.
- Output JSON only.

Bad output:
{"japanese":"りんご","english":"Apple"}

Good output:
{"japanese":"私は朝食にりんごを食べました。","english":"I ate an apple for breakfast."}
"#;

#[derive(Debug, Serialize, Deserialize)]
struct FlashcardInput {
    word: String,
    meaning: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct FlashcardOutput {
    word: String,
    meaning: String,
    japanese: String,
    english: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SentenceResult {
    japanese: String,
    english: String,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct Options {
    temperature: f32,
    num_predict: u32,
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    format: String,
    options: Options,
}

#[derive(Deserialize)]
struct OllamaResponse {
    message: MessageContent,
}

#[derive(Deserialize)]
struct MessageContent {
    content: String,
}

fn is_valid_result(result: &SentenceResult, word: &str) -> bool {
    let japanese = result.japanese.trim();
    let english = result.english.trim();

    if japanese.is_empty() || english.is_empty() {
        return false;
    }

    if !english.to_lowercase().contains(&word.to_lowercase()) {
        return false;
    }

    if english.split_whitespace().count() < 5 {
        return false;
    }

    let english_cleaned = english.trim_end_matches(|c| c == '.' || c == '!' || c == '?');
    if english_cleaned.to_lowercase() == word.to_lowercase() {
        return false;
    }

    true
}

fn generate_sentence(
    client: &Client,
    word: &str,
    meaning: &str,
) -> Result<SentenceResult, Box<dyn Error>> {
    let user_prompt = format!(
        r#"
Given word: {}
Japanese meaning: {}

Create a flashcard example sentence.

Requirements:
- English sentence must be 10 to 20 words.
- English sentence must include the exact word: "{}"
- Japanese sentence must mean the same thing as the English sentence.
- Output valid JSON only.
- Use this exact schema:
{{
  "japanese": "...",
  "english": "..."
}}

Remember:
The English field must be a complete sentence.
"#,
        word, meaning, word
    );

    let payload = OllamaRequest {
        model: MODEL.to_string(),
        messages: vec![
            Message {
                role: "system".to_string(),
                content: SYSTEM_PROMPT.to_string(),
            },
            Message {
                role: "user".to_string(),
                content: user_prompt,
            },
        ],
        stream: false,
        format: "json".to_string(),
        options: Options {
            temperature: 0.2,
            num_predict: 100,
        },
    };

    for _ in 0..3 {
        let response = client
            .post(OLLAMA_URL)
            .json(&payload)
            .timeout(Duration::from_secs(120))
            .send()?;

        let ollama_response: OllamaResponse = response.json()?;
        let content = ollama_response.message.content.trim();

        let result = match serde_json::from_str::<SentenceResult>(content) {
            Ok(r) => r,
            Err(_) => SentenceResult {
                japanese: String::new(),
                english: content.to_string(),
            },
        };

        if is_valid_result(&result, word) {
            return Ok(result);
        }
    }

    Ok(SentenceResult {
        japanese: String::new(),
        english: String::new(),
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    let mut rows = Vec::new();

    let mut reader = Reader::from_path("flashcards.csv")?;

    for result in reader.deserialize() {
        let input: FlashcardInput = result?;
        let word = input.word.trim();
        let meaning = input.meaning.trim();

        let sentence_result = generate_sentence(&client, word, meaning)?;

        let output = FlashcardOutput {
            word: word.to_string(),
            meaning: meaning.to_string(),
            japanese: sentence_result.japanese.clone(),
            english: sentence_result.english.clone(),
        };

        println!("{} => {:?}", word, sentence_result);

        rows.push(output);
    }

    let mut writer = Writer::from_path("sentences.csv")?;

    for row in rows {
        writer.serialize(row)?;
    }

    writer.flush()?;

    Ok(())
}
