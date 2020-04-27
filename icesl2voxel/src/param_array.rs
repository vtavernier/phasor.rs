use std::borrow::Cow;
use std::convert::TryFrom;

use serde_derive::{Deserialize, Serialize};

use super::param::Param;
use super::parse::Parse;

#[derive(Debug, Serialize, Deserialize)]
enum ParamArrayStorage {
    Bool(Vec<bool>),
    Float(Vec<f64>),
    String(Vec<String>),
}

impl ParamArrayStorage {
    /// Returns Ok(is_empty)
    fn parse_and_add<T: Default + Parse<T> + Clone>(
        tgt: &mut Vec<T>,
        idx: usize,
        value: &str,
    ) -> Result<bool, failure::Error> {
        let is_empty = value.is_empty();
        let parsed_val = if is_empty {
            T::default()
        } else {
            T::parse(value).map_err(|_| {
                failure::err_msg(format!("failed to parse attribute value: `{}`", value))
            })?
        };

        Self::add_value(tgt, idx, parsed_val)?;

        Ok(is_empty)
    }

    /// Returns Ok(is_empty)
    fn add_value<T: Default + Parse<T> + Clone>(
        tgt: &mut Vec<T>,
        idx: usize,
        value: T,
    ) -> Result<(), failure::Error> {
        if tgt.len() <= idx {
            tgt.resize(idx + 1, T::default());
        }

        tgt[idx] = value;

        Ok(())
    }

    /// Returns Ok(is_empty)
    fn try_add(&mut self, idx: usize, value: &str) -> Result<bool, failure::Error> {
        match self {
            Self::Bool(tgt) => Self::parse_and_add(tgt, idx, value),
            Self::Float(tgt) => Self::parse_and_add(tgt, idx, value),
            Self::String(tgt) => Self::parse_and_add(tgt, idx, value),
        }
    }

    fn add(&mut self, idx: usize, value: Param) {
        match self {
            Self::Bool(tgt) => Self::add_value(tgt, idx, value.into()),
            Self::Float(tgt) => Self::add_value(tgt, idx, value.into()),
            Self::String(tgt) => Self::add_value(tgt, idx, value.into()),
        }
        .unwrap();
    }

    fn len(&self) -> usize {
        match self {
            Self::Bool(tgt) => tgt.len(),
            Self::Float(tgt) => tgt.len(),
            Self::String(tgt) => tgt.len(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ParamArray {
    values: ParamArrayStorage,
}

impl ParamArray {
    pub fn from_val(idx: usize, val: Param) -> Self {
        let mut this = match val {
            Param::Bool(_) => Self {
                values: ParamArrayStorage::Bool(Vec::with_capacity(256)),
            },
            Param::Float(_) => Self {
                values: ParamArrayStorage::Float(Vec::with_capacity(256)),
            },
            Param::String(_) => Self {
                values: ParamArrayStorage::String(Vec::with_capacity(256)),
            },
        };

        this.add_param(idx, val);
        this
    }

    pub fn new(idx: usize, value: &str) -> Result<Self, failure::Error> {
        Ok(if value.is_empty() {
            Self::from_val(idx, Param::String(String::new()))
        } else {
            Self::from_val(idx, Param::try_from(value)?)
        })
    }

    pub fn add(&mut self, idx: usize, value: &str) -> Result<(), failure::Error> {
        self.values.try_add(idx, value).map(|_| ())
    }

    pub fn add_param(&mut self, idx: usize, param: Param) {
        self.values.add(idx, param)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn as_f64_slice(&self) -> Option<Cow<[f64]>> {
        match &self.values {
            ParamArrayStorage::Float(vec) => Some(Cow::Borrowed(&vec[..])),
            ParamArrayStorage::Bool(vec) => {
                let other: Vec<_> = vec.iter().map(|x| if *x { 1. } else { 0. }).collect();
                Some(Cow::Owned(other))
            }
            _ => None,
        }
    }

    pub fn write_hdf5(&self, path: &str, file: &hdf5::File) -> Result<(), failure::Error> {
        match &self.values {
            ParamArrayStorage::Bool(value) => {
                file.new_dataset::<u8>()
                    .gzip(6)
                    .create(&path, (value.len(),))?
                    .write(
                        &value
                            .iter()
                            .map(|b| if *b { 1 } else { 0 })
                            .collect::<Vec<_>>(),
                    )?;
            }
            ParamArrayStorage::Float(value) => {
                file.new_dataset::<f64>()
                    .gzip(6)
                    .create(&path, (value.len(),))?
                    .write(&value[..])?;
            }
            _ => {
                return Err(failure::err_msg("unsupported array type"));
            }
        }

        Ok(())
    }

    pub fn xdmf_type(&self) -> Option<(&'static str, usize)> {
        match &self.values {
            ParamArrayStorage::Bool(_) => Some(("UInt", 1)),
            ParamArrayStorage::Float(_) => Some(("Float", 8)),
            _ => None,
        }
    }
}
