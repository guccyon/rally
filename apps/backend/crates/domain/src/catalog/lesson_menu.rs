use std::time::SystemTime;

use super::{LessonFormat, LessonMenuContent, LessonMenuDescription, LessonMenuId, LessonMenuName};
use crate::{
    DomainError,
    identity::{AccountId, Coach, Sport},
};

/// コーチのレッスンメニュー（glossary の lesson「レッスン種別」）。集約。
///
/// コーチ・スポーツ（そのコーチの提供スポーツのどれか）・形式（同期なら 1 回の時間、非同期）・名前・説明・
/// 公開中かを持つ。作る・直す・非公開にするのはそのコーチだけで、コーチは作ったあと変わらない。
/// 公開中でないメニューは生徒に見せない（どこに見せるかは application と境界が決める）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonMenu {
    id: LessonMenuId,
    coach_id: AccountId,
    content: LessonMenuContent,
    is_published: bool,
    created_at: SystemTime,
}

impl LessonMenu {
    /// コーチ `coach_id`（属性は `coach`）がメニューを作る。スポーツがコーチの提供スポーツでなければ
    /// [`DomainError::SportNotOfferedByCoach`]。
    pub fn create(
        id: LessonMenuId,
        coach_id: AccountId,
        coach: &Coach,
        content: LessonMenuContent,
        is_published: bool,
        now: SystemTime,
    ) -> Result<Self, DomainError> {
        ensure_offered(coach, content.sport)?;
        Ok(Self {
            id,
            coach_id,
            content,
            is_published,
            created_at: now,
        })
    }

    /// コーチ `coach_id`（属性は `coach`）がメニューの中身と公開中かを丸ごと置き換える（非公開にするのもこれ）。
    ///
    /// メニューのコーチでなければ [`DomainError::NotTheLessonMenuCoach`]、スポーツがコーチの提供スポーツでなければ
    /// [`DomainError::SportNotOfferedByCoach`] で、どちらも何も変えない。
    pub fn revise(
        &mut self,
        coach_id: AccountId,
        coach: &Coach,
        content: LessonMenuContent,
        is_published: bool,
    ) -> Result<(), DomainError> {
        if coach_id != self.coach_id {
            return Err(DomainError::NotTheLessonMenuCoach);
        }
        ensure_offered(coach, content.sport)?;
        self.content = content;
        self.is_published = is_published;
        Ok(())
    }

    /// 保存先から読んだメニューを組み立てる（保存先の実装用）。
    pub const fn restore(
        id: LessonMenuId,
        coach_id: AccountId,
        content: LessonMenuContent,
        is_published: bool,
        created_at: SystemTime,
    ) -> Self {
        Self {
            id,
            coach_id,
            content,
            is_published,
            created_at,
        }
    }

    pub const fn id(&self) -> LessonMenuId {
        self.id
    }

    pub const fn coach_id(&self) -> AccountId {
        self.coach_id
    }

    pub const fn content(&self) -> &LessonMenuContent {
        &self.content
    }

    pub const fn sport(&self) -> Sport {
        self.content.sport
    }

    pub const fn format(&self) -> LessonFormat {
        self.content.format
    }

    pub const fn name(&self) -> &LessonMenuName {
        &self.content.name
    }

    pub const fn description(&self) -> &LessonMenuDescription {
        &self.content.description
    }

    pub const fn is_published(&self) -> bool {
        self.is_published
    }

    pub const fn created_at(&self) -> SystemTime {
        self.created_at
    }
}

fn ensure_offered(coach: &Coach, sport: Sport) -> Result<(), DomainError> {
    if coach.offers(sport) {
        Ok(())
    } else {
        Err(DomainError::SportNotOfferedByCoach { sport })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{catalog::LessonDuration, lesson::LessonType};

    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn coach() -> Coach {
        Coach::new([Sport::Tennis, Sport::Swimming]).unwrap()
    }

    fn content(sport: Sport, format: LessonFormat, name: &str) -> LessonMenuContent {
        LessonMenuContent {
            sport,
            format,
            name: LessonMenuName::new(name).unwrap(),
            description: LessonMenuDescription::new("初心者向け").unwrap(),
        }
    }

    fn live(minutes: i64) -> LessonFormat {
        LessonFormat::new(
            LessonType::Live,
            Some(LessonDuration::from_minutes(minutes).unwrap()),
        )
        .unwrap()
    }

    fn menu() -> LessonMenu {
        LessonMenu::create(
            LessonMenuId::from_u128(1),
            COACH,
            &coach(),
            content(Sport::Tennis, live(30), "フォア改善"),
            true,
            at(10),
        )
        .unwrap()
    }

    #[test]
    fn a_coach_creates_a_menu_for_one_of_its_sports() {
        let menu = menu();
        assert_eq!(menu.id(), LessonMenuId::from_u128(1));
        assert_eq!(menu.coach_id(), COACH);
        assert_eq!(menu.sport(), Sport::Tennis);
        assert_eq!(menu.format().lesson_type(), LessonType::Live);
        assert_eq!(
            menu.format().duration().map(LessonDuration::minutes),
            Some(30)
        );
        assert_eq!(menu.name().as_str(), "フォア改善");
        assert_eq!(menu.description().as_str(), "初心者向け");
        assert!(menu.is_published());
        assert_eq!(menu.created_at(), at(10));
    }

    #[test]
    fn a_menu_for_a_sport_the_coach_does_not_offer_is_rejected() {
        assert_eq!(
            LessonMenu::create(
                LessonMenuId::from_u128(1),
                COACH,
                &coach(),
                content(Sport::Golf, LessonFormat::Async, "スイング添削"),
                true,
                at(10),
            ),
            Err(DomainError::SportNotOfferedByCoach { sport: Sport::Golf })
        );
    }

    #[test]
    fn the_coach_revises_and_unpublishes_its_menu() {
        let mut menu = menu();
        let revised = content(Sport::Swimming, LessonFormat::Async, "クロールの添削");
        menu.revise(COACH, &coach(), revised.clone(), false)
            .unwrap();
        assert_eq!(menu.content(), &revised);
        assert!(!menu.is_published());
        assert_eq!(menu.coach_id(), COACH, "コーチは変わらない");
        assert_eq!(menu.created_at(), at(10), "作った時刻は変わらない");
    }

    #[test]
    fn only_the_coach_of_the_menu_can_revise_it() {
        let mut menu = menu();
        let before = menu.clone();
        assert_eq!(
            menu.revise(
                OTHER_COACH,
                &coach(),
                content(Sport::Tennis, live(60), "乗っ取り"),
                false
            ),
            Err(DomainError::NotTheLessonMenuCoach)
        );
        assert_eq!(menu, before, "何も変えない");
    }

    #[test]
    fn a_revision_to_a_sport_the_coach_does_not_offer_changes_nothing() {
        let mut menu = menu();
        let before = menu.clone();
        assert_eq!(
            menu.revise(
                COACH,
                &coach(),
                content(Sport::Soccer, live(60), "リフティング"),
                true
            ),
            Err(DomainError::SportNotOfferedByCoach {
                sport: Sport::Soccer
            })
        );
        assert_eq!(menu, before);
    }

    #[test]
    fn restore_keeps_every_field() {
        let menu = menu();
        let restored = LessonMenu::restore(
            menu.id(),
            menu.coach_id(),
            menu.content().clone(),
            menu.is_published(),
            menu.created_at(),
        );
        assert_eq!(restored, menu);
    }
}
