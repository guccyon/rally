use std::{
    cmp::Reverse,
    time::{Duration, SystemTime},
};

use super::Video;
use crate::{DomainError, identity::AccountId};

/// 本部が見る「返した動画添削の記録」の選び方（admin-web の `listReturnedVideoReviews`。規則の正本は
/// `specs/contracts/api/admin-web/openapi.yaml` の `listReturnedVideoReviews`）。
///
/// コーチが添削を返した動画（[`super::VideoStatus::Reviewed`]）を、返却の時刻が期間（開始 `starts_at` を含み、終了
/// `ends_at` を含まない）に入るもの・任意のコーチ `coach_id` の動画に絞る。期間は日付（日本時間）の始まりの 0 時から
/// 終わりの日付の翌日 0 時までにあたり、日付から時刻への変換（タイムゾーン）は application が決める。ここは期間の長さの
/// 規則（1 日以上 [`Self::MAX_DAYS`] 日まで）と、かかった時間（[`Self::lead_hours`]）と、並べ方（返却の新しい順）を持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReturnedVideoReviewFilter {
    starts_at: SystemTime,
    ends_at: SystemTime,
    coach_id: Option<AccountId>,
}

impl ReturnedVideoReviewFilter {
    /// 1 日の長さ（日本時間に夏時間はないので、どの日も 24 時間）
    pub const DAY: Duration = Duration::from_secs(24 * 3600);
    /// 日付を省いたときの期間の日数（終わりの日を含めて 7 日）
    pub const DEFAULT_DAYS: u16 = 7;
    /// 期間の日数の上限（始まりの日と終わりの日を含めて 31 日）
    pub const MAX_DAYS: u16 = 31;

    /// 返却の時刻が `starts_at`（含む）から `ends_at`（含まない）までで、`coach_id` があればそのコーチの動画。
    /// 終了が開始より後でなければ [`DomainError::ReturnedVideoReviewPeriodEndsBeforeItStarts`]、[`Self::MAX_DAYS`] 日
    /// より長ければ [`DomainError::ReturnedVideoReviewPeriodTooLong`]。
    pub fn new(
        starts_at: SystemTime,
        ends_at: SystemTime,
        coach_id: Option<AccountId>,
    ) -> Result<Self, DomainError> {
        let length = match ends_at.duration_since(starts_at) {
            Ok(length) if !length.is_zero() => length,
            _ => return Err(DomainError::ReturnedVideoReviewPeriodEndsBeforeItStarts),
        };
        if length > Self::DAY * u32::from(Self::MAX_DAYS) {
            return Err(DomainError::ReturnedVideoReviewPeriodTooLong {
                max_days: Self::MAX_DAYS,
            });
        }
        Ok(Self {
            starts_at,
            ends_at,
            coach_id,
        })
    }

    pub const fn starts_at(&self) -> SystemTime {
        self.starts_at
    }

    pub const fn ends_at(&self) -> SystemTime {
        self.ends_at
    }

    pub const fn coach_id(&self) -> Option<AccountId> {
        self.coach_id
    }

    /// 返却済みの `video` の、送付から返却までにかかった時間（時間。端数は切り捨て。数え方は
    /// [`Video::review_lead_hours`] の 1 か所）。返していなければ `None`。
    pub fn lead_hours(video: &Video) -> Option<u64> {
        video
            .review()
            .map(|review| video.review_lead_hours(review.returned_at()))
    }

    /// `video` がこの条件の返却済みの動画か（添削を返している・返却が期間に入る・コーチが合う）。
    pub fn matches(&self, video: &Video) -> bool {
        video.review().is_some_and(|review| {
            self.starts_at <= review.returned_at() && review.returned_at() < self.ends_at
        }) && self
            .coach_id
            .is_none_or(|coach_id| video.coach_id() == coach_id)
    }

    /// `videos` から条件に合う返却済みの動画だけを選び、返却の新しい順（同じ時刻は動画 ID の順）に並べる。
    pub fn select(&self, videos: Vec<Video>) -> Vec<Video> {
        let mut returned: Vec<Video> = videos
            .into_iter()
            .filter(|video| self.matches(video))
            .collect();
        returned.sort_by_key(|video| {
            (
                Reverse(video.review().map(|review| review.returned_at())),
                video.id(),
            )
        });
        returned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        async_lesson::{ReviewComment, VideoId, VideoReviewId},
        catalog::Sport,
    };

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(2);
    const OTHER_COACH: AccountId = AccountId::from_u128(3);
    const HOUR: Duration = Duration::from_secs(3600);

    fn day(n: u32) -> SystemTime {
        SystemTime::UNIX_EPOCH + ReturnedVideoReviewFilter::DAY * n
    }

    fn pending(id: u128, submitted_at: SystemTime) -> Video {
        Video::submit(
            VideoId::from_u128(id),
            STUDENT,
            COACH,
            Sport::Tennis,
            submitted_at,
        )
    }

