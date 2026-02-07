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
function getBookColor(title: string): string {
  const colors = [
    'from-amber-700 to-amber-900',
    'from-emerald-700 to-emerald-900',
    'from-blue-700 to-blue-900',
    'from-purple-700 to-purple-900',
    'from-rose-700 to-rose-900',
    'from-teal-700 to-teal-900',
    'from-orange-700 to-orange-900',
    'from-indigo-700 to-indigo-900',
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
          <div className="grid gap-3" style={{ gridTemplateColumns: 'repeat(auto-fill, minmax(140px, 1fr))' }}>
            {/* Add Book Card */}
            <button
              onClick={onAddBook}
              className="h-[200px] rounded-2xl border-2 border-dashed border-[var(--color-border)] 
                hover:border-[var(--color-accent)] hover:bg-[var(--color-accent-muted)]
                flex flex-col items-center justify-center gap-2 transition-colors"
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
                className="h-[200px] rounded-2xl overflow-hidden shadow-lg hover:shadow-xl 
                  transition-all hover:scale-[1.02] text-left flex flex-col cursor-pointer group relative select-none"
              >
                {/* Cover */}
                <div className={`flex-1 bg-gradient-to-br ${getBookColor(book.title)} p-3 flex flex-col justify-end`}>
                  <div className="flex items-center gap-1.5 mb-2">
                    <BookOpen className="w-4 h-4 text-white/60" />
                    <span className="text-[10px] text-white/60 uppercase tracking-wide">
                      {book.totalChapters} chapters
                    </span>
                  </div>
                  <h3 className="text-white font-semibold text-sm leading-tight line-clamp-2">
                    {book.title}
                  </h3>
                  <p className="text-white/70 text-xs mt-0.5 truncate">
                    {book.author}
                  </p>
                </div>

                {/* Progress Bar */}
                <div className="bg-[var(--color-bg-elevated)] px-3 py-2">
                  <div className="flex items-center justify-between text-[10px] mb-1">
                    <span className="text-[var(--color-text-muted)]">Progress</span>
                    <span className="text-[var(--color-text-secondary)] font-medium">
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
