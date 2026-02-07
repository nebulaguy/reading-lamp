use rusqlite::{Connection, params};
use std::path::PathBuf;
use crate::epub_parser::{BookChunk, ParsedBook, ParsedChapter};
use crate::embeddings::{EmbeddedChunk, Embedding};

/// Get the database path in the app data directory
pub fn get_db_path() -> PathBuf {
    let mut path = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("codex");
    std::fs::create_dir_all(&path).ok();
    path.push("codex.db");
    path
}

/// Initialize the database schema
pub fn init_db(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS books (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            author TEXT NOT NULL,
            filepath TEXT NOT NULL,
            content_hash TEXT NOT NULL UNIQUE,
            total_characters INTEGER NOT NULL,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS chapters (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            book_id TEXT NOT NULL,
            chapter_index INTEGER NOT NULL,
            title TEXT NOT NULL,
            start_offset INTEGER NOT NULL,
            end_offset INTEGER NOT NULL,
            FOREIGN KEY (book_id) REFERENCES books(id)
        );

        CREATE TABLE IF NOT EXISTS chunks (
            id TEXT PRIMARY KEY,
            book_id TEXT NOT NULL,
            chapter_index INTEGER NOT NULL,
            chapter_title TEXT NOT NULL,
            chunk_index INTEGER NOT NULL,
            text TEXT NOT NULL,
            start_offset INTEGER NOT NULL,
            end_offset INTEGER NOT NULL,
            FOREIGN KEY (book_id) REFERENCES books(id)
        );

        CREATE TABLE IF NOT EXISTS embeddings (
            chunk_id TEXT PRIMARY KEY,
            book_id TEXT NOT NULL,
            embedding BLOB NOT NULL,
            FOREIGN KEY (chunk_id) REFERENCES chunks(id),
            FOREIGN KEY (book_id) REFERENCES books(id)
        );

        CREATE TABLE IF NOT EXISTS reading_state (
            book_id TEXT PRIMARY KEY,
            high_water_chapter INTEGER NOT NULL,
            high_water_offset INTEGER NOT NULL,
            high_water_percent REAL NOT NULL,
            current_chapter INTEGER NOT NULL,
            current_offset INTEGER NOT NULL,
            current_percent REAL NOT NULL,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (book_id) REFERENCES books(id)
        );

        CREATE INDEX IF NOT EXISTS idx_chunks_book ON chunks(book_id);
        CREATE INDEX IF NOT EXISTS idx_embeddings_book ON embeddings(book_id);

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        "
    ).map_err(|e| format!("Failed to initialize database: {}", e))?;
    
    Ok(())
}

/// Save API key to database
pub fn save_api_key(conn: &Connection, api_key: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES ('api_key', ?1)",
        params![api_key],
    ).map_err(|e| format!("Failed to save API key: {}", e))?;
    Ok(())
}

/// Load API key from database
pub fn load_api_key(conn: &Connection) -> Option<String> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = 'api_key'",
        [],
        |row| row.get(0)
    ).ok()
}

/// Save selected model to database
pub fn save_model(conn: &Connection, model: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES ('gemini_model', ?1)",
        params![model],
    ).map_err(|e| format!("Failed to save model: {}", e))?;
    Ok(())
}

/// Load selected model from database (defaults to gemini-2.0-flash)
pub fn load_model(conn: &Connection) -> String {
    conn.query_row(
        "SELECT value FROM settings WHERE key = 'gemini_model'",
        [],
        |row| row.get(0)
    ).unwrap_or_else(|_| "gemini-2.0-flash".to_string())
}

/// Check if a book exists by content hash
pub fn book_exists_by_hash(conn: &Connection, content_hash: &str) -> Option<String> {
    conn.query_row(
        "SELECT id FROM books WHERE content_hash = ?1",
        params![content_hash],
        |row| row.get(0)
    ).ok()
}

