/// アカウントの役割。外部での文字列表現は境界のアダプタが決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// 生徒（glossary の student）
    Student,
    /// コーチ（glossary の coach）
    Coach,
    /// 本部（運営のスタッフ）
    Headquarters,
}

impl Role {
    pub const ALL: [Role; 3] = [Role::Student, Role::Coach, Role::Headquarters];
}
