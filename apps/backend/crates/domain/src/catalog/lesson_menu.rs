use std::time::SystemTime;

use super::{
    LessonDuration, LessonMenuContent, LessonMenuDescription, LessonMenuId, LessonMenuName,
};
use crate::{
    DomainError,
    identity::{AccountId, Coach, Sport},
    lesson::LessonType,
};

/// レッスンメニュー（glossary の lesson。レッスン種別）。コーチがスポーツごとに提供するレッスンで、同期 / 非同期の
/// 別を持つ。集約。
///
/// - スポーツはそのコーチの提供スポーツのどれか
/// - 同期レッスンは 1 回の時間（[`LessonDuration`]）を持ち、非同期レッスンは持たない
/// - 作る・直す・非公開にするのはそのコーチだけ。コーチ・スポーツ・レッスンの種類は作ったあとに変えない
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonMenu {
    id: LessonMenuId,
    coach_id: AccountId,
    sport: Sport,
    lesson_type: LessonType,
    content: LessonMenuContent,
    created_at: SystemTime,
    updated_at: SystemTime,
}

impl LessonMenu {
    /// コーチ `coach_id`（提供スポーツは `coach`）がメニューを作る。
    ///
    /// `sport` がコーチの提供スポーツでなければ [`DomainError::SportNotOfferedByCoach`]。1 回の時間が
    /// レッスンの種類に合わなければ [`DomainError::LessonDurationRequired`] か
    /// [`DomainError::LessonDurationNotApplicable`]。
    pub fn create(
        id: LessonMenuId,
        coach_id: AccountId,
        coach: &Coach,
        sport: Sport,
        lesson_type: LessonType,
        content: LessonMenuContent,
        now: SystemTime,
    ) -> Result<Self, DomainError> {
        if !coach.offers(sport) {
            return Err(DomainError::SportNotOfferedByCoach { sport });
        }
        check_duration(lesson_type, content.duration)?;
        Ok(Self {
            id,
            coach_id,
            sport,
            lesson_type,
            content,
            created_at: now,
            updated_at: now,
        })
    }

    /// アカウント `account_id` が中身を `content` に置き換える（非公開にするのも、公開し直すのもこれ）。
    ///
    /// メニューのコーチでなければ [`DomainError::NotTheLessonMenuCoach`]。1 回の時間の規則は
    /// [`LessonMenu::create`] と同じ。拒否したら何も変えない。
    pub fn revise(
        &mut self,
        account_id: AccountId,
        content: LessonMenuContent,
        now: SystemTime,
    ) -> Result<(), DomainError> {
        if account_id != self.coach_id {
            return Err(DomainError::NotTheLessonMenuCoach);
        }
        check_duration(self.lesson_type, content.duration)?;
        self.content = content;
        self.updated_at = now;
        Ok(())
    }

    /// 保存先から読んだメニューを組み立てる（保存先の実装用）。
    pub const fn restore(
        id: LessonMenuId,
        coach_id: AccountId,
        sport: Sport,
        lesson_type: LessonType,
        content: LessonMenuContent,
        created_at: SystemTime,
        updated_at: SystemTime,
    ) -> Self {
        Self {
            id,
            coach_id,
            sport,
            lesson_type,
            content,
            created_at,
            updated_at,
        }
    }

    pub const fn id(&self) -> LessonMenuId {
        self.id
    }

    pub const fn coach_id(&self) -> AccountId {
        self.coach_id
    }

    pub const fn sport(&self) -> Sport {
        self.sport
    }

    pub const fn lesson_type(&self) -> LessonType {
        self.lesson_type
    }

    pub const fn content(&self) -> &LessonMenuContent {
        &self.content
    }

    pub const fn name(&self) -> &LessonMenuName {
        &self.content.name
    }

    pub const fn description(&self) -> &LessonMenuDescription {
        &self.content.description
    }

    /// 同期レッスンの 1 回の時間。非同期レッスンは `None`。
    pub const fn duration(&self) -> Option<LessonDuration> {
        self.content.duration
    }

    /// 公開中なら true（生徒の一覧に出る）。
    pub const fn is_published(&self) -> bool {
        self.content.is_published
    }

    pub const fn created_at(&self) -> SystemTime {
        self.created_at
    }

    pub const fn updated_at(&self) -> SystemTime {
        self.updated_at
    }
}

