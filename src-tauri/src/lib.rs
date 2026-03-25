mod epub_parser;
mod search;
mod embeddings;
mod database;

use epub_parser::{parse_epub, ParsedBook, BookChunk};
use search::BookSearchIndex;
use embeddings::{EmbeddingStore, EmbeddedChunk, embed_chunks, get_embedding};
use database::{get_db_path, init_db, book_exists_by_hash, save_book, load_book as db_load_book, 
               save_embeddings, load_embeddings, has_embeddings, save_reading_state, load_reading_state,
               save_api_key as db_save_api_key, load_api_key, save_model as db_save_model, load_model,
               load_all_books, delete_book_by_id};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{State, AppHandle, Emitter, Manager, Window};
use tauri::menu::{Menu, MenuItemBuilder, ContextMenu};
use futures_util::StreamExt;

// Data structures exposed to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Book {
    pub id: String,
    pub title: String,
    pub author: String,
    pub filepath: String,
    pub content_hash: String,
    pub chapters: Vec<Chapter>,
    pub total_characters: usize,
    pub chunk_count: usize,
    pub from_cache: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    pub index: usize,
    pub title: String,
    pub start_offset: usize,
    pub end_offset: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub chapter_index: usize,
    pub char_offset: usize,
    pub percent_complete: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingState {
    pub book_id: String,
    pub high_water_mark: Position,
    pub current_context: Position,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchResult {
    pub position: Position,
    pub matched_text: String,
    pub confidence: f32,
    pub is_retrospective: bool,
    pub chapter_title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookContext {
    pub title: String,
    pub author: String,
    pub current_chapter: String,
    pub percent_complete: f32,
    pub spoiler_boundary_chapter: usize,
    pub passage_being_discussed: String,
    pub spoiler_mode_enabled: bool,
}

// Application state
pub struct AppState {
    pub current_book: Mutex<Option<ParsedBook>>,
    pub reading_state: Mutex<Option<ReadingState>>,
    pub api_key: Mutex<Option<String>>,
    pub gemini_model: Mutex<String>,
    pub search_index: Mutex<Option<BookSearchIndex>>,
    pub embedding_store: Mutex<EmbeddingStore>,
    pub embeddings_ready: Mutex<bool>,
    pub db_conn: Mutex<Connection>,
}

// Convert ParsedBook to frontend Book
fn to_frontend_book(parsed: &ParsedBook, from_cache: bool) -> Book {
    Book {
        id: parsed.id.clone(),
        title: parsed.title.clone(),
        author: parsed.author.clone(),
        filepath: parsed.filepath.clone(),
        content_hash: parsed.content_hash.clone(),
        chapters: parsed.chapters.iter().map(|c| Chapter {
            index: c.index,
            title: c.title.clone(),
            start_offset: c.start_offset,
            end_offset: c.end_offset,
        }).collect(),
        total_characters: parsed.total_characters,
        chunk_count: parsed.chunks.len(),
        from_cache,
    }
}

// Tauri commands
#[tauri::command]
fn load_book(filepath: String, state: State<AppState>) -> Result<Book, String> {
    // First compute content hash to check cache
    let content_hash = format!("{:x}", md5::compute(filepath.as_bytes()));
    
    let conn = state.db_conn.lock().unwrap();
    
    // Check if we already have this book
    if let Some(existing_book_id) = book_exists_by_hash(&conn, &content_hash) {
        println!("Found cached book: {}", existing_book_id);
        
        // Load from database
        let parsed_book = db_load_book(&conn, &existing_book_id)?;
        
        println!("Loaded from cache: {} by {}", parsed_book.title, parsed_book.author);
        println!("Chapters: {}, Chunks: {}", parsed_book.chapters.len(), parsed_book.chunks.len());
        
        // Create search index
        let search_index = BookSearchIndex::new(&parsed_book.chunks)?;
        
        // Check if embeddings exist
        let embeddings_exist = has_embeddings(&conn, &existing_book_id);
        
        // Load embeddings if they exist
        if embeddings_exist {
            let embeddings = load_embeddings(&conn, &existing_book_id)?;
            println!("Loaded {} embeddings from cache", embeddings.len());
            
            let mut store = state.embedding_store.lock().unwrap();
            *store = EmbeddingStore::new();
            for emb in embeddings {
                store.add(emb);
            }
            *state.embeddings_ready.lock().unwrap() = true;
        } else {
            *state.embeddings_ready.lock().unwrap() = false;
        }
        
        let frontend_book = to_frontend_book(&parsed_book, true);
        
        // Load reading state if exists
        if let Some((hwc, hwo, hwp, cc, co, cp)) = load_reading_state(&conn, &existing_book_id) {
            let reading_state = ReadingState {
                book_id: existing_book_id.clone(),
                high_water_mark: Position {
                    chapter_index: hwc,
                    char_offset: hwo,
                    percent_complete: hwp,
                },
                current_context: Position {
                    chapter_index: cc,
                    char_offset: co,
                    percent_complete: cp,
                },
                updated_at: chrono::Utc::now().to_rfc3339(),
            };
            *state.reading_state.lock().unwrap() = Some(reading_state);
        } else {
            // Initialize reading state
            let initial_state = ReadingState {
                book_id: frontend_book.id.clone(),
                high_water_mark: Position { chapter_index: 0, char_offset: 0, percent_complete: 0.0 },
                current_context: Position { chapter_index: 0, char_offset: 0, percent_complete: 0.0 },
                updated_at: chrono::Utc::now().to_rfc3339(),
            };
            *state.reading_state.lock().unwrap() = Some(initial_state);
        }
        
        *state.current_book.lock().unwrap() = Some(parsed_book);
        *state.search_index.lock().unwrap() = Some(search_index);
        
        return Ok(frontend_book);
    }
    
    drop(conn); // Release lock before parsing
    
    // Parse the EPUB file
    let parsed_book = parse_epub(&filepath)?;
    
    println!("Loaded book: {} by {}", parsed_book.title, parsed_book.author);
    println!("Chapters: {}, Chunks: {}, Total chars: {}", 
        parsed_book.chapters.len(), 
        parsed_book.chunks.len(),
        parsed_book.total_characters
    );
    
    // Save to database
    let conn = state.db_conn.lock().unwrap();
    save_book(&conn, &parsed_book)?;
    println!("Saved book to database");
    drop(conn);
    
    // Create Tantivy search index
    let search_index = BookSearchIndex::new(&parsed_book.chunks)?;
    println!("Created search index with {} chunks", parsed_book.chunks.len());
    
    // Create frontend book
    let frontend_book = to_frontend_book(&parsed_book, false);
    
    // Store in state
    *state.current_book.lock().unwrap() = Some(parsed_book.clone());
    *state.search_index.lock().unwrap() = Some(search_index);
    *state.embeddings_ready.lock().unwrap() = false;
    
    // Initialize reading state at the beginning
    let initial_state = ReadingState {
        book_id: frontend_book.id.clone(),
        high_water_mark: Position {
            chapter_index: 0,
            char_offset: 0,
            percent_complete: 0.0,
        },
        current_context: Position {
            chapter_index: 0,
            char_offset: 0,
            percent_complete: 0.0,
        },
        updated_at: chrono::Utc::now().to_rfc3339(),
    };
    *state.reading_state.lock().unwrap() = Some(initial_state);
    
    Ok(frontend_book)
}

/// Generate embeddings for all chunks (called after book load)
#[tauri::command]
async fn generate_embeddings(state: State<'_, AppState>) -> Result<usize, String> {
    // Check if already ready (loaded from cache)
    if *state.embeddings_ready.lock().unwrap() {
        let store = state.embedding_store.lock().unwrap();
        return Ok(store.chunks().len());
    }
    
    let api_key = {
        let key_guard = state.api_key.lock().unwrap();
        key_guard.clone().ok_or("No API key configured")?
    };
    
    let (chunks, book_id) = {
        let book_guard = state.current_book.lock().unwrap();
        match &*book_guard {
            Some(book) => (book.chunks.clone(), book.id.clone()),
            None => return Err("No book loaded".to_string()),
        }
    };
    
    println!("Generating embeddings for {} chunks...", chunks.len());
    
    // Embed all chunks
    let embedded = embed_chunks(&chunks, &api_key).await?;
    let count = embedded.len();
    
    // Save to database
    {
        let conn = state.db_conn.lock().unwrap();
        save_embeddings(&conn, &book_id, &embedded)?;
        println!("Saved {} embeddings to database", count);
    }
    
    // Store embeddings in memory
    {
        let mut store = state.embedding_store.lock().unwrap();
        *store = EmbeddingStore::new();
        for chunk in embedded {
            store.add(chunk);
        }
    }
    
    *state.embeddings_ready.lock().unwrap() = true;
    println!("Generated {} embeddings", count);
    
    Ok(count)
}

#[tauri::command]
fn get_reading_state(book_id: String, state: State<AppState>) -> Option<ReadingState> {
    let reading_state = state.reading_state.lock().unwrap();
    reading_state.clone().filter(|s| s.book_id == book_id)
}

#[tauri::command]
fn update_reading_position(chapter_index: usize, char_offset: usize, state: State<AppState>) -> Result<(), String> {
    let mut reading_state = state.reading_state.lock().unwrap();
    let book_guard = state.current_book.lock().unwrap();
    
    if let (Some(rs), Some(book)) = (reading_state.as_mut(), book_guard.as_ref()) {
        let percent = char_offset as f32 / book.total_characters.max(1) as f32;
        
        // Only update high water mark if we're further along
        if char_offset > rs.high_water_mark.char_offset {
            rs.high_water_mark = Position {
                chapter_index,
                char_offset,
                percent_complete: percent,
            };
        }
        
        rs.current_context = Position {
            chapter_index,
            char_offset,
            percent_complete: percent,
        };
        rs.updated_at = chrono::Utc::now().to_rfc3339();
        
        // Persist to database
        let conn = state.db_conn.lock().unwrap();
        save_reading_state(
            &conn,
            &rs.book_id,
            rs.high_water_mark.chapter_index,
            rs.high_water_mark.char_offset,
            rs.high_water_mark.percent_complete,
            rs.current_context.chapter_index,
            rs.current_context.char_offset,
            rs.current_context.percent_complete,
        )?;
    }
    
    Ok(())
}

#[tauri::command]
fn get_book_chunks(state: State<AppState>) -> Vec<BookChunk> {
    let book_guard = state.current_book.lock().unwrap();
    match &*book_guard {
        Some(book) => book.chunks.clone(),
        None => Vec::new(),
    }
}

/// Find where a quote appears in the book (for position detection)
#[tauri::command]
fn locate_quote(quote: String, state: State<AppState>) -> Option<MatchResult> {
    println!("locate_quote called with: '{}'", &quote[..quote.len().min(100)]);
    
    let book_guard = state.current_book.lock().unwrap();
    let search_guard = state.search_index.lock().unwrap();
    let reading_guard = state.reading_state.lock().unwrap();
    
    let book = book_guard.as_ref()?;
    let search_index = search_guard.as_ref()?;
    let reading_state = reading_guard.as_ref()?;
    
    // Try exact match first, then fuzzy
    let quote_match = search_index.find_exact_quote(&quote, &book.chunks)?;
    
    // Check if this is retrospective (before current high water mark)
    let is_retrospective = quote_match.chapter_index < reading_state.high_water_mark.chapter_index
        || (quote_match.chapter_index == reading_state.high_water_mark.chapter_index 
            && quote_match.start_offset < reading_state.high_water_mark.char_offset);
    
    let percent = quote_match.start_offset as f32 / book.total_characters.max(1) as f32;
    
    println!("Match found! Chapter {}, offset {}, percent {:.2}%", 
        quote_match.chapter_index, quote_match.start_offset, percent * 100.0);
    
    Some(MatchResult {
        position: Position {
            chapter_index: quote_match.chapter_index,
            char_offset: quote_match.start_offset,
            percent_complete: percent,
        },
        matched_text: quote_match.matched_text,
        confidence: quote_match.score,
        is_retrospective,
        chapter_title: quote_match.chapter_title,
    })
}

#[tauri::command]
fn set_api_key(api_key: String, state: State<AppState>) -> Result<(), String> {
    // Save to database
    let conn = state.db_conn.lock().unwrap();
    db_save_api_key(&conn, &api_key)?;
    drop(conn);
    
    *state.api_key.lock().unwrap() = Some(api_key);
    Ok(())
}

#[tauri::command]
fn has_api_key(state: State<AppState>) -> bool {
    state.api_key.lock().unwrap().is_some()
}

#[tauri::command]
fn are_embeddings_ready(state: State<AppState>) -> bool {
    *state.embeddings_ready.lock().unwrap()
}

#[tauri::command]
fn get_gemini_model(state: State<AppState>) -> String {
    state.gemini_model.lock().unwrap().clone()
}

#[tauri::command]
fn set_gemini_model(model: String, state: State<AppState>) -> Result<(), String> {
    // Save to database
    let conn = state.db_conn.lock().unwrap();
    db_save_model(&conn, &model)?;
    drop(conn);
    
    *state.gemini_model.lock().unwrap() = model;
    Ok(())
}

/// Get the surrounding scene context for a passage (chunks before + after the matching chunk)
fn get_surrounding_context(
    passage: &str,
    state: &State<'_, AppState>,
) -> String {
    let book_guard = state.current_book.lock().unwrap();
    let book = match &*book_guard {
        Some(b) => b,
        None => return String::new(),
    };

    if passage.trim().is_empty() || book.chunks.is_empty() {
        return String::new();
    }

    // Find which chunk contains the passage (using normalized substring match)
    let passage_lower = passage.to_lowercase();
    let matched_idx = book.chunks.iter().position(|chunk| {
        chunk.text.to_lowercase().contains(&passage_lower)
    });

    // If exact match fails, try word overlap
    let matched_idx = matched_idx.or_else(|| {
        let passage_words: Vec<&str> = passage_lower.split_whitespace()
            .filter(|w| w.len() > 3)
            .collect();
        if passage_words.is_empty() { return None; }

        let mut best: Option<(usize, f32)> = None;
        for (i, chunk) in book.chunks.iter().enumerate() {
            let chunk_lower = chunk.text.to_lowercase();
            let overlap = passage_words.iter()
                .filter(|w| chunk_lower.contains(*w))
                .count() as f32 / passage_words.len() as f32;
            if overlap > 0.6 {
                if best.is_none() || overlap > best.unwrap().1 {
                    best = Some((i, overlap));
                }
            }
        }
        best.map(|(i, _)| i)
    });

    let matched_idx = match matched_idx {
        Some(i) => i,
        None => return String::new(),
    };

    // Gather surrounding chunks: 1 before, the match, 1 after
    let mut scene_parts: Vec<String> = Vec::new();

    // Previous chunk (same chapter only for coherence)
    if matched_idx > 0 {
        let prev = &book.chunks[matched_idx - 1];
        if prev.chapter_index == book.chunks[matched_idx].chapter_index {
            scene_parts.push(prev.text.clone());
        }
    }

    // The matched chunk itself
    scene_parts.push(book.chunks[matched_idx].text.clone());

    // Next chunk (same chapter only)
    if matched_idx + 1 < book.chunks.len() {
        let next = &book.chunks[matched_idx + 1];
        if next.chapter_index == book.chunks[matched_idx].chapter_index {
            scene_parts.push(next.text.clone());
        }
    }

    let chapter_title = &book.chunks[matched_idx].chapter_title;
    format!("SCENE CONTEXT (from \"{}\"):\n\"\"\"\n{}\n\"\"\"", chapter_title, scene_parts.join("\n"))
}

/// Get relevant chunks using semantic search
async fn get_semantic_context(
    query: &str,
    max_chapter: usize,
    top_k: usize,
    total_characters: usize,
    state: &State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let api_key = {
        let key_guard = state.api_key.lock().unwrap();
        key_guard.clone().ok_or("No API key")?
    };
    
    let embeddings_ready = *state.embeddings_ready.lock().unwrap();
    
    if !embeddings_ready {
        // Fall back to keyword search
        return Ok(get_keyword_context(query, max_chapter, top_k, total_characters, state));
    }
    
    // Get query embedding
    let query_embedding = get_embedding(query, &api_key).await?;
    
    // Find similar chunks
    let store = state.embedding_store.lock().unwrap();
    let similar = store.find_similar(&query_embedding, max_chapter, top_k);
    
    Ok(similar.iter()
        .map(|c| {
            let percent = if total_characters > 0 {
                (c.start_offset as f32 / total_characters as f32 * 100.0) as u32
            } else { 0 };
            format!("PASSAGE ({}, {}% through book):\n\"{}\"", c.chapter_title, percent, c.text)
        })
        .collect())
}

/// Fallback keyword-based context retrieval
fn get_keyword_context(query: &str, max_chapter: usize, top_k: usize, total_characters: usize, state: &State<'_, AppState>) -> Vec<String> {
    let book_guard = state.current_book.lock().unwrap();
    let book = match &*book_guard {
        Some(b) => b,
        None => return Vec::new(),
    };
    
    let allowed_chunks: Vec<&BookChunk> = book.chunks.iter()
        .filter(|c| c.chapter_index <= max_chapter)
        .collect();
    
    let query_lower = query.to_lowercase();
    let query_terms: Vec<&str> = query_lower
        .split_whitespace()
        .filter(|w| w.len() > 3)
        .collect();
    
    let mut scored: Vec<(f32, &BookChunk)> = allowed_chunks.iter()
        .map(|chunk| {
            let chunk_lower = chunk.text.to_lowercase();
            let mut score = 0.0;
            for term in &query_terms {
                if chunk_lower.contains(term) {
                    score += 1.0;
                }
            }
            (score, *chunk)
        })
        .filter(|(score, _)| *score > 0.0)
        .collect();
    
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    
    scored.into_iter()
        .take(top_k)
        .map(|(_, chunk)| {
            let percent = if total_characters > 0 {
                (chunk.start_offset as f32 / total_characters as f32 * 100.0) as u32
            } else { 0 };
            format!("PASSAGE ({}, {}% through book):\n\"{}\"", chunk.chapter_title, percent, chunk.text)
        })
        .collect()
}

/// Build the system prompt for the AI, shared between streaming and non-streaming endpoints
fn build_system_prompt(book_context: &BookContext, context_section: &str, scene_section: &str) -> String {
    let progress_pct = (book_context.percent_complete * 100.0) as u32;
    
    if book_context.spoiler_mode_enabled {
        format!(
            r#"You are a literary analysis assistant for the book "{title}" by {author}.

SPOILER RULES (CRITICAL):
- The reader is at: {chapter} ({pct}% through the book)
- NEVER reveal plot points, character developments, twists, or events AFTER this point
- If asked about something beyond their position, say you can't discuss it yet

READING POSITION:
- Book: "{title}" by {author}
- Current Chapter: {chapter}
- Progress: {pct}%

ANALYSIS GUIDELINES:
1. **Speaker Identification**: DO NOT GUESS. For identifying speakers, ONLY use the SCENE CONTEXT section (the text immediately surrounding the quote) and dialogue tags within it. Do NOT use the RELEVANT PASSAGES section for speaker identification — those come from other parts of the book and will mislead you. Only confirm a speaker if you can point to a specific cue in the scene context. If the scene context does not make the speaker clear, say "The speaker is not explicitly identified in this passage." Getting the speaker wrong is worse than not identifying them.
2. **Concrete Thematic Analysis**: Name specific themes explicitly (e.g., "This connects to the theme of isolation that {author} has been building since..."). Do NOT make vague statements like "this is interesting" — explain WHY it matters to the story.
3. **Supporting Quotes**: When making a claim about the text, support it with a direct quote from the provided passages. Format as: As [Character/narrator] says in [Chapter]: "..."
4. **Connections**: Draw connections between the current passage and earlier events, character arcs, or motifs that have appeared SO FAR in the book. Be specific about where those connections appear.
5. **Literary Technique**: When relevant, identify the literary technique being used (metaphor, foreshadowing, irony, unreliable narration, etc.) and explain its effect.
{scene}{context}

Keep responses focused and substantive. Avoid filler. Every sentence should add analytical value."#,
            title = book_context.title,
            author = book_context.author,
            chapter = book_context.current_chapter,
            pct = progress_pct,
            scene = scene_section,
            context = context_section,
        )
    } else {
        format!(
            r#"You are a literary analysis assistant for the book "{title}" by {author}.

SPOILER MODE: OFF — The reader has read the book before or doesn't mind spoilers.
- You may discuss ANY part of the book freely, including the ending
- The reader is currently at: {chapter} ({pct}% through) — use this to contextualize your analysis
- When referencing events, note whether they are before or after the reader's current position

READING POSITION:
- Book: "{title}" by {author}
- Current Position: {chapter} ({pct}%)

ANALYSIS GUIDELINES:
1. **Speaker Identification**: DO NOT GUESS. For identifying speakers, ONLY use the SCENE CONTEXT section (the text immediately surrounding the quote) and dialogue tags within it. Do NOT use the RELEVANT PASSAGES section for speaker identification — those come from other parts of the book and will mislead you. Only confirm a speaker if you can point to a specific cue in the scene context. If the scene context does not make the speaker clear, say "The speaker is not explicitly identified in this passage." Getting the speaker wrong is worse than not identifying them.
2. **Concrete Thematic Analysis**: Name specific themes explicitly (e.g., "This connects to the theme of moral ambiguity that runs throughout the novel..."). Do NOT make vague statements. Explain WHY the passage matters to the work as a whole.
3. **Supporting Quotes**: Back up your claims with direct quotes from the provided passages. Format as: As [Character/narrator] says in [Chapter]: "..."
4. **Full Arc Connections**: Connect the passage to the complete arc of the book — foreshadowing, callbacks, parallel scenes, and character development. Be specific about where those connections appear (e.g., "This mirrors the scene in [Chapter] where...").
5. **Literary Technique**: Name the literary techniques at work (symbolism, irony, motif, parallel structure, etc.) and explain their effect on the reader.
{scene}{context}

Keep responses focused and substantive. Avoid filler. Every sentence should add analytical value."#,
            title = book_context.title,
            author = book_context.author,
            chapter = book_context.current_chapter,
            pct = progress_pct,
            scene = scene_section,
            context = context_section,
        )
    }
}

#[tauri::command]
async fn send_chat_message(
    messages: Vec<ChatMessage>,
    book_context: BookContext,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // Get API key
    let api_key = {
        let key_guard = state.api_key.lock().unwrap();
        key_guard.clone().ok_or("No API key configured. Please add your Gemini API key in Settings.")?
    };

    // Get total characters for position calculation
    let total_characters = {
        let book_guard = state.current_book.lock().unwrap();
        book_guard.as_ref().map(|b| b.total_characters).unwrap_or(0)
    };

    // Get relevant context using semantic search if available
    let last_message = messages.last().map(|m| m.content.as_str()).unwrap_or("");
    let relevant_context = get_semantic_context(
        last_message,
        book_context.spoiler_boundary_chapter,
        5,
        total_characters,
        &state,
    ).await.unwrap_or_else(|_| Vec::new());

    // Get scene context around the passage being discussed
    let scene_context = get_surrounding_context(
        &book_context.passage_being_discussed,
        &state,
    );

    let context_section = if !relevant_context.is_empty() {
        format!(
            "\n\nRELEVANT PASSAGES FROM THE BOOK:\n{}",
            relevant_context.join("\n\n")
        )
    } else {
        String::new()
    };

    let scene_section = if !scene_context.is_empty() {
        format!("\n\n{}", scene_context)
    } else {
        String::new()
    };

    // Build system prompt with book context and retrieved passages
    let system_prompt = build_system_prompt(&book_context, &context_section, &scene_section);

    // Build conversation for Gemini API
    let mut gemini_contents: Vec<serde_json::Value> = vec![];
    
    gemini_contents.push(serde_json::json!({
        "role": "user",
        "parts": [{"text": system_prompt}]
    }));
    gemini_contents.push(serde_json::json!({
        "role": "model",
        "parts": [{"text": "Understood. I'm ready to help you analyze this book with concrete, thematic analysis grounded in the text. What would you like to explore?"}]
    }));

    // Add conversation history
    for msg in messages.iter() {
        gemini_contents.push(serde_json::json!({
            "role": if msg.role == "user" { "user" } else { "model" },
            "parts": [{"text": msg.content}]
        }));
    }

    // Get selected model
    let model = state.gemini_model.lock().unwrap().clone();

    // Make API request with retry logic for 429s
    let client = reqwest::Client::new();
    let mut last_error = String::new();
    
    for attempt in 0..3 {
        let response = client
            .post(format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
                model, api_key
            ))
            .json(&serde_json::json!({
                "contents": gemini_contents,
                "generationConfig": {
                    "temperature": 0.7,
                    "maxOutputTokens": 4096,
                }
            }))
            .send()
            .await
            .map_err(|e| format!("Failed to connect to Gemini API: {}", e))?;

        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let wait_secs = 2u64.pow(attempt);
            println!("Rate limited on chat message, waiting {}s...", wait_secs);
            tokio::time::sleep(tokio::time::Duration::from_secs(wait_secs)).await;
            last_error = "Resource exhausted (429). You might be over your Gemini API quota.".to_string();
            continue;
        }

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("Gemini API error: {}", error_text));
        }

        let response_json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Gemini response: {}", e))?;

        let text = response_json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .unwrap_or("I couldn't generate a response. Please try again.")
            .to_string();

        return Ok(text);
    }

    Err(format!("Gemini API error: {}", last_error))
}

