# ADR 0016: アクセストークンの署名鍵を必須にし、署名なしモックは明示の opt-in にする

- 状態: 提案（工場）（2026-11-16、Issue #1785。QA の判定 17319・#1782）
- 関係: ADR 0013（署名付きトークン。決定 2 の「鍵がなければモック」と、とらなかった案の「鍵がなければ起動しない」をこの ADR が改める）、ADR 0003（層。選ぶのは infra の `AccessTokenBackend`、組み立ては server）、`apps/backend/README.md` の「起動」「認証と認可」「staging でトークンを作る」、`specs/stories/cs-traces-vanished-booking.yaml` の must_not

## 文脈

ADR 0013 は署名付きトークンを決めたが、鍵（`ACCESS_TOKEN_SIGNING_KEY`）がなければ警告を 1 行書くだけで黙って署名なしのモック（`mock.<役割>.<アカウント ID>`）で動く fail-open の作りだった。鍵が配られないまま staging が立ち、QA（0.51.0）で生徒が枠の一覧に出るコーチの ID から `mock.coach.<coach_id>` を作って管理画面にコーチとして入り、ほかの生徒の予約の経緯（`GET /coaches/me/bookings`）・添削待ちの動画・チャットを見られた（s1。ストーリー `cs-traces-vanished-booking` の must_not「本部のロールでない人（生徒・コーチ）が、ほかの人の予約の経緯を見られる」）。

鍵の用意と配布は人の作業で、工場は鍵を作らない（ADR 0013）。

## 決定

1. **鍵がなければ backend は起動しない（fail-closed）**。`ACCESS_TOKEN_SIGNING_KEY` がないか空で、下の opt-in もなければ、`infra::AccessTokenBackend::from_vars` が `infra::AccessTokenConfigError::MissingSigningKey` を返し、`server` の `main` は原因（変数の名前と opt-in のしかた。鍵の値は書かない）を標準エラー出力に書いて 0 以外で終わる。staging でトークンを作る `access-token`（seed）も同じ選び方で失敗する
2. **署名なしのモックは明示の opt-in（`ACCESS_TOKEN_ALLOW_UNSIGNED_MOCK=1`）があるときだけ選べる**。値は `1` だけを受け付け、ほかの値（`true`・`0` など）は起動しない（書き間違いで黙ってどちらかに倒さない）。モックで起動したことは今までどおり標準エラー出力に書く。ローカルの起動とテスト用
3. **鍵があれば鍵を優先する**。鍵があるとき opt-in は読まず、署名付き（`infra::SignedTokenService`）にする。短い鍵は今までどおり起動しない（opt-in があってもモックに倒さない）
4. **`deploy/compose.yaml` は鍵を必須にし、opt-in は渡さない**。`ACCESS_TOKEN_SIGNING_KEY: ${ACCESS_TOKEN_SIGNING_KEY:?...}` で、shipper が鍵を渡さなければ compose の段で止まる。opt-in の変数は置かない（`crates/server/tests/it/startup.rs` が確かめる）
5. テストの組み立て（`server::router` などが使う `AccessTokenBackend::default()` のモック、`tests/` の `MockTokenService`）は変えない。起動の経路（`main`・`access-token`）だけが環境変数で選ぶ

## 結果

- 鍵が配られていない環境（staging・本番・使い捨て環境）は backend が起動しない。**鍵の用意と配布は人の作業として残り、それが済むまで Incident は閉じられず、その環境には出荷できない**。署名なしのモックで立てて回避しない
- 鍵のある環境では、生徒が `mock.coach.<coach_id>` でコーチとして入れず、`GET /coaches/me/bookings` などは 401
- ローカルの起動（`cargo run -p server`・`cargo run -p seed --bin access-token`）には `ACCESS_TOKEN_ALLOW_UNSIGNED_MOCK=1` か使い捨ての鍵が要る（`apps/backend/README.md` の「起動」）
- 権限の体系・API 契約・DB・画面は変えない

## とらなかった案

- **今のまま警告だけ出してモックを使う**: 鍵の配り忘れがそのままなりすましになる（今回の s1）。警告は誰も読まないまま staging が立った
- **staging・prod だけ起動時に鍵を必須にし、ほかはモックのままにする**: 環境の名前で分けると、名前を渡し忘れた・新しい環境（使い捨て環境など）で同じ fail-open が残る。どの環境でも既定を閉じ、開けるのは明示の opt-in だけにした
