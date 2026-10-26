use std::{
    collections::HashSet,
    time::{Duration, SystemTime},
};

use domain::identity::Sport;
use rand::{RngExt, SeedableRng, rngs::StdRng, seq::SliceRandom};

use crate::{CoachPlan, Retention, SlotPlan, Step, StudentPlan};

const MINUTE: i64 = 60;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;
/// 枠の時刻を日本時間で決める（利用者は日本の生徒。暦月の区切りと同じ。ADR 0005）
const JST_OFFSET: i64 = 9 * HOUR;
/// 枠を置く時間帯（日本時間の 9 時台〜21 時台に始まる 1 時間の枠）
const SLOT_HOURS: std::ops::RangeInclusive<i64> = 9..=21;

/// 月額の生徒の確定の間隔の下限。確定から開始までと、24 時間を切るキャンセルが翌月にずれる分を足しても
/// 暦月に 3 回目の枠の消費が起きない間隔（不変条件 1 を合成データの側で破らない）
const MIN_DAYS_BETWEEN_CONFIRMS: i64 = 20;

/// 合成データの計画（Blueprint の synthetic_seed）。乱数は `seed` だけから引くので、同じ `seed` と `now` なら
/// 同じ計画になる。時刻はすべて `now` からの相対で決める（`now` が違えば同じ形の履歴が平行移動する）。
///
/// - コーチ 30 人。スポーツはゴルフ以外の 4 つに順に配る。生徒の 50% が上位 3 人に付き、残りは長い裾（4 位からの 1/順位の重み）
/// - 生徒 1000 人（ゴルフ以外。ワンショットは扱わない）。登録日は 12 ヶ月に一様に散らす
/// - 直近 30 日に活動がある生徒は 100 人（直近 30 日の登録と、離脱していない [`Retention::Active`] の合計）
/// - 継続は [`Retention`] の分布で、初回レッスン後 30 日以内の離脱が最多
/// - 予約の 12% をキャンセルし、そのうち 40% は開始 24 時間を切ってから（回数を消費する側）
/// - 回線品質の低い通話 15% は通話基盤のモック（`MockVideoCallProvider`）が混ぜる
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedPlan {
    pub seed: u64,
    pub now: SystemTime,
    pub coaches: Vec<CoachPlan>,
    pub students: Vec<StudentPlan>,
    /// 誰も予約しない枠の公開（[`Step::PublishSlot`] だけ。コーチの予約枠充足率が 100% にならないように）
    pub open_slots: Vec<Step>,
}

impl SeedPlan {
    pub const COACHES: usize = 30;
    pub const STUDENTS: usize = 1000;
    pub const HISTORY_DAYS: u64 = 365;
    /// 「活動がある」を数える直近の日数（MAU の窓）
    pub const ACTIVE_WINDOW_DAYS: u64 = 30;
    pub const ACTIVE_STUDENTS: usize = 100;
    pub const TOP_COACHES: usize = 3;
    /// 上位 3 人のコーチに付く生徒の割合
    pub const TOP_COACH_SHARE: f64 = 0.5;
    /// コーチに配るスポーツ（ゴルフ以外）
    pub const SPORTS: [Sport; 4] = [Sport::Tennis, Sport::Soccer, Sport::Baseball, Sport::Basketball];

    /// 上位 3 人の間の配分（%）
    const TOP_WEIGHTS: [f64; 3] = [45.0, 32.0, 23.0];
    /// 直近 30 日に活動しない生徒の継続の分布（%）
    const RETENTION_WEIGHTS: [(Retention, u32); 5] = [
        (Retention::NoLesson, 15),
        (Retention::Within30Days, 50),
        (Retention::Within90Days, 20),
        (Retention::Within180Days, 10),
        (Retention::After180Days, 5),
    ];

    pub fn generate(seed: u64, now: SystemTime) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        let now_secs = secs(now);
        let coaches: Vec<CoachPlan> = (0..Self::COACHES)
            .map(|i| CoachPlan {
                sport: Self::SPORTS[i % Self::SPORTS.len()],
                review_lead_hours: rng.random_range(6..=72),
            })
            .collect();
        let assignment = coach_assignment(&mut rng);
        let registered = registration_times(&mut rng, now_secs);
        let window_start = now_secs - days(Self::ACTIVE_WINDOW_DAYS);
        let retentions = retentions(&mut rng, &registered, window_start);