/// Streaming chat message - emits chunks via events
#[tauri::command]
async fn send_chat_message_stream(
    messages: Vec<ChatMessage>,
    book_context: BookContext,
    session_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Get API key
    let api_key = {
        let key_guard = state.api_key.lock().unwrap();
        key_guard.clone().ok_or("No API key configured. Please add your Gemini API key in Settings.")?
    };

    // Get total characters for position calculation
    let total_characters = {
        let book_guard = state.current_book.lock().unwrap();
        book_guard.as_ref().map(|b| b.total_characters).unwrap_or(0)
    };

    // Get relevant context using semantic search if available
    let last_message = messages.last().map(|m| m.content.as_str()).unwrap_or("");
    let relevant_context = get_semantic_context(
        last_message,
        book_context.spoiler_boundary_chapter,
        5,
        total_characters,
        &state,
    ).await.unwrap_or_else(|_| Vec::new());

    // Get scene context around the passage being discussed
    let scene_context = get_surrounding_context(
        &book_context.passage_being_discussed,
        &state,
    );

    let context_section = if !relevant_context.is_empty() {
        format!(
            "\n\nRELEVANT PASSAGES FROM THE BOOK:\n{}",
            relevant_context.join("\n\n")
        )
    } else {
        String::new()
    };

    let scene_section = if !scene_context.is_empty() {
        format!("\n\n{}", scene_context)
    } else {
        String::new()
    };

    // Build system prompt with book context and retrieved passages
    let system_prompt = build_system_prompt(&book_context, &context_section, &scene_section);

    // Build conversation for Gemini API
    let mut gemini_contents: Vec<serde_json::Value> = vec![];
    
    gemini_contents.push(serde_json::json!({
        "role": "user",
        "parts": [{"text": system_prompt}]
    }));
    gemini_contents.push(serde_json::json!({
        "role": "model",
        "parts": [{"text": "Understood. I'm ready to help you analyze this book with concrete, thematic analysis grounded in the text. What would you like to explore?"}]
    }));

    // Add conversation history
    for msg in messages.iter() {
        gemini_contents.push(serde_json::json!({
            "role": if msg.role == "user" { "user" } else { "model" },
            "parts": [{"text": msg.content}]
        }));
    }

    // Get selected model
    let model = state.gemini_model.lock().unwrap().clone();
    println!("Starting streaming request with model: {}", model);

    // Make streaming API request (no alt=sse, returns JSON chunks)
    let client = reqwest::Client::new();
    
    let response = client
        .post(format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?key={}",
            model, api_key
        ))
        .json(&serde_json::json!({
            "contents": gemini_contents,
            "generationConfig": {
                "temperature": 0.7,
                "maxOutputTokens": 4096,
            }
        }))
        .send()
        .await
        .map_err(|e| format!("Failed to connect to Gemini API: {}", e))?;

    println!("Response status: {}", response.status());

    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        let _ = app.emit(&format!("chat-stream-error:{}", session_id), "Rate limited");
        return Err("Resource exhausted (429). You might be over your Gemini API quota.".to_string());
    }

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        println!("API error: {}", error_text);
        let _ = app.emit(&format!("chat-stream-error:{}", session_id), &error_text);
        return Err(format!("Gemini API error: {}", error_text));
    }

    // Stream the response - Gemini returns JSON objects as chunks
    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new(); // Use a byte buffer to handle potential UTF-8 splits
    
    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                // Append new bytes to our buffer
                buffer.extend_from_slice(&chunk);
                
                // Process the buffer, looking for complete JSON objects (balanced braces)
                let mut processed_len = 0;
                let mut brace_count = 0;
                let mut in_string = false;
                let mut escape_next = false;
                let mut object_start: Option<usize> = None;
                
                // Iterate through the buffer to find boundaries
                for (i, &byte) in buffer.iter().enumerate() {
                    if escape_next {
                        escape_next = false;
                        continue;
                    }
                    
                    if byte == b'\\' && in_string {
                        escape_next = true;
                        continue;
                    }
                    
                    if byte == b'"' {
                        in_string = !in_string;
                        continue;
                    }
                    
                    if in_string {
                        continue;
                    }
                    
                    if byte == b'{' {
                        if brace_count == 0 {
                            object_start = Some(i);
                        }
                        brace_count += 1;
                    } else if byte == b'}' {
                        brace_count -= 1;
                        
                        if brace_count == 0 && object_start.is_some() {
                            // We have a complete JSON object!
                            let start = object_start.unwrap();
                            let end = i + 1; // inclusive
                            
                            // Try to parse this slice as JSON
                            if let Ok(json_str) = std::str::from_utf8(&buffer[start..end]) {
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
                                    if let Some(text) = json["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                                        // println!("Streaming chunk ({} bytes): {}", end - start, text);
                                        let _ = app.emit(&format!("chat-stream:{}", session_id), text);
                                    }
                                } else {
                                    println!("Failed to parse JSON chunk: {}", json_str);
                                }
                            } else {
                                println!("Invalid UTF-8 in JSON chunk range");
                            }
                            
                            // Reset for next object
                            object_start = None;
                            processed_len = end;
                        }
                    }
                }
                
                // Remove processed bytes from the buffer
                if processed_len > 0 {
                    buffer.drain(0..processed_len);
                }
            }
            Err(e) => {
                println!("Stream error: {}", e);
                let _ = app.emit(&format!("chat-stream-error:{}", session_id), e.to_string());
                return Err(e.to_string());
            }
        }
    }
    
    println!("Stream completed for session: {}", session_id);
    let _ = app.emit(&format!("chat-stream-end:{}", session_id), ());
    
    Ok(())
}

