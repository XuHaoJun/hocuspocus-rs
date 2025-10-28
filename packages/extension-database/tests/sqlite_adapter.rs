use hocuspocus_extension_database::{DatabaseExtension, SqliteDatabase};
use hocuspocus_extension_database::types::{FetchContext, StoreContext};

#[tokio::test]
async fn round_trip_state() {
    let db = SqliteDatabase::connect("sqlite::memory:").await.unwrap();

    // initially missing
    let fetched = db.fetch(FetchContext { document_name: "a".into() }).await.unwrap();
    assert!(fetched.is_none());

    // store
    let state = vec![9u8, 8, 7, 6];
    db.store(StoreContext {
        document_name: "a".into(),
        state: &state,
        updated_at_millis: 123,
    }).await.unwrap();

    // fetch again
    let fetched = db.fetch(FetchContext { document_name: "a".into() }).await.unwrap();
    assert_eq!(fetched.as_deref(), Some(&state[..]));
}
