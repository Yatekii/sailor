/// Plain GET returning the body bytes, or None on any non-success/error.
pub async fn get(url: &str) -> Option<Vec<u8>> {
    match reqwest::get(url).await {
        Ok(r) if r.status().is_success() => r.bytes().await.ok().map(|b| b.to_vec()),
        Ok(r) => {
            log::warn!("http {} for {url}", r.status());
            None
        }
        Err(e) => {
            log::warn!("http error for {url}: {e}");
            None
        }
    }
}

/// Range GET of `length` bytes starting at `offset` (HTTP 206). None on failure.
pub async fn get_range(url: &str, offset: u64, length: u64) -> Option<Vec<u8>> {
    let end = offset + length - 1;
    let client = reqwest::Client::new();
    match client
        .get(url)
        .header("Range", format!("bytes={offset}-{end}"))
        .send()
        .await
    {
        Ok(r) if r.status() == reqwest::StatusCode::PARTIAL_CONTENT => {
            r.bytes().await.ok().map(|b| b.to_vec())
        }
        Ok(r) => {
            log::warn!("range http {} for {url}", r.status());
            None
        }
        Err(e) => {
            log::warn!("range http error for {url}: {e}");
            None
        }
    }
}

/// POST a JSON `body` and return the response bytes, or None on any
/// non-success/error. Used for STAC search, which is POST-only.
pub async fn post_json(url: &str, body: &str) -> Option<Vec<u8>> {
    let client = reqwest::Client::new();
    match client
        .post(url)
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r.bytes().await.ok().map(|b| b.to_vec()),
        Ok(r) => {
            log::warn!("post {} for {url}", r.status());
            None
        }
        Err(e) => {
            log::warn!("post error for {url}: {e}");
            None
        }
    }
}
