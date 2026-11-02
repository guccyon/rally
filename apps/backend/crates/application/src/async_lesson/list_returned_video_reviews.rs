use std::collections::{BTreeMap, BTreeSet};

use domain::{
    DomainError,
    async_lesson::{ReturnedVideoReviewFilter, Video},
    catalog::Sport,
    identity::AccountId,
};
use time::{Date, Duration};

use super::{
    ReturnedVideoReviewList, VideoCoachDisplayNameLookup, VideoRepository,
    VideoStudentDisplayNameLookup,
};
use crate::{Unavailable, clock::Clock, scheduling::calendar_day};

/// 本部が見る返した動画添削の記録のユースケース（admin-web の `listReturnedVideoReviews`。本部だけが呼ぶ）。
/// 照会だけで、何も保存しない。
///
/// 返却の日付（日本時間。[`calendar_day`] で区切る）が `from` から `to` まで（両端を含む）の返却済みの動画をすべての
/// コーチについて読み、domain の [`ReturnedVideoReviewFilter::select`] で期間と任意のコーチと任意のスポーツで絞って
/// 返却の新しい順に並べる（期間の長さの規則・かかった時間の数え方・並べ方・動画のスポーツの決め方は domain だけ）。どちらも省けば今日までの
/// [`ReturnedVideoReviewFilter::DEFAULT_DAYS`] 日、片方だけなら、もう片方はそこから
/// [`ReturnedVideoReviewFilter::DEFAULT_DAYS`] 日目（両端を含む）。各動画には担当のコーチと生徒の表示名を profile に
/// 問い合わせて（[`VideoCoachDisplayNameLookup`]・[`VideoStudentDisplayNameLookup`]）添える。誰が呼べるか（本部だけ）は
/// 境界（interface）が確かめる。
pub struct ListReturnedVideoReviews<V, K, D, C> {
    videos: V,
    coaches: K,
    students: D,
    clock: C,
}

/// 本部が見る返した動画添削の記録の条件。どれも省ける。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ListReturnedVideoReviewsQuery {
    /// 期間の始まりの日付（日本時間。含む）
    pub from: Option<Date>,
    /// 期間の終わりの日付（日本時間。含む）
    pub to: Option<Date>,
    /// あればこのコーチの動画だけ
    pub coach_id: Option<AccountId>,
    /// あればこのスポーツの動画だけ
    pub sport: Option<Sport>,
}

