use std::time::SystemTime;

use super::{AnnouncementBody, AnnouncementDraftId, AnnouncementTitle, NotificationError};
use crate::{catalog::Sport, identity::AccountId};

/// 本部からのお知らせの下書き（glossary の announcement_draft）。本部が書きかけで残したお知らせで、作った本部の
/// アカウント・題・本文・対象のスポーツ（全員なら `None`）・作った時刻・最後に直した時刻を持つ。
///
/// 下書きは生徒に届かない（お知らせ（[`super::Notification`]）もプッシュ通知も作らない）。書きかけなので題と本文は
/// 空でもよく、上限（[`AnnouncementTitle::MAX_CHARS`]・[`AnnouncementBody::MAX_CHARS`]）だけを守る。送るときの中身は
/// [`AnnouncementDraft::sendable`] で、送るお知らせ（[`super::Announcement`]）と同じ規則で確かめる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnouncementDraft {
    id: AnnouncementDraftId,
    created_by: AccountId,
    title: String,
    body: String,
    sport: Option<Sport>,
    created_at: SystemTime,
    updated_at: SystemTime,
}

impl AnnouncementDraft {
    /// 本部のアカウント `created_by` が時刻 `created_at` に作る下書き（最後に直した時刻も `created_at`）。題・本文が上限を
    /// 超えれば [`NotificationError::AnnouncementTitleTooLong`]・[`NotificationError::AnnouncementBodyTooLong`]。
    /// 保存先から読んだものを組み立てるのも同じ（最後に直した時刻は [`Self::with_updated_at`] で添える）。
    pub fn new(
        id: AnnouncementDraftId,
        created_by: AccountId,
        title: impl Into<String>,
        body: impl Into<String>,
        sport: Option<Sport>,
        created_at: SystemTime,
    ) -> Result<Self, NotificationError> {
        let (title, body) = (title.into(), body.into());
        check_lengths(&title, &body)?;
        Ok(Self {
            id,
            created_by,
            title,
            body,
            sport,
            created_at,
            updated_at: created_at,
        })
    }

    /// 最後に直した時刻 `updated_at` を添える（保存先の実装用）。
    pub const fn with_updated_at(mut self, updated_at: SystemTime) -> Self {
        self.updated_at = updated_at;
        self
    }

    pub fn id(&self) -> AnnouncementDraftId {
        self.id
    }

    /// 作った本部のアカウント。
    pub fn created_by(&self) -> AccountId {
        self.created_by
    }

    /// 題（空のこともある）。
    pub fn title(&self) -> &str {
        &self.title
    }

    /// 本文（空のこともある）。
    pub fn body(&self) -> &str {
        &self.body
    }

    /// 対象のスポーツ。生徒全員なら `None`。
    pub fn sport(&self) -> Option<Sport> {
        self.sport
    }

    pub fn created_at(&self) -> SystemTime {
        self.created_at
    }

    /// 最後に直した時刻（直していなければ作った時刻）。
    pub fn updated_at(&self) -> SystemTime {
        self.updated_at
    }

    /// 時刻 `now` に題・本文・対象のスポーツをまとめて書き換える。作った本部のアカウントと作った時刻は変えない。
    ///
    /// 題・本文が上限を超えれば [`Self::new`] と同じ誤りで、何も変えない。
    pub fn revise(
        &mut self,
        title: impl Into<String>,
        body: impl Into<String>,
        sport: Option<Sport>,
        now: SystemTime,
    ) -> Result<(), NotificationError> {
        let (title, body) = (title.into(), body.into());
        check_lengths(&title, &body)?;
        self.title = title;
        self.body = body;
        self.sport = sport;
        self.updated_at = now;
        Ok(())
    }

    /// 送るお知らせの題と本文。送るお知らせと同じ規則（[`AnnouncementTitle::new`]・[`AnnouncementBody::new`]）で確かめ、
    /// 空（空白だけ）なら [`NotificationError::EmptyAnnouncementTitle`]・[`NotificationError::EmptyAnnouncementBody`]
    /// （題を先に確かめる）。
    pub fn sendable(&self) -> Result<(AnnouncementTitle, AnnouncementBody), NotificationError> {
        Ok((
            AnnouncementTitle::new(self.title.clone())?,
            AnnouncementBody::new(self.body.clone())?,
        ))
    }
}

