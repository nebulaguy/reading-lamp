use epub::doc::EpubDoc;
use html2text::from_read;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;

/// A parsed chunk of book content for RAG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookChunk {
    pub id: String,
    pub chapter_index: usize,
    pub chapter_title: String,
    pub chunk_index: usize,
    pub text: String,
    pub start_offset: usize,
    pub end_offset: usize,
}

/// Parsed book with all content and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedBook {
    pub id: String,
    pub title: String,
    pub author: String,
    pub filepath: String,
    pub content_hash: String,
    pub chapters: Vec<ParsedChapter>,
    pub chunks: Vec<BookChunk>,
    pub total_characters: usize,
    pub cover_image: Option<Vec<u8>>,
    pub cover_mime: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedChapter {
    pub index: usize,
    pub title: String,
    pub content: String,
    pub start_offset: usize,
    pub end_offset: usize,
}

/// Configuration for chunking
const CHUNK_SIZE: usize = 800;      // Target characters per chunk
const CHUNK_OVERLAP: usize = 100;   // Overlap between chunks for context

/// Parse an EPUB file and extract all content
pub fn parse_epub(filepath: &str) -> Result<ParsedBook, String> {
    let path = Path::new(filepath);
    if !path.exists() {
        return Err(format!("File not found: {}", filepath));
    }

    let mut doc = EpubDoc::new(filepath).map_err(|e| format!("Failed to open EPUB: {}", e))?;

    // Extract metadata
    let title = doc.mdata("title")
        .map(|m| m.value.clone())
        .unwrap_or_else(|| "Unknown Title".to_string());
    let author = doc.mdata("creator")
        .map(|m| m.value.clone())
        .unwrap_or_else(|| "Unknown Author".to_string());

    // Generate book ID from content hash
    let content_hash = format!("{:x}", md5::compute(filepath.as_bytes()));
    let book_id = uuid::Uuid::new_v4().to_string();
    
    // Build TOC map for better chapter titles
    let mut toc_map = HashMap::new();
    for nav_point in &doc.toc {
        // nav_point.content is PathBuf in newer versions, use to_string_lossy
        let content_str = nav_point.content.to_string_lossy();
        // Remove anchor fragment if present
        let path = content_str.split('#').next().unwrap_or(&content_str).to_string();
        toc_map.insert(path, nav_point.label.clone());
    }

    // Extract all chapters
    let mut chapters: Vec<ParsedChapter> = Vec::new();
    let mut all_chunks: Vec<BookChunk> = Vec::new();
    let mut global_offset: usize = 0;
    let mut chunk_counter: usize = 0;

    // Get spine (reading order) - iterate through spine items
    let spine_len = doc.spine.len();
    
    for chapter_idx in 0..spine_len {
        let spine_id = doc.spine[chapter_idx].idref.clone();
        
        // Try to get chapter title from TOC first
        let mut chapter_title = None;
        
        // Resource lookup - handle tuple struct or named struct
        // We'll try accessing key fields assuming standard epub structure
        if let Some(resource) = doc.resources.get(&spine_id) {
            // Attempt to get path from resource struct ResourceItem { path, mime, properties }
            let path_opt = Some(&resource.path); 
            
            if let Some(path_buf) = path_opt {
                let path_str = path_buf.to_string_lossy().to_string();
                if let Some(title) = toc_map.get(&path_str) {
                    chapter_title = Some(title.clone());
                }
            }
        }
        
        if let Some((content_bytes, _mime)) = doc.get_resource(&spine_id) {
            let html_content = String::from_utf8_lossy(&content_bytes);
            let plain_text = html_to_text(&html_content);
            
            if plain_text.trim().is_empty() {
                continue;
            }

            let final_title = chapter_title.unwrap_or_else(|| {
                extract_chapter_title(&html_content)
                    .unwrap_or_else(|| format!("Chapter {}", chapter_idx + 1))
            });

            let chapter_start = global_offset;
            let chapter_end = global_offset + plain_text.len();

            let chapter = ParsedChapter {
                index: chapters.len(),
                title: final_title.clone(),
                content: plain_text.clone(),
                start_offset: chapter_start,
                end_offset: chapter_end,
            };

            let chapter_chunks = chunk_text(
                &plain_text,
                chapters.len(),
                &final_title,
                global_offset,
                &mut chunk_counter,
            );
            all_chunks.extend(chapter_chunks);

            global_offset = chapter_end;
            chapters.push(chapter);
        }
    }

    if chapters.is_empty() {
        return Err("No readable content found in EPUB".to_string());
    }

    // Extract cover image
    let (cover_image, cover_mime) = extract_cover(filepath);

    Ok(ParsedBook {
        id: book_id,
        title,
        author,
        filepath: filepath.to_string(),
        content_hash,
        chapters,
        chunks: all_chunks,
        total_characters: global_offset,
        cover_image,
        cover_mime,
    })
}