#[derive(Debug)]
pub enum ListReturnedVideoReviewsError {
    /// 期間が規則を満たさない（[`ReturnedVideoReviewFilter::new`] の違反）
    Invalid(DomainError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

impl From<DomainError> for ListReturnedVideoReviewsError {
    fn from(error: DomainError) -> Self {
        Self::Invalid(error)
    }
}

impl From<Unavailable> for ListReturnedVideoReviewsError {
    fn from(error: Unavailable) -> Self {
        Self::Unavailable(error)
    }
}

impl<V, K, D, C> ListReturnedVideoReviews<V, K, D, C>
where
    V: VideoRepository,
    K: VideoCoachDisplayNameLookup,
    D: VideoStudentDisplayNameLookup,
    C: Clock,
{
    pub fn new(videos: V, coaches: K, students: D, clock: C) -> Self {
        Self {
            videos,
            coaches,
            students,
            clock,
        }
    }

    pub async fn execute(
        &self,
        query: ListReturnedVideoReviewsQuery,
    ) -> Result<ReturnedVideoReviewList, ListReturnedVideoReviewsError> {
        let span = Duration::days(i64::from(ReturnedVideoReviewFilter::DEFAULT_DAYS) - 1);
        let (from, to) = match (query.from, query.to) {
            (Some(from), Some(to)) => (from, to),
            (Some(from), None) => (from, from.saturating_add(span)),
            (None, to) => {
                let to = to.unwrap_or_else(|| calendar_day::of(self.clock.now()));
                (to.saturating_sub(span), to)
            }
        };
        let filter = ReturnedVideoReviewFilter::new(
            calendar_day::start(from),
            calendar_day::end(to),
            query.coach_id,
            query.sport,
        )?;
        let videos = filter.select(
            self.videos
                .videos_returned_between(filter.starts_at(), filter.ends_at())
                .await?,
        );
        let (coaches, students) = if videos.is_empty() {
            (BTreeMap::new(), BTreeMap::new())
        } else {
            let coach_ids: BTreeSet<AccountId> = videos.iter().map(Video::coach_id).collect();
            let student_ids: BTreeSet<AccountId> = videos.iter().map(Video::student_id).collect();
            (
                self.coaches.find_coach_display_names(&coach_ids).await?,
                self.students
                    .find_student_display_names(&student_ids)
                    .await?,
            )
        };
        Ok(ReturnedVideoReviewList {
            from,
            to,
            videos,
            coaches,
            students,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        time::SystemTime,
    };

    use domain::{
        async_lesson::{ReviewComment, VideoId, VideoReviewId},
        catalog::Sport,
        profile::DisplayName,
    };
    use time::Month;

    use super::*;
    use crate::in_memory::{FixedClock, InMemoryVideoRepository};

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn date(month: Month, day: u8) -> Date {
        Date::from_calendar_date(2026, month, day).unwrap()
    }

    /// 2026 年 10 月 `day` 日の日本時間 `hour` 時
    fn at(day: u8, hour: u8) -> SystemTime {
        calendar_day::start(date(Month::October, day))
            + std::time::Duration::from_secs(u64::from(hour) * 3600)
    }

    /// 照会する時刻（2026-10-31 の日本時間 12 時）
    fn now() -> SystemTime {
        at(31, 12)
    }

    /// `id` のテニスの動画を `student` が `coach` に `submitted_at` に送る。`returned_at` があればそのときに返却済み。
    fn video(
        id: u128,
        student: AccountId,
        coach: AccountId,
        submitted_at: SystemTime,
        returned_at: Option<SystemTime>,
    ) -> Video {
        video_in(id, Sport::Tennis, student, coach, submitted_at, returned_at)
    }

    /// `id` の `sport` の動画を `student` が `coach` に `submitted_at` に送る。`returned_at` があればそのときに返却済み。
    fn video_in(
        id: u128,
        sport: Sport,
        student: AccountId,
        coach: AccountId,
        submitted_at: SystemTime,
        returned_at: Option<SystemTime>,
    ) -> Video {
        Video::restore(
            VideoId::from_u128(id),
            student,
            coach,
            sport,
            submitted_at,
            returned_at.map(|returned_at| {
                (
                    VideoReviewId::from_u128(id + 100),
                    ReviewComment::new("テイクバックが早い").unwrap(),
                    returned_at,
                )
            }),
            None,
        )
        .unwrap()
    }

    /// `COACH` と `STUDENT` の表示名だけを知っていて、問い合わせた ID を記録する。
    #[derive(Clone, Default)]
    struct RecordingLookup {
        asked_coaches: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
        asked_students: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
    }

    fn known(
        ids: &BTreeSet<AccountId>,
        who: AccountId,
        name: &str,
    ) -> BTreeMap<AccountId, DisplayName> {
        ids.iter()
            .filter(|id| **id == who)
            .map(|id| (*id, DisplayName::new(name).unwrap()))
            .collect()
    }

    impl VideoCoachDisplayNameLookup for RecordingLookup {
        async fn find_coach_display_names(
            &self,
            coach_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            self.asked_coaches.lock().unwrap().push(coach_ids.clone());
            Ok(known(coach_ids, COACH, "たなかコーチ"))
        }
    }

    impl VideoStudentDisplayNameLookup for RecordingLookup {
        async fn find_student_display_names(
            &self,
            student_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            self.asked_students
                .lock()
                .unwrap()
                .push(student_ids.clone());
            Ok(known(student_ids, STUDENT, "あおき"))
        }
    }

    fn videos() -> InMemoryVideoRepository {
        InMemoryVideoRepository::with_videos([
            // 返却が 10/31 の日本時間 0 時ちょうど（期間の終わりの日に入る）
            video(1, STUDENT, COACH, at(29, 0), Some(at(31, 0))),
            video(2, OTHER_STUDENT, OTHER_COACH, at(26, 3), Some(at(30, 9))),
            // 返却が 10/25 の日本時間 0 時（今日までの 7 日の始まり）
            video(3, STUDENT, OTHER_COACH, at(20, 0), Some(at(25, 0))),
            // 返却が 10/24 の終わり（7 日の外）
            video(
                4,
                STUDENT,
                COACH,
                at(20, 0),
                Some(at(25, 0) - std::time::Duration::from_secs(1)),
            ),
            // 返していない動画は載らない
            video(5, STUDENT, COACH, at(30, 0), None),
        ])
    }

    fn use_case(
        videos: InMemoryVideoRepository,
        lookup: RecordingLookup,
    ) -> ListReturnedVideoReviews<
        InMemoryVideoRepository,
        RecordingLookup,
        RecordingLookup,
        FixedClock,
    > {
        ListReturnedVideoReviews::new(videos, lookup.clone(), lookup, FixedClock(now()))
    }

    fn ids(list: &ReturnedVideoReviewList) -> Vec<u128> {
        list.videos.iter().map(|v| v.id().as_u128()).collect()
    }

    #[tokio::test]
    async fn lists_returned_videos_of_the_last_seven_days_latest_first_with_display_names() {
        let lookup = RecordingLookup::default();
        let list = use_case(videos(), lookup.clone())
            .execute(ListReturnedVideoReviewsQuery::default())
            .await
            .unwrap();
        assert_eq!(
            (list.from, list.to),
            (date(Month::October, 25), date(Month::October, 31)),
            "省けば今日（日本時間）までの 7 日"
        );
        assert_eq!(
            ids(&list),
            [1, 2, 3],
            "期間の外（4）と返していない動画（5）は出ない"
        );
        assert_eq!(
            list.coaches,
            BTreeMap::from([(COACH, DisplayName::new("たなかコーチ").unwrap())])
        );
        assert_eq!(
            list.students,
            BTreeMap::from([(STUDENT, DisplayName::new("あおき").unwrap())])
        );
        assert_eq!(
            *lookup.asked_coaches.lock().unwrap(),
            [BTreeSet::from([COACH, OTHER_COACH])],
            "一覧のコーチを重ねずに 1 回で問い合わせる"
        );
        assert_eq!(
            *lookup.asked_students.lock().unwrap(),
            [BTreeSet::from([STUDENT, OTHER_STUDENT])]
        );
        assert_eq!(
            list.videos
                .iter()
                .map(ReturnedVideoReviewFilter::lead_hours)
                .collect::<Vec<_>>(),
            [Some(48), Some(102), Some(120)]
        );
    }

    #[tokio::test]
    async fn the_period_and_coach_narrow_the_list() {
        let run = |query| async move {
            use_case(videos(), RecordingLookup::default())
                .execute(query)
                .await
                .unwrap()
        };
        let list = run(ListReturnedVideoReviewsQuery {
            from: Some(date(Month::October, 24)),
            to: Some(date(Month::October, 24)),
            coach_id: None,
            sport: None,
        })
        .await;
        assert_eq!(ids(&list), [4]);

        let list = run(ListReturnedVideoReviewsQuery {
            coach_id: Some(OTHER_COACH),
            ..Default::default()
        })
        .await;
        assert_eq!(ids(&list), [2, 3]);

        // from だけなら from から 7 日目まで、to だけなら to から遡って 7 日
        let list = run(ListReturnedVideoReviewsQuery {
            from: Some(date(Month::October, 20)),
            ..Default::default()
        })
        .await;
        assert_eq!(
            (list.from, list.to),
            (date(Month::October, 20), date(Month::October, 26))
        );
        assert_eq!(ids(&list), [3, 4]);
        let list = run(ListReturnedVideoReviewsQuery {
            to: Some(date(Month::October, 30)),
            ..Default::default()
        })
        .await;
        assert_eq!(
            (list.from, list.to),
            (date(Month::October, 24), date(Month::October, 30))
        );
        assert_eq!(ids(&list), [2, 3, 4]);
    }

    #[tokio::test]
    async fn the_sport_narrows_the_list_with_the_period_and_coach() {
        let mut all = videos().all();
        all.extend([
            video_in(11, Sport::Golf, STUDENT, COACH, at(28, 0), Some(at(30, 9))),
            video_in(
                12,
                Sport::Golf,
                OTHER_STUDENT,
                OTHER_COACH,
                at(20, 0),
                Some(at(26, 0)),
            ),
            // 期間の外（10/24）と返していないゴルフの動画は載らない
            video_in(13, Sport::Golf, STUDENT, COACH, at(20, 0), Some(at(24, 0))),
            video_in(14, Sport::Golf, STUDENT, COACH, at(30, 0), None),
        ]);
        let run = |query| {
            let all = all.clone();
            async move {
                use_case(
                    InMemoryVideoRepository::with_videos(all),
                    RecordingLookup::default(),
                )
                .execute(query)
                .await
                .unwrap()
            }
        };
        let list = run(ListReturnedVideoReviewsQuery {
            sport: Some(Sport::Golf),
            ..Default::default()
        })
        .await;
        assert_eq!(
            (list.from, list.to),
            (date(Month::October, 25), date(Month::October, 31)),
            "スポーツで絞っても期間を省けば今日までの 7 日"
        );
        assert_eq!(ids(&list), [11, 12], "返却の新しい順のまま");
        assert_eq!(
            list.videos
                .iter()
                .map(ReturnedVideoReviewFilter::lead_hours)
                .collect::<Vec<_>>(),
            [Some(57), Some(144)],
            "かかった時間の数え方は変わらない"
        );

        let list = run(ListReturnedVideoReviewsQuery {
            sport: Some(Sport::Golf),
            coach_id: Some(OTHER_COACH),
            ..Default::default()
        })
        .await;
        assert_eq!(ids(&list), [12]);

        let list = run(ListReturnedVideoReviewsQuery {
            from: Some(date(Month::October, 24)),
            to: Some(date(Month::October, 24)),
            coach_id: None,
            sport: Some(Sport::Golf),
        })
        .await;
        assert_eq!(ids(&list), [13]);

        let list = run(ListReturnedVideoReviewsQuery {
            sport: Some(Sport::Tennis),
            ..Default::default()
        })
        .await;
        assert_eq!(ids(&list), [1, 2, 3]);

        let list = run(ListReturnedVideoReviewsQuery {
            sport: Some(Sport::Swimming),
            ..Default::default()
        })
        .await;
        assert!(list.videos.is_empty(), "そのスポーツの返却がなければ空");
    }

    #[tokio::test]
    async fn periods_breaking_the_rule_are_invalid() {
        for (from, to, expected) in [
            (
                date(Month::October, 2),
                date(Month::October, 1),
                DomainError::ReturnedVideoReviewPeriodEndsBeforeItStarts,
            ),
            (
                date(Month::October, 1),
                date(Month::November, 1),
                DomainError::ReturnedVideoReviewPeriodTooLong { max_days: 31 },
            ),
        ] {
            let error = use_case(videos(), RecordingLookup::default())
                .execute(ListReturnedVideoReviewsQuery {
                    from: Some(from),
                    to: Some(to),
                    coach_id: None,
                    sport: None,
                })
                .await
                .unwrap_err();
            assert!(
                matches!(&error, ListReturnedVideoReviewsError::Invalid(e) if *e == expected),
                "{from}〜{to}: {error:?}"
            );
        }
        // 31 日ちょうどは受け付ける
        assert!(
            use_case(videos(), RecordingLookup::default())
                .execute(ListReturnedVideoReviewsQuery {
                    from: Some(date(Month::October, 1)),
                    to: Some(date(Month::October, 31)),
                    coach_id: None,
                    sport: None,
                })
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn no_returned_videos_is_an_empty_list_without_asking_for_display_names() {
        let lookup = RecordingLookup::default();
        let list = use_case(videos(), lookup.clone())
            .execute(ListReturnedVideoReviewsQuery {
                coach_id: Some(AccountId::from_u128(99)),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(list.videos.is_empty());
        assert!(list.coaches.is_empty() && list.students.is_empty());
        assert!(lookup.asked_coaches.lock().unwrap().is_empty());
        assert!(lookup.asked_students.lock().unwrap().is_empty());
    }
}
