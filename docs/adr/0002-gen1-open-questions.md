# ADR 0002: Gen1 の未決の Question 3 件への回答

- 状態: 承認（2026-10-26。建設者の委任により Claude が案を決め、Tetsuro が否定しなければ採用。否定があれば後継の ADR で置き換える）
- 関係: Blueprint v0.4 `questions:`、`specs/product-context.md`「未決の Question」、`docs/plans/p1-walking-skeleton.md`（T09 / T12 / T16 の前提）

## Q1. 通話品質スコアの定義（モックが生成する指標のうち何を使うか）

**決定**: `call.ended` の 4 属性（`packet_loss_rate`・`jitter_ms`・`round_trip_ms`・`disconnect_count`）から 0〜100 の減点式スコアを出す。式はセマンティック層（`specs/analytics/kpi.yaml`、T15 で置く）に 1 か所だけ書き、イベントには生の指標だけを持たせる。

```
score = clamp(100
  - 8  * packet_loss_rate(%)
  - 2  * max(0, (jitter_ms - 30) / 10)
  - 3  * max(0, (round_trip_ms - 150) / 50)
  - 10 * disconnect_count, 0, 100)
```

初期値は ITU-T G.114 / G.1010 の目安（往復 150 ms、ジッタ 30 ms）に寄せた仮の重み。合成シードで分布を見て P1 中に改訂する。

**選ばなかった案**: MOS 推定（E-model）。入力に符号化の情報が要り、モックでは作れない。Twilio のネットワーク品質レベル（1〜5）をそのまま使う案は、基盤を差し替えたときに消える。

## Q2. キャンセル規定の判定を scheduling と entitlement のどちらに置くか

**決定**: **判定は scheduling**、**消費の実行は entitlement**。scheduling は予約の開始時刻とキャンセル時刻の差（24 時間前以降か）を知っていて、`BookingCancelled` イベントに `consumes_quota: true|false` を載せて発行する。entitlement はそれを購読して暦月の通話枠を消費する。判定の規則（24 時間）は scheduling の 1 か所にだけ置く（不変条件 3「どちらか一方で判定する」）。

**選ばなかった案**: entitlement で判定。契約種別ごとに規定を変えやすいが、scheduling の時刻の知識を entitlement に持ち込むことになり、予約のリスケや枠の変更のたびに両方を直すことになる。

## Q3. コーチ側の最小操作を admin-web に同居させる案でよいか

**決定**: **同居でよい（Gen1）**。admin-web に「コーチ」ロールでログインできる最小の画面（予約枠の登録、添削の返却、通話への参加）を置く。本部ロールの画面と同じアプリ・同じ契約（`specs/contracts/api/admin-web/`）で、画面はロールで出し分ける。コーチ専用の Web / アプリは後続の Opportunity とし、契約の枝（coach-web / coach-app）は空のまま残す。

**選ばなかった案**: 薄い coach-web を別に切る。Gen1 の一周シナリオに要る操作は 3 つだけで、別アプリにする手間（認証・配信・契約）に見合わない。2 号機以降でコーチの操作が増えたときに分ける。

## 結果

- Blueprint の `questions:` 3 件は回答済みとして `decisions:` に移す（資料フォルダの 10_blueprint-rally.yaml）
- `specs/product-context.md` の「未決の Question」を更新する（このリポジトリ、同じ PR）
- T09 / T12 / T15 / T16 の Issue はこの ADR を前提に書く
