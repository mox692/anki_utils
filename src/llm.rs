use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::error::Error;

const OLLAMA_URL: &str = "http://localhost:11434/api/chat";
const OLLAMA_MODEL: &str = "gemma3:12b";

#[derive(Debug, Serialize, Deserialize)]
pub struct SentenceResult {
    pub japanese: String,
    pub english: String,
}

pub fn generate_sentence(
    client: &Client,
    word: &str,
    meaning: &str,
) -> Result<SentenceResult, Box<dyn Error>> {
    let clean_word = crate::utils::clean_html_entities(word);

    let system_prompt = r#"
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

    let user_prompt = format!(
        r#"
Given word: {}
Japanese meaning: {}

Create a flashcard example sentence.

Requirements:
- English sentence must be 7 to 20 words.
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
        clean_word, meaning, clean_word
    );

    let payload = json!({
        "model": OLLAMA_MODEL,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt},
        ],
        "stream": false,
        "format": "json",
        "options": {
            "temperature": 0.2,
            "num_predict": 100
        }
    });

    for attempt in 0..3 {
        let response = client.post(OLLAMA_URL).json(&payload).send()?;
        let json: Value = response.json()?;

        if let Some(content) = json["message"]["content"].as_str() {
            println!(
                "    Debug - LLM response (attempt {}): {}",
                attempt + 1,
                content
            );
            if let Ok(result) = serde_json::from_str::<SentenceResult>(content) {
                println!(
                    "    Debug - Parsed: japanese={}, english={}",
                    result.japanese, result.english
                );

                if !result.japanese.is_empty() && !result.english.is_empty() {
                    return Ok(result);
                }
            } else {
                println!("    Debug - Failed to parse JSON");
            }
        }
    }

    Ok(SentenceResult {
        japanese: String::new(),
        english: String::new(),
    })
}
