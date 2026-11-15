//! 動画の保存先のモック（`MockVideoStorage`）の検査。動画ごとに固定の形の URL を返すことを確かめる。

use application::async_lesson::VideoStorage;
use domain::async_lesson::{VideoContentType, VideoId};
use infra::MockVideoStorage;

#[tokio::test]
async fn issues_a_fixed_url_per_video() {
    let video = VideoId::from_u128(0x0123_4567_89ab_4def_8123_4567_89ab_cdef);
    for content_type in VideoContentType::ALL {
        assert_eq!(
            MockVideoStorage
                .issue_upload_url(video, content_type)
                .await
                .unwrap(),
            "https://mock-storage.invalid/videos/01234567-89ab-4def-8123-456789abcdef"
        );
    }
    let other = MockVideoStorage
        .issue_upload_url(VideoId::from_u128(1), VideoContentType::Mp4)
        .await
        .unwrap();
    assert!(other.starts_with(MockVideoStorage::URL_PREFIX), "{other}");
    assert!(
        other.ends_with("00000000-0000-0000-0000-000000000001"),
        "{other}"
    );
}
