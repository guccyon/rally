/// メッセージを送ったのがスレッドのどちらの当事者か。外部での文字列表現は境界のアダプタが決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageSender {
    /// スレッドの生徒（glossary の student）
    Student,
    /// スレッドの担当コーチ（glossary の coach）
    Coach,
}

impl MessageSender {
    pub const ALL: [MessageSender; 2] = [MessageSender::Student, MessageSender::Coach];
}
