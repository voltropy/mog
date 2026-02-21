//! Mog type system — all types used during compilation.

use std::collections::BTreeMap;
use std::fmt;

// ── Numeric kind enums ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntegerKind {
    I8,
    I16,
    I32,
    I64,
    I128,
    I256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnsignedKind {
    U8,
    U16,
    U32,
    U64,
    U128,
    U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FloatKind {
    F8,
    F16,
    F32,
    F64,
    F128,
    F256,
    Bf16,
}

impl IntegerKind {
    pub fn bits(self) -> u32 {
        match self {
            Self::I8 => 8,
            Self::I16 => 16,
            Self::I32 => 32,
            Self::I64 => 64,
            Self::I128 => 128,
            Self::I256 => 256,
        }
    }
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "i8" => Some(Self::I8),
            "i16" => Some(Self::I16),
            "i32" => Some(Self::I32),
            "i64" => Some(Self::I64),
            "i128" => Some(Self::I128),
            "i256" => Some(Self::I256),
            _ => None,
        }
    }
}

impl UnsignedKind {
    pub fn bits(self) -> u32 {
        match self {
            Self::U8 => 8,
            Self::U16 => 16,
            Self::U32 => 32,
            Self::U64 => 64,
            Self::U128 => 128,
            Self::U256 => 256,
        }
    }
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "u8" => Some(Self::U8),
            "u16" => Some(Self::U16),
            "u32" => Some(Self::U32),
            "u64" => Some(Self::U64),
            "u128" => Some(Self::U128),
            "u256" => Some(Self::U256),
            _ => None,
        }
    }
}

impl FloatKind {
    pub fn bits(self) -> u32 {
        match self {
            Self::F8 => 8,
            Self::F16 => 16,
            Self::F32 => 32,
            Self::F64 => 64,
            Self::F128 => 128,
            Self::F256 => 256,
            Self::Bf16 => 16,
        }
    }
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "f8" => Some(Self::F8),
            "f16" => Some(Self::F16),
            "f32" => Some(Self::F32),
            "f64" => Some(Self::F64),
            "f128" => Some(Self::F128),
            "f256" => Some(Self::F256),
            "bf16" => Some(Self::Bf16),
            _ => None,
        }
    }
}

impl fmt::Display for IntegerKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::I8 => write!(f, "i8"),
            Self::I16 => write!(f, "i16"),
            Self::I32 => write!(f, "i32"),
            Self::I64 => write!(f, "i64"),
            Self::I128 => write!(f, "i128"),
            Self::I256 => write!(f, "i256"),
        }
    }
}

impl fmt::Display for UnsignedKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::U8 => write!(f, "u8"),
            Self::U16 => write!(f, "u16"),
            Self::U32 => write!(f, "u32"),
            Self::U64 => write!(f, "u64"),
            Self::U128 => write!(f, "u128"),
            Self::U256 => write!(f, "u256"),
        }
    }
}

impl fmt::Display for FloatKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::F8 => write!(f, "f8"),
            Self::F16 => write!(f, "f16"),
            Self::F32 => write!(f, "f32"),
            Self::F64 => write!(f, "f64"),
            Self::F128 => write!(f, "f128"),
            Self::F256 => write!(f, "f256"),
            Self::Bf16 => write!(f, "bf16"),
        }
    }
}

// ── The Type enum — Mog's type system ────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Integer(IntegerKind),
    Unsigned(UnsignedKind),
    Float(FloatKind),
    Bool,
    String,
    Void,
    Pointer(Option<Box<Type>>),
    Array {
        element_type: Box<Type>,
        dimensions: Vec<usize>,
    },
    Map {
        key_type: Box<Type>,
        value_type: Box<Type>,
    },
    Struct {
        name: String,
        fields: BTreeMap<String, Type>,
    },
    AOS {
        element_type: Box<Type>,
        size: Option<usize>,
    },
    SOA {
        struct_type: Box<Type>,
        capacity: Option<usize>,
    },
    Custom(String),
    TypeAlias {
        name: String,
        aliased_type: Box<Type>,
    },
    Function {
        param_types: Vec<Type>,
        return_type: Box<Type>,
    },
    Tensor {
        dtype: Box<Type>,
        shape: Option<Vec<usize>>,
    },
    Result(Box<Type>),
    Optional(Box<Type>),
    Future(Box<Type>),
}