/// Extract cover image from an EPUB file
fn extract_cover(filepath: &str) -> (Option<Vec<u8>>, Option<String>) {
    let mut doc = match EpubDoc::new(filepath) {
        Ok(d) => d,
        Err(_) => return (None, None),
    };

    // Try get_cover() which returns (bytes, mime)
    if let Some((bytes, mime)) = doc.get_cover() {
        if !bytes.is_empty() {
            return (Some(bytes), Some(mime));
        }
    }

    // Fallback: look for a resource with id containing "cover" and image mime type
    let cover_ids: Vec<String> = doc.resources.keys()
        .filter(|k| k.to_lowercase().contains("cover"))
        .cloned()
        .collect();
    
    for id in cover_ids {
        if let Some(resource) = doc.resources.get(&id) {
            let mime = resource.mime.clone();
            if mime.starts_with("image/") {
                if let Some((bytes, _)) = doc.get_resource(&id) {
                    if !bytes.is_empty() {
                        return (Some(bytes), Some(mime));
                    }
                }
            }
        }
    }

    (None, None)
}

fn html_to_text(html: &str) -> String {
    let cursor = Cursor::new(html.as_bytes());
    match from_read(cursor, 80) {
        Ok(text) => {
            let lines: Vec<&str> = text
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .collect();
            lines.join("\n")
        }
        Err(_) => String::new(),
    }
}

fn extract_chapter_title(html: &str) -> Option<String> {
    let patterns = [
        (r"<h1[^>]*>([^<]+)</h1>", 1),
        (r"<h2[^>]*>([^<]+)</h2>", 1),
        (r"<title[^>]*>([^<]+)</title>", 1),
    ];

    for (pattern, group) in patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            if let Some(caps) = re.captures(html) {
                if let Some(m) = caps.get(group) {
                    let title = m.as_str().trim();
                    if !title.is_empty() && title.len() < 200 {
                        return Some(title.to_string());
                    }
                }
            }
        }
    }
    
    None
}

fn chunk_text(
    text: &str,
    chapter_index: usize,
    chapter_title: &str,
    global_offset: usize,
    chunk_counter: &mut usize,
) -> Vec<BookChunk> {
    let mut chunks = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let total_len = chars.len();
    
    if total_len == 0 {
        return chunks;
    }

    let mut start = 0;
    
    while start < total_len {
        let mut end = (start + CHUNK_SIZE).min(total_len);
        
        if end < total_len {
            end = find_sentence_boundary(&chars, start, end);
        }
        
        let chunk_text: String = chars[start..end].iter().collect();
        
        if !chunk_text.trim().is_empty() {
            chunks.push(BookChunk {
                id: format!("chunk_{}", *chunk_counter),
                chapter_index,
                chapter_title: chapter_title.to_string(),
                chunk_index: *chunk_counter,
                text: chunk_text.trim().to_string(),
                start_offset: global_offset + start,
                end_offset: global_offset + end,
            });
            *chunk_counter += 1;
        }
        
        if end >= total_len {
            break;
        }
        start = end.saturating_sub(CHUNK_OVERLAP);
        if start >= end {
            start = end;
        }
    }
    
    chunks
}

fn find_sentence_boundary(chars: &[char], start: usize, target: usize) -> usize {
    let search_start = if target > 50 { target - 50 } else { start };
    
    for i in (search_start..target).rev() {
        let c = chars[i];
        if (c == '.' || c == '!' || c == '?' || c == '\n') && i + 1 < chars.len() {
            let next = chars.get(i + 1);
            if next.is_none() || next.unwrap().is_whitespace() || next.unwrap().is_uppercase() {
                return i + 1;
            }
        }
    }
    
    target
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_text() {
        let text = "This is a test. This is another sentence. And one more.";
        let mut counter = 0;
        let chunks = chunk_text(text, 0, "Test Chapter", 0, &mut counter);
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_html_to_text_basic() {
        let html = "<html><body><h1>Title</h1><p>Hello world.</p></body></html>";
        let text = html_to_text(html);
        assert!(text.contains("Title") || text.contains("Hello"));
    }
}
