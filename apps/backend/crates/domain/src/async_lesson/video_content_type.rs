/// 送付できる動画の形式。外部での文字列表現（契約の `content_type`）は境界のアダプタが決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoContentType {
    /// MP4
    Mp4,
    /// QuickTime（iOS の撮影の既定）
    QuickTime,
}

impl VideoContentType {
    pub const ALL: [VideoContentType; 2] = [VideoContentType::Mp4, VideoContentType::QuickTime];
}
