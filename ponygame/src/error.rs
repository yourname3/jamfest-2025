pub struct PonyError {
    // TODO????

    // For now, we'll just use a message string. We might swap this out
    // for more structured things later.
    message: String,
}

impl From<image::ImageError> for PonyError {
    fn from(value: image::ImageError) -> Self {
        Self { message: format!("{value}") }
    }
}

pub type PonyResult<T> = Result<T, PonyError>;

impl std::fmt::Debug for PonyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}