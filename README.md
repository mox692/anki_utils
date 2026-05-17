# Anki Utils

Generate English flashcard sentences with Japanese translations using local LLMs via Ollama.

## Requirements

- Python 3.7+
- Ollama

## Installation

### Install Ollama

**macOS:**
```bash
brew install ollama
```

**Linux:**
```bash
curl -fsSL https://ollama.ai/install.sh | sh
```

**Windows:**
Download from [ollama.ai](https://ollama.ai)

### Setup

1. Start Ollama:
```bash
ollama serve
```

2. Pull the model:
```bash
ollama pull qwen2.5:1.5b
```

3. Install Python dependencies:
```bash
pip install requests
```

## Usage

1. Create `flashcards.csv`:
```csv
word,meaning
apple,りんご
borrow,借りる
```

2. Run the generator:
```bash
python gen_text.py
```

3. Output will be saved to `sentences.csv`:
```csv
word,meaning,japanese,english
apple,りんご,私は朝食にりんごを食べました。,I ate an apple for breakfast.
```

## Configuration

Edit `gen_text.py` to change settings:
- `MODEL`: Switch LLM model (try `qwen2.5:3b` for better quality)
- `temperature`: Adjust creativity (0.0-1.0)

## Troubleshooting

**Connection error:** Ensure Ollama is running with `ollama serve`

**Model not found:** Run `ollama pull qwen2.5:1.5b`
