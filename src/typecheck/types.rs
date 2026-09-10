use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    I32,
    I64,
    F32,
    F64,
    Bool,
    Void,
    Array(Box<Type>, usize),
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

    pub fn is_array(&self) -> bool {
        matches!(self, Type::Array(_, _))
    }

    pub fn size_bytes(&self) -> usize {
        match self {
            Type::I32 | Type::F32 => 4,
            Type::I64 | Type::F64 => 8,
            Type::Bool => 1,
            Type::Void => 0,
            Type::Array(elem, len) => elem.size_bytes() * len,
        }
    }

    pub fn element_type(&self) -> Option<&Type> {
        match self {
            Type::Array(elem, _) => Some(elem),
            _ => None,
        }
    }

    pub fn array_len(&self) -> Option<usize> {
        match self {
            Type::Array(_, len) => Some(*len),
            _ => None,
        }
    }

    pub fn from_name(name: &str) -> Option<Type> {
        let s = name.trim();
        if s.starts_with('[') && s.ends_with(']') {
            let inner = &s[1..s.len() - 1];
            if let Some((elem_str, len_str)) = inner.split_once(';') {
                let elem = Type::from_name(elem_str.trim())?;
                let len = len_str.trim().parse::<usize>().ok()?;
                return Some(Type::Array(Box::new(elem), len));
            }
        }

        match s {
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
            Type::Array(elem, len) => write!(f, "[{}; {}]", elem, len),
        }
    }
}
