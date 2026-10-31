use super::{LessonCompletion, LessonType};
use crate::identity::AccountId;

/// コーチ 1 人の完了したレッスンの数を、同期レッスン（通話）と非同期レッスン（動画添削）に分けたもの
/// （admin-web の `getMyLessonStats`）。
///
/// 数え方はこの型の 1 か所だけに置く。どの期間の記録を渡すか（暦月で区切る）は呼ぶ側が決める。
/// グループレッスンの通話セッションは、完了したレッスン（結び付いた予約の生徒ごとに 1 つ）の数だけ数える。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoachLessonCount {
    coach_id: AccountId,
    live_count: u32,
    async_count: u32,
}

impl CoachLessonCount {
    /// `completions` のうち、コーチ `coach_id` の完了したレッスンを種類ごとに数える（他のコーチの記録は数えない）。
    pub fn tally(coach_id: AccountId, completions: &[LessonCompletion]) -> Self {
        let (live_count, async_count) = completions
            .iter()
            .filter(|completion| completion.coach_id() == coach_id)
            .fold(
                (0u32, 0u32),
                |(live, async_), completion| match completion.lesson_type() {
                    LessonType::Live => (live + 1, async_),
                    LessonType::Async => (live, async_ + 1),
                },
            );
        Self {
            coach_id,
            live_count,
            async_count,
        }
    }

    pub const fn coach_id(&self) -> AccountId {
        self.coach_id
    }

    /// 同期レッスン（通話）の完了したレッスンの数。
    pub const fn live_count(&self) -> u32 {
        self.live_count
    }

    /// 非同期レッスン（動画添削）の完了したレッスンの数。
    pub const fn async_count(&self) -> u32 {
        self.async_count
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;
    use crate::{
        async_lesson::VideoId,
        lesson::{LessonCompletionId, LessonSource},
        live_lesson::SessionId,
    };

    const COACH: AccountId = AccountId::from_u128(3);
    const OTHER_COACH: AccountId = AccountId::from_u128(4);

    fn completion(id: u128, coach_id: AccountId, source: LessonSource) -> LessonCompletion {
        LessonCompletion::restore(
            LessonCompletionId::from_u128(id),
            AccountId::from_u128(100 + id),
            coach_id,
            source,
            SystemTime::UNIX_EPOCH,
            false,
        )
    }

    #[test]
    fn no_completions_count_zero() {
        let count = CoachLessonCount::tally(COACH, &[]);
        assert_eq!(count.coach_id(), COACH);
        assert_eq!((count.live_count(), count.async_count()), (0, 0));
    }

    #[test]
    fn counts_live_and_async_lessons_of_the_coach_separately() {
        let session = LessonSource::Live(SessionId::from_u128(10));
        let completions = [
            // グループレッスンの同じセッションでも生徒ごとに数える
            completion(1, COACH, session),
            completion(2, COACH, session),
            completion(3, COACH, LessonSource::Async(VideoId::from_u128(20))),
            completion(4, OTHER_COACH, LessonSource::Async(VideoId::from_u128(21))),
        ];
        let count = CoachLessonCount::tally(COACH, &completions);
        assert_eq!((count.live_count(), count.async_count()), (2, 1));
    }
}
