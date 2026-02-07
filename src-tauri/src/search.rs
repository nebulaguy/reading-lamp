use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{doc, Index, IndexWriter, ReloadPolicy, TantivyDocument};
use tempfile::TempDir;

use crate::epub_parser::BookChunk;

/// Search engine for finding quotes in book content
pub struct BookSearchIndex {
    index: Index,
    schema: Schema,
    chunk_id_field: Field,
    chapter_index_field: Field,
    chapter_title_field: Field,
    text_field: Field,
    start_offset_field: Field,
    end_offset_field: Field,
    _temp_dir: Option<TempDir>,
}

/// Result from searching for a quote
#[derive(Debug, Clone)]
pub struct QuoteMatch {
    pub chunk_id: String,
    pub chapter_index: usize,
    pub chapter_title: String,
    pub matched_text: String,
    pub start_offset: usize,
    pub end_offset: usize,
    pub score: f32,
}

impl BookSearchIndex {
    /// Create a new search index from book chunks
    pub fn new(chunks: &[BookChunk]) -> Result<Self, String> {
        // Build schema
        let mut schema_builder = Schema::builder();
        
        let chunk_id_field = schema_builder.add_text_field("chunk_id", STRING | STORED);
        let chapter_index_field = schema_builder.add_u64_field("chapter_index", INDEXED | STORED);
        let chapter_title_field = schema_builder.add_text_field("chapter_title", TEXT | STORED);
        let text_field = schema_builder.add_text_field("text", TEXT | STORED);
        let start_offset_field = schema_builder.add_u64_field("start_offset", STORED);
        let end_offset_field = schema_builder.add_u64_field("end_offset", STORED);
        
        let schema = schema_builder.build();
        
        // Create index in RAM for speed
        let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;
        let index = Index::create_in_dir(temp_dir.path(), schema.clone())
            .map_err(|e| format!("Failed to create index: {}", e))?;
        
        // Index all chunks
        let mut index_writer: IndexWriter = index
            .writer(50_000_000) // 50MB buffer
            .map_err(|e| format!("Failed to create index writer: {}", e))?;
        
        for chunk in chunks {
            index_writer.add_document(doc!(
                chunk_id_field => chunk.id.clone(),
                chapter_index_field => chunk.chapter_index as u64,
                chapter_title_field => chunk.chapter_title.clone(),
                text_field => chunk.text.clone(),
                start_offset_field => chunk.start_offset as u64,
                end_offset_field => chunk.end_offset as u64,
            )).map_err(|e| format!("Failed to add document: {}", e))?;
        }
        
        index_writer.commit().map_err(|e| format!("Failed to commit index: {}", e))?;
        
        Ok(Self {
            index,
            schema,
            chunk_id_field,
            chapter_index_field,
            chapter_title_field,
            text_field,
            start_offset_field,
            end_offset_field,
            _temp_dir: Some(temp_dir),
        })
    }
    
    /// Search for a quote and return matching chunks
    pub fn search_quote(&self, query: &str, max_results: usize) -> Result<Vec<QuoteMatch>, String> {
        let reader = self.index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| format!("Failed to create reader: {}", e))?;
        
        let searcher = reader.searcher();
        
        // Use query parser for full-text search
        let query_parser = QueryParser::for_index(&self.index, vec![self.text_field]);
        
        // Escape special characters and create query
        let escaped_query = escape_query(query);
        let parsed_query = query_parser
            .parse_query(&escaped_query)
            .map_err(|e| format!("Failed to parse query: {}", e))?;
        
        let top_docs = searcher
            .search(&parsed_query, &TopDocs::with_limit(max_results))
            .map_err(|e| format!("Search failed: {}", e))?;
        
        let mut results = Vec::new();
        
        for (score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| format!("Failed to retrieve doc: {}", e))?;
            
