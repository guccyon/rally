/// 本部が生徒を「返却待ちの動画のある生徒」に絞るときの条件（admin-web の `listStudents` の `has_pending_video`）。
/// 「返却待ちの動画がある」をどう決めるかはここだけに置く。
///
/// - `has_pending_video` が `true`: 生徒が送った動画のうち、照会した時刻に返却を待っている動画の数（数え方は
///   [`super::PendingVideoReviewFilter`] の既定の条件の [`super::PendingVideoReviewFilter::count`]。`getStudentActivity` の
///   `pending_video_review_count` と同じ数で、`listPendingVideoReviews` にその生徒の `student_id` だけを付けたときに載る動画の
///   数）が 1 件以上の生徒だけ
/// - `false`（既定）: 絞らない（どの生徒も通す）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PendingVideoStudentFilter(bool);

impl PendingVideoStudentFilter {
    /// `has_pending_video` が `true` なら返却待ちの動画のある生徒だけに絞る条件。`false` なら絞らない。
    pub const fn new(has_pending_video: bool) -> Self {
        Self(has_pending_video)
    }

    /// 返却待ちの動画のある生徒だけに絞るか。
    pub const fn has_pending_video(&self) -> bool {
        self.0
    }

    /// 照会した時刻に返却を待っている動画が `pending_review_count` 件の生徒がこの条件に合うか。
    pub const fn matches(&self, pending_review_count: u32) -> bool {
        !self.0 || pending_review_count > 0
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;
    use crate::{
        async_lesson::{PendingVideoReviewFilter, ReviewComment, Video, VideoId, VideoReviewId},
        catalog::Sport,
        identity::AccountId,
    };

    const HOUR: Duration = Duration::from_secs(3600);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);

    fn now() -> SystemTime {
        // 2026-11-01T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200)
    }

    fn pending(id: u128, student: AccountId, submitted_at: SystemTime) -> Video {
        Video::submit(
            VideoId::from_u128(id),
            student,
            COACH,
            Sport::Tennis,
            submitted_at,
        )
    }

    fn returned(id: u128, student: AccountId, submitted_at: SystemTime) -> Video {
        Video::restore(
            VideoId::from_u128(id),
            student,
            COACH,
            Sport::Tennis,
            submitted_at,
            Some((
                VideoReviewId::from_u128(id + 100),
                ReviewComment::new("テイクバックが早い").unwrap(),
                submitted_at + HOUR,
            )),
            None,
        )
        .unwrap()
    }

    /// 生徒 `student_id` の `videos` を `getStudentActivity` の `pending_video_review_count` と同じく数える。
    fn pending_count(student_id: AccountId, videos: &[Video], at: SystemTime) -> u32 {
        let of_student: Vec<Video> = videos
            .iter()
            .filter(|video| video.student_id() == student_id)
            .cloned()
            .collect();
        u32::try_from(PendingVideoReviewFilter::default().count(&of_student, at)).unwrap()
    }

    #[test]
    fn has_pending_video_matches_only_students_with_a_pending_video() {
        let filter = PendingVideoStudentFilter::new(true);
        assert!(filter.has_pending_video());
        assert!(!filter.matches(0), "返却待ちの動画がなければ載らない");
        assert!(filter.matches(1), "1 件でもあれば載る");
        assert!(filter.matches(u32::MAX));
    }

    #[test]
    fn has_pending_video_is_the_same_as_the_pending_list_of_the_student_being_non_empty() {
        let videos = vec![
            // 生徒は返却待ちが 1 本（照会の直前に送った）と返却済みが 1 本
            pending(1, STUDENT, now() - HOUR),
            returned(2, STUDENT, now() - 50 * HOUR),
            // 他の生徒は返却済みだけ
            returned(3, OTHER_STUDENT, now() - 72 * HOUR),
        ];
        let filter = PendingVideoStudentFilter::new(true);
        for student_id in [STUDENT, OTHER_STUDENT, COACH] {
            for at in [now(), now() - 2 * HOUR, now() + 100 * HOUR] {
                let listed = PendingVideoReviewFilter {
                    student_id: Some(student_id),
                    ..PendingVideoReviewFilter::default()
                }
                .select(videos.clone(), at);
                assert_eq!(
                    filter.matches(pending_count(student_id, &videos, at)),
                    !listed.is_empty(),
                    "{student_id:?} {at:?}"
                );
            }
        }
        assert!(filter.matches(pending_count(STUDENT, &videos, now())));
        assert!(
            !filter.matches(pending_count(OTHER_STUDENT, &videos, now())),
            "返却済みの動画だけなら載らない"
        );
    }

    #[test]
    fn not_has_pending_video_matches_every_student() {
        let filter = PendingVideoStudentFilter::default();
        assert_eq!(filter, PendingVideoStudentFilter::new(false));
        assert!(!filter.has_pending_video());
        assert!(filter.matches(0));
        assert!(filter.matches(3));
    }
}
