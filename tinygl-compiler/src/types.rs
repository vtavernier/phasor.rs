use std::fmt;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum AtomType {
    Int,
    Float,
    UInt,
    Bool,
}

impl AtomType {
    fn vec_name(&self) -> &'static str {
        match self {
            Self::Int => "ivec",
            Self::Float => "vec",
            Self::UInt => "uvec",
            Self::Bool => "bvec",
        }
    }

    fn cgmath_name(&self, coerce_i32: bool) -> &'static str {
        match self {
            Self::Int => "i32",
            Self::Float => "f32",
            Self::UInt => {
                if coerce_i32 {
                    "i32"
                } else {
                    "u32"
                }
            }
            Self::Bool => "bool",
        }
    }
}

impl fmt::Display for AtomType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Int => write!(f, "int"),
            Self::Float => write!(f, "float"),
            Self::UInt => write!(f, "uint"),
            Self::Bool => write!(f, "bool"),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum VectorType {
    Scalar(AtomType),
    Vector(AtomType, u32),
}

impl VectorType {
    pub fn cgmath_name(&self) -> String {
        // TODO: Use a formatter
        match self {
            Self::Scalar(atom_type) => atom_type.cgmath_name(false).to_owned(),
            Self::Vector(vector_type, components) => format!(
                "::cgmath::Vector{}<{}>",
                components,
                vector_type.cgmath_name(false)
            ),
        }
    }

    pub fn rstype(&self) -> &'static str {
        match self {
            Self::Scalar(atom_type) | Self::Vector(atom_type, _) => atom_type.cgmath_name(false),
        }
    }

    pub fn api_rstype(&self) -> &'static str {
        match self {
            Self::Scalar(atom_type) | Self::Vector(atom_type, _) => atom_type.cgmath_name(true),
        }
    }

    pub fn components(&self) -> u32 {
        match self {
            Self::Scalar(_) => 1,
            Self::Vector(_, components) => *components,
        }
    }
}

impl fmt::Display for VectorType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Scalar(atom_type) => fmt::Display::fmt(atom_type, f),
            Self::Vector(atom_type, components) => {
                write!(f, "{}{}", atom_type.vec_name(), components)
            }
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum GenericType {
    Atom(AtomType),
    Vector(VectorType),
    Array(VectorType, u32),
}

impl fmt::Display for GenericType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Atom(atom_type) => fmt::Display::fmt(atom_type, f),
            Self::Vector(vector_type) => fmt::Display::fmt(vector_type, f),
            Self::Array(vector_type, components) => write!(f, "{}[{}]", vector_type, components),
        }
    }
}

impl GenericType {
    pub fn array(inner_type: Self, components: u32) -> Self {
        match inner_type {
            Self::Atom(atom_type) => Self::Array(VectorType::Scalar(atom_type), components),
            Self::Vector(vector_type) => Self::Array(vector_type, components),
            _ => panic!(
                "unsupported type combination: {:?}[{}]",
                inner_type, components
            ),
        }
    }

    pub fn vector(inner_type: Self, components: u32) -> Self {
        match inner_type {
            Self::Atom(atom_type) => Self::Vector(VectorType::Vector(atom_type, components)),
            _ => panic!("unsupported type combination"),
        }
    }

    #[allow(dead_code)]
    pub fn named<'a>(&'a self, name: &'a str) -> NamedGenericType<'a> {
        NamedGenericType { name, gt: self }
    }

    pub fn cgmath_name(&self) -> String {
        // TODO: Use a formatter
        match self {
            Self::Atom(atom_type) => atom_type.cgmath_name(false).to_owned(),
            Self::Vector(vector_type) => vector_type.cgmath_name(),
            Self::Array(inner_type, _size) => format!("&[{}]", inner_type.cgmath_name()),
        }
    }

    pub fn rstype(&self) -> &'static str {
        match self {
            Self::Atom(atom_type) => atom_type.cgmath_name(false),
            Self::Vector(vector_type) | Self::Array(vector_type, _) => vector_type.rstype(),
        }
    }

    pub fn api_rstype(&self) -> &'static str {
        match self {
            Self::Atom(atom_type) => atom_type.cgmath_name(true),
            Self::Vector(vector_type) | Self::Array(vector_type, _) => vector_type.api_rstype(),
        }
    }

    pub fn components(&self) -> u32 {
        match self {
            Self::Atom(_) => 1,
            Self::Vector(vector_type) | Self::Array(vector_type, _) => vector_type.components(),
        }
    }

    pub fn glow_value(&self, name: &str) -> String {
        match self {
            Self::Atom(AtomType::UInt) => format!("std::mem::transmute(&[{}][..])", name),
            Self::Atom(_) => format!("&[{}]", name),
            Self::Vector(inner_type) => format!(
                "std::mem::transmute(&::std::convert::AsRef::<[{base_ty}; {components}]>::as_ref(&{})[..])",
                name,
                components = self.components(),
                base_ty = inner_type.rstype()
            ),
            Self::Array(inner_type, size) => format!(
                "std::mem::transmute(::std::slice::from_raw_parts({name}.as_ptr() as *const {base_ty}, {size}))",
                name = name,
                size = *size * self.components(),
                base_ty = inner_type.rstype()
            ),
        }
    }
}

pub struct NamedGenericType<'a> {
    name: &'a str,
    gt: &'a GenericType,
}

impl<'a> fmt::Display for NamedGenericType<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.gt {
            GenericType::Atom(atom_type) => write!(f, "{} {}", atom_type, self.name),
            GenericType::Vector(vector_type) => write!(f, "{} {}", vector_type, self.name),
            GenericType::Array(vector_type, components) => {
                write!(f, "{} {}[{}]", vector_type, self.name, components)
            }
        }
    }
}
