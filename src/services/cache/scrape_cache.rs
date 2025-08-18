// crates/snm-brightdata-client/src/services/scrape_cache.rs
use redis::{AsyncCommands, Client, RedisResult};
use serde_json::{json, Value};
use std::env;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use log::{info, warn, error, debug};
use crate::error::BrightDataError;
use url::Url;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone)]
pub struct ScrapeCache {
    client: Client,
    ttl: u64,
}

impl ScrapeCache {
    /// Initialize Redis scraping cache service
    pub async fn new() -> Result<Self, BrightDataError> {
        let redis_url = env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379/".to_string());
        
        let client = Client::open(redis_url.as_str())
            .map_err(|e| BrightDataError::ToolError(format!("Failed to create Redis client: {}", e)))?;
        
        // Test connection with the correct async method
        let mut conn = client.get_multiplexed_async_connection().await
            .map_err(|e| BrightDataError::ToolError(format!("Failed to connect to Redis: {}", e)))?;
        
        // Test ping
        let _: String = conn.ping().await
            .map_err(|e| BrightDataError::ToolError(format!("Redis ping failed: {}", e)))?;
        
        let ttl = env::var("SCRAPE_CACHE_TTL_SECONDS")
            .or_else(|_| env::var("CACHE_TTL_SECONDS"))
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(7200); // Default 2 hours (web content changes more frequently than stock data)
        
        info!("✅ Scraping cache service initialized (TTL: {}s)", ttl);
        
        Ok(Self { client, ttl })
    }
    
    /// Generate scraping cache key: session_id:URL_HASH
    fn generate_key(&self, session_id: &str, url: &str) -> String {
        // Create a hash of the URL to ensure consistent key length and handle special characters
        let mut hasher = DefaultHasher::new();
        url.hash(&mut hasher);
        let url_hash = hasher.finish();
        
        // Also normalize the URL for better cache hits
        let normalized_url = self.normalize_url(url);
        let mut url_hasher = DefaultHasher::new();
        normalized_url.hash(&mut url_hasher);
        let normalized_hash = url_hasher.finish();
        
        let key = format!("scrape:{}:{:x}", session_id, normalized_hash);
        debug!("Scraping Cache Key: {} (URL: {})", key, url);
        key
    }
    
    /// Normalize URL for better cache hits (remove unnecessary parameters, fragments, etc.)
    fn normalize_url(&self, url: &str) -> String {
        match Url::parse(url) {
            Ok(mut parsed_url) => {
                // Remove fragment (everything after #)
                parsed_url.set_fragment(None);
                
                // Remove common tracking parameters
                let tracking_params = vec![
                    "utm_source", "utm_medium", "utm_campaign", "utm_content", "utm_term",
                    "gclid", "fbclid", "ref", "source", "_ga", "mc_cid", "mc_eid"
                ];
                
                // Collect query pairs into Vec<(String, String)>
                let pairs: Vec<(String, String)> = parsed_url.query_pairs().into_owned().collect();
                
                let filtered_pairs: Vec<(String, String)> = pairs
                    .into_iter()
                    .filter(|(key, _)| !tracking_params.contains(&key.as_str()))
                    .collect();
                
                if filtered_pairs.is_empty() {
                    parsed_url.set_query(None);
                } else {
                    let query_string = filtered_pairs
                        .iter()
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect::<Vec<_>>()
                        .join("&");
                    parsed_url.set_query(Some(&query_string));
                }
                
                parsed_url.to_string()
            }
            Err(_) => url.to_string(), // If URL parsing fails, use original
        }
    }

    /// Check if cached scraping data exists and is valid
    pub async fn get_cached_scrape_data(
        &self, 
        session_id: &str,
        url: &str
    ) -> Result<Option<Value>, BrightDataError> {
        let key = self.generate_key(session_id, url);
        
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| BrightDataError::ToolError(format!("Redis connection failed: {}", e)))?;
        