        let mut calendar = Calendar::new(Self::COACHES);
        let students = (0..Self::STUDENTS)
            .map(|i| {
                let coach = assignment[i];
                let history = History {
                    now: now_secs,
                    window_start,
                    coach,
                    review_lead_hours: coaches[coach].review_lead_hours,
                };
                StudentPlan {
                    coach,
                    registered_at: time(registered[i]),
                    retention: retentions[i],
                    steps: history.build(&mut rng, &mut calendar, registered[i], retentions[i]),
                }
            })
            .collect();
        let open_slots = open_slots(&mut rng, &mut calendar, now_secs);

        Self {
            seed,
            now,
            coaches,
            students,
            open_slots,
        }
    }

    /// コーチを登録する時刻（生徒の履歴より前）。
    pub fn coaches_registered_at(&self) -> SystemTime {
        self.now - Duration::from_secs((Self::HISTORY_DAYS + 30) * 24 * 3600)
    }

    /// 「直近 30 日」の始まり（この時刻以後に出来事がある生徒を、活動がある生徒と数える）。
    pub fn active_window_start(&self) -> SystemTime {
        self.now - Duration::from_secs(Self::ACTIVE_WINDOW_DAYS * 24 * 3600)
    }
}

/// 生徒ごとの担当のコーチ。上位 3 人に [`SeedPlan::TOP_COACH_SHARE`]、残りに 4 位からの 1/順位 の重みで配り、順番を混ぜる。
fn coach_assignment(rng: &mut StdRng) -> Vec<usize> {
    let top_total = (SeedPlan::STUDENTS as f64 * SeedPlan::TOP_COACH_SHARE).round() as usize;
    let mut counts = apportion(top_total, &SeedPlan::TOP_WEIGHTS);
    let tail_weights: Vec<f64> = (0..SeedPlan::COACHES - SeedPlan::TOP_COACHES)
        // 上位 3 人の続きの順位（4 位から）の 1/順位。裾の先頭が 3 位を超えない
        .map(|rank| 1.0 / (rank + SeedPlan::TOP_COACHES + 1) as f64)
        .collect();
    counts.extend(apportion(SeedPlan::STUDENTS - top_total, &tail_weights));
    let mut assignment: Vec<usize> = counts
        .into_iter()
        .enumerate()
        .flat_map(|(coach, count)| std::iter::repeat_n(coach, count))
        .collect();
    assignment.shuffle(rng);
    assignment
}

/// `total` を `weights` の比で整数に分ける（最大剰余法。端数の大きい順、同じなら前から 1 ずつ足す）。
fn apportion(total: usize, weights: &[f64]) -> Vec<usize> {
    let sum: f64 = weights.iter().sum();
    let exact: Vec<f64> = weights.iter().map(|w| total as f64 * w / sum).collect();
    let mut counts: Vec<usize> = exact.iter().map(|e| e.floor() as usize).collect();
    let mut order: Vec<usize> = (0..weights.len()).collect();
    order.sort_by(|&a, &b| {
        let (fa, fb) = (exact[a] - exact[a].floor(), exact[b] - exact[b].floor());
        fb.total_cmp(&fa).then(a.cmp(&b))
    });
    let short = total - counts.iter().sum::<usize>();
    for &i in order.iter().take(short) {
        counts[i] += 1;
    }
    counts
}

/// 登録の時刻。12 ヶ月を生徒の数で等分した区間に 1 人ずつ置き（区間の中は一様）、区間の順番を混ぜる。
fn registration_times(rng: &mut StdRng, now: i64) -> Vec<i64> {
    let history = days(SeedPlan::HISTORY_DAYS);
    let mut strata: Vec<usize> = (0..SeedPlan::STUDENTS).collect();
    strata.shuffle(rng);
    strata
        .into_iter()
        .map(|stratum| {
            let position = (stratum as f64 + rng.random_range(0.0..1.0)) / SeedPlan::STUDENTS as f64;
            now - history + (position * history as f64) as i64
        })
        .collect()
}

