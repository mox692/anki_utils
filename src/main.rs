use reqwest::blocking::Client;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::error::Error;

const ANKI_CONNECT_URL: &str = "http://localhost:8765";
const TARGET_DECK: &str = "youtube5";
const OLLAMA_URL: &str = "http://localhost:11434/api/chat";
const OLLAMA_MODEL: &str = "gemma3:12b";
const MASTERY_THRESHOLD: f64 = 30.0;

#[derive(Debug, Deserialize)]
struct AnkiResponse<T> {
    result: Option<T>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NoteInfo {
    #[serde(rename = "noteId")]
    note_id: i64,
    #[serde(default)]
    fields: HashMap<String, FieldValue>,
}

#[derive(Debug, Deserialize)]
struct FieldValue {
    value: String,
    order: i32,
}

#[derive(Debug, Deserialize)]
struct CardInfo {
    #[serde(rename = "cardId")]
    card_id: i64,
    note: i64,  // AnkiConnect returns "note" not "noteId"
    interval: i64,
    #[serde(default)]
    lapses: i64,
    #[serde(default)]
    reps: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct SentenceResult {
    japanese: String,
    english: String,
}

fn invoke<T>(client: &Client, action: &str, params: Value) -> Result<T, Box<dyn Error>>
where
    T: for<'de> Deserialize<'de>,
{
    let body = json!({
        "action": action,
        "version": 6,
        "params": params
    });

    let response: AnkiResponse<T> = client.post(ANKI_CONNECT_URL).json(&body).send()?.json()?;

    if let Some(error) = response.error {
        return Err(error.into());
    }

    response
        .result
        .ok_or_else(|| "AnkiConnect returned no result".into())
}

fn sync_anki(client: &Client) -> Result<(), Box<dyn Error>> {
    let body = json!({
        "action": "sync",
        "version": 6,
        "params": {}
    });

    let response: AnkiResponse<Value> = client.post(ANKI_CONNECT_URL).json(&body).send()?.json()?;

    if let Some(error) = response.error {
        return Err(error.into());
    }

    Ok(())
}

fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

fn calculate_mastery_score(interval: i64) -> f64 {
    if interval <= 0 {
        return 0.0;
    }
    let score = 100.0_f64 * ((interval as f64) + 1.0).ln() / (365.0_f64 + 1.0).ln();
    score.min(100.0)
}

fn clean_html_entities(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .trim()
        .to_string()
}

fn generate_sentence(
    client: &Client,
    word: &str,
    meaning: &str,
) -> Result<SentenceResult, Box<dyn Error>> {
    // Clean HTML entities from the word
    let clean_word = clean_html_entities(word);

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
            println!("    Debug - LLM response (attempt {}): {}", attempt + 1, content);
            if let Ok(result) = serde_json::from_str::<SentenceResult>(content) {
                let word_count = result.english.split_whitespace().count();
                let contains_word = result.english.to_lowercase().contains(&clean_word.to_lowercase());

                println!("    Debug - Parsed: japanese={}, english={}", result.japanese, result.english);
                println!("    Debug - Word count: {}, Contains word '{}': {}", word_count, clean_word, contains_word);

                if !result.japanese.is_empty()
                    && !result.english.is_empty()
                    && contains_word
                    && word_count >= 5
                {
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

fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting Anki sync process...");

    let client = Client::new();

    println!("Step 1: Syncing Anki...");
    sync_anki(&client)?;
    println!("Sync complete.");

    println!("\nStep 2: Setting up database...");
    let db = Connection::open("anki_mastery.db")?;

    db.execute(
        "CREATE TABLE IF NOT EXISTS flashcards (
            hash TEXT PRIMARY KEY,
            note_id INTEGER NOT NULL,
            word TEXT NOT NULL,
            meaning TEXT,
            mastery_score REAL,
            interval INTEGER,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    db.execute(
        "CREATE INDEX IF NOT EXISTS idx_flashcards_note_id ON flashcards(note_id)",
        [],
    )?;

    println!("Database setup complete.");

    println!("\nStep 3: Fetching notes from deck '{}'...", TARGET_DECK);
    let query = format!("deck:\"{}\"", TARGET_DECK);
    let note_ids: Vec<i64> = invoke(&client, "findNotes", json!({ "query": query }))?;
    println!("Found {} notes.", note_ids.len());

    if note_ids.is_empty() {
        println!("No notes found in deck. Exiting.");
        return Ok(());
    }

    let notes: Vec<NoteInfo> = invoke(&client, "notesInfo", json!({ "notes": note_ids }))?;

    println!("\nStep 4: Processing notes and updating database...");
    for note in &notes {
        // Debug: Print all field names
        if notes.iter().position(|n| n.note_id == note.note_id) == Some(0) {
            println!("Debug - Available field names: {:?}", note.fields.keys().collect::<Vec<_>>());
        }

        let word_raw = note
            .fields
            .get("単語")
            .or_else(|| note.fields.get("Front"))
            .or_else(|| note.fields.get("表面"))
            .map(|f| f.value.clone())
            .unwrap_or_default();

        let meaning_raw = note
            .fields
            .get("意味")
            .or_else(|| note.fields.get("Back"))
            .or_else(|| note.fields.get("裏面"))
            .map(|f| f.value.clone())
            .unwrap_or_default();

        // Clean HTML entities
        let word = clean_html_entities(&word_raw);
        let meaning = clean_html_entities(&meaning_raw);

        if word.is_empty() {
            println!("Debug - Skipping note {} due to empty word field", note.note_id);
            continue;
        }

        let hash = compute_hash(&format!("{}-{}", note.note_id, word));

        let exists: bool = db
            .query_row(
                "SELECT 1 FROM flashcards WHERE hash = ?",
                params![hash],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if !exists {
            db.execute(
                "INSERT INTO flashcards (hash, note_id, word, meaning) VALUES (?, ?, ?, ?)",
                params![hash, note.note_id, word.clone(), meaning],
            )?;
            println!("  Added: {} (note_id: {})", word, note.note_id);
        }
    }

    println!("\nStep 5: Calculating mastery scores...");
    let card_ids: Vec<i64> = invoke(&client, "findCards", json!({ "query": query }))?;
    let cards: Vec<CardInfo> = invoke(&client, "cardsInfo", json!({ "cards": card_ids }))?;

    let mut note_to_mastery: HashMap<i64, f64> = HashMap::new();
    for card in cards {
        let score = calculate_mastery_score(card.interval);
        note_to_mastery
            .entry(card.note)
            .and_modify(|s| *s = s.max(score))
            .or_insert(score);
    }

    for (note_id, score) in &note_to_mastery {
        db.execute(
            "UPDATE flashcards SET mastery_score = ?, updated_at = CURRENT_TIMESTAMP WHERE note_id = ?",
            params![*score, *note_id],
        )?;
    }

    println!("Mastery scores updated.");

    println!("\nStep 6: Generating sentences for low-mastery cards...");

    // Debug: Check all cards in database
    let total_cards: i64 = db.query_row("SELECT COUNT(*) FROM flashcards", [], |row| row.get(0))?;
    println!("Debug - Total cards in database: {}", total_cards);

    let mut debug_stmt = db.prepare("SELECT note_id, word, mastery_score FROM flashcards LIMIT 5")?;
    let debug_cards: Vec<(i64, String, Option<f64>)> = debug_stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    for (note_id, word, score) in debug_cards {
        println!("Debug - note_id: {}, word: {}, mastery_score: {:?}", note_id, word, score);
    }

    let mut stmt = db.prepare(
        "SELECT note_id, word, meaning, mastery_score FROM flashcards WHERE mastery_score IS NULL OR mastery_score < ?",
    )?;

    let low_mastery_cards: Vec<(i64, String, String, Option<f64>)> = stmt
        .query_map(params![MASTERY_THRESHOLD], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    println!(
        "Found {} cards below mastery threshold (< {}).",
        low_mastery_cards.len(),
        MASTERY_THRESHOLD
    );

    let output_deck = format!("{}-output", TARGET_DECK);

    let deck_names: Vec<String> = invoke(&client, "deckNames", json!({}))?;
    if !deck_names.contains(&output_deck) {
        println!("Creating output deck: {}", output_deck);
        invoke::<Value>(&client, "createDeck", json!({ "deck": output_deck }))?;
    }

    for (note_id, word, meaning, _score) in low_mastery_cards {
        println!("  Generating for: {} (note_id: {})", word, note_id);
        match generate_sentence(&client, &word, &meaning) {
            Ok(result) if !result.japanese.is_empty() && !result.english.is_empty() => {
                let note_params = json!({
                    "note": {
                        "deckName": output_deck,
                        "modelName": "Basic",
                        "fields": {
                            "Front": result.japanese,
                            "Back": result.english
                        },
                        "tags": ["generated"]
                    }
                });

                match invoke::<i64>(&client, "addNote", note_params) {
                    Ok(_) => println!("    Added note to {}", output_deck),
                    Err(e) => println!("    Failed to add note: {}", e),
                }
            }
            Ok(_) => println!("    Generated invalid result, skipping."),
            Err(e) => println!("    Generation failed: {}", e),
        }
    }

    println!("\nStep 7: Final sync...");
    sync_anki(&client)?;
    println!("Sync complete.");

    println!("\nAll done!");
    Ok(())
}
