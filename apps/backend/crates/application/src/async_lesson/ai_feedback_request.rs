use domain::{
    async_lesson::{VideoContentType, VideoId},
    catalog::Sport,
};

/// AI の一次フィードバックを作る材料（[`super::AiFeedbackGenerator`] に渡す）。送付した動画の ID・スポーツ・形式と、
/// 送った生徒の一言（student-app の `SubmitVideoRequest.note`。コーチへの相談内容。Gen1 では保存しない）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiFeedbackRequest {
    pub video_id: VideoId,
    pub sport: Sport,
    pub content_type: VideoContentType,
    /// 送った生徒の一言。書かなければ `None`
    pub note: Option<String>,
}
