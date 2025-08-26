// crates/snm-brightdata-client/src/services/crypto_cache.rs
use redis::{AsyncCommands, Client, RedisResult};
use serde_json::{json, Value};
use std::env;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use log::{info, warn, error, debug};
use crate::error::BrightDataError;

#[derive(Debug, Clone)]
pub struct CryptoCache {
    client: Client,
    ttl: u64,
}

impl CryptoCache {
    /// Initialize Redis crypto cache service
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
        
        let ttl = env::var("CRYPTO_CACHE_TTL_SECONDS")
            .or_else(|_| env::var("CACHE_TTL_SECONDS"))
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(3600); // Default 1 hour
        
        info!("Crypto cache service initialized (TTL: {}s)", ttl);
        
        Ok(Self { client, ttl })
    }
    
    /// Generate crypto cache key: session_id:SYMBOL
    fn generate_key(&self, session_id: &str, symbol: &str) -> String {
        let key = format!("crypto:{}:{}", session_id, symbol.to_lowercase());
        info!("Cache Key: {}", key);
        key
    }

    /// Check if cached crypto data exists and is valid
    pub async fn get_cached_crypto_data(
        &self, 
        session_id: &str,
        symbol: &str
    ) -> Result<Option<Value>, BrightDataError> {
        let key = self.generate_key(session_id, symbol);
        
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| BrightDataError::ToolError(format!("Redis connection failed: {}", e)))?;
        
        match conn.get::<_, Option<String>>(&key).await {
            Ok(Some(cached_json)) => {
                match serde_json::from_str::<Value>(&cached_json) {
                    Ok(cached_data) => {
                        if let Some(timestamp) = cached_data.get("crypto_cached_at").and_then(|t| t.as_u64()) {
                            let current_time = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_secs();
                            
                            if current_time - timestamp < self.ttl {
                                info!("Crypto Cache HIT for {} in session {}", symbol, session_id);
                                return Ok(Some(cached_data));
                            } else {
                                info!("Crypto Cache EXPIRED for {} in session {}", symbol, session_id);
                                // Remove expired data
                                let _: RedisResult<()> = conn.del(&key).await;
                            }
                        } else {
                            // DEBUG: Show what fields are available
                            let available_fields: Vec<String> = cached_data.as_object()
                                .map(|obj| obj.keys().map(|k| k.to_string()).collect())
                                .unwrap_or_default();
                            warn!("Missing 'crypto_cached_at' field for key: {} (available: {:?})", key, available_fields);
                            // Remove corrupted data
                            let _: RedisResult<()> = conn.del(&key).await;
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse cached crypto data for {}: {}", key, e);
                        // Remove corrupted data
                        let _: RedisResult<()> = conn.del(&key).await;
                    }
                }
            }
            Ok(None) => {
                debug!("Crypto Cache MISS for {} in session {} (key not found)", symbol, session_id);
            }
            Err(e) => {
                error!("Redis get error for crypto {}: {}", key, e);
                return Err(BrightDataError::ToolError(format!("Crypto cache get failed: {}", e)));
            }
        }
        
        Ok(None)
    }
    
    /// Cache crypto data with metadata
    pub async fn cache_crypto_data(
        &self,
        session_id: &str,
        symbol: &str,
        data: Value,
    ) -> Result<(), BrightDataError> {
        let key = self.generate_key(session_id, symbol);
        
        // Add crypto caching metadata
        let mut cached_data = data;
        cached_data["crypto_cached_at"] = json!(SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs());
        cached_data["crypto_cache_key"] = json!(key.clone());
        cached_data["crypto_cache_ttl"] = json!(self.ttl);
        cached_data["symbol"] = json!(symbol.to_lowercase());
        cached_data["session_id"] = json!(session_id);
        
        let json_string = serde_json::to_string(&cached_data)
            .map_err(|e| BrightDataError::ToolError(format!("Crypto JSON serialization failed: {}", e)))?;
        
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| BrightDataError::ToolError(format!("Redis connection failed: {}", e)))?;
        
        // Set with TTL
        conn.set_ex::<_, _, ()>(&key, &json_string, self.ttl as u64).await
            .map_err(|e| BrightDataError::ToolError(format!("Crypto cache set failed: {}", e)))?;
        
        info!("Cached crypto data for {} in session {} (TTL: {}s)", symbol, session_id, self.ttl);
        
        Ok(())
    }
    
    /// Get all cached crypto symbols for a session
    pub async fn get_session_crypto_symbols(&self, session_id: &str) -> Result<Vec<String>, BrightDataError> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| BrightDataError::ToolError(format!("Redis connection failed: {}", e)))?;
        
        // Get all keys for this session's crypto
        let pattern = format!("crypto:{}:*", session_id);
        let keys: Vec<String> = conn.keys(&pattern).await
            .map_err(|e| BrightDataError::ToolError(format!("Redis keys command failed: {}", e)))?;
        
        // Extract symbols from keys (crypto:session:SYMBOL -> SYMBOL)
        let symbols: Vec<String> = keys
            .iter()
            .filter_map(|key| {
                let parts: Vec<&str> = key.split(':').collect();
                if parts.len() >= 3 {
                    Some(parts[2].to_string())
                } else {
                    None
                }
            })
            .collect();
        
        debug!("Found {} cached crypto symbols for session {}: {:?}", symbols.len(), session_id, symbols);
        
        Ok(symbols)
    }
    
    /// Clear cache for specific crypto symbol in session
    pub async fn clear_crypto_symbol_cache(
        &self,
        session_id: &str,
        symbol: &str,
    ) -> Result<(), BrightDataError> {
        let key = self.generate_key(session_id, symbol);
        
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| BrightDataError::ToolError(format!("Redis connection failed: {}", e)))?;
        
        let _: RedisResult<()> = conn.del(&key).await;
        
        info!("Cleared crypto cache for {} in session {}", symbol, session_id);
        
        Ok(())
    }
    
    /// Clear all crypto cache for entire session
    pub async fn clear_session_crypto_cache(&self, session_id: &str) -> Result<u32, BrightDataError> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| BrightDataError::ToolError(format!("Redis connection failed: {}", e)))?;
        
        // Get all crypto keys for this session
        let pattern = format!("crypto:{}:*", session_id);
        let keys: Vec<String> = conn.keys(&pattern).await
            .map_err(|e| BrightDataError::ToolError(format!("Redis keys command failed: {}", e)))?;
        
        if keys.is_empty() {
            return Ok(0);
        }
        
        let deleted_count: u32 = conn.del(&keys).await
            .map_err(|e| BrightDataError::ToolError(format!("Redis delete failed: {}", e)))?;
        
        info!("Cleared {} cached crypto items for session {}", deleted_count, session_id);
        
        Ok(deleted_count)
    }
    
    /// Get crypto cache statistics
    pub async fn get_crypto_cache_stats(&self) -> Result<Value, BrightDataError> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| BrightDataError::ToolError(format!("Redis connection failed: {}", e)))?;
        
        // Get crypto cache key count
        let crypto_keys: Vec<String> = conn.keys("crypto:*").await.unwrap_or_default();
        
        // Group by session
        let mut session_counts = std::collections::HashMap::new();
        for key in &crypto_keys {
            let parts: Vec<&str> = key.split(':').collect();
            if parts.len() >= 2 {
                let session_id = parts[1];
                *session_counts.entry(session_id.to_string()).or_insert(0) += 1;
            }
        }
        
        let stats = json!({
            "total_crypto_cache_entries": crypto_keys.len(),
            "active_sessions": session_counts.len(),
            "session_breakdown": session_counts,
            "crypto_cache_ttl_seconds": self.ttl,
            "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
        });
        
        Ok(stats)
    }
    
    /// Check if specific crypto is cached for session
    pub async fn is_crypto_cached(&self, session_id: &str, symbol: &str) -> Result<bool, BrightDataError> {
        let key = self.generate_key(session_id, symbol);
        
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| BrightDataError::ToolError(format!("Redis connection failed: {}", e)))?;
        
        let exists: bool = conn.exists(&key).await
            .map_err(|e| BrightDataError::ToolError(format!("Redis exists check failed: {}", e)))?;
        
        Ok(exists)
    }
    
    /// Health check for Redis connection
    pub async fn health_check(&self) -> Result<bool, BrightDataError> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|_| BrightDataError::ToolError("Crypto cache Redis connection failed".into()))?;
        
        let _: String = conn.ping().await
            .map_err(|_| BrightDataError::ToolError("Crypto cache Redis ping failed".into()))?;
        
        Ok(true)
    }
    
    /// Batch cache multiple crypto symbols (useful for comparisons)
    pub async fn batch_cache_crypto_data(
        &self,
        session_id: &str,
        crypto_data: Vec<(String, Value)>, // Vec<(symbol, data)>
    ) -> Result<Vec<String>, BrightDataError> {
        let mut successful_symbols = Vec::new();
        
        for (symbol, data) in crypto_data {
            match self.cache_crypto_data(session_id, &symbol, data).await {
                Ok(_) => {
                    successful_symbols.push(symbol);
                }
                Err(e) => {
                    warn!("Failed to cache crypto data for {}: {}", symbol, e);
                }
            }
        }
        
        info!("Batch cached {} crypto symbols for session {}", successful_symbols.len(), session_id);
        
        Ok(successful_symbols)
    }
}

// Singleton instance for global access
use std::sync::Arc;
use tokio::sync::OnceCell;

static CRYPTO_CACHE: OnceCell<Arc<CryptoCache>> = OnceCell::const_new();

/// Get global crypto cache service instance
pub async fn get_crypto_cache() -> Result<Arc<CryptoCache>, BrightDataError> {
    CRYPTO_CACHE.get_or_try_init(|| async {
        let service = CryptoCache::new().await?;
        Ok(Arc::new(service))
    }).await.map(|arc| arc.clone())
}

/// Initialize crypto cache service (call this at startup)
pub async fn init_crypto_cache() -> Result<(), BrightDataError> {
    let _service = get_crypto_cache().await?;
    info!("Global crypto cache service initialized");
    Ok(())
}