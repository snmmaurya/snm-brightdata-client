// crates/snm-brightdata-client/src/services/bond_cache.rs
use redis::{AsyncCommands, Client, RedisResult};
use serde_json::{json, Value};
use std::env;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use log::{info, warn, error, debug};
use crate::error::BrightDataError;

#[derive(Debug, Clone)]
pub struct BondCache {
    client: Client,
    ttl: u64,
}

impl BondCache {
    /// Initialize Redis ETF cache service
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
        
        let ttl = env::var("BOND_CACHE_TTL_SECONDS")
            .or_else(|_| env::var("CACHE_TTL_SECONDS"))
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(3600); // Default 1 hour
        
        info!("ETF cache service initialized (TTL: {}s)", ttl);
        
        Ok(Self { client, ttl })
    }
    
    /// Generate ETF cache key: session_id:SYMBOL
    fn generate_key(&self, session_id: &str, symbol: &str) -> String {
        let key = format!("bond:{}:{}", session_id, symbol.to_uppercase());
        info!("Cache Key: {}", key);
        key
    }

    /// Check if cached ETF data exists and is valid
    pub async fn get_cached_bond_data(
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
                        if let Some(timestamp) = cached_data.get("bond_cached_at").and_then(|t| t.as_u64()) {
                            let current_time = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_secs();
                            
                            if current_time - timestamp < self.ttl {
                                info!("ETF Cache HIT for {} in session {}", symbol, session_id);
                                return Ok(Some(cached_data));
                            } else {
                                info!("ETF Cache EXPIRED for {} in session {}", symbol, session_id);
                                // Remove expired data
                                let _: RedisResult<()> = conn.del(&key).await;
                            }
                        } else {
                            // DEBUG: Show what fields are available
                            let available_fields: Vec<String> = cached_data.as_object()
                                .map(|obj| obj.keys().map(|k| k.to_string()).collect())
                                .unwrap_or_default();
                            warn!("Missing 'bond_cached_at' field for key: {} (available: {:?})", key, available_fields);
                            // Remove corrupted data
                            let _: RedisResult<()> = conn.del(&key).await;
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse cached ETF data for {}: {}", key, e);
                        // Remove corrupted data
                        let _: RedisResult<()> = conn.del(&key).await;
                    }
                }
            }
            Ok(None) => {
                debug!("ETF Cache MISS for {} in session {} (key not found)", symbol, session_id);
            }
            Err(e) => {
                error!("Redis get error for ETF {}: {}", key, e);
                return Err(BrightDataError::ToolError(format!("ETF cache get failed: {}", e)));
            }
        }
        
        Ok(None)
    }
    
    /// Cache ETF data with metadata
    pub async fn cache_bond_data(
        &self,
        session_id: &str,
        symbol: &str,
        data: Value,
    ) -> Result<(), BrightDataError> {
        let key = self.generate_key(session_id, symbol);
        
        // Add ETF caching metadata
        let mut cached_data = data;
        cached_data["bond_cached_at"] = json!(SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs());
        cached_data["bond_cache_key"] = json!(key.clone());
        cached_data["bond_cache_ttl"] = json!(self.ttl);
        cached_data["symbol"] = json!(symbol.to_uppercase());
        cached_data["session_id"] = json!(session_id);
        
        let json_string = serde_json::to_string(&cached_data)
            .map_err(|e| BrightDataError::ToolError(format!("ETF JSON serialization failed: {}", e)))?;
        
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| BrightDataError::ToolError(format!("Redis connection failed: {}", e)))?;
        
        // Set with TTL
        conn.set_ex::<_, _, ()>(&key, &json_string, self.ttl as u64).await
            .map_err(|e| BrightDataError::ToolError(format!("ETF cache set failed: {}", e)))?;
        
        info!("Cached ETF data for {} in session {} (TTL: {}s)", symbol, session_id, self.ttl);
        
        Ok(())
    }
    
    /// Get all cached ETF symbols for a session
    pub async fn get_session_bond_symbols(&self, session_id: &str) -> Result<Vec<String>, BrightDataError> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| BrightDataError::ToolError(format!("Redis connection failed: {}", e)))?;
        
        // Get all keys for this session's ETFs
        let pattern = format!("bond:{}:*", session_id);
        let keys: Vec<String> = conn.keys(&pattern).await
            .map_err(|e| BrightDataError::ToolError(format!("Redis keys command failed: {}", e)))?;
        
        // Extract symbols from keys (bond:session:SYMBOL -> SYMBOL)
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
        
        debug!("Found {} cached ETF symbols for session {}: {:?}", symbols.len(), session_id, symbols);
        
        Ok(symbols)
    }
    
    /// Clear cache for specific ETF symbol in session
    pub async fn clear_bond_symbol_cache(
        &self,
        session_id: &str,
        symbol: &str,
    ) -> Result<(), BrightDataError> {
        let key = self.generate_key(session_id, symbol);
        
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| BrightDataError::ToolError(format!("Redis connection failed: {}", e)))?;
        
        let _: RedisResult<()> = conn.del(&key).await;
        
        info!("Cleared ETF cache for {} in session {}", symbol, session_id);
        
        Ok(())
    }
    
    /// Clear all ETF cache for entire session
    pub async fn clear_session_bond_cache(&self, session_id: &str) -> Result<u32, BrightDataError> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| BrightDataError::ToolError(format!("Redis connection failed: {}", e)))?;
        
        // Get all ETF keys for this session
        let pattern = format!("bond:{}:*", session_id);
        let keys: Vec<String> = conn.keys(&pattern).await
            .map_err(|e| BrightDataError::ToolError(format!("Redis keys command failed: {}", e)))?;
        
        if keys.is_empty() {
            return Ok(0);
        }
        
        let deleted_count: u32 = conn.del(&keys).await
            .map_err(|e| BrightDataError::ToolError(format!("Redis delete failed: {}", e)))?;
        
        info!("Cleared {} cached ETF items for session {}", deleted_count, session_id);
        
        Ok(deleted_count)
    }
    
    /// Get ETF cache statistics
    pub async fn get_bond_cache_stats(&self) -> Result<Value, BrightDataError> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| BrightDataError::ToolError(format!("Redis connection failed: {}", e)))?;
        
        // Get ETF cache key count
        let bond_keys: Vec<String> = conn.keys("bond:*").await.unwrap_or_default();
        
        // Group by session
        let mut session_counts = std::collections::HashMap::new();
        for key in &bond_keys {
            let parts: Vec<&str> = key.split(':').collect();
            if parts.len() >= 2 {
                let session_id = parts[1];
                *session_counts.entry(session_id.to_string()).or_insert(0) += 1;
            }
        }
        
        let stats = json!({
            "total_bond_cache_entries": bond_keys.len(),
            "active_sessions": session_counts.len(),
            "session_breakdown": session_counts,
            "bond_cache_ttl_seconds": self.ttl,
            "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
        });
        
        Ok(stats)
    }
    
    /// Check if specific ETF is cached for session
    pub async fn is_bond_cached(&self, session_id: &str, symbol: &str) -> Result<bool, BrightDataError> {
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
            .map_err(|_| BrightDataError::ToolError("ETF cache Redis connection failed".into()))?;
        
        let _: String = conn.ping().await
            .map_err(|_| BrightDataError::ToolError("ETF cache Redis ping failed".into()))?;
        
        Ok(true)
    }
    
    /// Batch cache multiple ETF symbols (useful for comparisons)
    pub async fn batch_cache_bond_data(
        &self,
        session_id: &str,
        bond_data: Vec<(String, Value)>, // Vec<(symbol, data)>
    ) -> Result<Vec<String>, BrightDataError> {
        let mut successful_symbols = Vec::new();
        
        for (symbol, data) in bond_data {
            match self.cache_bond_data(session_id, &symbol, data).await {
                Ok(_) => {
                    successful_symbols.push(symbol);
                }
                Err(e) => {
                    warn!("Failed to cache ETF data for {}: {}", symbol, e);
                }
            }
        }
        
        info!("Batch cached {} ETF symbols for session {}", successful_symbols.len(), session_id);
        
        Ok(successful_symbols)
    }
}

// Singleton instance for global access
use std::sync::Arc;
use tokio::sync::OnceCell;

static BOND_CACHE: OnceCell<Arc<BondCache>> = OnceCell::const_new();

/// Get global ETF cache service instance
pub async fn get_bond_cache() -> Result<Arc<BondCache>, BrightDataError> {
    BOND_CACHE.get_or_try_init(|| async {
        let service = BondCache::new().await?;
        Ok(Arc::new(service))
    }).await.map(|arc| arc.clone())
}

/// Initialize ETF cache service (call this at startup)
pub async fn init_bond_cache() -> Result<(), BrightDataError> {
    let _service = get_bond_cache().await?;
    info!("Global ETF cache service initialized");
    Ok(())
}