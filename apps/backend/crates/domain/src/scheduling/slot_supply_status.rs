/// スポーツごとの予約枠の需給の判定（[`super::SlotSupply::status`]）。外部での文字列表現は境界のアダプタが決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotSupplyStatus {
    /// 予約しようとした生徒の数に対して、公開した枠が足りている
    Sufficient,
    /// 予約しようとした生徒の数に対して、公開した枠が足りていない
    Insufficient,
    /// 公開した枠も、予約しようとした生徒も 0（足りているかを数からは言えない）
    NoActivity,
    /// 月額の契約の対象でないスポーツ（ゴルフ）で、需給を判定しない
    NotApplicable,
}
