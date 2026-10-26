/// Blueprint のスポーツ 5 種類（specs/product-context.md）。
///
/// student-app が扱うのはゴルフ以外の 4 種類（ゴルフは golf-app の範囲）。
/// どのクライアントでどれを受け付けるか、外部での文字列表現は境界のアダプタが決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sport {
    Tennis,
    Golf,
    Baseball,
    Soccer,
    Basketball,
}

impl Sport {
    pub const ALL: [Sport; 5] = [
        Sport::Tennis,
        Sport::Golf,
        Sport::Baseball,
        Sport::Soccer,
        Sport::Basketball,
    ];
}
