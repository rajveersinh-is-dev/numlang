use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    Usize,
    F32,
    F64,
    Bool,
    Void,
    Array(Box<Type>, usize),
    Str,
    Struct(String),
    Enum(String),
}

impl Type {
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Type::I8
                | Type::I16
                | Type::I32
                | Type::I64
                | Type::U8
                | Type::U16
                | Type::U32
                | Type::U64
                | Type::Usize
                | Type::F32
                | Type::F64
        )
    }

    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Type::I8
                | Type::I16
                | Type::I32
                | Type::I64
                | Type::U8
                | Type::U16
                | Type::U32
                | Type::U64
                | Type::Usize
        )
    }

    pub fn is_unsigned(&self) -> bool {
        matches!(
            self,
            Type::U8 | Type::U16 | Type::U32 | Type::U64 | Type::Usize
        )
    }

    pub fn is_signed(&self) -> bool {
        matches!(self, Type::I8 | Type::I16 | Type::I32 | Type::I64)
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Type::F32 | Type::F64)
    }

    pub fn is_struct(&self) -> bool {
        matches!(self, Type::Struct(_))
    }

    pub fn is_enum(&self) -> bool {
        matches!(self, Type::Enum(_))
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Type::Array(_, _))
    }

    pub fn size_bytes(&self) -> usize {
        match self {
            Type::I8 | Type::U8 => 1,
            Type::I16 | Type::U16 => 2,
            Type::I32 | Type::U32 | Type::F32 => 4,
            Type::I64 | Type::U64 | Type::Usize | Type::F64 => 8,
            Type::Bool => 1,
            Type::Void => 0,
            Type::Str => 8,
            Type::Array(elem, len) => elem.size_bytes() * len,
            Type::Struct(_) => 8,
            Type::Enum(_) => 8,
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

    pub fn wrap_int(&self, val: i64) -> i64 {
        match self {
            Type::I8 => (val as i8) as i64,
            Type::I16 => (val as i16) as i64,
            Type::I32 => (val as i32) as i64,
            Type::U8 => (val as u8) as i64,
            Type::U16 => (val as u16) as i64,
            Type::U32 => (val as u32) as i64,
            _ => val,
        }
    }
}

pub fn wrap_int_by_type(val: i64, ty: Option<&Type>) -> i64 {
    ty.map_or(val, |t| t.wrap_int(val))
}

impl Type {

    pub fn from_name(name: &str) -> Option<Type> {
        let s = name.trim();
        if s.starts_with('[') && s.ends_with(']') {
            let inner = &s[1..s.len() - 1];
            let mut depth = 0;
            let mut split_idx = None;
            for (i, c) in inner.char_indices() {
                match c {
                    '[' => depth += 1,
                    ']' => depth -= 1,
                    ';' if depth == 0 => {
                        split_idx = Some(i);
                        break;
                    }
                    _ => {}
                }
            }
            if let Some(idx) = split_idx {
                let elem_str = &inner[..idx];
                let len_str = &inner[idx + 1..];
                let elem = Type::from_name(elem_str.trim())?;
                let len = len_str.trim().parse::<usize>().ok()?;
                return Some(Type::Array(Box::new(elem), len));
            }
        }

        match s {
            "i8" => Some(Type::I8),
            "i16" => Some(Type::I16),
            "i32" => Some(Type::I32),
            "i64" => Some(Type::I64),
            "u8" => Some(Type::U8),
            "u16" => Some(Type::U16),
            "u32" => Some(Type::U32),
            "u64" => Some(Type::U64),
            "usize" => Some(Type::Usize),
            "f32" => Some(Type::F32),
            "f64" => Some(Type::F64),
            "bool" => Some(Type::Bool),
            "str" => Some(Type::Str),
            "void" | "()" => Some(Type::Void),
            _ => {
                if s.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_')
                    && s.chars().all(|c| c.is_alphanumeric() || c == '_')
                {
                    Some(Type::Struct(s.to_string()))
                } else {
                    None
                }
            }
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::I8 => write!(f, "i8"),
            Type::I16 => write!(f, "i16"),
            Type::I32 => write!(f, "i32"),
            Type::I64 => write!(f, "i64"),
            Type::U8 => write!(f, "u8"),
            Type::U16 => write!(f, "u16"),
            Type::U32 => write!(f, "u32"),
            Type::U64 => write!(f, "u64"),
            Type::Usize => write!(f, "usize"),
            Type::F32 => write!(f, "f32"),
            Type::F64 => write!(f, "f64"),
            Type::Bool => write!(f, "bool"),
            Type::Str => write!(f, "str"),
            Type::Void => write!(f, "void"),
            Type::Array(elem, len) => write!(f, "[{}; {}]", elem, len),
            Type::Struct(name) => write!(f, "{}", name),
            Type::Enum(name) => write!(f, "{}", name),
        }
    }
}
