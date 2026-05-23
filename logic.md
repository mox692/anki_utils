下記を実行するbinaryを作って. turso(sqliteみたいなやつ) dbのセットアップが何もないから, dbもセットアップする必要がある.

- 
- ankiのsyncを実行
- ankiのdeckを1つ選ぶ (選ばれるdeckはprogramの中で定数で持つ)
- そのdeckの中の全ての単語をfetch
- それぞれの単語に関して下記を実施
	- 単語のhash値をとる
	- turso (sqlite) dbのtableを見て, 単語がすでに記録されているかを確認
	- 記録されていない場合, dbにinsertする
- そのdeckに対応するdb内部のエントリに関して, 下記を実施
	- そのflashcardの習熟度を計算
		- score.mdを参考にして, 習熟度を計算するロジックを適当に考えて。
	- dbにその習熟度を登録
	- 習熟度が一定以下のもの or 習熟度がない(初めて追加されたエントリ)に対して下記を実施
		- gen_text.rs相当のコードで, そのflash cardの例文を作成。
		- 生成された 日本語 -> 英語 のflashcardを [元々のdeck名]-output というdeckに追加していく(元々のdeckとは別のdeckにする)
			- deckがもしなかったら(初回実行)だったらdeckを追加してそこに追加
- ankiのsyncを実行
