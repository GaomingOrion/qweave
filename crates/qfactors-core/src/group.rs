#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupInfo {
    pub id: u32,
    pub label_key: String,
    pub start: usize,
    pub end: usize,
}