/// 継続の区分。直近 30 日より前に登録した生徒から、活動のある生徒が合わせて [`SeedPlan::ACTIVE_STUDENTS`] 人に
/// なるだけを [`Retention::Active`] にし、残りは [`SeedPlan::RETENTION_WEIGHTS`] から引く。
fn retentions(rng: &mut StdRng, registered: &[i64], window_start: i64) -> Vec<Retention> {
    let mut older: Vec<usize> = (0..registered.len())
        .filter(|&i| registered[i] < window_start)
        .collect();
    let recent = registered.len() - older.len();
    let active = SeedPlan::ACTIVE_STUDENTS.saturating_sub(recent);
    older.shuffle(rng);
    let active: HashSet<usize> = older.into_iter().take(active).collect();
    (0..registered.len())
        .map(|i| {
            if active.contains(&i) {
                return Retention::Active;
            }
            let total: u32 = SeedPlan::RETENTION_WEIGHTS.iter().map(|(_, w)| w).sum();
            let mut draw = rng.random_range(0..total);
            for (retention, weight) in SeedPlan::RETENTION_WEIGHTS {
                if draw < weight {
                    return retention;
                }
                draw -= weight;
            }
            unreachable!("重みの合計より小さい値を引いた")
        })
        .collect()
}

/// コーチごとに埋まっている時間（1 時間の枠の番号）。同じコーチの枠を重ねない。
struct Calendar {
    occupied: Vec<HashSet<i64>>,
}

impl Calendar {
    fn new(coaches: usize) -> Self {
        Self {
            occupied: vec![HashSet::new(); coaches],
        }
    }

    /// `coach` の、`at` 以後で最初の空いている枠の開始時刻（日本時間の [`SLOT_HOURS`] に始まる正時）。
    fn first_free(&self, coach: usize, at: i64) -> i64 {
        let mut hour = at.div_euclid(HOUR) + i64::from(at.rem_euclid(HOUR) != 0);
        loop {
            let hour_of_day = (hour * HOUR + JST_OFFSET).rem_euclid(DAY) / HOUR;
            if SLOT_HOURS.contains(&hour_of_day) && !self.occupied[coach].contains(&hour) {
                return hour * HOUR;
            }
            hour += 1;
        }
    }

    fn take(&mut self, coach: usize, starts_at: i64) {
        self.occupied[coach].insert(starts_at.div_euclid(HOUR));
    }

    fn taken(&self, coach: usize) -> usize {
        self.occupied[coach].len()
    }
}

/// 生徒 1 人の履歴を作るときの前提。
struct History {
    now: i64,
    window_start: i64,
    coach: usize,
    review_lead_hours: u64,
}

/// 予約のキャンセル。
#[derive(Clone, Copy)]
enum Cancel {
    /// 開始 24 時間前より前（回数を消費しない）
    Early,
    /// 開始 24 時間を切ってから（回数を消費する）
    Late,
}

/// 作った出来事の連なりを履歴に入れたか。
enum Outcome {
    /// すべて入れた。値は最後の出来事の時刻
    Complete(i64),
    /// 途中まで（今より後の出来事は入れない）か、何も入れなかった。以後の連なりは作らない
    Stopped,
}

