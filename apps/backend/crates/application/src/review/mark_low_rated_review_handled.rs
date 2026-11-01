use domain::{
    identity::AccountId,
    review::{LessonReview, LessonReviewId, ReviewError},
};

use super::LessonReviewRepository;
use crate::{Unavailable, clock::Clock};

/// 低い評価への本部の対応済みの印の入力。`account_id` は認証した本部のアカウント（本部の役割かは境界が確かめる）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkLowRatedReviewHandledCommand {
    pub account_id: AccountId,
    pub review_id: LessonReviewId,
}

#[derive(Debug)]
pub enum MarkLowRatedReviewHandledError {
    /// 評価がない
    ReviewNotFound,
    /// 印が評価の不変条件を満たさない（低い評価でない [`ReviewError::NotLowRated`]）
    Invalid(ReviewError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// 低い評価に本部が「対応済み」の印を付けるユースケース（admin-web の `markLowRatedLessonReviewHandled`）。
///
/// 評価を読み、domain の [`LessonReview::mark_handled`] で印を付けて保存し、保存先にある印を付けた評価を返す。印がすでに
/// あれば保存せずにそのまま返す（何度呼んでも結果は同じ。最初のアカウントと時刻を残す）。拒否・失敗では何も保存しない。
/// 行動イベントは発火しない。
pub struct MarkLowRatedReviewHandled<R, C> {
    reviews: R,
    clock: C,
}

impl<R, C> MarkLowRatedReviewHandled<R, C>
where
    R: LessonReviewRepository,
    C: Clock,
{
    pub fn new(reviews: R, clock: C) -> Self {
        Self { reviews, clock }
    }

    pub async fn execute(
        &self,
        command: MarkLowRatedReviewHandledCommand,
    ) -> Result<LessonReview, MarkLowRatedReviewHandledError> {
        let now = self.clock.now();
        let mut review = self
            .find(command.review_id)
            .await?
            .ok_or(MarkLowRatedReviewHandledError::ReviewNotFound)?;
        if review.handling().is_some() {
            // 2 回目は何も変えない。低い評価でない評価に印はない（domain が付けさせない）
            return Ok(review);
        }
        review
            .mark_handled(command.account_id, now)
            .map_err(MarkLowRatedReviewHandledError::Invalid)?;
        if let Some(handling) = review.handling() {
            self.reviews
                .save_handling(review.id(), handling)
                .await
                .map_err(MarkLowRatedReviewHandledError::Unavailable)?;
        }
        // 並行に付けた印が先に保存されていれば、そちらが残る（保存先が最初の印を守る）ので、読み直して返す
        self.find(command.review_id)
            .await?
            .ok_or(MarkLowRatedReviewHandledError::ReviewNotFound)
    }

    async fn find(
        &self,
        review_id: LessonReviewId,
    ) -> Result<Option<LessonReview>, MarkLowRatedReviewHandledError> {
        self.reviews
            .find_by_id(review_id)
            .await
            .map_err(MarkLowRatedReviewHandledError::Unavailable)
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
        review::{
            LessonReviewComment, LessonReviewReply, LessonReviewReplyComment,
            LowRatedReviewHandling, Rating,
        },
    };

    use super::*;
    use crate::in_memory::{FixedClock, InMemoryLessonReviewRepository};

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(7);
    const HEADQUARTERS: AccountId = AccountId::from_u128(8);
    const OTHER_HEADQUARTERS: AccountId = AccountId::from_u128(9);
    const LOW: LessonReviewId = LessonReviewId::from_u128(40);
    const HIGH: LessonReviewId = LessonReviewId::from_u128(41);

    fn at(hours: u64) -> SystemTime {
        // 2026-10-26T00:00:00Z から `hours` 時間後
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800 + hours * 3600)
    }

    fn review(id: LessonReviewId, rating: i64) -> LessonReview {
        LessonReview::restore(
            id,
            LessonCompletionId::from_u128(id.as_u128()),
            STUDENT,
            COACH,
            Rating::new(rating).unwrap(),
            LessonReviewComment::new("いまひとつ").unwrap(),
            at(1),
        )
    }

    fn reviews() -> InMemoryLessonReviewRepository {
        InMemoryLessonReviewRepository::with_reviews([review(LOW, 2), review(HIGH, 3)])
    }

    fn command(
        account_id: AccountId,
        review_id: LessonReviewId,
    ) -> MarkLowRatedReviewHandledCommand {
        MarkLowRatedReviewHandledCommand {
            account_id,
            review_id,
        }
    }

    #[tokio::test]
    async fn headquarters_marks_a_low_review_handled_and_it_is_saved() {
        let use_case = MarkLowRatedReviewHandled::new(reviews(), FixedClock(at(2)));

        let handled = use_case.execute(command(HEADQUARTERS, LOW)).await.unwrap();

        assert_eq!(
            handled.handling(),
            Some(&LowRatedReviewHandling::new(HEADQUARTERS, at(2)))
        );
        assert_eq!(handled.rating().value(), 2, "点と一言は変えない");
        assert_eq!(
            use_case.reviews().find_by_id(LOW).await.unwrap(),
            Some(handled)
        );
    }

    #[tokio::test]
    async fn a_second_mark_returns_the_same_result_and_keeps_the_first() {
        let reviews = reviews();
        let first = MarkLowRatedReviewHandled::new(reviews.clone(), FixedClock(at(2)))
            .execute(command(HEADQUARTERS, LOW))
            .await
            .unwrap();

        for account in [HEADQUARTERS, OTHER_HEADQUARTERS] {
            let again = MarkLowRatedReviewHandled::new(reviews.clone(), FixedClock(at(3)))
                .execute(command(account, LOW))
                .await
                .unwrap();
            assert_eq!(again, first, "{account:?}");
        }
        assert_eq!(reviews.find_by_id(LOW).await.unwrap(), Some(first));
    }

    #[tokio::test]
    async fn keeps_the_reply_of_the_coach() {
        let reply = LessonReviewReply::new(
            LessonReviewReplyComment::new("ご意見ありがとうございます").unwrap(),
            at(1),
        );
        let reviews = InMemoryLessonReviewRepository::with_reviews([
            review(LOW, 1).with_reply(Some(reply.clone()))
        ]);

        let handled = MarkLowRatedReviewHandled::new(reviews, FixedClock(at(2)))
            .execute(command(HEADQUARTERS, LOW))
            .await
            .unwrap();
        assert_eq!(handled.reply(), Some(&reply));
    }

    #[tokio::test]
    async fn a_review_that_is_not_low_is_rejected_and_nothing_is_saved() {
        let use_case = MarkLowRatedReviewHandled::new(reviews(), FixedClock(at(2)));

        let result = use_case.execute(command(HEADQUARTERS, HIGH)).await;
        assert!(
            matches!(
                result,
                Err(MarkLowRatedReviewHandledError::Invalid(
                    ReviewError::NotLowRated
                ))
            ),
            "{result:?}"
        );
        assert_eq!(
            use_case.reviews().all(),
            vec![review(LOW, 2), review(HIGH, 3)]
        );
    }

    #[tokio::test]
    async fn an_unknown_review_is_not_found() {
        let use_case = MarkLowRatedReviewHandled::new(reviews(), FixedClock(at(2)));

        let result = use_case
            .execute(command(HEADQUARTERS, LessonReviewId::from_u128(99)))
            .await;
        assert!(
            matches!(result, Err(MarkLowRatedReviewHandledError::ReviewNotFound)),
            "{result:?}"
        );
        assert_eq!(
            use_case.reviews().all(),
            vec![review(LOW, 2), review(HIGH, 3)]
        );
    }
}
