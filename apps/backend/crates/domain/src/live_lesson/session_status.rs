/// 通話セッションの状態。外部での文字列表現は境界のアダプタが決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionStatus {
    /// 開始した。まだ終了していない
    InProgress,
    /// 終了した。接続品質の指標を持つ
    Ended,
}
