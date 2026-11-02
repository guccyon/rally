use std::time::SystemTime;

use super::Video;
use crate::{catalog::Sport, identity::AccountId};

/// 本部が見る「返却を待っている動画添削」の選び方（admin-web の `listPendingVideoReviews`。規則の正本は
/// `specs/contracts/api/admin-web/openapi.yaml` の `listPendingVideoReviews`）。
///
/// コーチがまだ添削を返していない動画（[`super::VideoStatus::Submitted`]）を、任意のスポーツ `sport` と、送付からの
/// 経過時間の下限 `min_elapsed_hours`（時間。0 なら下限なし）と、任意のコーチ `coach_id` で絞る。経過時間の数え方（[`Self::elapsed_hours`]）と
/// 並べ方（送付の古い順）はこの型の 1 か所だけに置く。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PendingVideoReviewFilter {
    /// あれば、このスポーツの動画だけ
    pub sport: Option<Sport>,
    /// 送付からの経過時間（[`Self::elapsed_hours`]）がこの時間以上の動画だけ。0 なら絞らない
    pub min_elapsed_hours: u32,
    /// あれば、このコーチ（動画の送り先）の動画だけ
    pub coach_id: Option<AccountId>,
}

impl PendingVideoReviewFilter {
    /// `video` の送付から `now` までの経過時間（時間。1 時間に満たない端数は切り捨てる）。送付が `now` より後なら 0。
    /// 返却待ちの動画の [`Video::review_lead_hours`]（数え方はそこ 1 か所）。
    pub fn elapsed_hours(video: &Video, now: SystemTime) -> u64 {
        video.review_lead_hours(now)
    }

    /// `video` が `now` に照会したときにこの条件の返却待ちの動画か（添削を返していない・スポーツが合う・コーチが合う・
    /// 経過時間が下限以上）。
    pub fn matches(&self, video: &Video, now: SystemTime) -> bool {
        video.review().is_none()
            && video.is_in_sport(self.sport)
            && self
                .coach_id
                .is_none_or(|coach_id| video.coach_id() == coach_id)
            && Self::elapsed_hours(video, now) >= u64::from(self.min_elapsed_hours)
    }

    /// `videos` から条件に合う返却待ちの動画だけを選び、送付の古い順（経過の長い順。同じ時刻は動画 ID の順）に並べる。
    pub fn select(&self, videos: Vec<Video>, now: SystemTime) -> Vec<Video> {
        let mut pending: Vec<Video> = videos
            .into_iter()
            .filter(|video| self.matches(video, now))
            .collect();
        pending.sort_by_key(|video| (video.submitted_at(), video.id()));
        pending
    }

    /// `videos` のうち条件に合う返却待ちの動画の数（[`Self::matches`] で数える。本部が見る生徒 1 人の利用の様子の
    /// 返却待ちの数も、既定の条件でここを使う）。
    pub fn count(&self, videos: &[Video], now: SystemTime) -> usize {
        videos
            .iter()
            .filter(|video| self.matches(video, now))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        async_lesson::{ReviewComment, VideoId, VideoReviewId},
        identity::AccountId,
    };

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(2);
    const OTHER_COACH: AccountId = AccountId::from_u128(3);

    fn now() -> SystemTime {
        // 2026-10-31T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
    }

    fn hours_ago(hours: u64) -> SystemTime {
        now() - Duration::from_secs(hours * 3600)
    }

    fn pending(id: u128, sport: Sport, submitted_at: SystemTime) -> Video {
        Video::submit(VideoId::from_u128(id), STUDENT, COACH, sport, submitted_at)
    }

    fn pending_to(id: u128, coach: AccountId, sport: Sport, submitted_at: SystemTime) -> Video {
        Video::submit(VideoId::from_u128(id), STUDENT, coach, sport, submitted_at)
    }

    fn returned(id: u128, submitted_at: SystemTime) -> Video {
        Video::restore(
            VideoId::from_u128(id),
            STUDENT,
            COACH,
            Sport::Tennis,
            submitted_at,
            Some((
                VideoReviewId::from_u128(id + 100),
                ReviewComment::new("テイクバックが早い").unwrap(),
                now(),
            )),
            None,
        )
        .unwrap()
    }

    #[test]
    fn elapsed_hours_are_whole_hours_rounded_down() {
        let at = |secs: u64| pending(1, Sport::Tennis, now() - Duration::from_secs(secs));
        for (secs, hours) in [
            (0, 0),
            (3599, 0),
            (3600, 1),
            (48 * 3600 - 1, 47),
            (48 * 3600, 48),
        ] {
            assert_eq!(
                PendingVideoReviewFilter::elapsed_hours(&at(secs), now()),
                hours,
                "{secs} 秒前"
            );
        }
        // 送付が照会より後（時計のずれ）は 0
        let future = pending(1, Sport::Tennis, now() + Duration::from_secs(10));
        assert_eq!(PendingVideoReviewFilter::elapsed_hours(&future, now()), 0);
    }

    #[test]
    fn returned_videos_are_never_pending() {
        let filter = PendingVideoReviewFilter::default();
        assert!(filter.matches(&pending(1, Sport::Tennis, hours_ago(100)), now()));
        assert!(!filter.matches(&returned(2, hours_ago(100)), now()));
    }