    /// `id` の動画を `coach` に `submitted_at` に送り、`returned_at` に返した。
    fn returned(
        id: u128,
        coach: AccountId,
        submitted_at: SystemTime,
        returned_at: SystemTime,
    ) -> Video {
        Video::restore(
            VideoId::from_u128(id),
            STUDENT,
            coach,
            Sport::Tennis,
            submitted_at,
            Some((
                VideoReviewId::from_u128(id + 100),
                ReviewComment::new("テイクバックが早い").unwrap(),
                returned_at,
            )),
            None,
        )
        .unwrap()
    }

    fn filter(from: u32, to: u32, coach_id: Option<AccountId>) -> ReturnedVideoReviewFilter {
        ReturnedVideoReviewFilter::new(day(from), day(to), coach_id).unwrap()
    }

    #[test]
    fn accepts_one_to_max_days() {
        let one = filter(10, 11, Some(COACH));
        assert_eq!(
            (one.starts_at(), one.ends_at(), one.coach_id()),
            (day(10), day(11), Some(COACH))
        );
        assert!(
            ReturnedVideoReviewFilter::new(
                day(10),
                day(10 + u32::from(ReturnedVideoReviewFilter::MAX_DAYS)),
                None
            )
            .is_ok()
        );
    }

    #[test]
    fn rejects_periods_that_end_before_or_when_they_start() {
        for ends_at in [day(10), day(9)] {
            assert_eq!(
                ReturnedVideoReviewFilter::new(day(10), ends_at, None),
                Err(DomainError::ReturnedVideoReviewPeriodEndsBeforeItStarts)
            );
        }
    }

    #[test]
    fn rejects_periods_longer_than_max_days() {
        let ends_at = day(10 + u32::from(ReturnedVideoReviewFilter::MAX_DAYS));
        assert_eq!(
            ReturnedVideoReviewFilter::new(day(10), ends_at + Duration::from_secs(1), None),
            Err(DomainError::ReturnedVideoReviewPeriodTooLong { max_days: 31 })
        );
    }

    #[test]
    fn lead_hours_are_whole_hours_from_submission_to_return() {
        let video = returned(1, COACH, day(10), day(12) - Duration::from_secs(1));
        assert_eq!(
            ReturnedVideoReviewFilter::lead_hours(&video),
            Some(47),
            "47 時間 59 分 59 秒は 47"
        );
        let video = returned(1, COACH, day(10), day(12));
        assert_eq!(ReturnedVideoReviewFilter::lead_hours(&video), Some(48));
        assert_eq!(
            ReturnedVideoReviewFilter::lead_hours(&pending(2, day(10))),
            None
        );
    }

    #[test]
    fn matches_returned_videos_whose_return_is_in_the_period() {
        let filter = filter(10, 17, None);
        // 返却が期間の始まりちょうどは含み、終わりちょうどは含まない。送付が期間より前でもよい
        assert!(filter.matches(&returned(1, COACH, day(1), day(10))));
        assert!(filter.matches(&returned(
            2,
            COACH,
            day(9),
            day(17) - Duration::from_nanos(1)
        )));
        assert!(!filter.matches(&returned(3, COACH, day(10), day(17))));
        assert!(!filter.matches(&returned(
            4,
            COACH,
            day(1),
            day(10) - Duration::from_nanos(1)
        )));
        // 返していない動画は、送付が期間に入っていても含まない
        assert!(!filter.matches(&pending(5, day(11))));
    }

    #[test]
    fn coach_keeps_only_videos_of_that_coach() {
        let filter = filter(10, 17, Some(COACH));
        assert!(filter.matches(&returned(1, COACH, day(10), day(11))));
        assert!(!filter.matches(&returned(2, OTHER_COACH, day(10), day(11))));
    }

    #[test]
    fn select_keeps_returned_videos_latest_return_first_then_by_id() {
        let videos = vec![
            returned(1, COACH, day(9), day(11)),
            returned(2, OTHER_COACH, day(10), day(15)),
            pending(3, day(12)),
            // 同じ返却の時刻は ID の順
            returned(5, COACH, day(12), day(13) + HOUR),
            returned(4, OTHER_COACH, day(11), day(13) + HOUR),
            // 期間の外
            returned(6, COACH, day(1), day(2)),
        ];
        let ids = |filter: ReturnedVideoReviewFilter| -> Vec<u128> {
            filter
                .select(videos.clone())
                .iter()
                .map(|video| video.id().as_u128())
                .collect()
        };
        assert_eq!(ids(filter(10, 17, None)), [2, 4, 5, 1]);
        assert_eq!(ids(filter(10, 17, Some(COACH))), [5, 1]);
        assert_eq!(ids(filter(20, 21, None)), Vec::<u128>::new());
    }
}