/// Save a parsed book to the database
pub fn save_book(conn: &Connection, book: &ParsedBook) -> Result<(), String> {
    // Start transaction to ensure atomicity and speed
    let mut stmt = conn.prepare("BEGIN TRANSACTION").map_err(|e| format!("Failed to start transaction: {}", e))?;
    stmt.execute([]).map_err(|e| format!("Failed to execute BEGIN: {}", e))?;

    // We use a closure to handle the operations so we can rollback on error
    let result = (|| -> Result<(), String> {
        conn.execute(
            "INSERT OR REPLACE INTO books (id, title, author, filepath, content_hash, total_characters) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![book.id, book.title, book.author, book.filepath, book.content_hash, book.total_characters as i64],
        ).map_err(|e| format!("Failed to save book: {}", e))?;
        
        // Save chapters
        for chapter in &book.chapters {
            conn.execute(
                "INSERT OR REPLACE INTO chapters (book_id, chapter_index, title, start_offset, end_offset)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![book.id, chapter.index as i64, chapter.title, chapter.start_offset as i64, chapter.end_offset as i64],
            ).map_err(|e| format!("Failed to save chapter: {}", e))?;
        }
        
        // Save chunks
        for chunk in &book.chunks {
            conn.execute(
                "INSERT OR REPLACE INTO chunks (id, book_id, chapter_index, chapter_title, chunk_index, text, start_offset, end_offset)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    chunk.id, book.id, chunk.chapter_index as i64, chunk.chapter_title,
                    chunk.chunk_index as i64, chunk.text, chunk.start_offset as i64, chunk.end_offset as i64
                ],
            ).map_err(|e| format!("Failed to save chunk: {}", e))?;
        }
        
        Ok(())
    })();

    match result {
        Ok(_) => {
            conn.execute("COMMIT", []).map_err(|e| format!("Failed to commit transaction: {}", e))?;
            Ok(())
        },
        Err(e) => {
            let _ = conn.execute("ROLLBACK", []);
            Err(e)
        }
    }
}


/// Delete a book and all its associated data from the database
pub fn delete_book_by_id(conn: &Connection, book_id: &str) -> Result<(), String> {
    // Start transaction
    let mut stmt = conn.prepare("BEGIN TRANSACTION").map_err(|e| format!("Failed to start transaction: {}", e))?;
    stmt.execute([]).map_err(|e| format!("Failed to execute BEGIN: {}", e))?;
    
    let result = (|| -> Result<(), String> {
        // Delete embeddings
        conn.execute(
            "DELETE FROM embeddings WHERE book_id = ?1",
            params![book_id],
        ).map_err(|e| format!("Failed to delete embeddings: {}", e))?;
        
        // Delete chunks
        conn.execute(
            "DELETE FROM chunks WHERE book_id = ?1",
            params![book_id],
        ).map_err(|e| format!("Failed to delete chunks: {}", e))?;
        
        // Delete chapters
        conn.execute(
            "DELETE FROM chapters WHERE book_id = ?1",
            params![book_id],
        ).map_err(|e| format!("Failed to delete chapters: {}", e))?;
        
        // Delete reading state
        conn.execute(
            "DELETE FROM reading_state WHERE book_id = ?1",
            params![book_id],
        ).map_err(|e| format!("Failed to delete reading state: {}", e))?;
        
        // Finally delete the book
        conn.execute(
            "DELETE FROM books WHERE id = ?1",
            params![book_id],
        ).map_err(|e| format!("Failed to delete book: {}", e))?;
        
        Ok(())
    })();
    
    match result {
        Ok(_) => {
            conn.execute("COMMIT", []).map_err(|e| format!("Failed to commit transaction: {}", e))?;
            Ok(())
        },
        Err(e) => {
            let _ = conn.execute("ROLLBACK", []);
            Err(e)
        }
    }
}

