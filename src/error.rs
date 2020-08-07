use std::io;

pub type Result<T> = Result<T, RenetError>;

#[derive(Debug)]
pub enum RenetError {
    MaximumFragmentsExceeded,
    CouldNotFindFragment,
    InvalidNumberFragment,
    FragmentAlreadyProcessed,
    InvalidHeaderType,
    IOError(io::Error),
}

impl From<io::Error> for RenetError {
    fn from(inner: io::Error) -> RenetError {
        RenetError::IOError(inner)
    }
}