        match conn.get::<_, Option<String>>(&key).await {
            Ok(Some(cached_json)) => {
                match serde_json::from_str::<Value>(&cached_json) {
                    Ok(cached_data) => {
                        // Check for scraping-specific cached timestamp
                        if let Some(timestamp) = cached_data.get("scrape_cached_at").and_then(|t| t.as_u64()) {
                            let current_time = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_secs();
                            
                            if current_time - timestamp < self.ttl {
                                info!("🎯 Scraping Cache HIT for {} in session {}", url, session_id);
                                return Ok(Some(cached_data));
                            } else {
                                info!("⏰ Scraping Cache EXPIRED for {} in session {}", url, session_id);
                                // Remove expired data
                                let _: RedisResult<()> = conn.del(&key).await;
                            }
                        } else {
                            // DEBUG: Show what fields are available
                            let available_fields: Vec<String> = cached_data.as_object()
                                .map(|obj| obj.keys().map(|k| k.to_string()).collect())
                                .unwrap_or_default();
                            warn!("❌ Missing 'scrape_cached_at' field for key: {} (available: {:?})", key, available_fields);
                            // Remove corrupted data
                            let _: RedisResult<()> = conn.del(&key).await;
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse cached scraping data for {}: {}", key, e);
                        // Remove corrupted data
                        let _: RedisResult<()> = conn.del(&key).await;
                    }
                }
            }
            Ok(None) => {
                debug!("💾 Scraping Cache MISS for {} in session {} (key not found)", url, session_id);
            }
            Err(e) => {
                error!("Redis get error for scraping {}: {}", key, e);
                return Err(BrightDataError::ToolError(format!("Scraping cache get failed: {}", e)));
            }
        }
        
        Ok(None)
    }
    
    /// Cache scraping data with metadata
    pub async fn cache_scrape_data(
        &self,
        session_id: &str,
        url: &str,
        data: Value,
    ) -> Result<(), BrightDataError> {
        let key = self.generate_key(session_id, url);
        let normalized_url = self.normalize_url(url);
        
        // Add scraping caching metadata
        let mut cached_data = data;
        cached_data["scrape_cached_at"] = json!(SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs());
        cached_data["scrape_cache_key"] = json!(key.clone());
        cached_data["scrape_cache_ttl"] = json!(self.ttl);
        cached_data["original_url"] = json!(url);
        cached_data["normalized_url"] = json!(normalized_url);
        cached_data["session_id"] = json!(session_id);
        
        let json_string = serde_json::to_string(&cached_data)
            .map_err(|e| BrightDataError::ToolError(format!("Scraping JSON serialization failed: {}", e)))?;
        
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| BrightDataError::ToolError(format!("Redis connection failed: {}", e)))?;
        
        // Set with TTL
        conn.set_ex::<_, _, ()>(&key, &json_string, self.ttl as u64).await
            .map_err(|e| BrightDataError::ToolError(format!("Scraping cache set failed: {}", e)))?;
        
        info!("💾 Cached scraping data for {} in session {} (TTL: {}s)", url, session_id, self.ttl);
        
