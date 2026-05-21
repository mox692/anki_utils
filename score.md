できる。結論としては、まずは **`interval` / `ivl` を習熟度の主指標にする**のが一番簡単で堅いです。
「次に出てくる日付が遠いほど習熟している」もほぼ正しいですが、APIで扱うなら **次回日付そのものより、現在の復習間隔 `interval` の方が取りやすくて安定**です。

AnkiConnectには `findCards` と `cardsInfo` があり、`findCards` は検索クエリからカードIDを返し、`cardsInfo` はカードごとのフィールドやデッキ名に加えて `interval` を返します。AnkiConnectのREADMEでも、`cardsInfo` は「ease and interval」を含むと説明されています。([GitHub][1])

たとえば「単語 = abandon」のカードを探して習熟度を取るなら、流れはこうです。

```js
async function anki(action, params = {}) {
  const res = await fetch("http://127.0.0.1:8765", {
    method: "POST",
    body: JSON.stringify({ action, version: 5, params }),
  });
  const json = await res.json();
  if (json.error) throw new Error(json.error);
  return json.result;
}

async function getMasteryByWord(word) {
  const cards = await anki("findCards", {
    query: `"${word}"`
    // 例: `deck:English "${word}"` のように絞ってもよい
  });

  if (cards.length === 0) return null;

  const infos = await anki("cardsInfo", { cards });

  return infos.map(card => ({
    cardId: card.cardId,
    deckName: card.deckName,
    intervalDays: card.interval,
    masteryScore: intervalToScore(card.interval),
    fields: card.fields,
  }));
}

function intervalToScore(intervalDays) {
  // 例: 0〜100に正規化。365日以上で100点扱い
  if (!intervalDays || intervalDays <= 0) return 0;
  return Math.min(100, Math.round(100 * Math.log1p(intervalDays) / Math.log1p(365)));
}
```

`intervalDays` の解釈はだいたいこうでいいです。

| interval | 解釈          |
| -------: | ----------- |
|     0〜1日 | まだ弱い / 学習直後 |
|     3〜7日 | 多少覚え始めた     |
|   14〜30日 | そこそこ定着      |
|  60〜180日 | かなり定着       |
|   365日以上 | かなりマスター寄り   |

Anki公式の検索仕様でも、`prop:ivl>=10` は「間隔が10日以上のカード」、`prop:due=1` は「明日が期限のカード」、`prop:due>=1` は「未来に期限があるカード」として検索できます。FSRS有効かつAnki 23.10以降なら、`prop:s>21` のように stability、`prop:d` difficulty、`prop:r` retrievability でも検索できます。([docs.ankiweb.net][2])

なので、実装方針としては3段階あります。

**1. まずは `interval` ベース**
これが一番簡単。`cardsInfo` または `getIntervals` で取れます。AnkiConnectの `getIntervals` は、直近の間隔、または `complete: true` で過去の間隔履歴を返し、正の値は日数、負の値は秒数です。([GitHub][1])

```js
const intervals = await anki("getIntervals", {
  cards,
  complete: false
});
```

**2. due date ベースで「あと何日で出るか」を使う**
Anki検索の `prop:due` を使えば、「30日以上先」「365日以上先」みたいな判定はできます。

```js
// abandon が30日以上先に出るカードか
const matureCards = await anki("findCards", {
  query: `"abandon" prop:due>=30`
});
```

ただし、AnkiConnect標準APIだけだと「正確な次回日付」を直接きれいに取るのは少し面倒です。`cardsInfo` の実装・バージョンによって `due` が返ることがありますが、公式README上で安定して説明されているのは `interval` です。正確な日付が必須なら、Anki内部DBの `cards.due` を読むか、Ankiアドオン側でカスタムAPIを追加して `col.sched.today` と組み合わせて返すのが安全です。

**3. FSRSを使っているなら `stability` / `retrievability` を使う**
これが理想に近いです。FSRSでは「どれくらいマスターしているか」は本来、単なる次回日付より **stability** や **retrievability** の方が意味があります。Ankiの検索では、FSRS有効時に `prop:s` / `prop:d` / `prop:r` が使えます。([docs.ankiweb.net][2])

ただ、AnkiConnectの標準 `cardsInfo` で `s/d/r` を直接返すかは環境依存になりやすいので、まずは検索でバケット化するのが実用的です。

```js
const buckets = [
  { label: "weak", query: `"abandon" prop:ivl<7` },
  { label: "ok", query: `"abandon" prop:ivl>=7 prop:ivl<30` },
  { label: "strong", query: `"abandon" prop:ivl>=30 prop:ivl<180` },
  { label: "mastered", query: `"abandon" prop:ivl>=180` },
];

for (const b of buckets) {
  const ids = await anki("findCards", { query: b.query });
  console.log(b.label, ids.length);
}
```

おすすめの指標はこれです。

```ts
mastery = {
  intervalDays,        // メイン指標
  dueInDays?,          // 取れるなら補助指標
  reps?,               // 復習回数
  lapses?,             // 忘れた回数
  fsrsStability?,      // 取れるなら最強
  fsrsRetrievability?, // 今日時点の想起確率として使える
}
```

単語アプリ的な「習熟度スコア」にするなら、まずはこれで十分です。

```js
score =
  0.70 * normalizedInterval
+ 0.20 * normalizedReps
- 0.25 * normalizedLapses;
```

実務的には、**`intervalDays` が長い = Ankiが「このカードは長く空けても大丈夫」と判断している**なので、あなたの考えている「次に出てくる日付が先なら習熟している」はかなり良い近似です。最初の実装は `findCards → cardsInfo → interval` で作って、あとから FSRS の `stability` を取れる環境なら置き換えるのがいいと思います。

[1]: https://github.com/amikey/anki-connect "GitHub - amikey/anki-connect: https://github.com/FooSoft/anki-connect.git · GitHub"
[2]: https://docs.ankiweb.net/searching.html?utm_source=chatgpt.com "Searching - Anki Manual"
