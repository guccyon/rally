use crate::catalog::Sport;

/// 本部がお知らせを競技で絞るときの条件（admin-web の `listAnnouncements` と `listAnnouncementDrafts` の `sport`）。
/// 送ったお知らせ（[`super::Announcement`]）も下書き（[`super::AnnouncementDraft`]）も、対象のスポーツをどう照らすかは
/// ここだけに置く。
///
/// - 競技 `Some(sport)`: 対象のスポーツが `sport` と同じものだけ。生徒全員に向けたもの（対象のスポーツが `None`）は入らない
/// - `None`（既定）: 絞らない（対象のスポーツを問わずすべて入る）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnnouncementSportFilter(Option<Sport>);

impl AnnouncementSportFilter {
    /// 競技 `sport` で絞る条件。`None` なら絞らない。
    pub const fn new(sport: Option<Sport>) -> Self {
        Self(sport)
    }

    /// 絞る競技。絞らないなら `None`。
    pub const fn sport(&self) -> Option<Sport> {
        self.0
    }

    /// 対象のスポーツ `audience`（生徒全員に向けたものなら `None`）がこの条件に合うか。
    pub fn matches(&self, audience: Option<Sport>) -> bool {
        self.0.is_none_or(|sport| audience == Some(sport))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_sport_matches_every_audience() {
        let any = AnnouncementSportFilter::default();
        assert_eq!(any, AnnouncementSportFilter::new(None));
        assert!(any.matches(None), "生徒全員に向けたものも入る");
        for sport in Sport::ALL {
            assert!(any.matches(Some(sport)));
        }
    }

    #[test]
    fn a_sport_matches_only_the_same_sport() {
        let tennis = AnnouncementSportFilter::new(Some(Sport::Tennis));
        assert_eq!(tennis.sport(), Some(Sport::Tennis));
        assert!(tennis.matches(Some(Sport::Tennis)));
        assert!(!tennis.matches(Some(Sport::Golf)));
    }

    #[test]
    fn a_sport_never_matches_every_student() {
        for sport in Sport::ALL {
            assert!(
                !AnnouncementSportFilter::new(Some(sport)).matches(None),
                "生徒全員に向けたものはスポーツで絞ると入らない: {sport:?}"
            );
        }
    }
}
