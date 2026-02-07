import { useState, useCallback, useEffect } from 'react';
import { BookLoader } from './components/BookLoader';
import { ChatInterface } from './components/ChatInterface';
import { PositionContext } from './components/PositionContext';
import { ContextPrompt } from './components/ContextPrompt';
import { Settings } from './components/Settings';
import { Library } from './components/Library';
import { Onboarding } from './components/Onboarding';
import { useToast } from './components/Toast';
import { Settings as SettingsIcon, LibraryBig } from 'lucide-react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { ask } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import type { Book, ReadingState, Message, MatchResult, AttachedQuote, CachedBook } from './lib/types';

interface TauriDragDropEvent {
  paths: string[];
  position: { x: number; y: number };
}

function App() {
  const { showToast } = useToast();
  const [book, setBook] = useState<Book | null>(null);
  const [readingState, setReadingState] = useState<ReadingState | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [pendingMatch, setPendingMatch] = useState<MatchResult | null>(null);
  const [isMonitoring, setIsMonitoring] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [isBookLoading, setIsBookLoading] = useState(false);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [embeddingStatus, setEmbeddingStatus] = useState<'idle' | 'generating' | 'ready'>('idle');
  const [attachedQuote, setAttachedQuote] = useState<AttachedQuote | null>(null);
  const [theme, setTheme] = useState<'light' | 'dark'>(() => {
    const saved = localStorage.getItem('reading-lamp-theme');
    return (saved === 'dark' ? 'dark' : 'light') as 'light' | 'dark';
  });
  const [spoilerMode, setSpoilerMode] = useState<boolean>(() => {
    const saved = localStorage.getItem('reading-lamp-spoiler-mode');
    return saved !== 'false'; // Default to true (spoiler protection ON)
  });

  // App view state: onboarding -> library -> chat
  const [appView, setAppView] = useState<'loading' | 'onboarding' | 'library' | 'chat'>('loading');
  const [cachedBooks, setCachedBooks] = useState<CachedBook[]>([]);
  const [isLoadingLibrary, setIsLoadingLibrary] = useState(false);

  // Check if API key exists on startup
  useEffect(() => {
    const checkSetup = async () => {
      try {
        const hasKey = await invoke<boolean>('has_api_key');
        if (!hasKey) {
          setAppView('onboarding');
        } else {
          // Load cached books
          await loadCachedBooks();
          setAppView('library');
        }
      } catch (err) {
        console.error('Failed to check API key:', err);
        setAppView('onboarding');
      }
    };
    checkSetup();
  }, []);

  // Global Keyboard Shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // CMD + , => Open Settings
      if ((e.metaKey || e.ctrlKey) && e.key === ',') {
        e.preventDefault();
        setIsSettingsOpen(prev => !prev);
      }
      
      // CMD + W => Prevent closing if just want to close modal/settings (optional, browser default closes tab/window)
      // Actually standard macOS behavior is Cmd+W closes window. We should let Tauri handle that 
      // or if we have a modal open, close it.
      if ((e.metaKey || e.ctrlKey) && e.key === 'w') {
        if (isSettingsOpen) {
          e.preventDefault();
          setIsSettingsOpen(false);
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isSettingsOpen]);

  // Load cached books from database
  const loadCachedBooks = async () => {
    setIsLoadingLibrary(true);
    try {
      const books = await invoke<CachedBook[]>('get_cached_books');
      setCachedBooks(books);
    } catch (err) {
      console.error('Failed to load cached books:', err);
    } finally {
      setIsLoadingLibrary(false);
    }
  };

  // Apply theme to document
  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('reading-lamp-theme', theme);
  }, [theme]);

  // Save spoiler mode preference
  useEffect(() => {
    localStorage.setItem('reading-lamp-spoiler-mode', String(spoilerMode));
  }, [spoilerMode]);


  const handleBookLoaded = useCallback(async (loadedBook: Book) => {
    setBook(loadedBook);
    setIsMonitoring(true);
    
    // Check if this was loaded from cache
    const cacheNote = loadedBook.fromCache ? ' (loaded from cache)' : '';
    
    // Initialize reading state (will be overwritten if cached)
    // Try to load persisted reading state
    try {
      const persistedState = await invoke<ReadingState | null>('get_reading_state', { bookId: loadedBook.id });
      if (persistedState) {
        setReadingState(persistedState);
      } else {
        // Initialize new state if none exists
        setReadingState({
          bookId: loadedBook.id,
          highWaterMark: { chapterIndex: 0, charOffset: 0, percentComplete: 0 },
          currentContext: { chapterIndex: 0, charOffset: 0, percentComplete: 0 },
          updatedAt: new Date().toISOString()
        });
      }
    } catch (err) {
      console.error('Failed to load reading state:', err);
      // Fallback to default
      setReadingState({
        bookId: loadedBook.id,
        highWaterMark: { chapterIndex: 0, charOffset: 0, percentComplete: 0 },
        currentContext: { chapterIndex: 0, charOffset: 0, percentComplete: 0 },
        updatedAt: new Date().toISOString()
      });
    }
    
    // Check if embeddings are already ready (from cache)
    const alreadyReady = await invoke<boolean>('are_embeddings_ready');
    
    if (alreadyReady) {
      setEmbeddingStatus('ready');
      setMessages([{
        id: 'welcome',
        role: 'assistant',
        content: `I've loaded \"${loadedBook.title}\" by ${loadedBook.author}${cacheNote}. Semantic search is ready! Paste any text from your eBook reader and I'll find exactly where it is.`,
        timestamp: new Date().toISOString()
      }]);
      return;
    }
    
    setMessages([{
      id: 'welcome',
      role: 'assistant',
      content: `I've loaded \"${loadedBook.title}\" by ${loadedBook.author}${cacheNote} (${loadedBook.chunkCount} chunks indexed). Paste any text from your eBook reader and I'll find exactly where it is and help you discuss it!`,
      timestamp: new Date().toISOString()
    }]);

    // Start generating embeddings in background
    setEmbeddingStatus('generating');
    try {
      await invoke<number>('generate_embeddings');
      setEmbeddingStatus('ready');
      
      // Add a message about embeddings being ready
      setMessages(prev => [...prev, {
        id: 'embeddings-ready',
        role: 'assistant',
        content: `✨ Semantic search is now enabled! I can now understand the meaning of your questions, not just keywords.`,
        timestamp: new Date().toISOString()
      }]);
    } catch (err) {
      console.error('Failed to generate embeddings:', err);
      setEmbeddingStatus('idle');
    }
  }, []);

  // Listen for native file drops from Tauri
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;

    const setupListener = async () => {
      unlisten = await listen<TauriDragDropEvent>('tauri://drag-drop', async (event) => {
        const paths = event.payload.paths;
        const epubPath = paths.find(p => p.toLowerCase().endsWith('.epub'));
        
        if (epubPath) {
          setIsBookLoading(true);
          try {
            const loadedBook = await invoke<Book>('load_book', { filepath: epubPath });
            handleBookLoaded(loadedBook);
          } catch (err) {
            console.error('Failed to load dropped book:', err);
          } finally {
            setIsBookLoading(false);
          }
        } else {
          console.log('Dropped file is not an EPUB:', paths);
        }
      });
    };

    setupListener();

    return () => {
      if (unlisten) unlisten();
    };
  }, [handleBookLoaded]);

  // Function to locate a quote and update reading position
  const locateAndProcessQuote = useCallback(async (quote: string) => {
    if (!book) return null;
    
    try {
      const match = await invoke<MatchResult | null>('locate_quote', { quote });
      
      if (match) {
        console.log('Found quote at:', match);
        
        // Update reading position if this is further than before
        if (readingState && (
          match.position.chapterIndex > readingState.highWaterMark.chapterIndex ||
          (match.position.chapterIndex === readingState.highWaterMark.chapterIndex && 
           match.position.charOffset > readingState.highWaterMark.charOffset)
        )) {
          await invoke('update_reading_position', {
            chapterIndex: match.position.chapterIndex,
            charOffset: match.position.charOffset,
          });
          
          setReadingState(prev => prev ? {
            ...prev,
            highWaterMark: match.position,
            currentContext: match.position,
            updatedAt: new Date().toISOString()
          } : null);
        }
        
        return match;
      }
    } catch (err) {
      console.error('Failed to locate quote:', err);
    }
    return null;
  }, [book, readingState]);

  // Clean pasted quote text from ebook reader metadata
  const cleanQuoteText = (text: string): string => {
    let cleaned = text;
    
    // Remove Apple Books / Kindle citation patterns
    // "Excerpt From [Title] [Author]" - matches newline or space before "Excerpt"
    cleaned = cleaned.replace(/[\n\s]*Excerpt From\s+.+$/i, '');
    
    // Remove copyright notices
    cleaned = cleaned.replace(/[\n\s]*This material may be protected by copyright\.?$/i, '');
    
    // Remove "Read more at location X" (Kindle)
    cleaned = cleaned.replace(/[\n\s]*Read more at location \d+$/i, '');
    
    // Remove trailing attribution like "- Author Name" or "— Author"
    cleaned = cleaned.replace(/[\n\s]*[—–-]\s*[A-Z][a-z]+\s+[A-Z][a-z]+\s*$/m, '');
    
    // Normalize smart quotes to regular quotes for better matching
    cleaned = cleaned
      .replace(/[\u201C\u201D\u201E\u201F\u2033\u2036]/g, '"')  // Smart double quotes
      .replace(/[\u2018\u2019\u201A\u201B\u2032\u2035]/g, "'"); // Smart single quotes
    
    // Trim whitespace
    cleaned = cleaned.trim();
    
    return cleaned;
  };

  // Handle attaching a quote (from paste)
  const handleAttachQuote = useCallback(async (text: string) => {
    if (!book) return;
    
    // Clean the pasted text
    const cleanedText = cleanQuoteText(text);
    
    // Set the quote immediately with loading state
    setAttachedQuote({
      text: cleanedText,
      position: null,
      chapterTitle: null,
      isLocating: true
    });
    
    // Locate the quote in the book
    const match = await locateAndProcessQuote(cleanedText);
    
    // Update with position if found
    setAttachedQuote({
      text: cleanedText,
      position: match?.position ?? null,
      chapterTitle: match?.chapterTitle ?? null,
      isLocating: false
    });
  }, [book, locateAndProcessQuote]);

  // Clear attached quote
  const handleClearQuote = useCallback(() => {
    setAttachedQuote(null);
  }, []);

  // Update reading position from attached quote
  const handleUpdatePosition = useCallback(async () => {
    if (!attachedQuote?.position || !book) return;
    
    const newPosition = attachedQuote.position;
    
    // Update local reading state
    setReadingState(prev => {
      if (!prev) return null;
      
      // Only update high water mark if new position is further
      const newHighWater = newPosition.percentComplete > prev.highWaterMark.percentComplete
        ? newPosition
        : prev.highWaterMark;
      
      return {
        ...prev,
        highWaterMark: newHighWater,
        currentContext: newPosition,
        updatedAt: new Date().toISOString()
      };
    });
    
    // Persist to database
    try {
      await invoke('update_reading_position', {
        chapterIndex: newPosition.chapterIndex,
        charOffset: newPosition.charOffset,
        percentComplete: newPosition.percentComplete
      });
      
      // Add confirmation message
      const statusMsg = `📍 Position updated to **${attachedQuote.chapterTitle}** (${Math.round(newPosition.percentComplete * 100)}% complete)`;
      setMessages(prev => [...prev, {
        id: Date.now().toString(),
        role: 'assistant',
        content: statusMsg,
        timestamp: new Date().toISOString()
      }]);
      
      showToast('success', `Position set to ${attachedQuote.chapterTitle}`);
      
      // Clear the quote
      setAttachedQuote(null);
    } catch (err) {
      console.error('Failed to update position:', err);
    }
  }, [attachedQuote, book]);

  // Paste from clipboard to update position directly (for the paste button in PositionContext)
  const handlePasteToUpdatePosition = useCallback(async (): Promise<{ success: boolean; chapterTitle?: string; percent?: number }> => {
    if (!book) return { success: false };
    
    try {
      // Read from clipboard using Tauri plugin (no permission prompts)
      const { readText } = await import('@tauri-apps/plugin-clipboard-manager');
      const clipboardText = await readText();
      
      if (!clipboardText || clipboardText.length < 20) {
        return { success: false };
      }
      
      // Locate the quote
      const match = await invoke<MatchResult | null>('locate_quote', { quote: clipboardText });
      
      if (!match) {
        showToast('warning', 'Could not locate that text in the book');
        return { success: false };
      }
      
      // Update reading state
      const newPosition = match.position;
      
      setReadingState(prev => {
        if (!prev) return null;
        
        const newHighWater = newPosition.percentComplete > prev.highWaterMark.percentComplete
          ? newPosition
          : prev.highWaterMark;
        
        return {
          ...prev,
          highWaterMark: newHighWater,
          currentContext: newPosition,
          updatedAt: new Date().toISOString()
        };
      });
      
      // Persist to database
      await invoke('update_reading_position', {
        chapterIndex: newPosition.chapterIndex,
        charOffset: newPosition.charOffset,
        percentComplete: newPosition.percentComplete
      });
      
      showToast('success', `Updated: ${match.chapterTitle} (${Math.round(newPosition.percentComplete * 100)}%)`);
      
      return { 
        success: true, 
        chapterTitle: match.chapterTitle, 
        percent: Math.round(newPosition.percentComplete * 100) 
      };
    } catch (err) {
      console.error('Failed to paste and update position:', err);
      showToast('error', 'Failed to update position');
      return { success: false };
    }
  }, [book]);

  const handleSendMessage = useCallback(async (content: string, quote?: AttachedQuote) => {
    if (!book) return;

    // Build the message content
    let messageContent = content;
    let contextPosition = readingState?.currentContext;
    
    // If there's an attached quote, include it
    if (quote) {
      if (content) {
        // User provided both quote and question
        messageContent = `Regarding this passage: "${quote.text.slice(0, 500)}${quote.text.length > 500 ? '...' : ''}"\n\n${content}`;
      } else {
        // Just the quote, ask for analysis
        messageContent = `Analyze this passage: "${quote.text.slice(0, 500)}${quote.text.length > 500 ? '...' : ''}"`;
      }
      
      if (quote.position) {
        contextPosition = quote.position;
      }
    }
    // Note: We intentionally do NOT run quote matching on plain messages.
    // Position matching only happens when the user explicitly pastes a quote.

    if (!messageContent) return;

    const userMessage: Message = {
      id: Date.now().toString(),
      role: 'user',
      content: messageContent,
      timestamp: new Date().toISOString(),
      contextPosition
    };

    setMessages(prev => [...prev, userMessage]);
    setIsLoading(true);
    
    // Clear the attached quote after sending
    setAttachedQuote(null);

    // Determine context for AI
    const currentChapterIndex = contextPosition?.chapterIndex ?? readingState?.highWaterMark?.chapterIndex ?? 0;
    const currentPercent = contextPosition?.percentComplete ?? readingState?.highWaterMark?.percentComplete ?? 0;

    // Create assistant message placeholder for streaming
    const assistantMessageId = (Date.now() + 1).toString();
    const sessionId = `session-${assistantMessageId}`;
    
    // Add empty assistant message that we'll stream into
    const assistantMessage: Message = {
      id: assistantMessageId,
      role: 'assistant',
      content: '',
      timestamp: new Date().toISOString()
    };
    setMessages(prev => [...prev, assistantMessage]);

    try {
      // Set up listeners for streaming
      const unlistenChunk = await listen<string>(`chat-stream:${sessionId}`, (event) => {
        setMessages(prev => prev.map(m => 
          m.id === assistantMessageId 
            ? { ...m, content: m.content + event.payload }
            : m
        ));
      });

      const unlistenEnd = await listen(`chat-stream-end:${sessionId}`, () => {
        setIsLoading(false);
        unlistenChunk();
        unlistenEnd();
      });

      const unlistenError = await listen<string>(`chat-stream-error:${sessionId}`, (event) => {
        setMessages(prev => prev.map(m =>
          m.id === assistantMessageId
            ? { ...m, content: `Error: ${event.payload}` }
            : m
        ));
        setIsLoading(false);
        unlistenChunk();
        unlistenEnd();
        unlistenError();
      });

      // Call streaming endpoint
      await invoke('send_chat_message_stream', {
        messages: [...messages, userMessage].map(m => ({
          role: m.role,
          content: m.content
        })),
        bookContext: {
          title: book.title,
          author: book.author,
          currentChapter: book.chapters[currentChapterIndex]?.title ?? 'Chapter 1',
          percentComplete: currentPercent,
          spoilerBoundaryChapter: readingState?.highWaterMark?.chapterIndex ?? 0,
          passageBeingDiscussed: quote?.text ?? content,
          spoilerModeEnabled: spoilerMode
        },
        sessionId
      });
    } catch (err) {
      console.error('Failed to send message:', err);
      setMessages(prev => prev.map(m =>
        m.id === assistantMessageId
          ? { ...m, content: `Sorry, I couldn't process your request. ${err}. Make sure your Gemini API key is configured in Settings.` }
          : m
      ));
      setIsLoading(false);
    }
  }, [book, readingState, messages, locateAndProcessQuote]);

  const handleContextModeSelect = useCallback((mode: 'retrospective' | 'current') => {
    if (!pendingMatch) return;
    
    const contextMessage: Message = {
      id: Date.now().toString(),
      role: 'user',
      content: `Analyze this passage: "${pendingMatch.matchedText}"`,
      timestamp: new Date().toISOString(),
      contextPosition: mode === 'retrospective' 
        ? pendingMatch.position 
        : readingState?.highWaterMark
    };
    
    setMessages(prev => [...prev, contextMessage]);
    setPendingMatch(null);
    handleSendMessage(`Analyze this passage: "${pendingMatch.matchedText}"`);
  }, [pendingMatch, readingState, handleSendMessage]);

  // Handle onboarding completion
  const handleOnboardingComplete = useCallback(async (apiKey: string) => {
    try {
      await invoke('set_api_key', { apiKey });
      // If a book was uploaded during onboarding, go straight to chat
      if (book) {
        setAppView('chat');
      } else {
        await loadCachedBooks();
        setAppView('library');
      }
    } catch (err) {
      console.error('Failed to save API key:', err);
    }
  }, [book]);

  // Handle selecting a book from the library
  const handleSelectBook = useCallback(async (bookId: string) => {
    setIsBookLoading(true);
    try {
      const loadedBook = await invoke<Book>('open_cached_book', { bookId });
      await handleBookLoaded(loadedBook);
      setAppView('chat');
    } catch (err) {
      console.error('Failed to open book:', err);
    } finally {
      setIsBookLoading(false);
    }
  }, [handleBookLoaded]);

  // Handle adding a new book (from library or onboarding)
  const handleAddBook = useCallback(async () => {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const selected = await open({
      multiple: false,
      filters: [{ name: 'EPUB Files', extensions: ['epub'] }]
    });
    
    if (selected) {
      setIsBookLoading(true);
      try {
        const loadedBook = await invoke<Book>('load_book', { filepath: selected });
        await handleBookLoaded(loadedBook);
        setAppView('chat');
      } catch (err) {
        console.error('Failed to load book:', err);
      } finally {
        setIsBookLoading(false);
      }
    }
  }, [handleBookLoaded]);

  // Handle deleting a book
  const handleDeleteBook = useCallback(async (bookId: string) => {
    const yes = await ask('Are you sure you want to delete this book? This cannot be undone.', {
        title: 'Delete Book',
        kind: 'warning',
        okLabel: 'Delete',
        cancelLabel: 'Cancel'
    });
    
    if (!yes) return;

    try {
      await invoke('delete_book', { bookId });
      showToast('success', 'Book deleted');
      // Reload library
      await loadCachedBooks();
    } catch (err) {
      console.error('Failed to delete book:', err);
      showToast('error', 'Failed to delete book');
    }
  }, [showToast]);

  // Listen for delete requests from native context menu
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen('request-delete-book', (event) => {
      handleDeleteBook(event.payload as string);
    }).then((u) => { unlisten = u; });
    
    return () => {
      if (unlisten) unlisten();
    };
  }, [handleDeleteBook]);

  // Loading screen
  if (appView === 'loading') {
    return (
      <div className="h-full w-full flex items-center justify-center bg-[var(--color-bg-secondary)]">
        <div className="animate-pulse text-[var(--color-text-muted)]">Loading...</div>
      </div>
    );
  }

  // Onboarding screen
  if (appView === 'onboarding') {
    return (
      <Onboarding
        onComplete={handleOnboardingComplete}
        onUploadBook={handleAddBook}
        isUploading={isBookLoading}
        bookUploaded={book !== null}
      />
    );
  }

  // Library screen
  if (appView === 'library') {
    return (
      <div className="h-full w-full flex flex-col bg-[var(--color-bg-secondary)]">
        {/* Header */}
        <header 
          data-tauri-drag-region
          className="drag-region h-16 flex items-center justify-between px-4 border-b border-[var(--color-border)] bg-[var(--color-bg-secondary)] shrink-0"
        >
          <div />
          <div className="flex items-center gap-1.5 pointer-events-none">
            <div className="relative">
              <div className="absolute -inset-1 bg-amber-400/30 rounded-full blur-md"></div>
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" className="relative">
                <path d="M12 2L12 4M12 4L9 7M12 4L15 7" stroke="var(--color-accent)" strokeWidth="1.5" strokeLinecap="round" opacity="0.6"/>
                <path d="M7 10C7 10 8 6 12 6C16 6 17 10 17 10L16 14H8L7 10Z" fill="var(--color-accent)" />
                <rect x="9" y="14" width="6" height="2" rx="0.5" fill="var(--color-accent)" />
                <rect x="10" y="16" width="4" height="2" rx="0.5" fill="var(--color-accent)" />
                <ellipse cx="12" cy="11" rx="3" ry="2" fill="var(--color-bg-elevated)" opacity="0.5"/>
              </svg>
            </div>
            <span className="font-semibold text-sm text-[var(--color-text-primary)] tracking-tight">
              Reading Lamp
            </span>
          </div>
          <button 
            onClick={() => setIsSettingsOpen(true)}
            className="no-drag p-1.5 hover:bg-[var(--color-bg-tertiary)] rounded-md transition-colors"
          >
            <SettingsIcon className="w-4 h-4 text-[var(--color-text-muted)]" />
          </button>
        </header>

        {/* Library content */}
        <div className="flex-1 min-h-0">
          <Library
            books={cachedBooks}
            onSelectBook={handleSelectBook}
            onAddBook={handleAddBook}
            isLoading={isLoadingLibrary || isBookLoading}
          />
        </div>

        <Settings 
          isOpen={isSettingsOpen} 
          onClose={() => setIsSettingsOpen(false)}
          theme={theme}
          onThemeChange={setTheme}
          spoilerMode={spoilerMode}
          onSpoilerModeChange={setSpoilerMode}
        />
      </div>
    );
  }

  // Chat screen (appView === 'chat')
  return (
    <div className="h-full w-full flex flex-col bg-[var(--color-bg-secondary)]">
      {/* Header */}
      <header 
        data-tauri-drag-region
        className="drag-region h-20 flex items-center justify-between pl-4 pr-4 border-b border-[var(--color-border)] bg-[var(--color-bg-secondary)] shrink-0 relative"
      >
        {/* Back to library button */}
        <button 
          onClick={() => {
            loadCachedBooks();
            setAppView('library');
          }}
          className="no-drag p-1.5 hover:bg-[var(--color-bg-tertiary)] rounded-md transition-colors z-10"
        >
          <LibraryBig className="w-4 h-4 text-[var(--color-text-muted)]" />
        </button>

        {/* Centered Logo & Status */}
        <div className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 flex flex-col items-center">
          {book ? (
            <div className="flex flex-col items-center animate-fade-in">
              <span className="font-semibold text-sm text-[var(--color-text-primary)] tracking-tight line-clamp-1 max-w-[300px] text-center">
                {book.title}
              </span>
              <div className="flex items-center gap-1.5">
                <span className="text-[10px] text-[var(--color-text-muted)] line-clamp-1 max-w-[200px]">
                  {book.author}
                </span>
                {embeddingStatus === 'generating' && (
                  <span className="text-[9px] text-[var(--color-accent)] animate-pulse border-l border-[var(--color-border)] pl-1.5">
                    Indexing...
                  </span>
                )}
              </div>
            </div>
          ) : (
            <div className="flex items-center gap-1.5 pointer-events-none">
              <div className="relative">
                <div className="absolute -inset-1 bg-amber-400/30 rounded-full blur-md"></div>
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" className="relative">
                  <path d="M12 2L12 4M12 4L9 7M12 4L15 7" stroke="var(--color-accent)" strokeWidth="1.5" strokeLinecap="round" opacity="0.6"/>
                  <path d="M7 10C7 10 8 6 12 6C16 6 17 10 17 10L16 14H8L7 10Z" fill="var(--color-accent)" />
                  <rect x="9" y="14" width="6" height="2" rx="0.5" fill="var(--color-accent)" />
                  <rect x="10" y="16" width="4" height="2" rx="0.5" fill="var(--color-accent)" />
                  <ellipse cx="12" cy="11" rx="3" ry="2" fill="var(--color-bg-elevated)" opacity="0.5"/>
                </svg>
              </div>
              <span className="font-semibold text-sm text-[var(--color-text-primary)] tracking-tight">
                Reading Lamp
              </span>
            </div>
          )}
        </div>
        
        <button 
          onClick={() => setIsSettingsOpen(true)}
          className="no-drag p-1.5 hover:bg-[var(--color-bg-tertiary)] rounded-md transition-colors relative z-10"
        >
          <SettingsIcon className="w-4 h-4 text-[var(--color-text-muted)]" />
        </button>
      </header>

      {/* Main content */}
      <div className="flex-1 flex flex-col min-h-0 px-4 pb-4">
        {book ? (
          <>
            <PositionContext 
              book={book} 
              readingState={readingState} 
              isMonitoring={isMonitoring}
              onPasteToUpdatePosition={handlePasteToUpdatePosition}
            />
            
            {pendingMatch && (
              <ContextPrompt
                match={pendingMatch}
                onSelectMode={handleContextModeSelect}
                onDismiss={() => setPendingMatch(null)}
              />
            )}
            
            <ChatInterface
              messages={messages}
              onSendMessage={handleSendMessage}
              isLoading={isLoading}
              attachedQuote={attachedQuote}
              onAttachQuote={handleAttachQuote}
              onClearQuote={handleClearQuote}
              onUpdatePosition={handleUpdatePosition}
            />
          </>
        ) : (
          <BookLoader 
            onBookLoaded={handleBookLoaded}
            isLoading={isBookLoading}
            setIsLoading={setIsBookLoading}
          />
        )}
      </div>

      {/* Settings Modal */}
      <Settings 
        isOpen={isSettingsOpen} 
        onClose={() => setIsSettingsOpen(false)}
        theme={theme}
        onThemeChange={setTheme}
        spoilerMode={spoilerMode}
        onSpoilerModeChange={setSpoilerMode}
      />
    </div>
  );
}

export default App;

