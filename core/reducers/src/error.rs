use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Msg(String),
    #[error("Type error: {0}")]
    Type(String),
    #[error("Unbound wildcard: {0}")]
    Unbound(String),
}

impl From<pod2::middleware::Error> for Error {
    fn from(e: pod2::middleware::Error) -> Self {
        Error::Msg(e.to_string())
    }
}
