use std::time::SystemTime;

use super::{LessonId, LessonSource, LessonType};
use crate::{DomainError, identity::AccountId};

/// レッスンの完了記録（glossary の lesson が終わったこと）。通話セッション・動画 1 つにつき 1 つ。
///
/// 作るのは live-lesson の [`crate::live_lesson::LiveLessonSession::complete`] と async-lesson の
/// [`crate::async_lesson::Video::complete`] だけ（元の集約の不変条件はそこで確かめる）。
/// `is_first_lesson`（north star「初回レッスン後の 1 ヶ月継続率」の起点）は、その生徒の完了記録が 0 件のときだけ
/// true にする。判定はこの型の 1 か所だけに置き、同期・非同期を区別せずに数える。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonCompletion {
    id: LessonId,
    student_id: AccountId,
    coach_id: AccountId,
    source: LessonSource,
    completed_at: SystemTime,
    is_first_lesson: bool,
}

impl LessonCompletion {
    /// 生徒 `student_id` がコーチ `coach_id` から受けたレッスン（元は `source`）の完了を `now` に記録する。
    ///
    /// `history` は既存の完了記録（少なくともその生徒のもの。他の生徒の記録は数えない）。同じ元の記録がすでに
    /// あれば [`DomainError::LessonAlreadyCompleted`]。
    pub(crate) fn record(
        id: LessonId,
        student_id: AccountId,
        coach_id: AccountId,
        source: LessonSource,
        history: &[LessonCompletion],
        now: SystemTime,
    ) -> Result<Self, DomainError> {
        if history.iter().any(|completion| completion.source == source) {
            return Err(DomainError::LessonAlreadyCompleted);
        }
        let is_first_lesson = !history
            .iter()
            .any(|completion| completion.student_id == student_id);
        Ok(Self {
            id,
            student_id,
            coach_id,
            source,
            completed_at: now,
            is_first_lesson,
        })
    }

    /// 保存先から読んだ完了記録を組み立てる（保存先の実装用）。`is_first_lesson` は記録した時点の判定
    /// （[`LessonCompletion::record`]）をそのまま持つ。
    pub const fn restore(
        id: LessonId,
        student_id: AccountId,
        coach_id: AccountId,
        source: LessonSource,
        completed_at: SystemTime,
        is_first_lesson: bool,
    ) -> Self {
        Self {
            id,
            student_id,
            coach_id,
            source,
            completed_at,
            is_first_lesson,
        }
    }

    pub const fn id(&self) -> LessonId {
        self.id
    }

    pub const fn student_id(&self) -> AccountId {
        self.student_id
    }

    pub const fn coach_id(&self) -> AccountId {
        self.coach_id
    }

    pub const fn source(&self) -> LessonSource {
        self.source
    }

    pub const fn lesson_type(&self) -> LessonType {
        self.source.lesson_type()
    }

    pub const fn completed_at(&self) -> SystemTime {
        self.completed_at
    }

    /// その生徒にとって初回のレッスンか（記録した時点でその生徒の完了記録が 0 件だった）。
    pub const fn is_first_lesson(&self) -> bool {
        self.is_first_lesson
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{async_lesson::VideoId, live_lesson::SessionId};

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(3);
    const NOW: SystemTime = SystemTime::UNIX_EPOCH;
    const LIVE: LessonSource = LessonSource::Live(SessionId::from_u128(10));
    const ASYNC: LessonSource = LessonSource::Async(VideoId::from_u128(20));

    fn record(
        id: u128,
        student_id: AccountId,
        source: LessonSource,
        history: &[LessonCompletion],
    ) -> Result<LessonCompletion, DomainError> {
        LessonCompletion::record(
            LessonId::from_u128(id),
            student_id,
            COACH,
            source,
            history,
            NOW,
        )
    }

    #[test]
    fn the_first_completion_of_a_student_is_the_first_lesson() {
        let first = record(1, STUDENT, LIVE, &[]).unwrap();
        assert_eq!(first.id(), LessonId::from_u128(1));
        assert_eq!((first.student_id(), first.coach_id()), (STUDENT, COACH));
        assert_eq!(first.source(), LIVE);
        assert_eq!(first.lesson_type(), LessonType::Live);
        assert_eq!(first.completed_at(), NOW);
        assert!(first.is_first_lesson());
    }

    #[test]
    fn the_second_completion_is_not_the_first_lesson_whatever_its_type() {
        let first = record(1, STUDENT, LIVE, &[]).unwrap();
        let second = record(2, STUDENT, ASYNC, &[first]).unwrap();
        assert_eq!(second.lesson_type(), LessonType::Async);
        assert!(!second.is_first_lesson());
    }

    #[test]
    fn other_students_completions_do_not_count() {
        let others = record(1, OTHER_STUDENT, LIVE, &[]).unwrap();
        assert!(
            record(2, STUDENT, ASYNC, &[others])
                .unwrap()
                .is_first_lesson()
        );
    }

    #[test]
    fn a_source_is_completed_only_once() {
        let first = record(1, STUDENT, ASYNC, &[]).unwrap();
        assert_eq!(
            record(2, STUDENT, ASYNC, &[first]),
            Err(DomainError::LessonAlreadyCompleted)
        );
    }

    #[test]
    fn restore_keeps_the_recorded_first_lesson_flag() {
        let first = record(1, STUDENT, LIVE, &[]).unwrap();
        let restored = LessonCompletion::restore(
            first.id(),
            STUDENT,
            COACH,
            LIVE,
            NOW,
            first.is_first_lesson(),
        );
        assert_eq!(restored, first);
    }
}
