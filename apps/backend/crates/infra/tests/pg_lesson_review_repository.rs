//! `PgLessonReviewRepository`（`review.lesson_reviews`）の検証。完了したレッスンを `PgLessonCompletionRepository::find`
//! で読んで評価を保存・読み取りできること、同じレッスンの 2 回目の保存の拒否、コーチごとの読み取り。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use application::{
    in_memory::{FixedClock, RecordingAnalyticsSink},
    lesson::{LessonCompletionLookup, LessonCompletionRepository},
    review::{
        GetCoachReviewSummary, InsertReviewError, LessonReviewRepository, SubmitLessonReview,
        SubmitLessonReviewCommand, SubmitLessonReviewError,
    },
};
use domain::{
    DomainError,
    lesson::{LessonCompletion, LessonId},
    review::{LessonReview, LessonReviewComment, Rating},
};
use infra::{PgLessonCompletionRepository, PgLessonReviewRepository};
use sqlx::PgPool;
use support::at;

/// 新しい生徒とコーチの、終了した通話セッションの完了記録。
async fn completed_lesson(pool: &PgPool) -> LessonCompletion {
    let completions = PgLessonCompletionRepository::new(pool.clone());
    let session = support::ended_session(pool, 11).await;
    let student = session.student_id();
    let id = completions.next_id();
    completions
        .record(student, |history| {
            session.complete(id, student, history, at(12))
        })
        .await
        .unwrap()
}

fn command(lesson: &LessonCompletion, rating: i64) -> SubmitLessonReviewCommand {
    SubmitLessonReviewCommand {
        account_id: lesson.student_id(),
        lesson_id: lesson.id(),
        rating: Rating::new(rating).unwrap(),
        comment: LessonReviewComment::new("分かりやすかった").unwrap(),
    }
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn reviews_a_completed_lesson_once_and_reads_it_back(pool: PgPool) {
    let lesson = completed_lesson(&pool).await;
    let reviews = PgLessonReviewRepository::new(pool.clone());
    let submit = SubmitLessonReview::new(
        LessonCompletionLookup::new(PgLessonCompletionRepository::new(pool.clone())),
        reviews.clone(),
        RecordingAnalyticsSink::default(),
        FixedClock(at(13)),
    );

    let review = submit.execute(command(&lesson, 4)).await.unwrap();
    assert_eq!(
        (review.student_id(), review.coach_id()),
        (lesson.student_id(), lesson.coach_id())
    );
    assert_eq!(
        reviews.find_by_lesson(lesson.id()).await.unwrap(),
        Some(review.clone())
    );

    // 2 回目はユースケース（読んだ既存の評価）でも、保存先（一意制約）でも拒否する
    let result = submit.execute(command(&lesson, 1)).await;
    assert!(
        matches!(
            result,
            Err(SubmitLessonReviewError::Invalid(
                DomainError::LessonAlreadyReviewed
            ))
        ),
        "{result:?}"
    );
    let duplicate = LessonReview::submit(
        reviews.next_id(),
        lesson.student_id(),
        &lesson,
        None,
        Rating::new(2).unwrap(),
        LessonReviewComment::new("並行").unwrap(),
        at(14),
    )
    .unwrap();
    assert!(matches!(
        reviews.insert(duplicate).await,
        Err(InsertReviewError::LessonAlreadyReviewed)
    ));
    assert_eq!(
        reviews.find_by_coach(lesson.coach_id()).await.unwrap(),
        [review]
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn an_unknown_lesson_is_not_found(pool: PgPool) {
    let completions = PgLessonCompletionRepository::new(pool.clone());
    assert_eq!(
        completions.find(LessonId::from_u128(99)).await.unwrap(),
        None
    );
    assert_eq!(
        PgLessonReviewRepository::new(pool)
            .find_by_lesson(LessonId::from_u128(99))
            .await
            .unwrap(),
        None
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn summarizes_only_the_reviews_of_the_coach(pool: PgPool) {
    let reviews = PgLessonReviewRepository::new(pool.clone());
    let lessons = [completed_lesson(&pool).await, completed_lesson(&pool).await];
    for (lesson, rating) in lessons.iter().zip([5, 2]) {
        SubmitLessonReview::new(
            LessonCompletionLookup::new(PgLessonCompletionRepository::new(pool.clone())),
            reviews.clone(),
            RecordingAnalyticsSink::default(),
            FixedClock(at(13)),
        )
        .execute(command(lesson, rating))
        .await
        .unwrap();
    }

    // コーチは完了記録ごとに別（support::ended_session は新しいコーチを作る）
    let summary = GetCoachReviewSummary::new(reviews)
        .execute(lessons[0].coach_id())
        .await
        .unwrap();
    assert_eq!(summary.review_count(), 1);
    assert_eq!(summary.average_rating(), Some(5.0));
}
