use domain::{
    DomainError,
    async_lesson::{Video, VideoId},
    identity::AccountId,
};

use super::VideoRepository;
use crate::Unavailable;

/// 添削結果の取得の入力。`account_id` は認証したアカウント（動画の当事者かは domain が確かめる）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetVideoReviewQuery {
    pub account_id: AccountId,
    pub video_id: VideoId,
}

#[derive(Debug)]
pub enum GetVideoReviewError {
    /// 動画がない
    VideoNotFound,
    /// 動画の当事者でない（[`DomainError::NotAVideoParty`]）
    Invalid(DomainError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// 添削結果の取得のユースケース（student-app の `getVideoReview`）。照会だけで、何も保存しない。
///
/// 動画を返す。添削は [`Video::review`] で、コーチがまだ返していなければ `None`（未返却）。読めるのは動画の当事者
/// （送った生徒・送り先のコーチ）だけ。
pub struct GetVideoReview<V> {
    videos: V,
}

impl<V> GetVideoReview<V>
where
    V: VideoRepository,
{
    pub fn new(videos: V) -> Self {
        Self { videos }
    }

    pub async fn execute(&self, query: GetVideoReviewQuery) -> Result<Video, GetVideoReviewError> {
        let video = self
            .videos
            .find(query.video_id)
            .await
            .map_err(GetVideoReviewError::Unavailable)?
            .ok_or(GetVideoReviewError::VideoNotFound)?;
        video
            .ensure_visible_to(query.account_id)
            .map_err(GetVideoReviewError::Invalid)?;
        Ok(video)
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use domain::{async_lesson::VideoStatus, catalog::Sport};

    use super::*;
    use crate::in_memory::InMemoryVideoRepository;

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(7);
    const VIDEO: VideoId = VideoId::from_u128(10);

    fn use_case() -> GetVideoReview<InMemoryVideoRepository> {
        GetVideoReview::new(InMemoryVideoRepository::with_videos([Video::submit(
            VIDEO,
            STUDENT,
            COACH,
            Sport::Tennis,
            SystemTime::UNIX_EPOCH,
        )]))
    }

    fn query(account_id: AccountId, video_id: VideoId) -> GetVideoReviewQuery {
        GetVideoReviewQuery {
            account_id,
            video_id,
        }
    }

    #[tokio::test]
    async fn a_video_before_its_review_is_returned_has_no_review() {
        for account in [STUDENT, COACH] {
            let video = use_case().execute(query(account, VIDEO)).await.unwrap();
            assert_eq!(video.status(), VideoStatus::Submitted);
            assert_eq!(video.review(), None);
        }
    }

    #[tokio::test]
    async fn only_the_parties_can_read_the_review() {
        let result = use_case()
            .execute(query(AccountId::from_u128(2), VIDEO))
            .await;
        assert!(
            matches!(
                result,
                Err(GetVideoReviewError::Invalid(DomainError::NotAVideoParty))
            ),
            "{result:?}"
        );
        let result = use_case()
            .execute(query(STUDENT, VideoId::from_u128(99)))
            .await;
        assert!(
            matches!(result, Err(GetVideoReviewError::VideoNotFound)),
            "{result:?}"
        );
    }
}
