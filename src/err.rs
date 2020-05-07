use eyre::{EyreContext, WrapErr as EyreWrapErr};
use std::fmt::{self, Debug, Display};

pub use std::{error::Error as StdError, result::Result as StdResult};

pub use color_eyre::{Help, Report};
pub use eyre::{bail, ensure};

pub type Result<T> = StdResult<T, Report>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoneError {}

impl StdError for NoneError {}

impl fmt::Display for NoneError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Option was None")
    }
}

pub trait Context<T, E, C>
where
    C: EyreContext,
{
    /// Wrap the error value with a new adhoc error
    fn context<D>(self, msg: D) -> StdResult<T, eyre::Report<C>>
    where
        D: Display + Debug + Send + Sync + 'static;

    /// Wrap the error value with a new adhoc error that is evaluated lazily
    /// only once an error does occur.
    fn with_context<D, F>(self, f: F) -> StdResult<T, eyre::Report<C>>
    where
        D: Display + Debug + Send + Sync + 'static,
        F: FnOnce() -> D;
}

impl<T, C> Context<T, NoneError, C> for Option<T>
where
    C: EyreContext,
{
    fn context<D>(self, msg: D) -> StdResult<T, eyre::Report<C>>
    where
        D: Display + Debug + Send + Sync + 'static,
    {
        self.ok_or_else(|| eyre::Report::msg(msg))
    }

    fn with_context<D, F>(self, msg: F) -> StdResult<T, eyre::Report<C>>
    where
        D: Display + Debug + Send + Sync + 'static,
        F: FnOnce() -> D,
    {
        self.ok_or_else(|| eyre::Report::msg(msg()))
    }
}

impl<T, E, C> Context<T, E, C> for StdResult<T, E>
where
    C: EyreContext,
    StdResult<T, E>: EyreWrapErr<T, E, C>,
    E: Send + Sync + 'static,
{
    fn context<D>(self, msg: D) -> StdResult<T, eyre::Report<C>>
    where
        D: Display + Debug + Send + Sync + 'static,
    {
        self.wrap_err(msg)
    }

    fn with_context<D, F>(self, msg: F) -> StdResult<T, eyre::Report<C>>
    where
        D: Display + Debug + Send + Sync + 'static,
        F: FnOnce() -> D,
    {
        self.wrap_err_with(msg)
    }
}
