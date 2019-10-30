// Based on https://github.com/dtolnay/anyhow/blob/1.0.18/src/context.rs

use std::fmt;

pub(crate) type Error = Box<dyn std::error::Error + Send + Sync + 'static>;
pub(crate) type Result<T> = std::result::Result<T, Error>;

macro_rules! format_err {
    ($($tt:tt)*) => {
        Box::new(crate::error::DisplayError(format!($($tt)*)));
    };
}

pub(crate) struct DisplayError<D>(pub(crate) D);

impl<D> fmt::Debug for DisplayError<D>
where
    D: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl<D> fmt::Display for DisplayError<D>
where
    D: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl<D> std::error::Error for DisplayError<D> where D: fmt::Display {}

pub(crate) struct ContextError<C> {
    context: C,
    error: Error,
}

impl<C> fmt::Debug for ContextError<C>
where
    C: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\nCaused by: {:?}", self.context, self.error)
    }
}

impl<C> fmt::Display for ContextError<C>
where
    C: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\nCaused by: {}", self.context, self.error)
    }
}

impl<C> std::error::Error for ContextError<C>
where
    C: fmt::Display,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&*self.error)
    }
}

/// Provides the `context` method for `Result`.
pub(crate) trait Context<T, E> {
    /// Wrap the error value with additional context.
    fn context<C>(self, context: C) -> Result<T>
    where
        C: fmt::Display + Send + Sync + 'static;

    /// Wrap the error value with additional context that is evaluated lazily
    /// only once an error does occur.
    fn with_context<C>(self, f: impl FnOnce() -> C) -> Result<T>
    where
        C: fmt::Display + Send + Sync + 'static;
}

impl<T, E> Context<T, E> for std::result::Result<T, E>
where
    E: Into<Error>,
{
    fn context<C>(self, context: C) -> Result<T>
    where
        C: fmt::Display + Send + Sync + 'static,
    {
        self.map_err(|error| Error::from(ContextError { context, error: error.into() }))
    }

    fn with_context<C>(self, context: impl FnOnce() -> C) -> Result<T>
    where
        C: fmt::Display + Send + Sync + 'static,
    {
        self.context(context())
    }
}

impl<T> Context<T, std::convert::Infallible> for Option<T> {
    fn context<C>(self, context: C) -> Result<T>
    where
        C: fmt::Display + Send + Sync + 'static,
    {
        self.ok_or_else(|| Error::from(DisplayError(context)))
    }

    fn with_context<C>(self, context: impl FnOnce() -> C) -> Result<T>
    where
        C: fmt::Display + Send + Sync + 'static,
    {
        self.ok_or_else(|| Error::from(DisplayError(context())))
    }
}
