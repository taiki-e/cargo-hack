use std::fmt;

pub(crate) type Error = Box<dyn std::error::Error>;
pub(crate) type Result<T> = std::result::Result<T, Error>;

macro_rules! bail {
    ($($tt:tt)*) => {
        return Err(Box::new(crate::error::StringError(format!($($tt)*))));
    };
}

#[derive(Debug)]
pub(crate) struct StringError(pub(crate) String);

impl fmt::Display for StringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for StringError {}