impl History {
    fn build(
        &self,
        rng: &mut StdRng,
        calendar: &mut Calendar,
        registered: i64,
        retention: Retention,
    ) -> Vec<Step> {
        let mut steps = vec![Step::Register {
            at: time(registered),
        }];
        // 直近 30 日より前に登録して離脱する生徒は、直近 30 日に出来事を残さない
        let mut end = if retention == Retention::Active || registered >= self.window_start {
            self.now
        } else {
            self.window_start - HOUR
        };
        let first_booking = registered + rng.random_range(6 * HOUR..5 * DAY);

        if retention == Retention::NoLesson {
            // 4 割は体験予約をしてキャンセルし、そのまま離脱する
            if rng.random_bool(0.4) {
                let cancel = self.cancel_kind(rng);
                self.live(rng, calendar, &mut steps, end, first_booking, true, Some(cancel));
            }
            return sorted(steps);
        }

        let video_first = rng.random_bool(0.15);
        let (first_confirm, first_done) = if video_first {
            let Outcome::Complete(done) = self.video(rng, &mut steps, end, first_booking) else {
                return sorted(steps);
            };
            (done, done)
        } else {
            let mut confirm = first_booking;
            // 1 割は体験予約を一度キャンセルしてから取り直す
            if rng.random_bool(0.1) {
                let cancel = self.cancel_kind(rng);
                let Outcome::Complete(cancelled) =
                    self.live(rng, calendar, &mut steps, end, confirm, true, Some(cancel))
                else {
                    return sorted(steps);
                };
                confirm = cancelled + rng.random_range(DAY..5 * DAY);
            }
            let Outcome::Complete(done) =
                self.live(rng, calendar, &mut steps, end, confirm, true, None)
            else {
                return sorted(steps);
            };
            (confirm, done)
        };

        if let Some((from, to)) = retention.days_after_first_lesson() {
            let churn = first_done + rng.random_range(days(from)..=days(to));
            end = end.min(churn);
        }
        let subscribes = match retention {
            Retention::Within30Days => rng.random_bool(0.3),
            _ => rng.random_bool(0.9),
        };
        if subscribes {
            let at = first_done + rng.random_range(HOUR..7 * DAY);
            if at < end {
                steps.push(Step::Subscribe { at: time(at) });
            }
        }

        if !video_first || rng.random_bool(0.3) {
            let mut confirm = first_confirm;
            loop {
                confirm += rng.random_range(MIN_DAYS_BETWEEN_CONFIRMS * DAY..35 * DAY);
                if confirm >= end {
                    break;
                }
                let cancel = rng.random_bool(0.12).then(|| self.cancel_kind(rng));
                if let Outcome::Stopped =
                    self.live(rng, calendar, &mut steps, end, confirm, false, cancel)
                {
                    break;
                }
            }
        }
        if video_first || rng.random_bool(0.4) {
            let mut submit = first_done + rng.random_range(3 * DAY..20 * DAY);
            while submit < end {
                if let Outcome::Stopped = self.video(rng, &mut steps, end, submit) {
                    break;
                }
                submit += rng.random_range(10 * DAY..30 * DAY);
            }
        }

        // 離脱していない生徒は直近 30 日に必ず何かする（間隔が空いていれば動画を送る）
        if retention == Retention::Active && steps.iter().all(|s| secs(s.at()) < self.window_start)
        {
            let latest = steps.iter().map(|s| secs(s.at())).max().unwrap_or(registered);
            let from = latest.max(self.window_start) + HOUR;
            let submit = rng.random_range(from..self.now);
            self.video(rng, &mut steps, self.now, submit);
        }
        sorted(steps)
    }

    fn cancel_kind(&self, rng: &mut StdRng) -> Cancel {
        if rng.random_bool(0.4) {
            Cancel::Late
        } else {
            Cancel::Early
        }
    }

    /// 枠の公開 → 予約 → `confirm` に確定 →（キャンセル |通話 → 完了）の連なり。
    #[allow(clippy::too_many_arguments)]
    fn live(
        &self,
        rng: &mut StdRng,
        calendar: &mut Calendar,
        steps: &mut Vec<Step>,
        end: i64,
        confirm: i64,
        is_trial: bool,
        cancel: Option<Cancel>,
    ) -> Outcome {
        let lead = rng.random_range(1..=6) * DAY;
        let starts_at = calendar.first_free(self.coach, confirm + lead);
        let slot = SlotPlan {
            coach: self.coach,
            starts_at: time(starts_at),
            ends_at: time(starts_at + HOUR),
        };
        let mut chain = vec![
            Step::PublishSlot {
                at: time(confirm - rng.random_range(HOUR..7 * DAY)),
                slot,
            },
            Step::Book {
                at: time(confirm - rng.random_range(MINUTE..30 * MINUTE)),
                is_trial,
            },
            Step::Confirm { at: time(confirm) },
        ];
        match cancel {
            Some(Cancel::Early) if starts_at - confirm > 26 * HOUR => {
                let at = rng.random_range(confirm + HOUR..starts_at - 25 * HOUR);
                chain.push(Step::Cancel { at: time(at) });
            }
            Some(_) => {
                let at = starts_at - rng.random_range(HOUR..23 * HOUR);
                chain.push(Step::Cancel {
                    at: time(at.max(confirm + MINUTE)),
                });
            }
            None => {
                let started = starts_at + rng.random_range(0..5 * MINUTE);
                let ended = started + rng.random_range(40 * MINUTE..60 * MINUTE);
                let completed = ended + rng.random_range(MINUTE..30 * MINUTE);
                chain.extend([
                    Step::StartCall { at: time(started) },
                    Step::EndCall { at: time(ended) },
                    Step::CompleteCall {
                        at: time(completed),
                        by_coach: rng.random_bool(0.3),
                    },
                ]);
            }
        }
        // 枠の公開は連なりの最初の出来事なので、何か入れば枠も公開される
        let before = steps.len();
        let outcome = self.accept(steps, chain, end);
        if steps.len() > before {
            calendar.take(self.coach, starts_at);
        }
        outcome
    }