/// Load a book from the database by ID
pub fn load_book(conn: &Connection, book_id: &str) -> Result<ParsedBook, String> {
    let (id, title, author, filepath, content_hash, total_characters): (String, String, String, String, String, i64) = conn.query_row(
        "SELECT id, title, author, filepath, content_hash, total_characters FROM books WHERE id = ?1",
        params![book_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
    ).map_err(|e| format!("Failed to load book: {}", e))?;
    
    // Load chapters
    let mut stmt = conn.prepare(
        "SELECT chapter_index, title, start_offset, end_offset FROM chapters WHERE book_id = ?1 ORDER BY chapter_index"
    ).map_err(|e| format!("Failed to prepare chapters query: {}", e))?;
    
    let chapters: Vec<ParsedChapter> = stmt.query_map(params![book_id], |row| {
        Ok(ParsedChapter {
            index: row.get::<_, i64>(0)? as usize,
            title: row.get(1)?,
            content: String::new(), // Don't load full content
            start_offset: row.get::<_, i64>(2)? as usize,
            end_offset: row.get::<_, i64>(3)? as usize,
        })
    }).map_err(|e| format!("Failed to query chapters: {}", e))?
    .filter_map(|r| r.ok())
    .collect();
    
    // Load chunks
    let mut stmt = conn.prepare(
        "SELECT id, chapter_index, chapter_title, chunk_index, text, start_offset, end_offset 
         FROM chunks WHERE book_id = ?1 ORDER BY chunk_index"
    ).map_err(|e| format!("Failed to prepare chunks query: {}", e))?;
    
    let chunks: Vec<BookChunk> = stmt.query_map(params![book_id], |row| {
        Ok(BookChunk {
            id: row.get(0)?,
            chapter_index: row.get::<_, i64>(1)? as usize,
            chapter_title: row.get(2)?,
            chunk_index: row.get::<_, i64>(3)? as usize,
            text: row.get(4)?,
            start_offset: row.get::<_, i64>(5)? as usize,
            end_offset: row.get::<_, i64>(6)? as usize,
        })
    }).map_err(|e| format!("Failed to query chunks: {}", e))?
    .filter_map(|r| r.ok())
    .collect();
    
    Ok(ParsedBook {
        id,
        title,
        author,
        filepath,
        content_hash,
        chapters,
        chunks,
        total_characters: total_characters as usize,
    })
}

/// Save embeddings for a book
pub fn save_embeddings(conn: &Connection, book_id: &str, embeddings: &[EmbeddedChunk]) -> Result<(), String> {
    // Start transaction
    let mut stmt = conn.prepare("BEGIN TRANSACTION").map_err(|e| format!("Failed to start transaction: {}", e))?;
    stmt.execute([]).map_err(|e| format!("Failed to execute BEGIN: {}", e))?;
    
    let result = (|| -> Result<(), String> {
        for emb in embeddings {
            // Convert f32 vec to bytes
            let bytes: Vec<u8> = emb.embedding.iter()
                .flat_map(|f| f.to_le_bytes())
                .collect();
            
            conn.execute(
                "INSERT OR REPLACE INTO embeddings (chunk_id, book_id, embedding) VALUES (?1, ?2, ?3)",
                params![emb.chunk_id, book_id, bytes],
            ).map_err(|e| format!("Failed to save embedding: {}", e))?;
        }
        Ok(())
    })();
    
    match result {
        Ok(_) => {
            conn.execute("COMMIT", []).map_err(|e| format!("Failed to commit transaction: {}", e))?;
            Ok(())
        },
        Err(e) => {
            let _ = conn.execute("ROLLBACK", []);
            Err(e)
        }
    }
}

/// Load embeddings for a book
pub fn load_embeddings(conn: &Connection, book_id: &str) -> Result<Vec<EmbeddedChunk>, String> {
    let mut stmt = conn.prepare(
        "SELECT e.chunk_id, e.embedding, c.chapter_index, c.chapter_title, c.text, c.start_offset, c.end_offset
         FROM embeddings e
         JOIN chunks c ON e.chunk_id = c.id
         WHERE e.book_id = ?1"
    ).map_err(|e| format!("Failed to prepare embeddings query: {}", e))?;
    
    let embeddings: Vec<EmbeddedChunk> = stmt.query_map(params![book_id], |row| {
        let chunk_id: String = row.get(0)?;
        let bytes: Vec<u8> = row.get(1)?;
        let chapter_index: i64 = row.get(2)?;
        let chapter_title: String = row.get(3)?;
        let text: String = row.get(4)?;
        let start_offset: i64 = row.get(5)?;
        let end_offset: i64 = row.get(6)?;
        
        // Convert bytes back to f32 vec
        let embedding: Embedding = bytes.chunks(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        
        Ok(EmbeddedChunk {
            chunk_id,
            chapter_index: chapter_index as usize,
            chapter_title,
            text,
            start_offset: start_offset as usize,
            end_offset: end_offset as usize,
            embedding,
        })
    }).map_err(|e| format!("Failed to query embeddings: {}", e))?
    .filter_map(|r| r.ok())
    .collect();
    
    Ok(embeddings)
}

/// Check if embeddings exist for a book
pub fn has_embeddings(conn: &Connection, book_id: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM embeddings WHERE book_id = ?1",
        params![book_id],
        |row| row.get::<_, i64>(0)
    ).map(|count| count > 0).unwrap_or(false)
}

/// Save reading state
pub fn save_reading_state(
    conn: &Connection,
    book_id: &str,
    high_water_chapter: usize,
    high_water_offset: usize,
    high_water_percent: f32,
    current_chapter: usize,
    current_offset: usize,
    current_percent: f32,
) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO reading_state 
         (book_id, high_water_chapter, high_water_offset, high_water_percent, current_chapter, current_offset, current_percent, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))",
        params![
            book_id,
            high_water_chapter as i64,
            high_water_offset as i64,
            high_water_percent as f64,
            current_chapter as i64,
            current_offset as i64,
            current_percent as f64
        ],
    ).map_err(|e| format!("Failed to save reading state: {}", e))?;
    Ok(())
}

