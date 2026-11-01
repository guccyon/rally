use super::{CoachBio, CoachProfileField};

/// コーチのプロフィールの足りない項目（空の項目）。本部が自己紹介の書きかけのコーチを拾う（admin-web の
/// `listIncompleteCoachProfiles`）ときの判定で、「足りない」とみなす規則はここだけに置く。
///
/// - 自己紹介文（[`CoachProfileField::Bio`]）: 空か、空白（全角の空白・改行を含む Unicode の空白）だけなら足りない
///
/// 項目は [`CoachProfileField::ALL`] の順に並ぶ。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MissingCoachProfileFields(Vec<CoachProfileField>);

impl MissingCoachProfileFields {
    /// 自己紹介文 `bio` のコーチのプロフィールの足りない項目。
    pub fn of(bio: &CoachBio) -> Self {
        Self(
            CoachProfileField::ALL
                .into_iter()
                .filter(|field| match field {
                    CoachProfileField::Bio => bio.as_str().trim().is_empty(),
                })
                .collect(),
        )
    }

    /// 足りない項目（[`CoachProfileField::ALL`] の順）。
    pub fn fields(&self) -> &[CoachProfileField] {
        &self.0
    }

    /// 足りない項目がない（プロフィールを書き終えている）か。
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn missing(bio: &str) -> MissingCoachProfileFields {
        MissingCoachProfileFields::of(&CoachBio::new(bio).unwrap())
    }

    #[test]
    fn empty_bio_is_missing() {
        let fields = MissingCoachProfileFields::of(&CoachBio::default());
        assert_eq!(fields.fields(), [CoachProfileField::Bio]);
        assert!(!fields.is_empty());
    }

    #[test]
    fn bio_of_only_spaces_is_missing() {
        for bio in [" ", "\u{3000}", "\n\t \u{3000}"] {
            assert_eq!(missing(bio).fields(), [CoachProfileField::Bio], "{bio:?}");
        }
    }

    #[test]
    fn written_bio_is_not_missing() {
        for bio in ["テニス歴 20 年", " よろしく ", "あ"] {
            let fields = missing(bio);
            assert!(fields.is_empty(), "{bio:?}");
            assert_eq!(fields.fields(), []);
        }
    }

    #[test]
    fn default_has_no_missing_fields() {
        assert!(MissingCoachProfileFields::default().is_empty());
    }
}
