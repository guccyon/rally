//! `PgLessonReviewRepository`（`review.lesson_reviews`）の検証。完了したレッスンを `PgLessonCompletionRepository::find`
//! で読んで評価を保存・読み取りできること、同じレッスンの 2 回目の保存の拒否、コーチごとの読み取り、送った時刻での
//! 読み取りと、それを使う本部の低い評価の一覧（コーチ・生徒の表示名はアカウントの表から読む）。評価へのコーチの返信の
//! 保存と上書き、評価の ID での読み取り。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use std::time::Duration;

use application::{
    in_memory::{FixedClock, RecordingAnalyticsSink},
    lesson::{LessonCompletionLookup, LessonCompletionMenuLookup, LessonCompletionRepository},
    profile::{ProfileCoachDisplayNameLookup, ProfileStudentDisplayNameLookup},
    review::{
        GetCoachReviewSummary, InsertReviewError, LessonReviewRepository, ListLowRatedReviews,
        ReplyToLessonReview, ReplyToLessonReviewCommand, ReplyToLessonReviewError,
        SubmitLessonReview, SubmitLessonReviewCommand, SubmitLessonReviewError,
    },
    scheduling::SchedulingBookingLookup,
};
use domain::{
    lesson::{LessonCompletion, LessonCompletionId},
    review::{
        LessonReview, LessonReviewComment, LessonReviewId, LessonReviewReply,
        LessonReviewReplyComment, Rating, ReviewError,
    },
};
use infra::{
    PgAccountRepository, PgBookingRepository, PgLessonCompletionRepository,
    PgLessonReviewRepository, PgLiveLessonSessionRepository, PgVideoRepository,
};
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
                ReviewError::LessonAlreadyReviewed
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
        completions
            .find(LessonCompletionId::from_u128(99))
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        PgLessonReviewRepository::new(pool)
            .find_by_lesson(LessonCompletionId::from_u128(99))
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

/// `lesson` に `rating` の評価を `submitted_at` に付ける（ユースケースを通して保存する）。
async fn review_at(
    pool: &PgPool,
    lesson: &LessonCompletion,
    rating: i64,
    submitted_at: std::time::SystemTime,
) -> LessonReview {
    SubmitLessonReview::new(
        LessonCompletionLookup::new(PgLessonCompletionRepository::new(pool.clone())),
        PgLessonReviewRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        FixedClock(submitted_at),
    )
    .execute(command(lesson, rating))
    .await
    .unwrap()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn reads_reviews_submitted_since_and_lists_the_low_ones(pool: PgPool) {
    let now = at(24 * 40);
    let since = now - Duration::from_secs(30 * 24 * 3600);
    let mut lessons = Vec::new();
    for _ in 0..4 {
        lessons.push(completed_lesson(&pool).await);
    }
    let old = review_at(&pool, &lessons[0], 1, since - Duration::from_secs(1)).await;
    let edge = review_at(&pool, &lessons[1], 2, since).await;
    let good = review_at(&pool, &lessons[2], 4, now).await;
    let latest = review_at(&pool, &lessons[3], 1, now).await;
    let reviews = PgLessonReviewRepository::new(pool.clone());

    let mut found = reviews.find_submitted_since(since).await.unwrap();
    found.sort_by_key(LessonReview::id);
    let mut expected = vec![edge.clone(), good, latest.clone()];
    expected.sort_by_key(LessonReview::id);
    assert_eq!(
        found, expected,
        "since ちょうどを含み、それより前（{old:?}）は含まない"
    );

    let accounts = PgAccountRepository::new(pool.clone());
    let list = ListLowRatedReviews::new(
        reviews,
        LessonCompletionMenuLookup::new(
            PgLessonCompletionRepository::new(pool.clone()),
            PgLiveLessonSessionRepository::new(pool.clone()),
            SchedulingBookingLookup::new(PgBookingRepository::new(pool.clone())),
            PgVideoRepository::new(pool.clone()),
        ),
        ProfileCoachDisplayNameLookup::new(accounts.clone()),
        ProfileStudentDisplayNameLookup::new(accounts),
        FixedClock(now),
    )
    .execute()
    .await
    .unwrap();
    assert_eq!(
        list.reviews,
        [latest.clone(), edge.clone()],
        "★2 以下を新しい順"
    );
    assert!(list.lesson_menus.is_empty(), "メニューのない予約のレッスン");
    for review in [&latest, &edge] {
        assert_eq!(
            list.coach_display_names[&review.coach_id()].as_str(),
            "テスト"
        );
        assert_eq!(
            list.student_display_names[&review.student_id()].as_str(),
            "テスト"
        );
    }
}

fn reply_command(
    review: &LessonReview,
    account: domain::identity::AccountId,
    text: &str,
) -> ReplyToLessonReviewCommand {
    ReplyToLessonReviewCommand {
        account_id: account,
        review_id: review.id(),
        comment: LessonReviewReplyComment::new(text).unwrap(),
    }
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn the_reviewed_coach_replies_and_a_second_reply_overwrites_it(pool: PgPool) {
    let lesson = completed_lesson(&pool).await;
    let review = review_at(&pool, &lesson, 4, at(13)).await;
    let reviews = PgLessonReviewRepository::new(pool.clone());
    assert_eq!(
        reviews.find_by_id(review.id()).await.unwrap(),
        Some(review.clone()),
        "返信のない評価"
    );

    ReplyToLessonReview::new(reviews.clone(), FixedClock(at(14)))
        .execute(reply_command(&review, lesson.coach_id(), "ありがとう"))
        .await
        .unwrap();
    let replied = ReplyToLessonReview::new(reviews.clone(), FixedClock(at(15)))
        .execute(reply_command(
            &review,
            lesson.coach_id(),
            "次回はサーブも見ます",
        ))
        .await
        .unwrap();
    assert_eq!(
        replied.reply(),
        Some(&LessonReviewReply::new(
            LessonReviewReplyComment::new("次回はサーブも見ます").unwrap(),
            at(15)
        ))
    );
    // どの読み取りも返信を含めて読み、点と一言は変わらない
    assert_eq!(
        reviews.find_by_id(review.id()).await.unwrap(),
        Some(replied.clone())
    );
    assert_eq!(
        reviews.find_by_lesson(lesson.id()).await.unwrap(),
        Some(replied.clone())
    );
    assert_eq!(
        reviews.find_by_coach(lesson.coach_id()).await.unwrap(),
        std::slice::from_ref(&replied)
    );
    assert_eq!(
        reviews.find_submitted_since(at(13)).await.unwrap(),
        std::slice::from_ref(&replied)
    );
    assert_eq!(replied.comment(), review.comment());

    // 生徒・ない評価は拒み、何も変えない
    let result = ReplyToLessonReview::new(reviews.clone(), FixedClock(at(16)))
        .execute(reply_command(&review, lesson.student_id(), "横取り"))
        .await;
    assert!(
        matches!(
            result,
            Err(ReplyToLessonReviewError::Invalid(
                ReviewError::NotTheReviewedCoach
            ))
        ),
        "{result:?}"
    );
    let mut unknown = reply_command(&review, lesson.coach_id(), "ありがとう");
    unknown.review_id = LessonReviewId::from_u128(99);
    let result = ReplyToLessonReview::new(reviews.clone(), FixedClock(at(16)))
        .execute(unknown)
        .await;
    assert!(
        matches!(result, Err(ReplyToLessonReviewError::ReviewNotFound)),
        "{result:?}"
    );
    assert_eq!(
        reviews.find_by_id(review.id()).await.unwrap(),
        Some(replied)
    );
}
