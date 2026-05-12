#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Simple(String),
    Integer(i64),
    Bulk(String),
    Array(Vec<Frame>),
    Error(String),
    Null,
}
