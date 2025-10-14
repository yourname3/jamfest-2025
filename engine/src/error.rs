pub struct EngineError {
    // TODO????

    // For now, we'll just use a message string. We might swap this out
    // for more structured things later.
    message: String,
}

impl From<image::ImageError> for EngineError {
    fn from(value: image::ImageError) -> Self {
        Self { message: format!("{value}") }
    }
}

pub type EngineResult<T> = Result<T, EngineError>;

impl std::fmt::Debug for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}