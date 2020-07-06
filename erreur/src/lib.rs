use std::fmt::{self, Debug, Display};
pub use std::{error::Error as StdError, result::Result as StdResult};

use color_eyre::eyre::WrapErr as EyreWrapErr;
pub use color_eyre::{
    eyre::{bail, ensure, Result},
    install as install_panic_handler, Help, Report,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoneError {}

impl StdError for NoneError {}

impl fmt::Display for NoneError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Option was None")
    }
}

pub trait Context<T, E> {
    /// Wrap the error value with a new adhoc error
    fn context<D>(self, msg: D) -> StdResult<T, Report>
    where
        D: Display + Send + Sync + 'static;

    /// Wrap the error value with a new adhoc error that is evaluated lazily
    /// only once an error does occur.
    fn with_context<D, F>(self, f: F) -> StdResult<T, Report>
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D;
}

impl<T> Context<T, NoneError> for Option<T> {
    fn context<D>(self, msg: D) -> StdResult<T, Report>
    where
        D: Display + Send + Sync + 'static,
    {
        self.ok_or_else(|| Report::new(NoneError {}).wrap_err(msg))
    }

    fn with_context<D, F>(self, msg: F) -> StdResult<T, Report>
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D,
    {
        self.ok_or_else(|| Report::new(NoneError {}).wrap_err(msg()))
    }
}

impl<T, E> Context<T, E> for StdResult<T, E>
where
    StdResult<T, E>: EyreWrapErr<T, E>,
    E: Send + Sync + 'static,
{
    fn context<D>(self, msg: D) -> StdResult<T, Report>
    where
        D: Display + Send + Sync + 'static,
    {
        self.wrap_err(msg)
    }

    fn with_context<D, F>(self, msg: F) -> StdResult<T, Report>
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D,
    {
        self.wrap_err_with(msg)
    }
}
