# ADR 0012: 生徒のログインはメールの 6 桁のワンタイムコードにする

- 状態: 提案（工場）（2026-11-14、Issue #1456 の t1 / #1591）
- 関係: ADR 0004（契約は壊さない足し方で変える）、ADR 0011（生徒とコーチはモバイルの同じ形の入口。coach-app のログイン）、`specs/contracts/api/student-app/openapi.yaml`（`requestLoginCode`・`createSession`・`registerAccount` の 409）、`specs/contracts/api/coach-app/openapi.yaml`（`requestCoachLoginCode`・`verifyCoachLoginCode`）

## 文脈

生徒アプリ（student-app）には登録（`registerAccount`）しかなく、ログインがない。端末を替えた・アプリを入れ直した・トークンが無効になった生徒は、登録済みのメールアドレスで入り直す方法がなく、登録し直すと 409 `email_already_registered` で止まる。

生徒の入口は student-app で、coach・本部の入口とは分かれている（ADR 0011）。coach-app のログインは「メールアドレス → 6 桁の確認コード → アクセストークン」の形で、生徒と同じ形にすると決めてある（ADR 0011 の決定 1）。生徒の登録はメールアドレスだけで、パスワードを持っていない。

## 決定

1. **生徒のログインは、メールアドレスに送る 6 桁のワンタイムコードにする**。契約は student-app の 2 操作。
   - `requestLoginCode`（`POST /login-codes`）: メールアドレスに 6 桁の確認コードを送る。登録のないメールアドレス・生徒でないアカウントでも同じ 202 を返し、何も送らない（登録の有無と役割を明かさない）。有効期限は 10 分、送り直すと前のコードは使えなくなり、送り直しは 60 秒あける（429 `too_many_requests`）
   - `createSession`（`POST /sessions`）: コードを確かめ、`registerAccount` の 201 と同じ形（`student` と `access_token`）を返す。違う・送ったコードがないなら 401 `login_code_invalid`、期限切れなら 401 `login_code_expired`。1 つのコードで 5 回間違えると 429 `too_many_requests`。確かめたコードは使えなくなる
2. **登録済みのメールアドレスでの登録は、409 のままログインへ案内する**。`registerAccount` の 409 `email_already_registered` の本文に、任意の `login_guidance`（`email` と `requestLoginCode` のパス）を足す。クライアントはそれを見てログインの画面へ進める
3. **形は coach-app のログインに揃える**。パス・期限・回数の上限・エラーの名前（`login_code_invalid`・`login_code_expired`・`too_many_requests`）を同じにし、backend の確認コードの発行・確認は生徒とコーチで同じ実装を使えるようにする。違うのは、生徒でないアカウントを `account_role_mismatch` で断らず、コードを送っていないものとして 401 `login_code_invalid` にすること（student-app には役割ごとの行き先の案内がないため）
4. **契約は足すだけで変える**（ADR 0004）。操作・スキーマ・`ErrorCode` の値を足し、409 の本文は `Error` の項目に任意の項目を足しただけにする。今のクライアントは今までどおり動く

## 結果

- student-app の契約に 2 操作と、`RequestLoginCodeRequest`・`CreateSessionRequest`（と応答の `RequestLoginCodeResponse`・`CreateSessionResponse`）、`ErrorCode` の `login_code_invalid`・`login_code_expired`・`too_many_requests`、409 の `EmailAlreadyRegisteredError`・`LoginGuidance` が増える（#1591）
- 確認コードの保存（db）と backend の 2 操作、409 に `login_guidance` を載せることは #1456 の後続のタスク。それまで backend は 2 操作を持たず、409 に `login_guidance` を載せない（任意の項目なので契約には合う）
- メールの送信は Gen1 ではモック（外部連携のポートは本物と同じ形で書く。`specs/contracts/README.md`）
- ログインの画面と、409 から案内する流れは別のチケット（#1404 の後続）
- 生徒・コーチで確認コードの仕組みが 1 つになり、上限・期限を変えるときは両方の契約を同じに直す

## とらなかった案

- **パスワード**: 登録でパスワードを持たせておらず、足すと登録の流れ・忘れたときの再設定・保存の安全性が増える。coach-app の形とも揃わない
- **マジックリンク（メールのリンクを押すとログイン）**: モバイルではリンクからアプリへ戻す設定（ユニバーサルリンク・App Links）が要り、メールを別の端末で開くと入れない。6 桁のコードは端末を問わず手で入れられる
- **409 を 200 にして、そのままログインさせる**: メールアドレスを知っているだけで他人のアカウントに入れてしまう
