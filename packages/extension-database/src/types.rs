#[derive(Debug, Clone)]
pub struct FetchContext {
    pub document_name: String,
}

#[derive(Debug)]
pub struct StoreContext<'a> {
    pub document_name: String,
    pub state: &'a [u8],
    pub updated_at_millis: i64,
}
