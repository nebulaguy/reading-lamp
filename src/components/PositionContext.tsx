import { useState } from 'react';
import { MapPin, Activity, ClipboardPaste, Loader2, Check } from 'lucide-react';
import type { Book, ReadingState } from '../lib/types';

interface PositionContextProps {
  book: Book;
  readingState: ReadingState | null;
  isMonitoring: boolean;
  onPasteToUpdatePosition?: () => Promise<{ success: boolean; chapterTitle?: string; percent?: number }>;
}

export function PositionContext({ book, readingState, isMonitoring, onPasteToUpdatePosition }: PositionContextProps) {
  const [isPasting, setIsPasting] = useState(false);
  const [justUpdated, setJustUpdated] = useState(false);

  const currentChapter = readingState 
    ? book.chapters[readingState.highWaterMark.chapterIndex]
    : book.chapters[0];
  
  const percentComplete = readingState 
    ? readingState.highWaterMark.percentComplete 
    : 0;

  const handlePasteUpdate = async () => {
    if (!onPasteToUpdatePosition || isPasting) return;
    
    setIsPasting(true);
    try {
      const result = await onPasteToUpdatePosition();
      if (result.success) {
        setJustUpdated(true);
        setTimeout(() => setJustUpdated(false), 2000);
      }
    } finally {
      setIsPasting(false);
    }
  };

  return (
    <div className="p-4 border-b border-[var(--color-border)] bg-[var(--color-bg-secondary)] rounded-[24px] mx-2 mt-2">
      {/* Progress bar with paste button */}
      <div className="mb-3">
        <div className="flex justify-between items-center mb-1.5">
          <span className="text-xs font-medium text-[var(--color-text-primary)] truncate pr-2" title={currentChapter?.title}>
            {currentChapter?.title || `Chapter ${(readingState?.highWaterMark.chapterIndex ?? 0) + 1}`}
          </span>
          <div className="flex items-center gap-2 shrink-0">
            <span className="text-xs font-medium text-[var(--color-text-secondary)]">
              {Math.round(percentComplete * 100)}%
            </span>
            {onPasteToUpdatePosition && (
              <button
                onClick={handlePasteUpdate}
                disabled={isPasting}
                className={`
                  flex items-center gap-1 px-2 py-1 rounded-md text-[10px] font-medium
                  transition-all duration-200
                  ${justUpdated 
                    ? 'bg-[var(--color-success)]/20 text-[var(--color-success)]' 
                    : 'bg-[var(--color-bg-tertiary)] text-[var(--color-text-muted)] hover:bg-[var(--color-accent-muted)] hover:text-[var(--color-accent)]'
                  }
                  disabled:opacity-50
                `}
                title="Paste quote to update position"
              >
                {isPasting ? (
                  <Loader2 className="w-3 h-3 animate-spin" />
                ) : justUpdated ? (
                  <Check className="w-3 h-3" />
                ) : (
                  <ClipboardPaste className="w-3 h-3" />
                )}
                {justUpdated ? 'Updated' : 'Paste'}
              </button>
            )}
          </div>
        </div>
        <div className="h-1.5 bg-[var(--color-bg-tertiary)] rounded-full overflow-hidden">
          <div 
            className="h-full bg-gradient-to-r from-[var(--color-success)] to-[var(--color-success)] rounded-full transition-all duration-500"
            style={{ width: `${percentComplete * 100}%` }}
          />
        </div>
      </div>

      {/* Current position */}
      <div className="flex items-center gap-4 text-xs">
        <div className="flex items-center gap-1.5 text-[var(--color-text-muted)]">
          <MapPin className="w-3.5 h-3.5" />
          <span className="truncate max-w-[140px]">
             Chapter {(readingState?.highWaterMark.chapterIndex ?? 0) + 1} of {book.chapters.length}
          </span>
        </div>
        
        <div className={`
          flex items-center gap-1.5 ml-auto
          ${isMonitoring ? 'text-[var(--color-success)]' : 'text-[var(--color-text-muted)]'}
        `}>
          <Activity className={`w-3.5 h-3.5 ${isMonitoring ? 'animate-pulse' : ''}`} />
          <span>{isMonitoring ? 'Monitoring' : 'Paused'}</span>
        </div>
      </div>
    </div>
  );
}