    #[test]
    fn min_elapsed_hours_is_inclusive() {
        let filter = PendingVideoReviewFilter {
            sport: None,
            min_elapsed_hours: 48,
            coach_id: None,
        };
        let just = pending(1, Sport::Tennis, hours_ago(48));
        let short = pending(2, Sport::Tennis, hours_ago(48) + Duration::from_secs(1));
        assert!(filter.matches(&just, now()), "48 時間ちょうどは含む");
        assert!(
            !filter.matches(&short, now()),
            "47 時間 59 分 59 秒は含まない"
        );
    }

    #[test]
    fn sport_keeps_only_videos_of_that_sport() {
        let filter = PendingVideoReviewFilter {
            sport: Some(Sport::Golf),
            min_elapsed_hours: 0,
            coach_id: None,
        };
        assert!(filter.matches(&pending(1, Sport::Golf, now()), now()));
        assert!(!filter.matches(&pending(2, Sport::Tennis, now()), now()));
    }

    #[test]
    fn coach_id_keeps_only_videos_sent_to_that_coach() {
        let filter = PendingVideoReviewFilter {
            sport: None,
            min_elapsed_hours: 0,
            coach_id: Some(OTHER_COACH),
        };
        assert!(filter.matches(&pending_to(1, OTHER_COACH, Sport::Tennis, now()), now()));
        assert!(!filter.matches(&pending_to(2, COACH, Sport::Tennis, now()), now()));
        // 返却済みはコーチが合っても出ない（returned は COACH 宛て）
        let filter = PendingVideoReviewFilter {
            coach_id: Some(COACH),
            ..PendingVideoReviewFilter::default()
        };
        assert!(!filter.matches(&returned(3, hours_ago(100)), now()));
    }

    #[test]
    fn coach_id_combines_with_sport_and_min_elapsed_hours_keeping_the_order() {
        let videos = vec![
            pending_to(1, COACH, Sport::Tennis, hours_ago(10)),
            pending_to(2, OTHER_COACH, Sport::Tennis, hours_ago(90)),
            pending_to(3, COACH, Sport::Golf, hours_ago(72)),
            pending_to(5, COACH, Sport::Tennis, hours_ago(50)),
            pending_to(4, COACH, Sport::Tennis, hours_ago(50)),
            pending_to(6, COACH, Sport::Tennis, hours_ago(60)),
        ];
        let ids = |filter: PendingVideoReviewFilter| -> Vec<u128> {
            filter
                .select(videos.clone(), now())
                .iter()
                .map(|video| video.id().as_u128())
                .collect()
        };
        assert_eq!(
            ids(PendingVideoReviewFilter {
                coach_id: Some(COACH),
                ..PendingVideoReviewFilter::default()
            }),
            [3, 6, 4, 5, 1]
        );
        assert_eq!(
            ids(PendingVideoReviewFilter {
                sport: Some(Sport::Tennis),
                min_elapsed_hours: 48,
                coach_id: Some(COACH),
            }),
            [6, 4, 5]
        );
        // 返却待ちのないコーチは空
        assert!(
            ids(PendingVideoReviewFilter {
                coach_id: Some(AccountId::from_u128(99)),
                ..PendingVideoReviewFilter::default()
            })
            .is_empty()
        );
    }

    #[test]
    fn count_is_the_number_of_videos_select_keeps() {
        let videos = vec![
            pending(1, Sport::Tennis, hours_ago(10)),
            returned(2, hours_ago(200)),
            pending(3, Sport::Golf, hours_ago(72)),
            returned(4, hours_ago(1)),
        ];
        let filter = PendingVideoReviewFilter::default();
        assert_eq!(filter.count(&videos, now()), 2, "返却済みは数えない");
        assert_eq!(
            filter.count(&videos, now()),
            filter.select(videos.clone(), now()).len()
        );
        let filter = PendingVideoReviewFilter {
            min_elapsed_hours: 48,
            ..PendingVideoReviewFilter::default()
        };
        assert_eq!(filter.count(&videos, now()), 1);
        assert_eq!(PendingVideoReviewFilter::default().count(&[], now()), 0);
    }

    #[test]
    fn select_keeps_pending_videos_oldest_first_then_by_id() {
        let videos = vec![
            pending(1, Sport::Tennis, hours_ago(10)),
            returned(2, hours_ago(200)),
            pending(3, Sport::Golf, hours_ago(72)),
            // 同じ時刻は ID の順
            pending(5, Sport::Tennis, hours_ago(50)),
            pending(4, Sport::Soccer, hours_ago(50)),
            pending(6, Sport::Tennis, hours_ago(1)),
        ];
        let ids = |filter: PendingVideoReviewFilter| -> Vec<u128> {
            filter
                .select(videos.clone(), now())
                .iter()
                .map(|video| video.id().as_u128())
                .collect()
        };
        assert_eq!(ids(PendingVideoReviewFilter::default()), [3, 4, 5, 1, 6]);
        assert_eq!(
            ids(PendingVideoReviewFilter {
                sport: None,
                min_elapsed_hours: 48,
                coach_id: None,
            }),
            [3, 4, 5]
        );
        assert_eq!(
            ids(PendingVideoReviewFilter {
                sport: Some(Sport::Tennis),
                min_elapsed_hours: 48,
                coach_id: None,
            }),
            [5]
        );
    }
}
