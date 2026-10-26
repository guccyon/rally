/// 動画の状態（添削の進み具合）。外部での文字列表現は境界のアダプタが決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoStatus {
    /// 送付した。コーチはまだ添削を返していない
    Submitted,
    /// コーチが添削を返した
    Reviewed,
}
