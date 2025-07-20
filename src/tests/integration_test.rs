// tests/integration_test.rs
#[tokio::test]
async fn test_scrape_markdown() {
    use snm_brightdata_client::tools::scrape::ScrapeMarkdown;
    use serde_json::json;

    let tool = ScrapeMarkdown;
    let result = tool
        .execute(json!({"url": "https://example.com"}))
        .await
        .expect("Tool should succeed");

    assert!(result.is_string() || result.is_object());
}

#[tokio::test]
async fn test_search_engine() {
    use snm_brightdata_client::tools::search::SearchEngine;
    use serde_json::json;

    let tool = SearchEngine;
    let result = tool
        .execute(json!({"query": "Rust programming"}))
        .await
        .expect("Tool should succeed");

    assert!(result.is_string() || result.is_object());
}

#[tokio::test]
async fn test_extract() {
    use snm_brightdata_client::tools::extract::Extractor;
    use serde_json::json;

    let tool = Extractor;
    let result = tool
        .execute(json!({"url": "https://example.com"}))
        .await
        .expect("Tool should succeed");

    assert!(result.is_string() || result.is_object());
}