    /// `submit` に動画を送る → コーチが添削を返す → 完了の連なり。
    fn video(&self, rng: &mut StdRng, steps: &mut Vec<Step>, end: i64, submit: i64) -> Outcome {
        let lead_hours = self.review_lead_hours as f64 * rng.random_range(0.5..1.5);
        let returned = submit + (lead_hours * HOUR as f64) as i64;
        let completed = returned + rng.random_range(HOUR..48 * HOUR);
        let chain = vec![
            Step::SubmitVideo { at: time(submit) },
            Step::ReturnReview { at: time(returned) },
            Step::CompleteVideo {
                at: time(completed),
                by_coach: rng.random_bool(0.2),
            },
        ];
        self.accept(steps, chain, end)
    }

    /// 連なりが `end` より前に終わればすべて入れる。終わらなければ、離脱していない（`end` が今）ときだけ
    /// 今までに起きた分を入れる（確定したまま開始を待つ予約・返却を待つ動画）。
    fn accept(&self, steps: &mut Vec<Step>, chain: Vec<Step>, end: i64) -> Outcome {
        let last = chain.iter().map(|s| secs(s.at())).max().unwrap_or(i64::MIN);
        if last < end {
            steps.extend(chain);
            return Outcome::Complete(last);
        }
        if end == self.now {
            steps.extend(chain.into_iter().filter(|s| secs(s.at()) <= self.now));
        }
        Outcome::Stopped
    }
}

/// 誰も予約しない枠。コーチごとに 5 枠と、予約に使った枠の 15%。開始は 12 ヶ月前から 2 週間後まで。
fn open_slots(rng: &mut StdRng, calendar: &mut Calendar, now: i64) -> Vec<Step> {
    let mut steps = Vec::new();
    for coach in 0..SeedPlan::COACHES {
        let count = 5 + calendar.taken(coach) * 15 / 100;
        for _ in 0..count {
            let from = now - days(SeedPlan::HISTORY_DAYS);
            let starts_at = calendar.first_free(coach, rng.random_range(from..now + 14 * DAY));
            calendar.take(coach, starts_at);
            let published = starts_at.min(now) - rng.random_range(HOUR..7 * DAY);
            steps.push(Step::PublishSlot {
                at: time(published),
                slot: SlotPlan {
                    coach,
                    starts_at: time(starts_at),
                    ends_at: time(starts_at + HOUR),
                },
            });
        }
    }
    sorted(steps)
}

fn sorted(mut steps: Vec<Step>) -> Vec<Step> {
    steps.sort_by_key(Step::at);
    steps
}

const fn days(n: u64) -> i64 {
    n as i64 * DAY
}

fn secs(at: SystemTime) -> i64 {
    match at.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(after) => after.as_secs() as i64,
        Err(before) => -(before.duration().as_secs() as i64),
    }
}