impl Type {
    // ── Constructors (convenience) ───────────────────────────────

    pub fn int() -> Self {
        Self::Integer(IntegerKind::I64)
    }
    pub fn float() -> Self {
        Self::Float(FloatKind::F64)
    }
    pub fn bool() -> Self {
        Self::Bool
    }
    pub fn string() -> Self {
        Self::String
    }
    pub fn void() -> Self {
        Self::Void
    }

    pub fn array(element_type: Type) -> Self {
        Self::Array {
            element_type: Box::new(element_type),
            dimensions: vec![],
        }
    }
    pub fn array_with_dims(element_type: Type, dimensions: Vec<usize>) -> Self {
        Self::Array {
            element_type: Box::new(element_type),
            dimensions,
        }
    }
    pub fn map(key_type: Type, value_type: Type) -> Self {
        Self::Map {
            key_type: Box::new(key_type),
            value_type: Box::new(value_type),
        }
    }
    pub fn struct_type(name: impl Into<String>, fields: BTreeMap<String, Type>) -> Self {
        Self::Struct {
            name: name.into(),
            fields,
        }
    }
    pub fn result(inner: Type) -> Self {
        Self::Result(Box::new(inner))
    }
    pub fn optional(inner: Type) -> Self {
        Self::Optional(Box::new(inner))
    }
    pub fn future(inner: Type) -> Self {
        Self::Future(Box::new(inner))
    }
    pub fn pointer(element: Option<Type>) -> Self {
        Self::Pointer(element.map(Box::new))
    }
    pub fn function(params: Vec<Type>, ret: Type) -> Self {
        Self::Function {
            param_types: params,
            return_type: Box::new(ret),
        }
    }
    pub fn tensor(dtype: Type, shape: Option<Vec<usize>>) -> Self {
        Self::Tensor {
            dtype: Box::new(dtype),
            shape,
        }
    }
    pub fn custom(name: impl Into<String>) -> Self {
        Self::Custom(name.into())
    }
    pub fn type_alias(name: impl Into<String>, aliased: Type) -> Self {
        Self::TypeAlias {
            name: name.into(),
            aliased_type: Box::new(aliased),
        }
    }

    // ── Type predicates ──────────────────────────────────────────

    pub fn is_integer(&self) -> bool {
        matches!(self, Self::Integer(_))
    }
    pub fn is_unsigned(&self) -> bool {
        matches!(self, Self::Unsigned(_))
    }
    pub fn is_float(&self) -> bool {
        matches!(self, Self::Float(_))
    }
    pub fn is_bool(&self) -> bool {
        matches!(self, Self::Bool)
    }
    pub fn is_string(&self) -> bool {
        matches!(self, Self::String)
    }
    pub fn is_void(&self) -> bool {
        matches!(self, Self::Void)
    }
    pub fn is_pointer(&self) -> bool {
        matches!(self, Self::Pointer(_))
    }
    pub fn is_array(&self) -> bool {
        matches!(self, Self::Array { .. })
    }
    pub fn is_map(&self) -> bool {
        matches!(self, Self::Map { .. })
    }
    pub fn is_struct(&self) -> bool {
        matches!(self, Self::Struct { .. })
    }
    pub fn is_aos(&self) -> bool {
        matches!(self, Self::AOS { .. })
    }
    pub fn is_soa(&self) -> bool {
        matches!(self, Self::SOA { .. })
    }
    pub fn is_custom(&self) -> bool {
        matches!(self, Self::Custom(_))
    }
    pub fn is_type_alias(&self) -> bool {
        matches!(self, Self::TypeAlias { .. })
    }
    pub fn is_function(&self) -> bool {
        matches!(self, Self::Function { .. })
    }
    pub fn is_tensor(&self) -> bool {
        matches!(self, Self::Tensor { .. })
    }
    pub fn is_result(&self) -> bool {
        matches!(self, Self::Result(_))
    }
    pub fn is_optional(&self) -> bool {
        matches!(self, Self::Optional(_))
    }
    pub fn is_future(&self) -> bool {
        matches!(self, Self::Future(_))
    }