/// 同期レッスンは 1 回の時間を持ち、非同期レッスンは持たない。
fn check_duration(
    lesson_type: LessonType,
    duration: Option<LessonDuration>,
) -> Result<(), DomainError> {
    match (lesson_type, duration) {
        (LessonType::Live, None) => Err(DomainError::LessonDurationRequired),
        (LessonType::Async, Some(_)) => Err(DomainError::LessonDurationNotApplicable),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const COACH: AccountId = AccountId::from_u128(3);
    const OTHER_COACH: AccountId = AccountId::from_u128(4);
    const STUDENT: AccountId = AccountId::from_u128(1);

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn coach() -> Coach {
        Coach::new([Sport::Tennis, Sport::Golf]).unwrap()
    }

    fn content(name: &str, duration: Option<i64>, is_published: bool) -> LessonMenuContent {
        LessonMenuContent {
            name: LessonMenuName::new(name).unwrap(),
            description: LessonMenuDescription::new("初心者向け").unwrap(),
            duration: duration.map(|minutes| LessonDuration::new(minutes).unwrap()),
            is_published,
        }
    }

    fn create(
        sport: Sport,
        lesson_type: LessonType,
        duration: Option<i64>,
    ) -> Result<LessonMenu, DomainError> {
        LessonMenu::create(
            LessonMenuId::from_u128(9),
            COACH,
            &coach(),
            sport,
            lesson_type,
            content("フォーム改善", duration, true),
            at(1),
        )
    }

    #[test]
    fn a_coach_creates_a_live_lesson_menu_with_a_duration() {
        let menu = create(Sport::Tennis, LessonType::Live, Some(60)).unwrap();
        assert_eq!(menu.id(), LessonMenuId::from_u128(9));
        assert_eq!(menu.coach_id(), COACH);
        assert_eq!(menu.sport(), Sport::Tennis);
        assert_eq!(menu.lesson_type(), LessonType::Live);
        assert_eq!(menu.name().as_str(), "フォーム改善");
        assert_eq!(menu.description().as_str(), "初心者向け");
        assert_eq!(menu.duration().map(LessonDuration::minutes), Some(60));
        assert!(menu.is_published());
        assert_eq!((menu.created_at(), menu.updated_at()), (at(1), at(1)));
    }

    #[test]
    fn an_async_lesson_menu_has_no_duration() {
        let menu = create(Sport::Golf, LessonType::Async, None).unwrap();
        assert_eq!(menu.lesson_type(), LessonType::Async);
        assert_eq!(menu.duration(), None);
    }

    #[test]
    fn the_duration_must_match_the_lesson_type() {
        assert_eq!(
            create(Sport::Tennis, LessonType::Live, None),
            Err(DomainError::LessonDurationRequired)
        );
        assert_eq!(
            create(Sport::Tennis, LessonType::Async, Some(30)),
            Err(DomainError::LessonDurationNotApplicable)
        );
    }

    #[test]
    fn the_sport_must_be_one_the_coach_offers() {
        assert_eq!(
            create(Sport::Soccer, LessonType::Async, None),
            Err(DomainError::SportNotOfferedByCoach {
                sport: Sport::Soccer
            })
        );
    }

    #[test]
    fn the_coach_revises_and_unpublishes_the_menu() {
        let mut menu = create(Sport::Tennis, LessonType::Live, Some(60)).unwrap();
        menu.revise(COACH, content("サーブ特訓", Some(90), false), at(5))
            .unwrap();
        assert_eq!(menu.name().as_str(), "サーブ特訓");
        assert_eq!(menu.duration().map(LessonDuration::minutes), Some(90));
        assert!(!menu.is_published());
        assert_eq!((menu.created_at(), menu.updated_at()), (at(1), at(5)));
        // コーチ・スポーツ・種類は変わらない
        assert_eq!(
            (menu.coach_id(), menu.sport(), menu.lesson_type()),
            (COACH, Sport::Tennis, LessonType::Live)
        );
    }

    #[test]
    fn only_the_coach_of_the_menu_can_revise_it() {
        let original = create(Sport::Tennis, LessonType::Live, Some(60)).unwrap();
        for account in [OTHER_COACH, STUDENT] {
            let mut menu = original.clone();
            assert_eq!(
                menu.revise(account, content("乗っ取り", Some(30), false), at(5)),
                Err(DomainError::NotTheLessonMenuCoach)
            );
            assert_eq!(menu, original, "拒否したら何も変えない");
        }
    }

    #[test]
    fn revising_keeps_the_duration_rule() {
        let original = create(Sport::Tennis, LessonType::Live, Some(60)).unwrap();
        let mut menu = original.clone();
        assert_eq!(
            menu.revise(COACH, content("時間なし", None, true), at(5)),
            Err(DomainError::LessonDurationRequired)
        );
        assert_eq!(menu, original);

        let original = create(Sport::Golf, LessonType::Async, None).unwrap();
        let mut menu = original.clone();
        assert_eq!(
            menu.revise(COACH, content("時間あり", Some(30), true), at(5)),
            Err(DomainError::LessonDurationNotApplicable)
        );
        assert_eq!(menu, original);
    }

    #[test]
    fn restore_keeps_every_field() {
        let mut menu = create(Sport::Tennis, LessonType::Live, Some(45)).unwrap();
        menu.revise(COACH, content("直した", Some(45), false), at(2))
            .unwrap();
        let restored = LessonMenu::restore(
            menu.id(),
            menu.coach_id(),
            menu.sport(),
            menu.lesson_type(),
            menu.content().clone(),
            menu.created_at(),
            menu.updated_at(),
        );
        assert_eq!(restored, menu);
    }
}
