use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::epub_parser::BookChunk;

/// Embedding vector (Gemini text-embedding-004 produces 768-dimensional vectors)
pub type Embedding = Vec<f32>;

/// A chunk with its embedding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedChunk {
    pub chunk_id: String,
    pub chapter_index: usize,
    pub chapter_title: String,
    pub text: String,
    pub start_offset: usize,
    pub end_offset: usize,
    pub embedding: Embedding,
}

/// Store for chunk embeddings
pub struct EmbeddingStore {
    chunks: Vec<EmbeddedChunk>,
    chunk_map: HashMap<String, usize>,
}

impl EmbeddingStore {
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            chunk_map: HashMap::new(),
        }
    }
    
    /// Add an embedded chunk
    pub fn add(&mut self, chunk: EmbeddedChunk) {
        let idx = self.chunks.len();
        self.chunk_map.insert(chunk.chunk_id.clone(), idx);
        self.chunks.push(chunk);
    }
    
    /// Find chunks most similar to query embedding
    pub fn find_similar(&self, query_embedding: &Embedding, max_chapter: usize, top_k: usize) -> Vec<&EmbeddedChunk> {
        let mut scored: Vec<(f32, &EmbeddedChunk)> = self.chunks
            .iter()
            .filter(|c| c.chapter_index <= max_chapter)
            .map(|chunk| {
                let similarity = cosine_similarity(query_embedding, &chunk.embedding);
                (similarity, chunk)
            })
            .collect();
        
        // Sort by similarity descending
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        
        scored.into_iter()
            .take(top_k)
            .map(|(_, chunk)| chunk)
            .collect()
    }
    
    /// Get all chunks
    pub fn chunks(&self) -> &[EmbeddedChunk] {
        &self.chunks
    }
    
    /// Check if store has embeddings
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }
    
    /// Create from a vector of embedded chunks (for loading from cache)
    pub fn from_embeddings(embeddings: Vec<EmbeddedChunk>) -> Self {
        let mut store = Self::new();
        for chunk in embeddings {
            store.add(chunk);
        }
        store
    }
}

/// Calculate cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    
    dot_product / (norm_a * norm_b)
}

/// Call Gemini embedding API
pub async fn get_embedding(text: &str, api_key: &str) -> Result<Embedding, String> {
    let client = reqwest::Client::new();
    
    // Retry logic for 429s (simple wait)
    for attempt in 0..3 {
        let response = client
            .post(format!(
                "https://generativelanguage.googleapis.com/v1beta/models/text-embedding-004:embedContent?key={}",
                api_key
            ))
            .json(&serde_json::json!({
                "content": {
                    "parts": [{"text": text}]
                }
            }))
            .send()
            .await
            .map_err(|e| format!("Failed to call embedding API: {}", e))?;
        
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let wait_secs = 2u64.pow(attempt);
            println!("Rate limited on query embedding, waiting {}s...", wait_secs);
            tokio::time::sleep(tokio::time::Duration::from_secs(wait_secs)).await;
            continue;
        }

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("Embedding API error: {}", error_text));
        }
        
        let response_json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse embedding response: {}", e))?;
        
        let values = response_json["embedding"]["values"]
            .as_array()
            .ok_or("No embedding values in response")?;
        
        let embedding: Embedding = values
            .iter()
            .filter_map(|v| v.as_f64().map(|f| f as f32))
            .collect();
        
        if embedding.is_empty() {
            return Err("Empty embedding returned".to_string());
        }
        
        return Ok(embedding);
    }
    
    Err("Exhausted retries for query embedding".to_string())
}

/// Batch embed multiple texts (with rate limiting and backoff)
pub async fn embed_chunks(chunks: &[BookChunk], api_key: &str) -> Result<Vec<EmbeddedChunk>, String> {
    let client = reqwest::Client::new();
    let mut embedded_chunks = Vec::new();
    
    // Use batching (max 100 per request)
    const BATCH_SIZE: usize = 100;
    
    for chunk_batch in chunks.chunks(BATCH_SIZE) {
        let mut attempts = 0;
        let max_attempts = 5;
        
        loop {
            let requests: Vec<serde_json::Value> = chunk_batch.iter().map(|c| {
                let text = if c.text.len() > 2000 {
                    &c.text[..2000]
                } else {
                    &c.text
                };
                serde_json::json!({
                    "model": "models/text-embedding-004",
                    "content": {
                        "parts": [{"text": text}]
                    }
                })
            }).collect();

            let response = client
                .post(format!(
                    "https://generativelanguage.googleapis.com/v1beta/models/text-embedding-004:batchEmbedContents?key={}",
                    api_key
                ))
                .json(&serde_json::json!({
                    "requests": requests
                }))
                .send()
                .await
                .map_err(|e| format!("Failed to call batch embedding API: {}", e))?;
            
            if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                attempts += 1;
                if attempts >= max_attempts {
                    println!("Warning: Max retries reached for batch. Skipping.");
                    break;
                }
                let wait_secs = 5 * attempts; // More aggressive wait
                println!("Rate limited on batch embedding, waiting {}s...", wait_secs);
                tokio::time::sleep(tokio::time::Duration::from_secs(wait_secs)).await;
                continue;
            }

            if !response.status().is_success() {
                let error_text = response.text().await.unwrap_or_default();
                println!("Warning: Batch embedding failed: {}", error_text);
                break;
            }
            
            let response_json: serde_json::Value = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse batch response: {}", e))?;
            
            if let Some(embeddings) = response_json["embeddings"].as_array() {
                for (i, emb_data) in embeddings.iter().enumerate() {
                    if let Some(values) = emb_data["values"].as_array() {
                        let embedding: Embedding = values
                            .iter()
                            .filter_map(|v| v.as_f64().map(|f| f as f32))
                            .collect();
                        
                        if !embedding.is_empty() {
                            let chunk = &chunk_batch[i];
                            embedded_chunks.push(EmbeddedChunk {
                                chunk_id: chunk.id.clone(),
                                chapter_index: chunk.chapter_index,
                                chapter_title: chunk.chapter_title.clone(),
                                text: chunk.text.clone(),
                                start_offset: chunk.start_offset,
                                end_offset: chunk.end_offset,
                                embedding,
                            });
                        }
                    }
                }
            }
            break; // Success or non-429 failure
        }
        
        // Wait between batches to be safe
        tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
    }
    
    Ok(embedded_chunks)
}
