use std::fmt;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Http { status: u16, body: String },
    Json(String),
    Crypto(String),
    Acme { status: u16, detail: String, error_type: String },
    Config(String),
    Provider(String),
    Dns(String),
    Cli(String),
    Utf8,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O: {e}"),
            Error::Http { status, body } => write!(f, "HTTP {status}: {body}"),
            Error::Json(e) => write!(f, "JSON: {e}"),
            Error::Crypto(e) => write!(f, "crypto: {e}"),
            Error::Acme { status, detail, error_type } => {
                write!(f, "ACME [{error_type}] ({status}): {detail}")
            }
            Error::Config(e) => write!(f, "config: {e}"),
            Error::Provider(e) => write!(f, "provider: {e}"),
            Error::Dns(e) => write!(f, "DNS: {e}"),
            Error::Cli(e) => write!(f, "{e}"),
            Error::Utf8 => write!(f, "invalid UTF-8"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self { Error::Io(e) }
}

impl From<std::str::Utf8Error> for Error {
    fn from(_: std::str::Utf8Error) -> Self { Error::Utf8 }
}
