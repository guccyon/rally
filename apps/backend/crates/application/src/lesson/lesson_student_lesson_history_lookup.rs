use std::cmp::Reverse;

use domain::{identity::AccountId, lesson::LessonCompletion};

use super::LessonCompletionRepository;
use crate::{Unavailable, identity::StudentLessonHistoryLookup};

/// identity の [`StudentLessonHistoryLookup`] の、完了記録の保存先による実装。読むだけで、何も変えない。
///
/// 並べ方は生徒が受けたレッスンの一覧（[`super::ListMyLessonCompletions`]）と同じく、完了した時刻の新しい順（同じ時刻は
/// 完了記録の ID の順）。
#[derive(Debug, Clone)]
pub struct LessonStudentLessonHistoryLookup<R> {
    completions: R,
}

impl<R> LessonStudentLessonHistoryLookup<R> {
    pub fn new(completions: R) -> Self {
        Self { completions }
    }
}

impl<R> StudentLessonHistoryLookup for LessonStudentLessonHistoryLookup<R>
where
    R: LessonCompletionRepository,
{
    async fn find_recent_lessons(
        &self,
        student_id: AccountId,
        limit: usize,
    ) -> Result<Vec<LessonCompletion>, Unavailable> {
        let mut completions = self.completions.completions_of_student(student_id).await?;
        completions.sort_by_key(|completion| (Reverse(completion.completed_at()), completion.id()));
        completions.truncate(limit);
        Ok(completions)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        async_lesson::VideoId,
        lesson::{LessonCompletionId, LessonSource},
    };

    use super::*;
    use crate::in_memory::InMemoryLessonCompletionRepository;

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);

    fn at(hour: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(hour * 3600)
    }

    fn completion(id: u128, student: AccountId, hour: u64) -> LessonCompletion {
        LessonCompletion::restore(
            LessonCompletionId::from_u128(id),
            student,
            COACH,
            LessonSource::Async(VideoId::from_u128(id)),
            at(hour),
            false,
        )
    }

    fn lookup() -> LessonStudentLessonHistoryLookup<InMemoryLessonCompletionRepository> {
        LessonStudentLessonHistoryLookup::new(InMemoryLessonCompletionRepository::with_completions(
            [
                completion(1, STUDENT, 10),
                completion(4, STUDENT, 30),
                // 同じ時刻は ID の順
                completion(3, STUDENT, 20),
                completion(2, STUDENT, 20),
                completion(5, OTHER_STUDENT, 40),
            ],
        ))
    }

    fn ids(completions: &[LessonCompletion]) -> Vec<u128> {
        completions.iter().map(|c| c.id().as_u128()).collect()
    }

    #[tokio::test]
    async fn returns_own_lessons_latest_first_up_to_the_limit() {
        let lookup = lookup();
        assert_eq!(
            ids(&lookup.find_recent_lessons(STUDENT, 10).await.unwrap()),
            [4, 2, 3, 1]
        );
        assert_eq!(
            ids(&lookup.find_recent_lessons(STUDENT, 2).await.unwrap()),
            [4, 2],
            "上限を超えた古いレッスンは含めない"
        );
    }

    #[tokio::test]
    async fn student_without_lessons_is_empty() {
        let lookup = lookup();
        assert!(
            lookup
                .find_recent_lessons(AccountId::from_u128(9), 10)
                .await
                .unwrap()
                .is_empty()
        );
    }
}
