use serde_derive::{Deserialize, Serialize};

use super::parse::Parse;

#[derive(Debug, Serialize, Deserialize)]
pub enum Param {
    Bool(bool),
    Float(f64),
    String(String),
}

impl std::convert::TryFrom<&str> for Param {
    type Error = failure::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(if let Ok(value) = <f64 as Parse<f64>>::parse(value) {
            Param::Float(value)
        } else if let Ok(value) = <bool as Parse<bool>>::parse(value) {
            Param::Bool(value)
        } else {
            Param::String(value.to_owned())
        })
    }
}

impl Param {
    pub fn as_bool(&self) -> bool {
        match self {
            Self::Bool(value) => *value,
            _ => panic!(),
        }
    }

    pub fn as_float(&self) -> f64 {
        match self {
            Self::Float(value) => *value,
            _ => panic!(),
        }
    }

    pub fn write_hdf5(&self, path: &str, file: &hdf5::File) -> Result<(), hdf5::Error> {
        match self {
            Self::Bool(value) => {
                file.new_dataset::<u8>()
                    .create(&path, (1,))?
                    .write(&[if *value { 1 } else { 0 }])?;
            }
            Self::Float(value) => {
                file.new_dataset::<f64>()
                    .create(&path, (1,))?
                    .write(&[*value])?;
            }
            Self::String(value) => {
                let bytes = value.as_bytes();

                file.new_dataset::<u8>()
                    .create(&path, (bytes.len(),))?
                    .write(bytes)?;
            }
        }

        Ok(())
    }
}

impl Into<bool> for Param {
    fn into(self) -> bool {
        self.as_bool()
    }
}

impl Into<f64> for Param {
    fn into(self) -> f64 {
        self.as_float()
    }
}

impl Into<String> for Param {
    fn into(self) -> String {
        match self {
            Self::String(value) => value,
            _ => panic!(),
        }
    }
}