    pub fn is_numeric(&self) -> bool {
        self.is_integer() || self.is_unsigned() || self.is_float()
    }

    pub fn is_signed(&self) -> bool {
        matches!(self, Self::Integer(_))
    }

    pub fn bits(&self) -> Option<u32> {
        match self {
            Self::Integer(k) => Some(k.bits()),
            Self::Unsigned(k) => Some(k.bits()),
            Self::Float(k) => Some(k.bits()),
            Self::Bool => Some(1),
            _ => None,
        }
    }

    // ── Type resolution ──────────────────────────────────────────

    pub fn resolve_alias(&self) -> &Type {
        match self {
            Self::TypeAlias { aliased_type, .. } => aliased_type.resolve_alias(),
            other => other,
        }
    }

    pub fn resolve_alias_owned(self) -> Type {
        match self {
            Self::TypeAlias { aliased_type, .. } => aliased_type.resolve_alias_owned(),
            other => other,
        }
    }

    // ── Type compatibility ───────────────────────────────────────

    pub fn same_type(&self, other: &Type) -> bool {
        let a = self.resolve_alias();
        let b = other.resolve_alias();
        match (a, b) {
            (Self::Integer(ak), Self::Integer(bk)) => ak == bk,
            (Self::Unsigned(ak), Self::Unsigned(bk)) => ak == bk,
            (Self::Float(ak), Self::Float(bk)) => ak == bk,
            (Self::Bool, Self::Bool) => true,
            (Self::String, Self::String) => true,
            (Self::Void, Self::Void) => true,
            (Self::Pointer(a), Self::Pointer(b)) => a == b,
            (
                Self::Array {
                    element_type: a, ..
                },
                Self::Array {
                    element_type: b, ..
                },
            ) => a == b,
            (
                Self::Map {
                    key_type: ak,
                    value_type: av,
                },
                Self::Map {
                    key_type: bk,
                    value_type: bv,
                },
            ) => ak == bk && av == bv,
            (Self::Struct { name: a, .. }, Self::Struct { name: b, .. }) => a == b,
            (Self::Custom(a), Self::Custom(b)) => a == b,
            (Self::Result(a), Self::Result(b)) => a == b,
            (Self::Optional(a), Self::Optional(b)) => a == b,
            (Self::Future(a), Self::Future(b)) => a == b,
            (Self::Tensor { dtype: a, .. }, Self::Tensor { dtype: b, .. }) => a == b,
            (
                Self::Function {
                    param_types: ap,
                    return_type: ar,
                },
                Self::Function {
                    param_types: bp,
                    return_type: br,
                },
            ) => ap == bp && ar == br,
            _ => false,
        }
    }

