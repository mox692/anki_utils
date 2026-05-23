use crate::anki::CardInfo;
use std::collections::HashMap;

pub fn calculate_mastery_score(interval: i64) -> f64 {
    if interval <= 0 {
        return 0.0;
    }
    let score = 100.0_f64 * ((interval as f64) + 1.0).ln() / (365.0_f64 + 1.0).ln();
    score.min(100.0)
}

pub fn calculate_note_to_mastery(cards: &[CardInfo]) -> HashMap<i64, f64> {
    let mut note_to_mastery: HashMap<i64, f64> = HashMap::new();
    for card in cards {
        let score = calculate_mastery_score(card.interval);
        note_to_mastery
            .entry(card.note)
            .and_modify(|s| *s = s.max(score))
            .or_insert(score);
    }
    note_to_mastery
}
