//! 動画の保存先（S3 への直接アップロード）のモック（Gen1 の外部連携はすべてモック）。

use application::{
    Unavailable,
    async_lesson::{VideoStorage, video_id_uuid},
};
use domain::async_lesson::{VideoContentType, VideoId};

/// [`VideoStorage`] のモック。実際の保存先は持たず、動画ごとに固定の形の URL
/// `https://mock-storage.invalid/videos/<動画 ID>` を返す（署名・有効期限はない。`.invalid` は解決されない名前）。
/// URL に PUT しても何も保存されない。
#[derive(Debug, Clone, Copy, Default)]
pub struct MockVideoStorage;

impl MockVideoStorage {
    /// 発行する URL の、動画 ID の前まで。
    pub const URL_PREFIX: &'static str = "https://mock-storage.invalid/videos/";
}

impl VideoStorage for MockVideoStorage {
    async fn issue_upload_url(
        &self,
        video_id: VideoId,
        _content_type: VideoContentType,
    ) -> Result<String, Unavailable> {
        Ok(format!(
            "{}{}",
            Self::URL_PREFIX,
            video_id_uuid::format(video_id)
        ))
    }
}
