import csv
import json
import requests

# MODEL = "hf.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF:Q4_K_M"
# MODEL = "qwen2.5:1.5b"
MODEL = "gemma3:12b"
OLLAMA_URL = "http://localhost:11434/api/chat"

SYSTEM_PROMPT = """
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
"""

def is_valid_result(result: dict, word: str) -> bool:
    japanese = result.get("japanese", "").strip()
    english = result.get("english", "").strip()

    if not japanese or not english:
        return False

    # 英文に単語が含まれているか
    if word.lower() not in english.lower():
        return False

    # 英文が短すぎる場合は失敗扱い
    if len(english.split()) < 5:
        return False

    # 単語だけを返している場合を弾く
    if english.lower().strip(".!?") == word.lower():
        return False

    return True


def generate_sentence(word: str, meaning: str = "") -> dict:
    user_prompt = f"""
Given word: {word}
Japanese meaning: {meaning}

Create a flashcard example sentence.

Requirements:
- English sentence must be 7 to 20 words.
- English sentence must include the exact word: "{word}"
- Japanese sentence must mean the same thing as the English sentence.
- Output valid JSON only.
- Use this exact schema:
{{
  "japanese": "...",
  "english": "..."
}}

Remember:
The English field must be a complete sentence.
"""

    payload = {
        "model": MODEL,
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": user_prompt},
        ],
        "stream": False,
        "format": "json",
        "options": {
            "temperature": 0.2,
            "num_predict": 100
        }
    }

    for _ in range(3):
        response = requests.post(OLLAMA_URL, json=payload, timeout=120)
        response.raise_for_status()

        content = response.json()["message"]["content"].strip()

        try:
            result = json.loads(content)
        except json.JSONDecodeError:
            result = {
                "japanese": "",
                "english": content
            }

        if is_valid_result(result, word):
            return result

    return {
        "japanese": "",
        "english": ""
    }

def main():
    rows = []

    with open("flashcards.csv", newline="", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for row in reader:
            word = row["word"].strip()
            meaning = row.get("meaning", "").strip()

            result = generate_sentence(word, meaning)

            rows.append({
                "word": word,
                "meaning": meaning,
                "japanese": result.get("japanese", ""),
                "english": result.get("english", "")
            })

            print(word, "=>", result)

    with open("sentences.csv", "w", newline="", encoding="utf-8") as f:
        fieldnames = ["word", "meaning", "japanese", "english"]
        writer = csv.DictWriter(f, fieldnames=fieldnames)
        writer.writeheader()
        writer.writerows(rows)

if __name__ == "__main__":
    main()
