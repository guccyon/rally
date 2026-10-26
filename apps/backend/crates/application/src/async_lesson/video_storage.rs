use std::future::Future;

use domain::async_lesson::{VideoContentType, VideoId};

use crate::Unavailable;

/// 動画本体の保存先（S3 への直接アップロード相当）のポート。動画本体はクライアントが保存先に直接 PUT し、
/// バックエンドは通さない。
///
/// Gen1 の実装は infra のモック（`MockVideoStorage`。固定の形の URL を返す）と、テスト用の
/// [`crate::in_memory::FixedVideoStorage`]。動画の変換は扱わない。
pub trait VideoStorage: Send + Sync {
    /// 動画 `video_id`（形式は `content_type`）の本体を PUT する URL（署名付き URL 相当。クライアントには不透明）を
    /// 発行する。
    fn issue_upload_url(
        &self,
        video_id: VideoId,
        content_type: VideoContentType,
    ) -> impl Future<Output = Result<String, Unavailable>> + Send;
}
