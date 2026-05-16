use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::error::Error;

const ANKI_CONNECT_URL: &str = "http://localhost:8765";

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
    tags: Vec<String>,

    #[serde(default)]
    fields: HashMap<String, FieldValue>,
}

#[derive(Debug, Deserialize)]
struct FieldValue {
    value: String,
    order: i32,
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

fn fetch_10_notes_from_deck(deck_name: &str) -> Result<Vec<NoteInfo>, Box<dyn Error>> {
    let client = Client::new();

    // deck名にスペースがある場合もあるので quote する
    let query = format!("deck:\"{}\"", deck_name);

    let mut note_ids: Vec<i64> = invoke(
        &client,
        "findNotes",
        json!({
            "query": query
        }),
    )?;

    // 先頭10件だけ取る
    note_ids.truncate(10);

    if note_ids.is_empty() {
        return Ok(vec![]);
    }

    let notes: Vec<NoteInfo> = invoke(
        &client,
        "notesInfo",
        json!({
            "notes": note_ids
        }),
    )?;

    Ok(notes)
}

fn main() -> Result<(), Box<dyn Error>> {
    let deck_name = "youtube4"; // ここを自分のdeck名に変更

    let notes = fetch_10_notes_from_deck(deck_name)?;

    println!("Fetched {} notes from deck: {}", notes.len(), deck_name);

    for note in notes {
        println!("-------------------------");
        println!("note_id: {}", note.note_id);
        println!("tags: {:?}", note.tags);

        for (field_name, field) in note.fields {
            println!("{}: {}", field_name, field.value);
        }
    }

    Ok(())
}
