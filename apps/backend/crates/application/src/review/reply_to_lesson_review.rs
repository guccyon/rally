use domain::{
    identity::AccountId,
    review::{LessonReview, LessonReviewId, LessonReviewReplyComment, ReviewError},
};

use super::LessonReviewRepository;
use crate::{Unavailable, clock::Clock};

/// 評価への返信の入力。`account_id` は認証したアカウント（評価されたコーチかは domain が確かめる）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyToLessonReviewCommand {
    pub account_id: AccountId,
    pub review_id: LessonReviewId,
    pub comment: LessonReviewReplyComment,
}

#[derive(Debug)]
pub enum ReplyToLessonReviewError {
    /// 評価がない
    ReviewNotFound,
    /// 返信が評価の不変条件を満たさない（評価されたコーチでない [`ReviewError::NotTheReviewedCoach`]）
    Invalid(ReviewError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// 評価への返信のユースケース（admin-web の `replyToLessonReview`）。
///
/// 評価を読み、domain の [`LessonReview::write_reply`] で返信を付けて（すでにあれば置き換えて）保存し、返信を付けた
/// 評価を返す。拒否・失敗では何も保存しない。行動イベントは発火しない。
pub struct ReplyToLessonReview<R, C> {
    reviews: R,
    clock: C,
}

impl<R, C> ReplyToLessonReview<R, C>
where
    R: LessonReviewRepository,
    C: Clock,
{
    pub fn new(reviews: R, clock: C) -> Self {
        Self { reviews, clock }
    }

    pub async fn execute(
        &self,
        command: ReplyToLessonReviewCommand,
    ) -> Result<LessonReview, ReplyToLessonReviewError> {
        let now = self.clock.now();
        let mut review = self
            .reviews
            .find_by_id(command.review_id)
            .await
            .map_err(ReplyToLessonReviewError::Unavailable)?
            .ok_or(ReplyToLessonReviewError::ReviewNotFound)?;
        review
            .write_reply(command.account_id, command.comment, now)
            .map_err(ReplyToLessonReviewError::Invalid)?;
        if let Some(reply) = review.reply() {
            self.reviews
                .save_reply(review.id(), reply)
                .await
                .map_err(ReplyToLessonReviewError::Unavailable)?;
        }
        Ok(review)
    }

    pub fn reviews(&self) -> &R {
        &self.reviews
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        lesson::LessonCompletionId,
        review::{LessonReviewComment, LessonReviewReply, Rating},
    };

    use super::*;
    use crate::in_memory::{FixedClock, InMemoryLessonReviewRepository};

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);
    const REVIEW: LessonReviewId = LessonReviewId::from_u128(40);

    fn at(hours: u64) -> SystemTime {
        // 2026-10-26T00:00:00Z から `hours` 時間後
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800 + hours * 3600)
    }

    fn review() -> LessonReview {
        LessonReview::restore(
            REVIEW,
            LessonCompletionId::from_u128(41),
            STUDENT,
            COACH,
            Rating::new(4).unwrap(),
            LessonReviewComment::new("分かりやすかった").unwrap(),
            at(1),
        )
    }

    fn use_case(hours: u64) -> ReplyToLessonReview<InMemoryLessonReviewRepository, FixedClock> {
        ReplyToLessonReview::new(
            InMemoryLessonReviewRepository::with_reviews([review()]),
            FixedClock(at(hours)),
        )
    }

    fn command(
        account_id: AccountId,
        review_id: LessonReviewId,
        text: &str,
    ) -> ReplyToLessonReviewCommand {
        ReplyToLessonReviewCommand {
            account_id,
            review_id,
            comment: LessonReviewReplyComment::new(text).unwrap(),
        }
    }

    fn reply(text: &str, hours: u64) -> LessonReviewReply {
        LessonReviewReply::new(LessonReviewReplyComment::new(text).unwrap(), at(hours))
    }

    #[tokio::test]
    async fn the_reviewed_coach_replies_and_it_is_saved() {
        let use_case = use_case(2);

        let replied = use_case
            .execute(command(COACH, REVIEW, "ありがとうございます"))
            .await
            .unwrap();

        assert_eq!(replied.reply(), Some(&reply("ありがとうございます", 2)));
        assert_eq!(replied.rating().value(), 4, "点と一言は変えない");
        assert_eq!(
            use_case.reviews().find_by_id(REVIEW).await.unwrap(),
            Some(replied)
        );
    }

    #[tokio::test]
    async fn a_second_reply_overwrites_the_first() {
        let reviews = InMemoryLessonReviewRepository::with_reviews([review()]);
        ReplyToLessonReview::new(reviews.clone(), FixedClock(at(2)))
            .execute(command(COACH, REVIEW, "ありがとうございます"))
            .await
            .unwrap();

        let replied = ReplyToLessonReview::new(reviews.clone(), FixedClock(at(3)))
            .execute(command(COACH, REVIEW, "次回はサーブも見ます"))
            .await
            .unwrap();

        assert_eq!(replied.reply(), Some(&reply("次回はサーブも見ます", 3)));
        assert_eq!(reviews.all(), vec![replied], "評価は 1 件のまま");
    }

    #[tokio::test]
    async fn only_the_reviewed_coach_can_reply_and_nothing_is_saved_otherwise() {
        let use_case = use_case(2);

        for account in [OTHER_COACH, STUDENT] {
            let result = use_case.execute(command(account, REVIEW, "横取り")).await;
            assert!(
                matches!(
                    result,
                    Err(ReplyToLessonReviewError::Invalid(
                        ReviewError::NotTheReviewedCoach
                    ))
                ),
                "{result:?}"
            );
        }
        assert_eq!(use_case.reviews().all(), vec![review()]);
    }

    #[tokio::test]
    async fn an_unknown_review_is_not_found() {
        let use_case = use_case(2);

        let result = use_case
            .execute(command(COACH, LessonReviewId::from_u128(99), "ありがとう"))
            .await;
        assert!(
            matches!(result, Err(ReplyToLessonReviewError::ReviewNotFound)),
            "{result:?}"
        );
        assert_eq!(use_case.reviews().all(), vec![review()]);
    }
}
