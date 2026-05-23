use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::error::Error;

const ANKI_CONNECT_URL: &str = "http://localhost:8765";

#[derive(Debug, Deserialize)]
pub struct AnkiResponse<T> {
    pub result: Option<T>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NoteInfo {
    #[serde(rename = "noteId")]
    pub note_id: i64,
    #[serde(default)]
    pub fields: HashMap<String, FieldValue>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct FieldValue {
    pub value: String,
    #[allow(dead_code)]
    pub order: i32,
}

#[derive(Debug, Deserialize)]
pub struct CardInfo {
    #[serde(rename = "cardId")]
    #[allow(dead_code)]
    pub card_id: i64,
    pub note: i64,
    pub interval: i64,
    #[serde(default)]
    #[allow(dead_code)]
    pub lapses: i64,
    #[serde(default)]
    #[allow(dead_code)]
    pub reps: i64,
}

pub fn invoke<T>(client: &Client, action: &str, params: Value) -> Result<T, Box<dyn Error>>
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

pub fn sync_anki(client: &Client) -> Result<(), Box<dyn Error>> {
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
