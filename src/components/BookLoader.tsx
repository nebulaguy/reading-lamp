import { useState, useCallback } from 'react';
import { Upload, BookOpen, Loader2 } from 'lucide-react';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import type { Book } from '../lib/types';

interface BookLoaderProps {
  onBookLoaded: (book: Book, filepath: string) => void;
  isLoading: boolean;
  setIsLoading: (loading: boolean) => void;
}

export function BookLoader({ onBookLoaded, isLoading, setIsLoading }: BookLoaderProps) {
  const [isDragging, setIsDragging] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadBookFromPath = useCallback(async (filepath: string) => {
    setError(null);
    setIsLoading(true);
    
    try {
      console.log('Loading book from:', filepath);
      const book = await invoke<Book>('load_book', { filepath });
      console.log('Book loaded:', book);
      onBookLoaded(book, filepath);
    } catch (err) {
      console.error('Failed to load book:', err);
      setError(`Failed to load book: ${err}`);
    } finally {
      setIsLoading(false);
    }
  }, [onBookLoaded, setIsLoading]);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);
  }, []);

  const handleDrop = useCallback(async (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);
    setError(null);

    const files = Array.from(e.dataTransfer.files);
    const epubFile = files.find(f => f.name.toLowerCase().endsWith('.epub'));
    
    if (!epubFile) {
      setError('Please drop an EPUB file (not PDF)');
      return;
    }

    // In Tauri, dropped files have a path property
    const filepath = (epubFile as any).path;
    
    if (!filepath) {
      // Fallback: web drop doesn't have path, use file picker instead
      setError('Drag & drop not supported in browser. Click to use file picker.');
      return;
    }

    await loadBookFromPath(filepath);
  }, [loadBookFromPath]);

  const handleFileSelect = useCallback(async () => {
    setError(null);
    
    try {
      const selected = await open({
        multiple: false,
        filters: [{
          name: 'EPUB Books',
          extensions: ['epub']
        }]
      });

      if (selected && typeof selected === 'string') {
        await loadBookFromPath(selected);
      }
    } catch (err) {
      console.error('File dialog error:', err);
      setError(`File picker error: ${err}`);
    }
  }, [loadBookFromPath]);

  return (
    <div className="flex-1 flex items-center justify-center p-8">
      <div
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
        onClick={handleFileSelect}
        className={`
          w-full max-w-sm aspect-[3/4] rounded-2xl border-2 border-dashed
          flex flex-col items-center justify-center gap-6 p-8
          transition-all duration-300 cursor-pointer
          ${isDragging 
            ? 'border-[var(--color-accent)] bg-[var(--color-accent-muted)] scale-[1.02]' 
            : 'border-[var(--color-border)] hover:border-[var(--color-border-hover)] hover:bg-[var(--color-bg-secondary)]'
          }
        `}
      >
        {isLoading ? (
          <>
            <div className="w-16 h-16 rounded-full bg-[var(--color-accent-muted)] flex items-center justify-center">
              <Loader2 className="w-8 h-8 text-[var(--color-accent)] animate-spin" />
            </div>
            <div className="text-center">
              <p className="text-[var(--color-text-primary)] font-medium">Loading book...</p>
              <p className="text-[var(--color-text-muted)] text-sm mt-1">Parsing EPUB content</p>
            </div>
          </>
        ) : (
          <>
            <div className={`
              w-16 h-16 rounded-full flex items-center justify-center transition-colors
              ${isDragging 
                ? 'bg-[var(--color-accent)]' 
                : 'bg-[var(--color-bg-tertiary)]'
              }
            `}>
              {isDragging ? (
                <BookOpen className="w-8 h-8 text-white" />
              ) : (
                <Upload className="w-8 h-8 text-[var(--color-text-muted)]" />
              )}
            </div>
            
            <div className="text-center">
              <p className="text-[var(--color-text-primary)] font-medium">
                {isDragging ? 'Drop your book here' : 'Drop an EPUB to get started'}
              </p>
              <p className="text-[var(--color-text-muted)] text-sm mt-1">
                or click to browse
              </p>
            </div>

            {error && (
              <p className="text-[var(--color-error)] text-sm animate-fade-in text-center">
                {error}
              </p>
            )}
          </>
        )}
      </div>
    </div>
  );
}
