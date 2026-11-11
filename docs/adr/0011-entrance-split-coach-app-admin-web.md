# ADR 0011: 入口の分け直し（生徒とコーチはモバイルの同じ形の入口、本部は Web の独立した入口）

- 状態: 提案（工場）（2026-11-11、Issue #1405 の t1 / #1415）
- 関係: ADR 0002 Q3（コーチの操作の admin-web 同居。入口についてこの ADR が置き換える）、ADR 0008（本部を別システムにする案。見送りのまま）、ADR 0003（層）、ADR 0004（expand / contract）、ADR 0010（生徒のログイン。#1404）、`specs/contracts/api/coach-app/openapi.yaml`、`specs/contracts/api/admin-web/openapi.yaml`（`getMyAccount` の `x-deprecated-roles`）、`specs/ux/issue-1405.yaml`、`specs/stories/actors.yaml`

## 文脈

ADR 0002 Q3 で、コーチの最小の操作（予約枠の登録・添削の返却・通話への参加）を admin-web に同居させた。admin-web のログインの画面は 1 つで、コーチと本部がロールの選択で同居している。ロールは利用者が選んだまま保存し、トークンの役割を確かめないため、ロールの選び間違い（#1374）が起きている。

コーチは本来スマホで使うので、入口は本部より生徒に近い。本部は PC の Web だけあればよく、コーチと同じ入口にある理由がない。生徒のログインはメールのワンタイムコードにする（#1404、ADR 0010）。

## 決定

1. **生徒とコーチはモバイルの同じ形の入口にする**。コーチは生徒アプリとは別の coach-app（契約の coach-app の枝）を使い、ログインは生徒と同じ形（メールアドレス → 6 桁の確認コード → アクセストークン）にする。契約は `specs/contracts/api/coach-app/openapi.yaml` の 4 操作（`requestCoachLoginCode`・`verifyCoachLoginCode`・`getMyAccount`・`revokeCoachSession`）
2. **ロールは利用者に選ばせない**。coach-app は `verifyCoachLoginCode` の答えで、admin-web は `getMyAccount` の `role` で役割を決める。coach-app はコーチ以外のアカウントを、確認コードを確かめたあとに 403 `account_role_mismatch` で断る（メールアドレスを入れた段では登録の有無も役割も明かさない）
3. **本部は Web の独立した入口にする**。admin-web の `/` を本部だけの入口にし、ロールの選択をなくす。コーチのトークンは本部の入口では受けず、コーチの入口への行き先を示す
4. **バックエンドと DB は共有のまま**。入口を分けるだけで、本部を別のバックエンド・別の DB にはしない（ADR 0008 は見送りのまま）

## 移行中の admin-web の扱い

- コーチの画面（予約枠の登録・添削の返却・通話への参加と、チャット・お知らせなど今のコーチの画面）は、coach-app に移るまで admin-web に残す。移すのは後続のチケットで、順は通話への参加・予約枠の登録・添削の返却の 3 つを先にし、残り（チャット・お知らせ・レッスンメニューなど）をそのあとにする
- それまでコーチは、本部の入口と切り離した admin-web の**コーチの入口（PC。`/coach/login`）**からログインする。中身は今のモックのトークンのままで、ロールは選ばず `getMyAccount` でコーチと確かめる。コーチの画面のログアウトと、権限がないときの戻り先もここにする
- 守るストーリー `coach-prejoin-check`・`coach-replies-to-new-message` は、このコーチの入口から入った admin-web のコーチの画面で流す
- 契約では、admin-web のログインでコーチの役割を受けるところ（`getMyAccount` の `role: coach`）を `x-deprecated-roles` で deprecated にした（`x-deprecated-at: 2026-11-11`）。消すまでは今のまま `coach` も返し、コーチ向けの admin-web の操作もそのまま動く（互換を壊さない）

## 消す日

admin-web のコーチの入口（`/coach/login`）と、admin-web のログインでコーチの役割を受けるところは、次の 2 つのうち**遅いほう**で消す。

- deprecated から 90 日（`deprecation_period_days`）: **2027-02-09** 以降
- コーチの画面が coach-app に移り終わった日

消すのは別のチケットで、そのときに admin-web の契約から `x-deprecated-roles` の注記を消し、actors.yaml の coach の apps から admin-web を外し、守るストーリーの entry を coach-app に書き直す。コーチ向けの admin-web の操作（`listMyCoachBookings` など）を契約から消すときも、同じく `x-deprecated-at` を付けて 90 日残す。

## 結果

- coach-app の契約（4 操作）が増える。コーチのアプリの配信と、ログインの確認コード・トークンの無効化が backend に増える（#1416）
- coach-app の足場とログイン・ホームの画面（#1417）、admin-web の本部専用の入口と `/coach/login`（#1418）は後続のタスク
- ADR 0002 Q3 は、入口（ログイン）について置き換わる。コーチの画面が admin-web にあることは、移り終わるまでそのまま
- 消す日まで、admin-web にはコーチの入口と本部の入口の 2 つが並ぶ

## とらなかった案

- **admin-web にコーチの入口を残し続ける（ロールの選択だけをなくす）**: 本部とコーチが同じ入口に残り、「本部は独立した入口」にならない。コーチの端末（モバイル）にも合わない
- **コーチ用の Web を別に作る（coach-web）**: コーチの端末はモバイルで、生徒と同じ形の入口にそろえる目的に合わない。配信するものが増える
- **生徒アプリと 1 つのアプリにしてロールで分ける**: 契約の「1 アプリ 1 枝」に反し、ロールの選択の同居という今の問題をモバイルに持ち込む
- **本部を別システムにする（ADR 0008）**: 見送りのまま。今回は入口を分けるまで