/// Response type for cached books list
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CachedBook {
    id: String,
    title: String,
    author: String,
    total_chapters: usize,
    reading_progress: f32,
    has_embeddings: bool,
    cover_image_url: Option<String>,
}

/// Get list of all cached books
#[tauri::command]
fn get_cached_books(state: State<'_, AppState>) -> Result<Vec<CachedBook>, String> {
    let conn = state.db_conn.lock().unwrap();
    let books = load_all_books(&conn)?;
    
    Ok(books.into_iter().map(|b| CachedBook {
        id: b.id,
        title: b.title,
        author: b.author,
        total_chapters: b.total_chapters,
        reading_progress: b.reading_progress,
        has_embeddings: b.has_embeddings,
        cover_image_url: b.cover_image_url,
    }).collect())
}

/// Delete a book by ID
#[tauri::command]
fn delete_book(book_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db_conn.lock().unwrap();
    
    // Check if we're deleting the currently open book
    {
        let mut current_book = state.current_book.lock().unwrap();
        if let Some(book) = &*current_book {
            if book.id == book_id {
                // Clear current book state
                *current_book = None;
                *state.reading_state.lock().unwrap() = None;
                *state.search_index.lock().unwrap() = None;
                *state.embedding_store.lock().unwrap() = EmbeddingStore::new();
                *state.embeddings_ready.lock().unwrap() = false;
            }
        }
    }
    
    delete_book_by_id(&conn, &book_id)?;
    Ok(())
}

