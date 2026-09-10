use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Type {
    I32,
    I64,
    F32,
    F64,
    Bool,
    Void,
}

impl Type {
    pub fn is_numeric(&self) -> bool {
        matches!(self, Type::I32 | Type::I64 | Type::F32 | Type::F64)
    }

    pub fn is_integer(&self) -> bool {
        matches!(self, Type::I32 | Type::I64)
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Type::F32 | Type::F64)
    }

    pub fn from_name(name: &str) -> Option<Type> {
        match name {
            "i32" => Some(Type::I32),
            "i64" => Some(Type::I64),
            "f32" => Some(Type::F32),
            "f64" => Some(Type::F64),
            "bool" => Some(Type::Bool),
            "void" | "()" => Some(Type::Void),
            _ => None,
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::I32 => write!(f, "i32"),
            Type::I64 => write!(f, "i64"),
            Type::F32 => write!(f, "f32"),
            Type::F64 => write!(f, "f64"),
            Type::Bool => write!(f, "bool"),
            Type::Void => write!(f, "void"),
        }
    }
}
