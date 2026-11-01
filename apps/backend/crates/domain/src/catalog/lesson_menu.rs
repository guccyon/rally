use std::time::SystemTime;

use super::{
    HeadquartersUnpublish, HeadquartersUnpublishReason, LessonFormat, LessonMenuContent,
    LessonMenuDescription, LessonMenuId, LessonMenuName, LessonMenuPublicationAction,
    LessonMenuPublicationChange,
};
use crate::{
    DomainError,
    catalog::Sport,
    identity::{AccountId, Coach},
};

/// コーチのレッスンメニュー（glossary の lesson「レッスン種別」）。集約。
///
/// コーチ・スポーツ（そのコーチの提供スポーツのどれか）・形式（同期なら 1 回の時間、非同期）・名前・説明・
/// 公開中かを持つ。作る・直す・非公開にするのはそのコーチで、コーチは作ったあと変わらない。
/// 公開中でないメニューは生徒に見せない（どこに見せるかは application と境界が決める）。
///
/// 本部はコーチに代わって非公開にでき（[`Self::unpublish_by_headquarters`]。時刻と理由を [`HeadquartersUnpublish`] で残す）、
/// そのあいだコーチは中身を直せても公開し直せない。公開に戻すのは本部だけ（[`Self::republish_by_headquarters`]）。
/// どちらも、誰がいつどうしたかの記録（[`LessonMenuPublicationChange`]）を返す。誰が本部か（役割）は境界が確かめる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonMenu {
    id: LessonMenuId,
    coach_id: AccountId,
    content: LessonMenuContent,
    is_published: bool,
    created_at: SystemTime,
    headquarters_unpublish: Option<HeadquartersUnpublish>,
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
            headquarters_unpublish: None,
        })
    }

    /// コーチ `coach_id`（属性は `coach`）がメニューの中身と公開中かを丸ごと置き換える（非公開にするのもこれ）。
    ///
    /// メニューのコーチでなければ [`DomainError::NotTheLessonMenuCoach`]、本部が非公開にしたメニューを公開中にしようと
    /// すれば [`DomainError::LessonMenuUnpublishedByHeadquarters`]、スポーツがコーチの提供スポーツでなければ
    /// [`DomainError::SportNotOfferedByCoach`] で、どれも何も変えない。本部が非公開にしたメニューでも、非公開のままなら
    /// 中身は直せる（本部が非公開にしたことと理由はそのまま）。
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
        if is_published && self.headquarters_unpublish.is_some() {
            return Err(DomainError::LessonMenuUnpublishedByHeadquarters);
        }
        ensure_offered(coach, content.sport)?;
        self.content = content;
        self.is_published = is_published;
        Ok(())
    }

    /// 本部がメニューを非公開にする（公開中かを問わない。コーチが非公開にしていたメニューも、コーチが公開し直せなくなる）。
    /// 理由 `reason`（空なら書かなかった）と時刻 `now` を残し、非公開にした本部のアカウント `changed_by` と時刻と理由の
    /// 記録を返す。
    ///
    /// すでに本部が非公開にしていれば [`DomainError::LessonMenuAlreadyUnpublishedByHeadquarters`] で、何も変えず記録も
    /// 作らない（最初の時刻と理由を残す）。
    pub fn unpublish_by_headquarters(
        &mut self,
        changed_by: AccountId,
        reason: HeadquartersUnpublishReason,
        now: SystemTime,
    ) -> Result<LessonMenuPublicationChange, DomainError> {
        if self.headquarters_unpublish.is_some() {
            return Err(DomainError::LessonMenuAlreadyUnpublishedByHeadquarters);
        }
        self.is_published = false;
        self.headquarters_unpublish = Some(HeadquartersUnpublish::new(reason.clone(), now));
        Ok(LessonMenuPublicationChange::restore(
            self.id,
            LessonMenuPublicationAction::Unpublished { reason },
            changed_by,
            now,
        ))
    }

    /// 本部が非公開にしたメニューを公開に戻す（公開中にし、本部が非公開にしたことと理由を消す）。以後はコーチがまた
    /// 公開・非公開を決められる。公開に戻した本部のアカウント `changed_by` と時刻 `now` の記録を返す。
    ///
    /// 本部が非公開にしたメニューでなければ（コーチが自分で非公開にしたメニューも）
    /// [`DomainError::LessonMenuNotUnpublishedByHeadquarters`] で、何も変えず記録も作らない。
    pub fn republish_by_headquarters(
        &mut self,
        changed_by: AccountId,
        now: SystemTime,
    ) -> Result<LessonMenuPublicationChange, DomainError> {
        if self.headquarters_unpublish.is_none() {
            return Err(DomainError::LessonMenuNotUnpublishedByHeadquarters);
        }
        self.is_published = true;
        self.headquarters_unpublish = None;
        Ok(LessonMenuPublicationChange::restore(
            self.id,
            LessonMenuPublicationAction::Republished,
            changed_by,
            now,
        ))
    }

    /// 保存先から読んだメニューを組み立てる（保存先の実装用）。本部が非公開にしたことは
    /// [`Self::with_headquarters_unpublish`] で添える（既定はなし）。
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
            headquarters_unpublish: None,
        }
    }

    /// 保存先から読んだ、本部が非公開にしたこと `headquarters_unpublish` を添える（保存先の実装用。[`Self::restore`] の続き）。
    pub fn with_headquarters_unpublish(
        self,
        headquarters_unpublish: Option<HeadquartersUnpublish>,
    ) -> Self {
        Self {
            headquarters_unpublish,
            ..self
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

    /// 本部が非公開にしたこと（時刻と理由）。本部が非公開にしていなければ `None`。
    pub const fn headquarters_unpublish(&self) -> Option<&HeadquartersUnpublish> {
        self.headquarters_unpublish.as_ref()
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
    const HEADQUARTERS: AccountId = AccountId::from_u128(1);
    const OTHER_HEADQUARTERS: AccountId = AccountId::from_u128(3);

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
        let mut menu = menu();
        let restored = LessonMenu::restore(
            menu.id(),
            menu.coach_id(),
            menu.content().clone(),
            menu.is_published(),
            menu.created_at(),
        );
        assert_eq!(restored, menu);
        assert_eq!(menu.headquarters_unpublish(), None);

        menu.unpublish_by_headquarters(HEADQUARTERS, reason("誤った記載"), at(20))
            .unwrap();
        let restored = LessonMenu::restore(
            menu.id(),
            menu.coach_id(),
            menu.content().clone(),
            menu.is_published(),
            menu.created_at(),
        )
        .with_headquarters_unpublish(menu.headquarters_unpublish().cloned());
        assert_eq!(restored, menu);
    }

    fn reason(text: &str) -> HeadquartersUnpublishReason {
        HeadquartersUnpublishReason::new(text).unwrap()
    }

    #[test]
    fn headquarters_unpublishes_a_menu_with_the_time_and_reason() {
        let mut menu = menu();
        let before = menu.clone();
        let change = menu
            .unpublish_by_headquarters(HEADQUARTERS, reason("不適切な表現"), at(20))
            .unwrap();
        assert_eq!(
            change,
            LessonMenuPublicationChange::restore(
                menu.id(),
                LessonMenuPublicationAction::Unpublished {
                    reason: reason("不適切な表現")
                },
                HEADQUARTERS,
                at(20)
            ),
            "誰がいつ理由をつけて非公開にしたかの記録を返す"
        );
        assert!(!menu.is_published());
        assert_eq!(
            menu.headquarters_unpublish(),
            Some(&HeadquartersUnpublish::new(reason("不適切な表現"), at(20)))
        );
        assert_eq!(menu.content(), before.content(), "中身は変えない");
        assert_eq!(menu.coach_id(), COACH);
    }

    #[test]
    fn headquarters_can_unpublish_a_menu_the_coach_already_unpublished() {
        let mut menu = menu();
        let current = menu.content().clone();
        menu.revise(COACH, &coach(), current, false).unwrap();
        menu.unpublish_by_headquarters(
            HEADQUARTERS,
            HeadquartersUnpublishReason::default(),
            at(20),
        )
        .unwrap();
        assert!(!menu.is_published());
        assert_eq!(
            menu.headquarters_unpublish().map(|u| u.reason().as_str()),
            Some("")
        );
    }

    #[test]
    fn unpublishing_twice_by_headquarters_keeps_the_first_record() {
        let mut menu = menu();
        menu.unpublish_by_headquarters(HEADQUARTERS, reason("最初"), at(20))
            .unwrap();
        let before = menu.clone();
        assert_eq!(
            menu.unpublish_by_headquarters(HEADQUARTERS, reason("二度目"), at(30)),
            Err(DomainError::LessonMenuAlreadyUnpublishedByHeadquarters)
        );
        assert_eq!(menu, before);
    }

    #[test]
    fn the_coach_cannot_republish_a_menu_headquarters_unpublished() {
        let mut menu = menu();
        menu.unpublish_by_headquarters(HEADQUARTERS, reason("誤った記載"), at(20))
            .unwrap();
        let before = menu.clone();
        assert_eq!(
            menu.revise(
                COACH,
                &coach(),
                content(Sport::Tennis, live(30), "直した"),
                true
            ),
            Err(DomainError::LessonMenuUnpublishedByHeadquarters)
        );
        assert_eq!(menu, before, "何も変えない");
    }

    #[test]
    fn the_coach_can_fix_the_content_while_unpublished_by_headquarters() {
        let mut menu = menu();
        menu.unpublish_by_headquarters(HEADQUARTERS, reason("誤った記載"), at(20))
            .unwrap();
        let fixed = content(Sport::Tennis, live(30), "直した");
        menu.revise(COACH, &coach(), fixed.clone(), false).unwrap();
        assert_eq!(menu.content(), &fixed);
        assert!(!menu.is_published());
        assert_eq!(
            menu.headquarters_unpublish(),
            Some(&HeadquartersUnpublish::new(reason("誤った記載"), at(20))),
            "本部が非公開にしたことは残る"
        );
    }

    #[test]
    fn headquarters_republishes_and_the_coach_decides_again() {
        let mut menu = menu();
        menu.unpublish_by_headquarters(HEADQUARTERS, reason("誤った記載"), at(20))
            .unwrap();
        let change = menu
            .republish_by_headquarters(OTHER_HEADQUARTERS, at(30))
            .unwrap();
        assert_eq!(change.lesson_menu_id(), menu.id());
        assert_eq!(change.action(), &LessonMenuPublicationAction::Republished);
        assert_eq!(change.changed_by(), OTHER_HEADQUARTERS);
        assert_eq!(change.changed_at(), at(30));
        assert!(menu.is_published());
        assert_eq!(menu.headquarters_unpublish(), None);
        let current = menu.content().clone();
        menu.revise(COACH, &coach(), current.clone(), false)
            .unwrap();
        menu.revise(COACH, &coach(), current, true).unwrap();
        assert!(menu.is_published());
    }

    #[test]
    fn republishing_a_menu_headquarters_did_not_unpublish_is_rejected() {
        let mut published = menu();
        let before = published.clone();
        assert_eq!(
            published.republish_by_headquarters(HEADQUARTERS, at(30)),
            Err(DomainError::LessonMenuNotUnpublishedByHeadquarters)
        );
        assert_eq!(published, before);

        let mut unpublished_by_coach = menu();
        let current = unpublished_by_coach.content().clone();
        unpublished_by_coach
            .revise(COACH, &coach(), current, false)
            .unwrap();
        let before = unpublished_by_coach.clone();
        assert_eq!(
            unpublished_by_coach.republish_by_headquarters(HEADQUARTERS, at(30)),
            Err(DomainError::LessonMenuNotUnpublishedByHeadquarters),
            "コーチが自分で非公開にしたメニューは本部が公開に戻さない"
        );
        assert_eq!(unpublished_by_coach, before);
    }
}