            let chunk_id = doc
                .get_first(self.chunk_id_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            
            let chapter_index = doc
                .get_first(self.chapter_index_field)
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            
            let chapter_title = doc
                .get_first(self.chapter_title_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            
            let matched_text = doc
                .get_first(self.text_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            
            let start_offset = doc
                .get_first(self.start_offset_field)
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            
            let end_offset = doc
                .get_first(self.end_offset_field)
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            
            results.push(QuoteMatch {
                chunk_id,
                chapter_index,
                chapter_title,
                matched_text,
                start_offset,
                end_offset,
                score,
            });
        }
        
        Ok(results)
    }
    
    /// Search using exact substring matching (for copied quotes)
    pub fn find_exact_quote(&self, quote: &str, chunks: &[BookChunk]) -> Option<QuoteMatch> {
        let quote_normalized = normalize_text(quote);
        
        println!("Looking for quote: '{}' (normalized: '{}')", 
            &quote[..quote.len().min(50)], 
            &quote_normalized[..quote_normalized.len().min(50)]);
        
        // First try exact substring match
        for chunk in chunks {
            let chunk_normalized = normalize_text(&chunk.text);
            if chunk_normalized.contains(&quote_normalized) {
                println!("Found exact match in chunk {} at chapter {}", chunk.id, chunk.chapter_index);
                return Some(QuoteMatch {
                    chunk_id: chunk.id.clone(),
                    chapter_index: chunk.chapter_index,
                    chapter_title: chunk.chapter_title.clone(),
                    matched_text: chunk.text.clone(),
                    start_offset: chunk.start_offset,
                    end_offset: chunk.end_offset,
                    score: 1.0,
                });
            }
        }
        
        println!("No exact match found, trying fuzzy matching...");
        
        // Fallback to fuzzy matching
        // We use normalized text to improve robustness against punctuation differences
        let quote_normalized = normalize_text(quote);
        let quote_words: Vec<&str> = quote_normalized.split_whitespace().collect();
        let quote_len = quote_words.len();
        
        if quote_len == 0 {
             return None;
        }

        let mut best_match: Option<(f64, &BookChunk)> = None;
        
        for chunk in chunks {
            let chunk_normalized = normalize_text(&chunk.text);
            
            // Calculate word overlap (bag of words containment)
            // We use a simple containment check on the normalized string first for speed
            // then verify individual words if needed.
            
            let matched_words = quote_words.iter()
                .filter(|&w| {
                    // Check if word exists in chunk as a standalone word (surrounded by spaces)
                    // Since normalize_text joins with space, we can pad chunk with spaces
                    let padded = format!(" {} ", chunk_normalized);
                    let target = format!(" {} ", w);
                    padded.contains(&target)
                })
                .count();
                
            let word_overlap = matched_words as f64 / quote_len as f64;
            
            // If we have high word overlap, calculate sequence preservation
            // This is a rough heuristic for "words appearing in order"
            let mut sequence_score = 0.0;
            if word_overlap > 0.5 {
                let mut last_idx = 0;
                let mut in_order = 0;
                
                for w in &quote_words {
                    if let Some(idx) = chunk_normalized[last_idx..].find(w) {
                        last_idx += idx;
                        in_order += 1;
                    }
                }
                sequence_score = in_order as f64 / quote_len as f64;
            }
            
            // Final score heavily weights word overlap and sequence
            // We ignore Levenshtein because it penalizes length differences (substring vs full doc)
            let combined_score = (word_overlap * 0.7) + (sequence_score * 0.3);
            
            if combined_score > 0.6 { // Higher threshold but we have better metrics now
                if best_match.is_none() || combined_score > best_match.as_ref().unwrap().0 {
                    best_match = Some((combined_score, chunk));
                }
            }
        }
        
        if let Some((score, chunk)) = &best_match {
            println!("Best fuzzy match: chunk {} at chapter {} (score: {:.2})", 
                chunk.id, chunk.chapter_index, score);
        } else {
            println!("No fuzzy match found above threshold");
        }
        
        best_match.map(|(score, chunk)| QuoteMatch {
            chunk_id: chunk.id.clone(),
            chapter_index: chunk.chapter_index,
            chapter_title: chunk.chapter_title.clone(),
            matched_text: chunk.text.clone(),
            start_offset: chunk.start_offset,
            end_offset: chunk.end_offset,
            score: score as f32,
        })
    }
}

/// Normalize text for comparison (lowercase, collapse whitespace)
/// Normalize text for comparison (lowercase, handle quotes, collapse whitespace)
fn normalize_text(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
        .collect::<String>()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Escape special query characters for Tantivy
fn escape_query(query: &str) -> String {
    let special_chars = ['+', '-', '&', '|', '!', '(', ')', '{', '}', '[', ']', '^', '"', '~', '*', '?', ':', '\\', '/'];
    let mut escaped = String::with_capacity(query.len() * 2);
    
    for c in query.chars() {
        if special_chars.contains(&c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_normalize_text() {
        let text = "  Hello,   World! \"Test\"  ";
        assert_eq!(normalize_text(text), "hello world test");
    }
    
    #[test]
    fn test_create_index() {
        let chunks = vec![
            BookChunk {
                id: "chunk_0".to_string(),
                chapter_index: 0,
                chapter_title: "Chapter 1".to_string(),
                chunk_index: 0,
                text: "It was the best of times, it was the worst of times.".to_string(),
                start_offset: 0,
                end_offset: 52,
            }
        ];
        
        let index = BookSearchIndex::new(&chunks);
        assert!(index.is_ok());
    }
}
