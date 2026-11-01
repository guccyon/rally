//! async-lesson: 非同期レッスン（specs/context-map.yaml）。
//!
//! 生徒が送る動画（glossary の async_lesson。[`Video`]）と、コーチが添削して返すこと（glossary の video_review。
//! [`VideoReview`]）と、添削の前に AI が返す一次フィードバック（glossary の ai_feedback。[`AiFeedback`]）。動画本体の保存先（S3 への直接アップロード）は application のポートで、ここでは扱わない。
//! 添削に着手してよいか（ゴルフはワンショット決済が確定した動画だけ。不変条件 2）の規則は
//! [`crate::entitlement::ContractType::may_start_review`] の 1 か所に置く。

mod ai_feedback;
mod pending_video_review_filter;
mod returned_video_review_filter;
mod review_comment;
mod video;
mod video_content_type;
mod video_id;
mod video_lesson_menu;
mod video_review;
mod video_review_id;
mod video_status;

pub use ai_feedback::AiFeedback;
pub use pending_video_review_filter::PendingVideoReviewFilter;
pub use returned_video_review_filter::ReturnedVideoReviewFilter;
pub use review_comment::ReviewComment;
pub use video::Video;
pub use video_content_type::VideoContentType;
pub use video_id::VideoId;
pub use video_lesson_menu::VideoLessonMenu;
pub use video_review::VideoReview;
pub use video_review_id::VideoReviewId;
pub use video_status::VideoStatus;
