use std::path::Path;
use std::sync::OnceLock;

use crate::platform;

/// One shared client for all wind fetches — mirrors fetch.rs. `reqwest::get`
/// builds a fresh Client per call (no keep-alive); reusing one pools connections.
fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

/// Fetch raw Open-Meteo JSON for a grid request, cache-as-you-go on disk.
///
/// A local cache is consulted first (keyed by `cache_key`, a no-op on the web
/// where the browser caches). On a miss the URL is fetched and written back.
pub async fn fetch_wind_json(cache_location: &Path, cache_key: &str, url: &str) -> Option<Vec<u8>> {
    let path = cache_location.join(format!("wind/{cache_key}.json"));
    let path = path.to_string_lossy();

    if let Some(cached) = platform::read_bytes(&path) {
        return Some(cached);
    }

    let data = match client().get(url).send().await {
        Ok(r) if r.status().is_success() => r.bytes().await.ok()?.to_vec(),
        Ok(r) => {
            log::warn!("wind fetch failed: {}", r.status());
            return None;
        }
        Err(e) => {
            log::warn!("wind fetch errored: {e}");
            return None;
        }
    };
    platform::write_bytes(&path, &data);
    Some(data)
}
