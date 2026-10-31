use super::{Coach, Role, Student};

/// 役割ごとの属性。役割と属性の組み合わせを型で固定する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoleAttributes {
    Student(Student),
    Coach(Coach),
    Headquarters,
}

impl RoleAttributes {
    pub const fn role(&self) -> Role {
        match self {
            RoleAttributes::Student(_) => Role::Student,
            RoleAttributes::Coach(_) => Role::Coach,
            RoleAttributes::Headquarters => Role::Headquarters,
        }
    }
}