    pub fn compatible_with(&self, target: &Type) -> bool {
        if self.same_type(target) {
            return true;
        }
        let from = self.resolve_alias();
        let to = target.resolve_alias();
        match (from, to) {
            // int <-> int of same signedness
            (Self::Integer(_), Self::Integer(_)) => true,
            (Self::Unsigned(_), Self::Unsigned(_)) => true,
            (Self::Float(_), Self::Float(_)) => true,
            // int -> float (explicit cast needed in Mog, but compatible for codegen)
            (Self::Integer(_), Self::Float(_)) => true,
            (Self::Unsigned(_), Self::Float(_)) => true,
            // ptr <-> string backward compat (both are pointers at IR level)
            (Self::Pointer(_), Self::String) => true,
            (Self::String, Self::Pointer(_)) => true,
            // [u8] <-> string (byte arrays are the runtime representation of strings)
            (
                Self::Array {
                    element_type: e, ..
                },
                Self::String,
            ) if matches!(e.as_ref(), Self::Unsigned(UnsignedKind::U8)) => true,
            (
                Self::String,
                Self::Array {
                    element_type: e, ..
                },
            ) if matches!(e.as_ref(), Self::Unsigned(UnsignedKind::U8)) => true,
            _ => false,
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Integer(k) => write!(f, "{k}"),
            Self::Unsigned(k) => write!(f, "{k}"),
            Self::Float(k) => write!(f, "{k}"),
            Self::Bool => write!(f, "bool"),
            Self::String => write!(f, "string"),
            Self::Void => write!(f, "void"),
            Self::Pointer(None) => write!(f, "ptr"),
            Self::Pointer(Some(t)) => write!(f, "ptr<{t}>"),
            Self::Array { element_type, .. } => write!(f, "[{element_type}]"),
            Self::Map {
                key_type,
                value_type,
            } => write!(f, "{{{key_type}: {value_type}}}"),
            Self::Struct { name, .. } => write!(f, "{name}"),
            Self::AOS { element_type, size } => match size {
                Some(n) => write!(f, "[{element_type}; {n}]"),
                None => write!(f, "[{element_type}]"),
            },
            Self::SOA {
                struct_type,
                capacity,
            } => match capacity {
                Some(n) => write!(f, "soa {struct_type}[{n}]"),
                None => write!(f, "soa {struct_type}"),
            },
            Self::Custom(name) => write!(f, "{name}"),
            Self::TypeAlias { name, .. } => write!(f, "{name}"),
            Self::Function {
                param_types,
                return_type,
            } => {
                write!(f, "fn(")?;
                for (i, p) in param_types.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{p}")?;
                }
                write!(f, ") -> {return_type}")
            }
            Self::Tensor { dtype, .. } => write!(f, "tensor<{dtype}>"),
            Self::Result(t) => write!(f, "Result<{t}>"),
            Self::Optional(t) => write!(f, "?{t}"),
            Self::Future(t) => write!(f, "Future<{t}>"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_convenience_constructors() {
        assert_eq!(Type::int(), Type::Integer(IntegerKind::I64));
        assert_eq!(Type::float(), Type::Float(FloatKind::F64));
        assert_eq!(Type::string(), Type::String);
        assert_eq!(Type::bool(), Type::Bool);
        assert_eq!(Type::void(), Type::Void);
    }

    #[test]
    fn test_integer_kind_bits() {
        assert_eq!(IntegerKind::I8.bits(), 8);
        assert_eq!(IntegerKind::I16.bits(), 16);
        assert_eq!(IntegerKind::I32.bits(), 32);
        assert_eq!(IntegerKind::I64.bits(), 64);
        assert_eq!(IntegerKind::I128.bits(), 128);
        assert_eq!(IntegerKind::I256.bits(), 256);
    }

    #[test]
    fn test_unsigned_kind_bits() {
        assert_eq!(UnsignedKind::U8.bits(), 8);
        assert_eq!(UnsignedKind::U16.bits(), 16);
        assert_eq!(UnsignedKind::U32.bits(), 32);
        assert_eq!(UnsignedKind::U64.bits(), 64);
        assert_eq!(UnsignedKind::U128.bits(), 128);
        assert_eq!(UnsignedKind::U256.bits(), 256);
    }

    #[test]
    fn test_float_kind_bits() {
        assert_eq!(FloatKind::F8.bits(), 8);
        assert_eq!(FloatKind::F16.bits(), 16);
        assert_eq!(FloatKind::F32.bits(), 32);
        assert_eq!(FloatKind::F64.bits(), 64);
        assert_eq!(FloatKind::F128.bits(), 128);
        assert_eq!(FloatKind::F256.bits(), 256);
        assert_eq!(FloatKind::Bf16.bits(), 16);
    }

    #[test]
    fn test_integer_kind_from_str() {
        assert_eq!(IntegerKind::from_str("i8"), Some(IntegerKind::I8));
        assert_eq!(IntegerKind::from_str("i16"), Some(IntegerKind::I16));
        assert_eq!(IntegerKind::from_str("i32"), Some(IntegerKind::I32));
        assert_eq!(IntegerKind::from_str("i64"), Some(IntegerKind::I64));
        assert_eq!(IntegerKind::from_str("i128"), Some(IntegerKind::I128));
        assert_eq!(IntegerKind::from_str("i256"), Some(IntegerKind::I256));
        assert_eq!(IntegerKind::from_str("i512"), None);
        assert_eq!(IntegerKind::from_str("u32"), None);

        assert_eq!(UnsignedKind::from_str("u8"), Some(UnsignedKind::U8));
        assert_eq!(UnsignedKind::from_str("u64"), Some(UnsignedKind::U64));
        assert_eq!(UnsignedKind::from_str("u256"), Some(UnsignedKind::U256));
        assert_eq!(UnsignedKind::from_str("i32"), None);

        assert_eq!(FloatKind::from_str("f32"), Some(FloatKind::F32));
        assert_eq!(FloatKind::from_str("f64"), Some(FloatKind::F64));
        assert_eq!(FloatKind::from_str("bf16"), Some(FloatKind::Bf16));
        assert_eq!(FloatKind::from_str("f512"), None);
    }

    #[test]
    fn test_type_predicates() {
        assert!(Type::int().is_integer());
        assert!(!Type::int().is_float());

        assert!(Type::float().is_float());
        assert!(!Type::float().is_integer());

        assert!(Type::string().is_string());
        assert!(Type::bool().is_bool());
        assert!(Type::void().is_void());

        assert!(Type::Unsigned(UnsignedKind::U32).is_unsigned());
        assert!(Type::pointer(None).is_pointer());
        assert!(Type::array(Type::int()).is_array());
        assert!(Type::map(Type::string(), Type::int()).is_map());

        let fields = BTreeMap::new();
        assert!(Type::struct_type("Foo".to_string(), fields).is_struct());

        assert!(Type::result(Type::int()).is_result());
        assert!(Type::optional(Type::int()).is_optional());
        assert!(Type::future(Type::int()).is_future());
        assert!(Type::custom("Bar".to_string()).is_custom());
        assert!(Type::function(vec![Type::int()], Type::bool()).is_function());
        assert!(Type::tensor(Type::float(), None).is_tensor());
    }

    #[test]
    fn test_is_numeric() {
        assert!(Type::int().is_numeric());
        assert!(Type::Unsigned(UnsignedKind::U8).is_numeric());
        assert!(Type::float().is_numeric());

        assert!(!Type::string().is_numeric());
        assert!(!Type::bool().is_numeric());
        assert!(!Type::void().is_numeric());

        // is_signed: only Integer variants
        assert!(Type::int().is_signed());
        assert!(!Type::Unsigned(UnsignedKind::U32).is_signed());
        assert!(!Type::float().is_signed());
    }

    #[test]
    fn test_same_type() {
        // Identical types
        assert!(Type::int().same_type(&Type::int()));
        assert!(Type::float().same_type(&Type::float()));
        assert!(Type::string().same_type(&Type::string()));
        assert!(Type::bool().same_type(&Type::bool()));
        assert!(Type::void().same_type(&Type::void()));

        // Different types
        assert!(!Type::int().same_type(&Type::float()));
        assert!(!Type::string().same_type(&Type::bool()));

        // Different bit widths within same category
        assert!(!Type::Integer(IntegerKind::I32).same_type(&Type::Integer(IntegerKind::I64)));

        // Arrays: same_type ignores dimensions, only checks element_type
        let a1 = Type::array(Type::int());
        let a2 = Type::array(Type::int());
        let a3 = Type::array(Type::float());
        assert!(a1.same_type(&a2));
        assert!(!a1.same_type(&a3));

        // Structs: same_type only checks name
        let s1 = Type::struct_type("Point".to_string(), BTreeMap::new());
        let s2 = Type::struct_type("Point".to_string(), BTreeMap::new());
        let s3 = Type::struct_type("Color".to_string(), BTreeMap::new());
        assert!(s1.same_type(&s2));
        assert!(!s1.same_type(&s3));
    }

    #[test]
    fn test_compatible_with() {
        // Same type is always compatible
        assert!(Type::int().compatible_with(&Type::int()));

        // int <-> int (same signedness, different widths)
        assert!(Type::Integer(IntegerKind::I32).compatible_with(&Type::Integer(IntegerKind::I64)));

        // unsigned <-> unsigned
        assert!(
            Type::Unsigned(UnsignedKind::U8).compatible_with(&Type::Unsigned(UnsignedKind::U64))
        );

        // float <-> float
        assert!(Type::Float(FloatKind::F32).compatible_with(&Type::Float(FloatKind::F64)));

        // int -> float (compatible)
        assert!(Type::int().compatible_with(&Type::float()));

        // unsigned -> float (compatible)
        assert!(Type::Unsigned(UnsignedKind::U32).compatible_with(&Type::float()));

        // float -> int (NOT compatible)
        assert!(!Type::float().compatible_with(&Type::int()));

        // int -> unsigned (NOT compatible)
        assert!(!Type::int().compatible_with(&Type::Unsigned(UnsignedKind::U32)));

        // string -> int (NOT compatible)
        assert!(!Type::string().compatible_with(&Type::int()));
    }

    #[test]
    fn test_display_formatting() {
        assert_eq!(format!("{}", Type::int()), "i64");
        assert_eq!(format!("{}", Type::Integer(IntegerKind::I32)), "i32");
        assert_eq!(format!("{}", Type::Unsigned(UnsignedKind::U16)), "u16");
        assert_eq!(format!("{}", Type::float()), "f64");
        assert_eq!(format!("{}", Type::Float(FloatKind::Bf16)), "bf16");
        assert_eq!(format!("{}", Type::bool()), "bool");
        assert_eq!(format!("{}", Type::string()), "string");
        assert_eq!(format!("{}", Type::void()), "void");
        assert_eq!(format!("{}", Type::pointer(None)), "ptr");
        assert_eq!(format!("{}", Type::pointer(Some(Type::int()))), "ptr<i64>");
        assert_eq!(format!("{}", Type::result(Type::int())), "Result<i64>");
        assert_eq!(format!("{}", Type::optional(Type::int())), "?i64");
        assert_eq!(format!("{}", Type::future(Type::int())), "Future<i64>");
        assert_eq!(
            format!("{}", Type::tensor(Type::float(), None)),
            "tensor<f64>"
        );
    }

    #[test]
    fn test_resolve_alias() {
        let alias = Type::type_alias("MyInt".to_string(), Type::int());
        assert_eq!(*alias.resolve_alias(), Type::int());

        // Nested alias
        let inner_alias = Type::type_alias("InnerInt".to_string(), Type::Integer(IntegerKind::I32));
        let outer_alias = Type::type_alias("OuterInt".to_string(), inner_alias);
        assert_eq!(
            *outer_alias.resolve_alias(),
            Type::Integer(IntegerKind::I32)
        );

        // resolve_alias_owned consumes and returns inner type
        let alias2 = Type::type_alias("MyFloat".to_string(), Type::float());
        assert_eq!(alias2.resolve_alias_owned(), Type::float());

        // same_type sees through aliases
        let alias3 = Type::type_alias("Alias64".to_string(), Type::int());
        assert!(alias3.same_type(&Type::int()));
    }

    #[test]
    fn test_complex_types() {
        // Array
        assert_eq!(format!("{}", Type::array(Type::int())), "[i64]");

        // Map
        assert_eq!(
            format!("{}", Type::map(Type::string(), Type::int())),
            "{string: i64}"
        );

        // Struct
        let mut fields = BTreeMap::new();
        fields.insert("x".to_string(), Type::float());
        fields.insert("y".to_string(), Type::float());
        assert_eq!(
            format!("{}", Type::struct_type("Point".to_string(), fields)),
            "Point"
        );

        // Function
        assert_eq!(
            format!(
                "{}",
                Type::function(vec![Type::int(), Type::float()], Type::bool())
            ),
            "fn(i64, f64) -> bool"
        );

        // Nested: array of maps
        let nested = Type::array(Type::map(Type::string(), Type::int()));
        assert_eq!(format!("{}", nested), "[{string: i64}]");

        // Custom
        assert_eq!(format!("{}", Type::custom("MyType".to_string())), "MyType");
    }
}
