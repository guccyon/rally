# admin-web

Rally の最小の管理画面（Next.js App Router・TypeScript strict・Tailwind CSS）。本部とコーチが同じアプリにロールで同居する（docs/adr/0002 Q3）。

## コマンド

```sh
npm ci
npm run typecheck   # tsc --noEmit
npm run lint        # eslint
npm test            # vitest（API はモック）
npm run dev         # http://localhost:3000
npm run gen:api     # 契約から API の型 src/api/schema.ts を作り直す
```

## API

- 型は契約 `specs/contracts/api/admin-web/openapi.yaml` から `openapi-typescript` で生成した `src/api/schema.ts` だけを使う（手書きの型を置かない）。契約を変えたら `npm run gen:api`。生成物が契約とずれていれば `tests/apiSchema.test.ts` が落ちる
- 呼び出しは `openapi-fetch`。コーチの操作は `src/api/coach.ts`、本部の操作は `src/api/headquarters.ts`（どちらも operationId と 1 対 1。結果の形と失敗の文言化は `src/api/call.ts`）、契約の `ErrorCode` → 画面の文言は `src/api/errorMessage.ts` の 1 か所
- ブラウザは同じオリジンの `/api/*` を呼び、`next.config.ts` の rewrites がバックエンドへ中継する。中継先は環境変数 `RALLY_API_URL`（既定 `http://127.0.0.1:8080`。バックエンドの既定の待ち受け）。rewrites は `next build` の時点で決まるので、`next start` で使うときはビルド時に設定する

## ログインとロール

Gen1 は本物の認証がない。ログイン画面でロール（コーチ / 本部）を選び、モックのアクセストークン（`mock.<ロール>.<アカウント ID>`）を入力すると localStorage に保持する。トークンは不透明なもの（契約の `bearerAuth`）として扱い、中身からロールを読まない。ロールとトークンが食い違えば API が 403 を返し、その旨を表示する。

| ロール | 画面 | 呼ぶ操作 |
|---|---|---|
| コーチ（`/coach`） | 予約枠の公開と、このログイン中に公開した枠の一覧 | `publishSlot` |
| | 動画の添削の返却（動画 ID を入力） | `returnVideoReview` |
| | 通話への参加（予約 ID で開始、同じセッションを終了。映像はモックなので出さない） | `startLiveLessonSession` / `endLiveLessonSession` |
| | 自分の評価（ID は入力しない。開いたときに本人のアカウントを API に聞き、その ID で評価の件数と点の平均と、その下に評価の一覧（点・一言・送った時刻。並びは API が返した新しい順のまま）を出す。0 件なら平均は「評価はまだありません」、一覧は「まだ評価はありません」。本人のアカウントが取れないとき（401 など）・コーチのアカウントでないとき・評価の API が失敗したときは、その文言だけを出す） | `getMyAccount` / `getCoachReviewSummary` / `listCoachReviews` |
| 本部（`/headquarters`） | 予約・完了記録の一覧 | なし（契約に API がないので「未実装」と表示） |
| 本部（`/headquarters/slot-supply`） | スポーツ別の予約枠の需給（足りている / 足りていない、公開した枠の数、予約しようとした生徒の数、集計の期間）。判定は API が返した値をそのまま出す（式は契約の `getSlotSupply` が正本） | `getSlotSupply` |
| 本部（`/headquarters/coach-review-summary`） | コーチの評価（ID は入力しない。開いたときにコーチの一覧を取り、表示名（API が返した表示名の順のまま）から選んだコーチの評価の件数と点の平均と、その下に評価の一覧を出す。件数と平均・一覧の表示はコーチの「自分の評価」と同じ部品。数え方と丸めは契約の `getCoachReviewSummary` が正本で、画面は返った値を小数第 2 位まで揃えて出すだけ。コーチが 1 人もいなければ「コーチがまだいません」。一覧や評価の API が失敗したときは、その文言だけを出す） | `listCoaches` / `getCoachReviewSummary` / `listCoachReviews` |

自分の枠・届いた動画・自分の予約の一覧を取る API は admin-web の契約にまだないので、画面に「未実装」と出し、ID を入力して操作する。本部はコーチの一覧の API（`listCoaches`。本部だけ）でコーチを選ぶ。トークンの中身は読まないので、本人のアカウント ID はログイン中の本人のアカウントを返す API（`getMyAccount`）で知る（今はコーチの「自分の評価」だけが使う）。
