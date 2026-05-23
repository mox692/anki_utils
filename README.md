# Anki Utils

Ankiの単語学習を自動化するツール群

## 機能

### `sync` binary

Ankiのflashcardを習熟度ベースで管理し、低習熟度の単語に対して自動的に例文を生成してAnkiに追加するツール。

#### 実行フロー

1. **Anki Sync**: Ankiのリモート同期を実行
2. **データベースセットアップ**: SQLiteデータベース (`anki_mastery.db`) を作成・初期化
3. **単語のフェッチ**: 指定したdeckから全ての単語を取得
4. **ハッシュ化と登録**: 各単語のハッシュ値を計算してDBに記録（既存のものはスキップ）
5. **習熟度の計算**: 
   - AnkiConnectの `interval` を使用して習熟度スコアを計算
   - スコアは0〜100で、intervalが長いほど高スコア
   - 計算式: `score = 100 * ln(interval + 1) / ln(366)` (365日以上で100点)
6. **例文生成**: 習熟度が閾値以下 or 初回追加の単語に対して:
   - Ollamaを使ってその単語を含む英語例文と日本語訳を生成
   - 生成された flashcard を `[元のdeck名]-output` という別deckに追加
7. **最終Sync**: 変更をAnkiと同期

#### 設定

`src/main.rs` の定数で設定:

```rust
const TARGET_DECK: &str = "youtube4";           // 対象のAnki deck名
const OLLAMA_MODEL: &str = "gemma3:12b";        // 使用するOllamaモデル
const MASTERY_THRESHOLD: f64 = 30.0;            // 習熟度の閾値 (この値以下で例文生成)
```

#### 必要な環境

1. **Anki + AnkiConnect**: 
   - Ankiが起動していること
   - AnkiConnectプラグインがインストールされていること
   - `http://localhost:8765` でアクセス可能であること

2. **Ollama**:
   - Ollamaがインストールされていること
   - 指定したモデル (デフォルト: `gemma3:12b`) がダウンロード済みであること
   - `http://localhost:11434` でアクセス可能であること

3. **Rust**:
   - Rustツールチェーンがインストールされていること

#### 実行方法

```bash
cargo run --bin sync
```

または

```bash
cargo build --release
./target/release/sync
```

#### データベーススキーマ

```sql
CREATE TABLE flashcards (
    hash TEXT PRIMARY KEY,              -- note_id + word のハッシュ値
    note_id INTEGER NOT NULL,           -- Ankiのnote ID
    word TEXT NOT NULL,                 -- 単語
    meaning TEXT,                       -- 意味
    mastery_score REAL,                 -- 習熟度スコア (0-100)
    interval INTEGER,                   -- Ankiのinterval (日数)
    created_at DATETIME,                -- 作成日時
    updated_at DATETIME                 -- 更新日時
);
```

### その他のツール

- `gen_text`: CSVファイルから単語を読み込んで例文を生成 (スタンドアロン)
- `anki`: Ankiからdeckの情報を取得するサンプル

## 習熟度スコアについて

`score.md` に基づいて、Ankiの `interval` (復習間隔) を主指標として習熟度を計算しています。

| interval | 解釈 | スコア目安 |
|----------|------|------------|
| 0〜1日 | まだ弱い / 学習直後 | 0-15 |
| 3〜7日 | 多少覚え始めた | 15-30 |
| 14〜30日 | そこそこ定着 | 30-50 |
| 60〜180日 | かなり定着 | 50-80 |
| 365日以上 | マスター | 80-100 |

## インストール

### Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Ollama

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

### Ollamaモデルのダウンロード

```bash
ollama pull gemma3:12b
```

### AnkiConnect

Ankiのプラグインとしてインストール:
1. Ankiを起動
2. `Tools` → `Add-ons` → `Get Add-ons...`
3. コード `2055492159` を入力してインストール
4. Ankiを再起動

## License

MIT
