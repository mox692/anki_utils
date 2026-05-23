use rusqlite::{Connection, params};
use std::collections::HashMap;
use std::error::Error;

pub fn setup_database(db: &Connection) -> Result<(), Box<dyn Error>> {
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

    Ok(())
}

pub fn insert_flashcard(
    db: &Connection,
    hash: &str,
    note_id: i64,
    word: &str,
    meaning: &str,
) -> Result<bool, Box<dyn Error>> {
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
            params![hash, note_id, word, meaning],
        )?;
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn update_mastery_scores(
    db: &Connection,
    note_to_mastery: &HashMap<i64, f64>,
) -> Result<(), Box<dyn Error>> {
    for (note_id, score) in note_to_mastery {
        db.execute(
            "UPDATE flashcards SET mastery_score = ?, updated_at = CURRENT_TIMESTAMP WHERE note_id = ?",
            params![*score, *note_id],
        )?;
    }
    Ok(())
}

pub fn get_low_mastery_cards(
    db: &Connection,
    threshold: f64,
) -> Result<Vec<(i64, String, String, Option<f64>)>, Box<dyn Error>> {
    let mut stmt = db.prepare(
        "SELECT note_id, word, meaning, mastery_score FROM flashcards WHERE mastery_score IS NULL OR mastery_score < ?",
    )?;

    let cards: Vec<(i64, String, String, Option<f64>)> = stmt
        .query_map(params![threshold], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(cards)
}
