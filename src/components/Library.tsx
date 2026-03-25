import { Plus, BookOpen, Loader2 } from 'lucide-react';
import type { CachedBook } from '../lib/types';
import { MouseEvent } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface LibraryProps {
  books: CachedBook[];
  onSelectBook: (bookId: string) => void;
  onAddBook: () => void;
  isLoading: boolean;
}

// Generate a deterministic color based on book title
function getBookColor(title: string): { from: string; to: string } {
  const colors = [
    { from: '#92400e', to: '#78350f' }, // amber
    { from: '#065f46', to: '#064e3b' }, // emerald
    { from: '#1e40af', to: '#1e3a5f' }, // blue
    { from: '#6b21a8', to: '#581c87' }, // purple
    { from: '#9f1239', to: '#881337' }, // rose
    { from: '#115e59', to: '#134e4a' }, // teal
    { from: '#9a3412', to: '#7c2d12' }, // orange
    { from: '#3730a3', to: '#312e81' }, // indigo
  ];
  const hash = title.split('').reduce((acc, char) => acc + char.charCodeAt(0), 0);
  return colors[hash % colors.length];
}

export function Library({ books, onSelectBook, onAddBook, isLoading }: LibraryProps) {
  const handleContextMenu = (e: MouseEvent, bookId: string) => {
    e.preventDefault();
    invoke('show_book_context_menu', { bookId });
  };

  return (
    <div className="flex flex-col h-full relative">
      {/* Header */}
      <div className="p-4 border-b border-[var(--color-border)]">
        <h1 className="text-lg font-semibold text-[var(--color-text-primary)]">
          Library
        </h1>
        <p className="text-xs text-[var(--color-text-muted)] mt-0.5">
          {books.length} {books.length === 1 ? 'book' : 'books'}
        </p>
      </div>

      {/* Book Grid */}
      <div className="flex-1 overflow-y-auto p-4">
        {isLoading ? (
          <div className="flex items-center justify-center h-32">
            <Loader2 className="w-6 h-6 animate-spin text-[var(--color-accent)]" />
          </div>
        ) : (
          <div className="book-grid">
            {/* Add Book Card */}
            <button
              onClick={onAddBook}
              className="book-card-add"
            >
              <div className="w-12 h-12 rounded-full bg-[var(--color-accent-muted)] flex items-center justify-center">
                <Plus className="w-6 h-6 text-[var(--color-accent)]" />
              </div>
              <span className="text-sm font-medium text-[var(--color-text-secondary)]">
                Add Book
              </span>
            </button>

            {/* Book Cards */}
            {books.map((book) => (
              <div
                key={book.id}
                onClick={() => onSelectBook(book.id)}
                onContextMenu={(e) => handleContextMenu(e, book.id)}
                className="book-card"
              >
                {/* Cover area */}
                {book.coverImageUrl ? (
                  <div className="book-card-cover">
                    <img
                      src={book.coverImageUrl}
                      alt={book.title}
                      className="book-card-cover-img"
                      draggable={false}
                    />
                  </div>
                ) : (
                  <div
                    className="book-card-fallback"
                    style={{
                      background: `linear-gradient(135deg, ${getBookColor(book.title).from}, ${getBookColor(book.title).to})`,
                    }}
                  >
                    <div className="flex items-center gap-1.5 mb-2">
                      <BookOpen className="w-4 h-4 text-white/60" />
                      <span className="text-[10px] text-white/60 uppercase tracking-wide">
                        {book.totalChapters} chapters
                      </span>
                    </div>
                    <h3 className="text-white font-semibold text-sm leading-tight line-clamp-3">
                      {book.title}
                    </h3>
                    <p className="text-white/70 text-xs mt-1 truncate">
                      {book.author}
                    </p>
                  </div>
                )}

                {/* Progress overlay at bottom */}
                <div className="book-card-progress">
                  <div className="flex items-center justify-between text-[10px] mb-1">
                    <span className="text-[var(--color-text-muted)]">
                      {Math.round(book.readingProgress * 100)}%
                    </span>
                  </div>
                  <div className="h-1 bg-[var(--color-bg-tertiary)] rounded-full overflow-hidden">
                    <div
                      className="h-full bg-[var(--color-success)] rounded-full transition-all"
                      style={{ width: `${book.readingProgress * 100}%` }}
                    />
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
