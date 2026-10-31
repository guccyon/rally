use domain::lesson::{LessonCompletion, LessonCompletionId};

use super::LessonCompletionRepository;
use crate::{Unavailable, review::CompletedLessonLookup};

/// review の [`CompletedLessonLookup`] の、完了記録の保存先による実装。保存先から読むだけで、何も変えない。
#[derive(Debug, Clone)]
pub struct LessonCompletionLookup<R> {
    completions: R,
}

impl<R> LessonCompletionLookup<R> {
    pub fn new(completions: R) -> Self {
        Self { completions }
    }
}

impl<R> CompletedLessonLookup for LessonCompletionLookup<R>
where
    R: LessonCompletionRepository,
{
    async fn find_completion(
        &self,
        lesson_id: LessonCompletionId,
    ) -> Result<Option<LessonCompletion>, Unavailable> {
        self.completions.find(lesson_id).await
    }
}