/// Load reading state for a book
pub fn load_reading_state(conn: &Connection, book_id: &str) -> Option<(usize, usize, f32, usize, usize, f32)> {
    conn.query_row(
        "SELECT high_water_chapter, high_water_offset, high_water_percent, current_chapter, current_offset, current_percent
         FROM reading_state WHERE book_id = ?1",
        params![book_id],
        |row| Ok((
            row.get::<_, i64>(0)? as usize,
            row.get::<_, i64>(1)? as usize,
            row.get::<_, f64>(2)? as f32,
            row.get::<_, i64>(3)? as usize,
            row.get::<_, i64>(4)? as usize,
            row.get::<_, f64>(5)? as f32,
        ))
    ).ok()
}

/// Cached book summary for library view
#[derive(Debug, Clone)]
pub struct CachedBookSummary {
    pub id: String,
    pub title: String,
    pub author: String,
    pub total_chapters: usize,
    pub reading_progress: f32, // 0.0 to 1.0
    pub has_embeddings: bool,
}

/// Load all cached books with their reading progress
pub fn load_all_books(conn: &Connection) -> Result<Vec<CachedBookSummary>, String> {
    let mut stmt = conn.prepare(
        "SELECT b.id, b.title, b.author, 
                (SELECT COUNT(*) FROM chapters WHERE book_id = b.id) as chapter_count,
                COALESCE(rs.high_water_percent, 0) as progress,
                (SELECT COUNT(*) > 0 FROM embeddings WHERE book_id = b.id) as has_emb
         FROM books b
         LEFT JOIN reading_state rs ON b.id = rs.book_id
         ORDER BY COALESCE(rs.updated_at, b.created_at) DESC"
    ).map_err(|e| format!("Failed to prepare query: {}", e))?;
    
    let books: Vec<CachedBookSummary> = stmt.query_map([], |row| {
        Ok(CachedBookSummary {
            id: row.get(0)?,
            title: row.get(1)?,
            author: row.get(2)?,
            total_chapters: row.get::<_, i64>(3)? as usize,
            reading_progress: row.get::<_, f64>(4)? as f32,
            has_embeddings: row.get::<_, i64>(5)? > 0,
        })
    }).map_err(|e| format!("Failed to query books: {}", e))?
    .filter_map(|r| r.ok())
    .collect();
    
    Ok(books)
}

