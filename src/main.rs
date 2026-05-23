mod anki;
mod database;
mod llm;
mod mastery;
mod utils;

use anki::{CardInfo, NoteInfo, invoke, sync_anki};
use chrono::Local;
use database::{get_low_mastery_cards, insert_flashcard, setup_database, update_mastery_scores};
use llm::generate_sentence;
use mastery::calculate_note_to_mastery;
use reqwest::blocking::Client;
use rusqlite::Connection;
use serde_json::{Value, json};
use std::error::Error;
use utils::{clean_html_entities, compute_hash};

const TARGET_DECK: &str = "youtube5";
const MASTERY_THRESHOLD: f64 = 30.0;

fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting Anki sync process...");

    let client = Client::new();

    // Step 1: 作業対象のdeckを書き出す (バックアップ用途)
    backup_target_deck(&client)?;

    // Step 2: ankiのsyncを実行
    sync_anki(&client)?;

    // Step 3: データベースのセットアップ
    let db = setup_db()?;

    // Step 4 & 5: そのdeckの中の全ての単語をfetchし、dbに記録
    let notes = fetch_and_store_notes(&client, &db)?;
    if notes.is_empty() {
        println!("No notes found in deck. Exiting.");
        return Ok(());
    }

    // Step 6: 習熟度を計算し、dbに登録
    calculate_and_update_mastery(&client, &db)?;

    // Step 7: 習熟度が一定以下のものに例文を生成し、output deckに追加
    generate_and_add_sentences(&client, &db)?;

    // Step 8: ankiのsyncを実行
    sync_anki(&client)?;

    println!("\nAll done!");
    Ok(())
}

fn backup_target_deck(client: &Client) -> Result<(), Box<dyn Error>> {
    println!("Step 1: Backing up target deck...");
    let backup_dir = std::env::current_dir()?.join("backup");
    std::fs::create_dir_all(&backup_dir)?;
    let timestamp = Local::now().format("%Y%m%d%H%M%S");
    let backup_file = format!("{}_{}_backup.apkg", TARGET_DECK, timestamp);
    let backup_path = backup_dir.join(&backup_file);

    invoke::<Value>(
        client,
        "exportPackage",
        json!({
            "deck": TARGET_DECK,
            "path": backup_path.to_string_lossy().to_string(),
            "includeSched": true
        }),
    )?;
    println!("Deck backup saved to: backup/{}", backup_file);
    Ok(())
}

fn setup_db() -> Result<Connection, Box<dyn Error>> {
    println!("\nStep 3: Setting up database...");
    let db = Connection::open("anki_mastery.db")?;
    setup_database(&db)?;
    println!("Database setup complete.");
    Ok(db)
}

fn fetch_and_store_notes(
    client: &Client,
    db: &Connection,
) -> Result<Vec<NoteInfo>, Box<dyn Error>> {
    println!("\nStep 4: Fetching notes from deck '{}'...", TARGET_DECK);
    let query = format!("deck:\"{}\"", TARGET_DECK);
    let note_ids: Vec<i64> = invoke(client, "findNotes", json!({ "query": query }))?;
    println!("Found {} notes.", note_ids.len());

    if note_ids.is_empty() {
        return Ok(Vec::new());
    }

    let notes: Vec<NoteInfo> = invoke(client, "notesInfo", json!({ "notes": note_ids }))?;

    println!("\nStep 5: Processing notes and updating database...");
    for note in &notes {
        if let Some((word, meaning)) = extract_word_and_meaning(note) {
            let hash = compute_hash(&format!("{}-{}", note.note_id, word));
            if insert_flashcard(db, &hash, note.note_id, &word, &meaning)? {
                println!("  Added: {} (note_id: {})", word, note.note_id);
            }
        }
    }

    Ok(notes)
}

fn extract_word_and_meaning(note: &NoteInfo) -> Option<(String, String)> {
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

    let word = clean_html_entities(&word_raw);
    let meaning = clean_html_entities(&meaning_raw);

    if word.is_empty() {
        println!(
            "Debug - Skipping note {} due to empty word field",
            note.note_id
        );
        return None;
    }

    Some((word, meaning))
}

fn calculate_and_update_mastery(client: &Client, db: &Connection) -> Result<(), Box<dyn Error>> {
    println!("\nStep 6: Calculating mastery scores...");
    let query = format!("deck:\"{}\"", TARGET_DECK);
    let card_ids: Vec<i64> = invoke(client, "findCards", json!({ "query": query }))?;
    let cards: Vec<CardInfo> = invoke(client, "cardsInfo", json!({ "cards": card_ids }))?;

    let note_to_mastery = calculate_note_to_mastery(&cards);
    update_mastery_scores(db, &note_to_mastery)?;
    println!("Mastery scores updated.");
    Ok(())
}

fn generate_and_add_sentences(client: &Client, db: &Connection) -> Result<(), Box<dyn Error>> {
    println!("\nStep 7: Generating sentences for low-mastery cards...");

    print_debug_info(db)?;

    let low_mastery_cards = get_low_mastery_cards(db, MASTERY_THRESHOLD)?;
    println!(
        "Found {} cards below mastery threshold (< {}).",
        low_mastery_cards.len(),
        MASTERY_THRESHOLD
    );

    let output_deck = format!("{}-output", TARGET_DECK);
    ensure_output_deck_exists(client, &output_deck)?;

    for (note_id, word, meaning, _score) in low_mastery_cards {
        generate_and_add_note(client, &output_deck, note_id, &word, &meaning)?;
    }

    Ok(())
}

fn print_debug_info(db: &Connection) -> Result<(), Box<dyn Error>> {
    let total_cards: i64 = db.query_row("SELECT COUNT(*) FROM flashcards", [], |row| row.get(0))?;
    println!("Debug - Total cards in database: {}", total_cards);

    let mut debug_stmt =
        db.prepare("SELECT note_id, word, mastery_score FROM flashcards LIMIT 5")?;
    let debug_cards: Vec<(i64, String, Option<f64>)> = debug_stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    for (note_id, word, score) in debug_cards {
        println!(
            "Debug - note_id: {}, word: {}, mastery_score: {:?}",
            note_id, word, score
        );
    }

    Ok(())
}

fn ensure_output_deck_exists(client: &Client, output_deck: &str) -> Result<(), Box<dyn Error>> {
    let deck_names: Vec<String> = invoke(client, "deckNames", json!({}))?;
    if !deck_names.contains(&output_deck.to_string()) {
        println!("Creating output deck: {}", output_deck);
        invoke::<Value>(client, "createDeck", json!({ "deck": output_deck }))?;
    }
    Ok(())
}

fn generate_and_add_note(
    client: &Client,
    output_deck: &str,
    note_id: i64,
    word: &str,
    meaning: &str,
) -> Result<(), Box<dyn Error>> {
    println!("  Generating for: {} (note_id: {})", word, note_id);

    match generate_sentence(client, word, meaning) {
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

            match invoke::<i64>(client, "addNote", note_params) {
                Ok(_) => println!("    Added note to {}", output_deck),
                Err(e) => println!("    Failed to add note: {}", e),
            }
        }
        Ok(_) => println!("    Generated invalid result, skipping."),
        Err(e) => println!("    Generation failed: {}", e),
    }

    Ok(())
}
