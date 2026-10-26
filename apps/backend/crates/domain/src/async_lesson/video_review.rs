use std::time::SystemTime;

use super::{ReviewComment, VideoId, VideoReviewId};

/// 動画添削（glossary の video_review）。コーチが動画に返した添削で、動画 1 本につき 1 つ。
///
/// 作るのは [`super::Video::return_review`] だけ（返却の不変条件はそこで確かめる）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoReview {
    id: VideoReviewId,
    video_id: VideoId,
    comment: ReviewComment,
    returned_at: SystemTime,
}

impl VideoReview {
    pub(super) const fn new(
        id: VideoReviewId,
        video_id: VideoId,
        comment: ReviewComment,
        returned_at: SystemTime,
    ) -> Self {
        Self {
            id,
            video_id,
            comment,
            returned_at,
        }
    }

    pub const fn id(&self) -> VideoReviewId {
        self.id
    }

    pub const fn video_id(&self) -> VideoId {
        self.video_id
    }

    pub const fn comment(&self) -> &ReviewComment {
        &self.comment
    }

    pub const fn returned_at(&self) -> SystemTime {
        self.returned_at
    }
}
