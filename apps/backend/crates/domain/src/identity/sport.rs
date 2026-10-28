/// 提供スポーツ 6 種類（specs/product-context.md。Blueprint の 5 種類と、変更シナリオ cs-001 で足した水泳）。
///
/// student-app が扱うのはゴルフ以外の 5 種類（ゴルフは golf-app の範囲）。
/// どのクライアントでどれを受け付けるか、外部での文字列表現は境界のアダプタが決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sport {
    Tennis,
    Golf,
    Baseball,
    Soccer,
    Basketball,
    Swimming,
}

impl Sport {
    pub const ALL: [Sport; 6] = [
        Sport::Tennis,
        Sport::Golf,
        Sport::Baseball,
        Sport::Soccer,
        Sport::Basketball,
        Sport::Swimming,
    ];
}
