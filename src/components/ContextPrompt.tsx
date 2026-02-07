import { History, ArrowRight, X } from 'lucide-react';
import type { MatchResult } from '../lib/types';

interface ContextPromptProps {
  match: MatchResult;
  onSelectMode: (mode: 'retrospective' | 'current') => void;
  onDismiss: () => void;
}

export function ContextPrompt({ match, onSelectMode, onDismiss }: ContextPromptProps) {
  const snippetPreview = match.matchedText.length > 60 
    ? match.matchedText.slice(0, 60) + '...' 
    : match.matchedText;

  return (
    <div className="p-4 border-b border-[var(--color-border)] bg-[var(--color-bg-tertiary)] animate-slide-up">
      <div className="flex items-start justify-between gap-2 mb-3">
        <div className="flex items-center gap-2">
          <div className="w-8 h-8 rounded-full bg-[var(--color-warning)]/20 flex items-center justify-center">
            <History className="w-4 h-4 text-[var(--color-warning)]" />
          </div>
          <div>
            <p className="text-sm font-medium text-[var(--color-text-primary)]">
              Looking back?
            </p>
            <p className="text-xs text-[var(--color-text-muted)]">
              This is from {match.chapterTitle}
            </p>
          </div>
        </div>
        <button 
          onClick={onDismiss}
          className="p-1 hover:bg-[var(--color-bg-elevated)] rounded transition-colors"
        >
          <X className="w-4 h-4 text-[var(--color-text-muted)]" />
        </button>
      </div>

      <p className="text-xs text-[var(--color-text-secondary)] mb-4 italic bg-[var(--color-bg-secondary)] p-2 rounded-lg">
        "{snippetPreview}"
      </p>

      <p className="text-xs text-[var(--color-text-muted)] mb-3">
        Analyze with context from:
      </p>

      <div className="flex gap-2">
        <button
          onClick={() => onSelectMode('retrospective')}
          className="flex-1 flex items-center justify-center gap-2 px-3 py-2 rounded-lg
            bg-[var(--color-bg-secondary)] border border-[var(--color-border)]
            hover:border-[var(--color-accent)] hover:bg-[var(--color-accent-muted)]
            transition-all text-sm"
        >
          <span className="text-[var(--color-text-secondary)]">
            Up to {match.chapterTitle}
          </span>
        </button>
        
        <button
          onClick={() => onSelectMode('current')}
          className="flex-1 flex items-center justify-center gap-2 px-3 py-2 rounded-lg
            bg-[var(--color-accent)] hover:bg-[var(--color-accent-hover)]
            transition-all text-sm text-white"
        >
          <span>Full context</span>
          <ArrowRight className="w-4 h-4" />
        </button>
      </div>
    </div>
  );
}
