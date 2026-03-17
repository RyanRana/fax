use thiserror::Error;

#[derive(Error, Debug)]
pub enum FaxError {
    #[error("invalid resource type: {0}")]
    InvalidResourceType(String),

    #[error("hash verification failed: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },

    #[error("credential chain broken at index {index}: {reason}")]
    BrokenChain { index: usize, reason: String },

    #[error("signature verification failed: {0}")]
    SignatureError(String),

    #[error("trade expired at {expiry}, current time {now}")]
    TradeExpired { expiry: u64, now: u64 },

    #[error("invalid trade state: expected {expected}, got {actual}")]
    InvalidState { expected: String, actual: String },

    #[error("secret does not match hash-lock")]
    HashLockMismatch,

    #[error("insufficient resource: need {need} {unit}, have {have}")]
    InsufficientResource { need: f64, have: f64, unit: String },

    #[error("RCU conversion failed: {0}")]
    RcuConversionError(String),

    #[error("chain interaction error: {0}")]
    ChainError(String),

    #[error("serialization error: {0}")]
    SerializationError(String),

    #[error("identity error: {0}")]
    IdentityError(String),

    #[error("{0}")]
    Other(String),
}

pub type FaxResult<T> = Result<T, FaxError>;
