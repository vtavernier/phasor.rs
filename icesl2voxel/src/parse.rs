pub trait Parse<T> {
    fn parse(value: &str) -> Result<T, failure::Error>;
}

impl Parse<f64> for f64 {
    fn parse(value: &str) -> Result<f64, failure::Error> {
        value.parse::<f64>().map_err(|e| e.into())
    }
}

impl Parse<bool> for bool {
    fn parse(value: &str) -> Result<bool, failure::Error> {
        match value {
            "True" => Ok(true),
            "False" => Ok(false),
            _ => Err(failure::err_msg("invalid bool")),
        }
    }
}

impl Parse<String> for String {
    fn parse(value: &str) -> Result<String, failure::Error> {
        Ok(value.to_owned())
    }
}
