#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Simple(String),
    Bulk(String),
    Array(Vec<Frame>),
    Error(String),
    Null,
}
