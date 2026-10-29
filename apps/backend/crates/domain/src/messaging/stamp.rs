/// スタンプ（チャットで本文の代わりに送る、決まった絵柄の 1 つ）。種類は決まった 8 つで、本文は持たない。
///
/// 外部での文字列表現（契約・DB の ID）と利用者に見せる呼び名は境界のアダプタが決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stamp {
    /// ありがとう
    Thanks,
    /// いいね
    Good,
    /// 了解
    GotIt,
    /// がんばろう
    LetsGo,
    /// ナイスショット
    NiceShot,
    /// おつかれさま
    Tired,
    /// 質問
    Question,
    /// OK
    Ok,
}

impl Stamp {
    pub const ALL: [Stamp; 8] = [
        Stamp::Thanks,
        Stamp::Good,
        Stamp::GotIt,
        Stamp::LetsGo,
        Stamp::NiceShot,
        Stamp::Tired,
        Stamp::Question,
        Stamp::Ok,
    ];
}