/// Show native context menu for a book
#[tauri::command]
fn show_book_context_menu(app: AppHandle, window: Window, book_id: String) -> Result<(), String> {
    let delete_id = format!("delete:{}", book_id);
    let delete_item = MenuItemBuilder::new("Delete Book")
        .id(tauri::menu::MenuId::new(&delete_id))
        .build(&app)
        .map_err(|e| e.to_string())?;
        
    let menu = Menu::new(&app).map_err(|e| e.to_string())?;
    menu.append(&delete_item).map_err(|e| e.to_string())?;
    
    // Popup at current cursor position
    // None means at cursor
    menu.popup(window).map_err(|e| e.to_string())?;
    
    Ok(())
}

/// Open a cached book by ID
#[tauri::command]
async fn open_cached_book(book_id: String, state: State<'_, AppState>) -> Result<Book, String> {
    let conn = state.db_conn.lock().unwrap();
    
    // Load book from database
    let parsed = db_load_book(&conn, &book_id)?;
    println!("Opening cached book: {} by {}", parsed.title, parsed.author);
    
    // Create search index
    let search_index = BookSearchIndex::new(&parsed.chunks)?;
    println!("Recreated search index with {} chunks", parsed.chunks.len());
    
    // Convert to frontend Book type
    let book = Book {
        id: parsed.id.clone(),
        title: parsed.title.clone(),
        author: parsed.author.clone(),
        filepath: parsed.filepath.clone(),
        content_hash: parsed.content_hash.clone(),
        chapters: parsed.chapters.iter().map(|c| Chapter {
            index: c.index,
            title: c.title.clone(),
            start_offset: c.start_offset,
            end_offset: c.end_offset,
        }).collect(),
        total_characters: parsed.total_characters,
        chunk_count: parsed.chunks.len(),
        from_cache: true,
    };
    
    // Load reading state if exists
    let reading_state = load_reading_state(&conn, &book_id);
    
    // Update app state
    {
        let mut current_book = state.current_book.lock().unwrap();
        *current_book = Some(parsed.clone());
    }
    {
        let mut idx = state.search_index.lock().unwrap();
        *idx = Some(search_index);
    }
    if let Some((hw_ch, hw_off, hw_pct, cur_ch, cur_off, cur_pct)) = reading_state {
        let mut rs = state.reading_state.lock().unwrap();
        *rs = Some(crate::ReadingState {
            book_id: book_id.clone(),
            high_water_mark: crate::Position {
                chapter_index: hw_ch,
                char_offset: hw_off,
                percent_complete: hw_pct,
            },
            current_context: crate::Position {
                chapter_index: cur_ch,
                char_offset: cur_off,
                percent_complete: cur_pct,
            },
            updated_at: chrono::Utc::now().to_rfc3339(),
        });
    }
    
    // Load embeddings if they exist
    drop(conn); // Release lock before async work
    
    if has_embeddings(&state.db_conn.lock().unwrap(), &book_id) {
        let embeddings = load_embeddings(&state.db_conn.lock().unwrap(), &book_id)?;
        println!("Loaded {} embeddings from cache", embeddings.len());
        
        let mut store = state.embedding_store.lock().unwrap();
        *store = EmbeddingStore::from_embeddings(embeddings);
        
        let mut ready = state.embeddings_ready.lock().unwrap();
        *ready = true;
    }
    
    Ok(book)
}


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize database
    let db_path = get_db_path();
    println!("Database path: {:?}", db_path);
    
    let conn = Connection::open(&db_path).expect("Failed to open database");
    init_db(&conn).expect("Failed to initialize database schema");
    
    // Load saved API key if it exists
    let saved_api_key = load_api_key(&conn);
    if saved_api_key.is_some() {
        println!("Loaded API key from database");
    }
    
    // Load saved model preference
    let saved_model = load_model(&conn);
    println!("Using model: {}", saved_model);
    
    let app_state = AppState {
        current_book: Mutex::new(None),
        reading_state: Mutex::new(None),
        api_key: Mutex::new(saved_api_key),
        gemini_model: Mutex::new(saved_model),
        search_index: Mutex::new(None),
        embedding_store: Mutex::new(EmbeddingStore::new()),
        embeddings_ready: Mutex::new(false),
        db_conn: Mutex::new(conn),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .on_menu_event(|app, event| {
            if event.id().as_ref().starts_with("delete:") {
                let book_id = event.id().as_ref().trim_start_matches("delete:");
                // Emit event to frontend to request confirmation
                let _ = app.emit("request-delete-book", book_id);
            }
        })
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            load_book,
            generate_embeddings,
            get_reading_state,
            update_reading_position,
            get_book_chunks,
            locate_quote,
            set_api_key,
            has_api_key,
            are_embeddings_ready,
            send_chat_message,
            send_chat_message_stream,
            get_gemini_model,
            set_gemini_model,
            get_cached_books,
            open_cached_book,
            delete_book,
            show_book_context_menu,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
