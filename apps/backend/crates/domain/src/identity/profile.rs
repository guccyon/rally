use super::{Coach, Role, Student};

/// 役割ごとの属性。役割と属性の組み合わせを型で固定する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Profile {
    Student(Student),
    Coach(Coach),
    Headquarters,
}

impl Profile {
    pub const fn role(&self) -> Role {
        match self {
            Profile::Student(_) => Role::Student,
            Profile::Coach(_) => Role::Coach,
            Profile::Headquarters => Role::Headquarters,
        }
    }
}