fn time(secs: i64) -> SystemTime {
    if secs >= 0 {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs as u64)
    } else {
        SystemTime::UNIX_EPOCH - Duration::from_secs(secs.unsigned_abs())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use super::*;

    /// 2026-10-26T12:00:00+09:00
    const NOW: Duration = Duration::from_secs(1_792_983_600);

    fn plan(seed: u64) -> SeedPlan {
        SeedPlan::generate(seed, SystemTime::UNIX_EPOCH + NOW)
    }

    #[test]
    fn the_same_seed_gives_the_same_plan_and_another_seed_does_not() {
        assert_eq!(plan(42), plan(42));
        assert_ne!(plan(42).students, plan(7).students);
    }

    #[test]
    fn thirty_coaches_spread_over_the_four_sports_other_than_golf() {
        let plan = plan(42);
        assert_eq!(plan.coaches.len(), 30);
        let mut by_sport: BTreeMap<String, usize> = BTreeMap::new();
        for coach in &plan.coaches {
            *by_sport.entry(format!("{:?}", coach.sport)).or_default() += 1;
        }
        assert!(!by_sport.contains_key("Golf"));
        assert_eq!(by_sport.len(), 4, "{by_sport:?}");
        assert!(by_sport.values().all(|n| (7..=8).contains(n)), "{by_sport:?}");
    }

    #[test]
    fn half_of_the_students_go_to_the_top_three_coaches_with_a_long_tail() {
        for seed in [1, 42, 2026] {
            let plan = plan(seed);
            assert_eq!(plan.students.len(), 1000);
            let mut per_coach = vec![0usize; plan.coaches.len()];
            for student in &plan.students {
                per_coach[student.coach] += 1;
            }
            let mut sorted = per_coach.clone();
            sorted.sort_unstable_by(|a, b| b.cmp(a));
            assert_eq!(sorted[..3], per_coach[..3], "上位 3 人は先頭の 3 人");
            assert_eq!(sorted[..3].iter().sum::<usize>(), 500);
            assert!(sorted.iter().all(|n| *n > 0), "全員に生徒がいる: {sorted:?}");
            assert!(sorted[3] < sorted[2], "裾の先頭は 3 位より少ない: {sorted:?}");
        }
    }

    #[test]
    fn registrations_spread_over_twelve_months() {
        let plan = plan(42);
        let now = secs(plan.now);
        let mut per_month = [0usize; 12];
        for student in &plan.students {
            let age = now - secs(student.registered_at);
            assert!((0..days(365)).contains(&age), "{age}");
            per_month[(age * 12 / days(365)) as usize] += 1;
        }
        assert!(per_month.iter().all(|n| (82..=85).contains(n)), "{per_month:?}");
    }

    #[test]
    fn about_one_hundred_students_are_active_in_the_last_thirty_days() {
        for seed in [1, 42, 2026] {
            let plan = plan(seed);
            let window_start = plan.active_window_start();
            let active = plan
                .students
                .iter()
                .filter(|s| {
                    s.steps
                        .iter()
                        .any(|step| !matches!(step, Step::PublishSlot { .. }) && step.at() >= window_start)
                })
                .count();
            assert_eq!(active, SeedPlan::ACTIVE_STUDENTS, "seed {seed}");
            assert!(
                plan.students
                    .iter()
                    .flat_map(|s| &s.steps)
                    .all(|step| step.at() <= plan.now),
                "今より後の出来事はない"
            );
        }
    }

    #[test]
    fn churn_within_thirty_days_after_the_first_lesson_is_the_most_common() {
        let plan = plan(42);
        let mut counts: HashMap<Retention, usize> = HashMap::new();
        for student in &plan.students {
            *counts.entry(student.retention).or_default() += 1;
        }
        let most = Retention::ALL
            .into_iter()
            .max_by_key(|r| counts.get(r).copied().unwrap_or(0))
            .unwrap();
        assert_eq!(most, Retention::Within30Days, "{counts:?}");
    }

    #[test]
    fn steps_are_in_time_order_and_slots_of_a_coach_never_overlap() {
        let plan = plan(42);
        let mut starts: HashSet<(usize, SystemTime)> = HashSet::new();
        let slots = plan
            .students
            .iter()
            .flat_map(|s| &s.steps)
            .chain(&plan.open_slots)
            .filter_map(|step| match step {
                Step::PublishSlot { slot, .. } => Some(slot),
                _ => None,
            });
        for slot in slots {
            assert_eq!(slot.ends_at, slot.starts_at + Duration::from_secs(3600));
            assert!(starts.insert((slot.coach, slot.starts_at)), "{slot:?}");
        }
        for student in &plan.students {
            assert!(student.steps.is_sorted_by_key(Step::at));
            let registered = student.steps.iter().find(|s| matches!(s, Step::Register { .. }));
            assert_eq!(registered.map(Step::at), Some(student.registered_at));
        }
    }

    #[test]
    fn some_bookings_are_cancelled_including_within_twenty_four_hours() {
        let plan = plan(42);
        let mut cancelled = 0;
        let mut late = 0;
        for student in &plan.students {
            let mut starts_at = None;
            for step in &student.steps {
                match step {
                    Step::PublishSlot { slot, .. } => starts_at = Some(slot.starts_at),
                    Step::Cancel { at } => {
                        cancelled += 1;
                        let start = starts_at.unwrap();
                        assert!(*at < start);
                        if start.duration_since(*at).unwrap() < Duration::from_secs(24 * 3600) {
                            late += 1;
                        }
                    }
                    _ => {}
                }
            }
        }
        assert!(cancelled > 100, "{cancelled}");
        assert!(late > 0 && late < cancelled, "{late} / {cancelled}");
    }
}
