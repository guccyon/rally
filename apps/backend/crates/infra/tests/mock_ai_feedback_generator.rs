//! AI の一次フィードバックのモック（`MockAiFeedbackGenerator`）の検査。スポーツごとの決まった文を返すことを確かめる。

use std::collections::BTreeSet;

use application::async_lesson::{AiFeedbackGenerator, AiFeedbackRequest};
use domain::{
    async_lesson::{VideoContentType, VideoId},
    identity::Sport,
};
use infra::MockAiFeedbackGenerator;

fn request(sport: Sport, video: u128, note: Option<&str>) -> AiFeedbackRequest {
    AiFeedbackRequest {
        video_id: VideoId::from_u128(video),
        sport,
        content_type: VideoContentType::Mp4,
        note: note.map(str::to_string),
    }
}

#[tokio::test]
async fn returns_a_fixed_comment_per_sport() {
    let mut comments = BTreeSet::new();
    for sport in Sport::ALL {
        let comment = MockAiFeedbackGenerator
            .generate(&request(sport, 1, None))
            .await
            .unwrap();
        assert_eq!(comment, MockAiFeedbackGenerator::comment_for(sport));
        assert!(!comment.trim().is_empty(), "{sport:?}");
        comments.insert(comment);
    }
    assert_eq!(comments.len(), Sport::ALL.len(), "スポーツごとに違う文");
}

#[tokio::test]
async fn the_comment_depends_only_on_the_sport() {
    let a = MockAiFeedbackGenerator
        .generate(&request(Sport::Swimming, 1, None))
        .await
        .unwrap();
    let mut other = request(Sport::Swimming, 2, Some("クロールを見てほしい"));
    other.content_type = VideoContentType::QuickTime;
    let b = MockAiFeedbackGenerator.generate(&other).await.unwrap();
    assert_eq!(a, b);
}
