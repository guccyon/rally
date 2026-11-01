use std::collections::{BTreeMap, BTreeSet};

use domain::{
    async_lesson::{PendingVideoReviewFilter, Video},
    identity::AccountId,
};

use super::{
    PendingVideoReviewList, VideoCoachDisplayNameLookup, VideoRepository,
    VideoStudentDisplayNameLookup,
};
use crate::{Unavailable, clock::Clock};

/// 本部が見る返却待ちの動画添削の一覧のユースケース（admin-web の `listPendingVideoReviews`。本部だけが呼ぶ）。
/// 照会だけで、何も保存しない。
///
/// コーチがまだ添削を返していない動画をすべてのコーチについて読み、domain の [`PendingVideoReviewFilter::select`] で
/// スポーツと経過時間の下限で絞って送付の古い順に並べる（経過時間の数え方と並べ方は domain だけ）。各動画には担当の
/// コーチと生徒の表示名を profile に問い合わせて（[`VideoCoachDisplayNameLookup`]・[`VideoStudentDisplayNameLookup`]）
/// 添える。誰が呼べるか（本部だけ）は境界（interface）が確かめる。
pub struct ListPendingVideoReviews<V, K, D, C> {
    videos: V,
    coaches: K,
    students: D,
    clock: C,
}

impl<V, K, D, C> ListPendingVideoReviews<V, K, D, C>
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
        filter: PendingVideoReviewFilter,
    ) -> Result<PendingVideoReviewList, Unavailable> {
        let as_of = self.clock.now();
        let videos = filter.select(self.videos.videos_awaiting_review().await?, as_of);
        if videos.is_empty() {
            return Ok(PendingVideoReviewList {
                as_of,
                videos,
                coaches: BTreeMap::new(),
                students: BTreeMap::new(),
            });
        }
        let coach_ids: BTreeSet<AccountId> = videos.iter().map(Video::coach_id).collect();
        let student_ids: BTreeSet<AccountId> = videos.iter().map(Video::student_id).collect();
        let coaches = self.coaches.find_coach_display_names(&coach_ids).await?;
        let students = self
            .students
            .find_student_display_names(&student_ids)
            .await?;
        Ok(PendingVideoReviewList {
            as_of,
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
        time::{Duration, SystemTime},
    };

    use domain::{
        async_lesson::{ReviewComment, VideoId, VideoReviewId},
        catalog::Sport,
        profile::DisplayName,
    };

    use super::*;
    use crate::in_memory::{FixedClock, InMemoryVideoRepository};

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn now() -> SystemTime {
        // 2026-10-31T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
    }

    fn hours_ago(hours: u64) -> SystemTime {
        now() - Duration::from_secs(hours * 3600)
    }

    /// `id` の動画を `student` が `coach` に `sport` で `hours` 時間前に送る。`returned` なら照会の時刻に返却済み。
    fn video(
        id: u128,
        student: AccountId,
        coach: AccountId,
        sport: Sport,
        hours: u64,
        returned: bool,
    ) -> Video {
        Video::restore(
            VideoId::from_u128(id),
            student,
            coach,
            sport,
            hours_ago(hours),
            returned.then(|| {
                (
                    VideoReviewId::from_u128(id + 100),
                    ReviewComment::new("テイクバックが早い").unwrap(),
                    now(),
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
            video(1, STUDENT, COACH, Sport::Tennis, 10, false),
            video(2, OTHER_STUDENT, OTHER_COACH, Sport::Golf, 72, false),
            // 返却済みは出ない
            video(3, STUDENT, COACH, Sport::Tennis, 200, true),
            video(4, OTHER_STUDENT, COACH, Sport::Tennis, 48, false),
        ])
    }

    fn use_case(
        videos: InMemoryVideoRepository,
        lookup: RecordingLookup,
    ) -> ListPendingVideoReviews<
        InMemoryVideoRepository,
        RecordingLookup,
        RecordingLookup,
        FixedClock,
    > {
        ListPendingVideoReviews::new(videos, lookup.clone(), lookup, FixedClock(now()))
    }

    fn ids(list: &PendingVideoReviewList) -> Vec<u128> {
        list.videos.iter().map(|v| v.id().as_u128()).collect()
    }

    #[tokio::test]
    async fn lists_pending_videos_of_all_coaches_oldest_first_with_display_names() {
        let lookup = RecordingLookup::default();
        let list = use_case(videos(), lookup.clone())
            .execute(PendingVideoReviewFilter::default())
            .await
            .unwrap();
        assert_eq!(list.as_of, now());
        assert_eq!(ids(&list), [2, 4, 1], "返却済み（3）は出ない");
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
    }

    #[tokio::test]
    async fn sport_and_min_elapsed_hours_narrow_the_list() {
        let list = use_case(videos(), RecordingLookup::default())
            .execute(PendingVideoReviewFilter {
                sport: None,
                min_elapsed_hours: 48,
            })
            .await
            .unwrap();
        assert_eq!(ids(&list), [2, 4], "48 時間ちょうど（4）は含む");

        let list = use_case(videos(), RecordingLookup::default())
            .execute(PendingVideoReviewFilter {
                sport: Some(Sport::Tennis),
                min_elapsed_hours: 48,
            })
            .await
            .unwrap();
        assert_eq!(ids(&list), [4]);
    }

    #[tokio::test]
    async fn no_pending_videos_is_an_empty_list_without_asking_for_display_names() {
        let lookup = RecordingLookup::default();
        let list = use_case(videos(), lookup.clone())
            .execute(PendingVideoReviewFilter {
                sport: Some(Sport::Swimming),
                min_elapsed_hours: 0,
            })
            .await
            .unwrap();
        assert!(list.videos.is_empty());
        assert!(list.coaches.is_empty() && list.students.is_empty());
        assert!(lookup.asked_coaches.lock().unwrap().is_empty());
        assert!(lookup.asked_students.lock().unwrap().is_empty());
    }
}
