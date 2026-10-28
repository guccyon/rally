# ADR 0009: ドメインの区分（コア・支援・汎用）とプロフィールの切り出し

- 状態: 承認（方針は 2026-10-28 Tetsuro。一部は builder の仮決めで、答え合わせ待ち）
- 関係: specs/context-map.yaml、specs/product-context.md、Golden Set 0013（評価の境界を変えた負例）

## 文脈

Golden Set の答え合わせで、Tetsuro が rally のドメインの捉え方を示した。ドメインでも代表的なファーストクラスのもの（コアドメイン）と、支援・汎用のサブドメインは分けて考えたい。ファーストクラスは生徒アプリでタブとして切り出す主業務で、レッスンの選択・予約、ライブレッスン、チャット、プロフィールがタブになる。評価（レビュー）は支援サブドメインで、なくてもオンラインレッスンは成り立つ。評価はコーチに付けられるが、評価した生徒のものでもあり、レッスンの一部でもある中間の概念。アカウントや権限は汎用サブドメイン。

今の context-map は区分を持たず、gen1_core（identity / scheduling / entitlement）は実装の順を表すだけだった。プロフィールは identity の中にあった。

## 決定

1. context-map の各コンテキストに subdomain（core / supporting / generic）を持たせる
2. プロフィール（profile）を identity から切り出し、コアにする。identity はアカウント・認証・権限（汎用）。コードの移動（表示名・自己紹介文など）は後の段で行い、それまで実装は identity にある
3. 区分: コア = catalog・matching（レッスンの選択）、scheduling、live-lesson、async-lesson、messaging、profile。支援 = review、entitlement、billing。汎用 = identity、notification、analytics

## 仮決め（Tetsuro の答え合わせ待ち）

- async-lesson（動画添削）をコアにした。タブにはならないが、生徒が動画を送りコーチが返すのはオンラインレッスンそのもの
- catalog・matching をコアにした。Tetsuro の「レッスンの選択」はコーチと枠を探して選ぶことで、予約の前段の主業務と読んだ
- entitlement・billing を支援にした。月額とワンショットの利用権・課金はこの事業に固有の規則（不変条件 1・2・4）を持つので汎用ではない

## 結果

- 検査・判定は、コアの変更を重く、支援・汎用は軽く見られる（重さの物差し）
- 境界の変更（あるコンテキストの持ち物を別のコンテキストに移す）は、区分をまたぐときに特に人に確かめる