/// 下書きの題と本文の上限（文字数の数え方は送るお知らせと同じ）。
fn check_lengths(title: &str, body: &str) -> Result<(), NotificationError> {
    if title.chars().count() > AnnouncementTitle::MAX_CHARS {
        return Err(NotificationError::AnnouncementTitleTooLong {
            max: AnnouncementTitle::MAX_CHARS,
        });
    }
    if body.chars().count() > AnnouncementBody::MAX_CHARS {
        return Err(NotificationError::AnnouncementBodyTooLong {
            max: AnnouncementBody::MAX_CHARS,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const HEADQUARTERS: AccountId = AccountId::from_u128(5);
    const DRAFT: AnnouncementDraftId = AnnouncementDraftId::from_u128(7);

    fn at(minute: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(minute * 60)
    }

    fn draft(title: &str, body: &str) -> Result<AnnouncementDraft, NotificationError> {
        AnnouncementDraft::new(
            DRAFT,
            HEADQUARTERS,
            title,
            body,
            Some(Sport::Tennis),
            at(10),
        )
    }

    #[test]
    fn keeps_what_headquarters_wrote() {
        let draft = draft("メンテナンスのお知らせ", " 11/3 は使えません\n").unwrap();
        assert_eq!(draft.id(), DRAFT);
        assert_eq!(draft.created_by(), HEADQUARTERS);
        assert_eq!(draft.title(), "メンテナンスのお知らせ");
        assert_eq!(draft.body(), " 11/3 は使えません\n", "書いたとおりに持つ");
        assert_eq!(draft.sport(), Some(Sport::Tennis));
        assert_eq!(draft.created_at(), at(10));
        assert_eq!(draft.updated_at(), at(10), "直していなければ作った時刻");
    }

    #[test]
    fn a_draft_may_be_empty_while_being_written() {
        for (title, body) in [("", ""), (" ", "本文"), ("題", "\n")] {
            let draft = draft(title, body).unwrap();
            assert_eq!((draft.title(), draft.body()), (title, body));
        }
    }

    #[test]
    fn a_draft_keeps_the_same_limits_as_an_announcement() {
        assert!(
            draft(
                &"あ".repeat(AnnouncementTitle::MAX_CHARS),
                &"あ".repeat(AnnouncementBody::MAX_CHARS)
            )
            .is_ok()
        );
        assert_eq!(
            draft(&"あ".repeat(AnnouncementTitle::MAX_CHARS + 1), ""),
            Err(NotificationError::AnnouncementTitleTooLong {
                max: AnnouncementTitle::MAX_CHARS
            })
        );
        assert_eq!(
            draft("", &"あ".repeat(AnnouncementBody::MAX_CHARS + 1)),
            Err(NotificationError::AnnouncementBodyTooLong {
                max: AnnouncementBody::MAX_CHARS
            })
        );
    }

    #[test]
    fn revising_replaces_the_content_and_keeps_who_created_it() {
        let mut draft = draft("題", "").unwrap();
        draft.revise("新しい題", "本文", None, at(20)).unwrap();
        assert_eq!(draft.title(), "新しい題");
        assert_eq!(draft.body(), "本文");
        assert_eq!(draft.sport(), None, "スポーツも書き換える");
        assert_eq!(draft.updated_at(), at(20));
        assert_eq!(draft.created_by(), HEADQUARTERS);
        assert_eq!(draft.created_at(), at(10));
    }

    #[test]
    fn a_revision_over_the_limit_changes_nothing() {
        let mut draft = draft("題", "本文").unwrap();
        let before = draft.clone();
        assert_eq!(
            draft.revise(
                "あ".repeat(AnnouncementTitle::MAX_CHARS + 1),
                "本文",
                None,
                at(20)
            ),
            Err(NotificationError::AnnouncementTitleTooLong {
                max: AnnouncementTitle::MAX_CHARS
            })
        );
        assert_eq!(draft, before);
    }

    #[test]
    fn a_stored_update_time_is_restored() {
        let restored = draft("題", "本文").unwrap().with_updated_at(at(30));
        assert_eq!(restored.updated_at(), at(30));
        assert_eq!(restored.created_at(), at(10));
    }

    #[test]
    fn sending_checks_the_content_like_an_announcement() {
        let (title, body) = draft("題", "本文").unwrap().sendable().unwrap();
        assert_eq!((title.as_str(), body.as_str()), ("題", "本文"));
        assert_eq!(
            draft(" ", "").unwrap().sendable(),
            Err(NotificationError::EmptyAnnouncementTitle),
            "題を先に確かめる"
        );
        assert_eq!(
            draft("題", "\u{3000}").unwrap().sendable(),
            Err(NotificationError::EmptyAnnouncementBody)
        );
    }
}