        Ok(())
    }
    
    /// Get all cached URLs for a session
    pub async fn get_session_scrape_urls(&self, session_id: &str) -> Result<Vec<String>, BrightDataError> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| BrightDataError::ToolError(format!("Redis connection failed: {}", e)))?;
        
        // Get all keys for this session's scraped data
        let pattern = format!("scrape:{}:*", session_id);
        let keys: Vec<String> = conn.keys(&pattern).await
            .map_err(|e| BrightDataError::ToolError(format!("Redis keys command failed: {}", e)))?;
        
        let mut urls = Vec::new();
        
        // Get the original URLs from the cached data
        for key in keys {
            match conn.get::<_, Option<String>>(&key).await {
                Ok(Some(cached_json)) => {
                    if let Ok(cached_data) = serde_json::from_str::<Value>(&cached_json) {
                        if let Some(original_url) = cached_data.get("original_url").and_then(|u| u.as_str()) {
                            urls.push(original_url.to_string());
                        }
                    }
                }
                _ => continue,
            }
        }
        
        debug!("📋 Found {} cached URLs for session {}: {:?}", urls.len(), session_id, urls);
        
        Ok(urls)
    }
    
    /// Clear cache for specific URL in session
    pub async fn clear_scrape_url_cache(
        &self,
        session_id: &str,
        url: &str,
    ) -> Result<(), BrightDataError> {
        let key = self.generate_key(session_id, url);
        
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| BrightDataError::ToolError(format!("Redis connection failed: {}", e)))?;
        
        let _: RedisResult<()> = conn.del(&key).await;
        
        info!("🗑️ Cleared scraping cache for {} in session {}", url, session_id);
        
        Ok(())
    }
    
    /// Clear all scraping cache for entire session
    pub async fn clear_session_scrape_cache(&self, session_id: &str) -> Result<u32, BrightDataError> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| BrightDataError::ToolError(format!("Redis connection failed: {}", e)))?;
        
        // Get all scraping keys for this session
        let pattern = format!("scrape:{}:*", session_id);
        let keys: Vec<String> = conn.keys(&pattern).await
            .map_err(|e| BrightDataError::ToolError(format!("Redis keys command failed: {}", e)))?;
        
        if keys.is_empty() {
            return Ok(0);
        }
        
        let deleted_count: u32 = conn.del(&keys).await
            .map_err(|e| BrightDataError::ToolError(format!("Redis delete failed: {}", e)))?;
        
        info!("🗑️ Cleared {} cached scraping items for session {}", deleted_count, session_id);
        
        Ok(deleted_count)
    }
    
    /// Get scraping cache statistics
    pub async fn get_scrape_cache_stats(&self) -> Result<Value, BrightDataError> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| BrightDataError::ToolError(format!("Redis connection failed: {}", e)))?;
        
        // Get scraping cache key count
        let scrape_keys: Vec<String> = conn.keys("scrape:*").await.unwrap_or_default();
        
        // Group by session
        let mut session_counts = std::collections::HashMap::new();
        let mut domain_counts = std::collections::HashMap::new();
        
        for key in &scrape_keys {
            let parts: Vec<&str> = key.split(':').collect();
            if parts.len() >= 2 {
                let session_id = parts[1];
                *session_counts.entry(session_id.to_string()).or_insert(0) += 1;
                
                // Try to get domain information from cached data
                if let Ok(Some(cached_json)) = conn.get::<_, Option<String>>(key).await {
                    if let Ok(cached_data) = serde_json::from_str::<Value>(&cached_json) {
                        if let Some(url) = cached_data.get("original_url").and_then(|u| u.as_str()) {
                            if let Ok(parsed_url) = Url::parse(url) {
                                if let Some(domain) = parsed_url.domain() {
                                    *domain_counts.entry(domain.to_string()).or_insert(0) += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        
        let stats = json!({
            "total_scrape_cache_entries": scrape_keys.len(),
            "active_sessions": session_counts.len(),
            "session_breakdown": session_counts,
            "domain_breakdown": domain_counts,
            "scrape_cache_ttl_seconds": self.ttl,
            "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
        });
        
        Ok(stats)
    }
    
    /// Check if specific URL is cached for session
    pub async fn is_url_cached(&self, session_id: &str, url: &str) -> Result<bool, BrightDataError> {
        let key = self.generate_key(session_id, url);
        
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| BrightDataError::ToolError(format!("Redis connection failed: {}", e)))?;
        
        let exists: bool = conn.exists(&key).await
            .map_err(|e| BrightDataError::ToolError(format!("Redis exists check failed: {}", e)))?;
        
        Ok(exists)
    }
    
    /// Health check for Redis connection
    pub async fn health_check(&self) -> Result<bool, BrightDataError> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|_| BrightDataError::ToolError("Scraping cache Redis connection failed".into()))?;
        
        let _: String = conn.ping().await
            .map_err(|_| BrightDataError::ToolError("Scraping cache Redis ping failed".into()))?;
        
        Ok(true)
    }
    
    /// Batch cache multiple URLs (useful for bulk scraping operations)
    pub async fn batch_cache_scrape_data(
        &self,
        session_id: &str,
        scrape_data: Vec<(String, Value)>, // Vec<(url, data)>
    ) -> Result<Vec<String>, BrightDataError> {
        let mut successful_urls = Vec::new();
        
        for (url, data) in scrape_data {
            match self.cache_scrape_data(session_id, &url, data).await {
                Ok(_) => {
                    successful_urls.push(url);
                }
                Err(e) => {
                    warn!("Failed to cache scraping data for {}: {}", url, e);
                }
            }
        }
        
        info!("📦 Batch cached {} URLs for session {}", successful_urls.len(), session_id);
        
        Ok(successful_urls)
    }
    
    /// Get cache entry by domain (useful for finding related cached content)
    pub async fn get_cached_urls_by_domain(
        &self,
        session_id: &str,
        domain: &str,
    ) -> Result<Vec<String>, BrightDataError> {
        let urls = self.get_session_scrape_urls(session_id).await?;
        
        let matching_urls: Vec<String> = urls
            .into_iter()
            .filter(|url| {
                if let Ok(parsed_url) = Url::parse(url) {
                    if let Some(url_domain) = parsed_url.domain() {
                        return url_domain.contains(domain) || domain.contains(url_domain);
                    }
                }
                false
            })
            .collect();
        
        debug!("🔍 Found {} cached URLs matching domain '{}' for session {}", 
               matching_urls.len(), domain, session_id);
        
        Ok(matching_urls)
    }
    
    /// Get cached data with content length information
    pub async fn get_cache_summary(&self, session_id: &str) -> Result<Value, BrightDataError> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| BrightDataError::ToolError(format!("Redis connection failed: {}", e)))?;
        
        let pattern = format!("scrape:{}:*", session_id);
        let keys: Vec<String> = conn.keys(&pattern).await
            .map_err(|e| BrightDataError::ToolError(format!("Redis keys command failed: {}", e)))?;
        
        let mut summary = Vec::new();
        let mut total_content_size = 0;
        
        for key in keys {
            if let Ok(Some(cached_json)) = conn.get::<_, Option<String>>(&key).await {
                if let Ok(cached_data) = serde_json::from_str::<Value>(&cached_json) {
                    let url = cached_data.get("original_url")
                        .and_then(|u| u.as_str())
                        .unwrap_or("unknown");
                    
                    let content_size = cached_data.get("content")
                        .and_then(|c| c.as_str())
                        .map(|s| s.len())
                        .unwrap_or(0);
                    
                    let cached_at = cached_data.get("scrape_cached_at")
                        .and_then(|t| t.as_u64())
                        .unwrap_or(0);
                    
                    total_content_size += content_size;
                    
                    summary.push(json!({
                        "url": url,
                        "content_size_bytes": content_size,
                        "cached_at": cached_at,
                        "cache_key": key
                    }));
                }
            }
        }
        
        Ok(json!({
            "session_id": session_id,
            "total_cached_urls": summary.len(),
            "total_content_size_bytes": total_content_size,
            "cached_items": summary,
            "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
        }))
    }
}

// Singleton instance for global access
use std::sync::Arc;
use tokio::sync::OnceCell;

static SCRAPE_CACHE: OnceCell<Arc<ScrapeCache>> = OnceCell::const_new();

/// Get global scraping cache service instance
pub async fn get_scrape_cache() -> Result<Arc<ScrapeCache>, BrightDataError> {
    SCRAPE_CACHE.get_or_try_init(|| async {
        let service = ScrapeCache::new().await?;
        Ok(Arc::new(service))
    }).await.map(|arc| arc.clone())
}

/// Initialize scraping cache service (call this at startup)
pub async fn init_scrape_cache() -> Result<(), BrightDataError> {
    let _service = get_scrape_cache().await?;
    info!("✅ Global scraping cache service initialized");
    Ok(())
}