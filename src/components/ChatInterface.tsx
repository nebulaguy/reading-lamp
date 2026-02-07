import { useState, useRef, useEffect } from 'react';
import { Send, Loader2, Sparkles, X, MapPin, ChevronDown, ChevronUp } from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import type { Message, AttachedQuote } from '../lib/types';

interface ChatInterfaceProps {
  messages: Message[];
  onSendMessage: (content: string, attachedQuote?: AttachedQuote) => void;
  isLoading: boolean;
  disabled?: boolean;
  attachedQuote: AttachedQuote | null;
  onAttachQuote: (text: string) => void;
  onClearQuote: () => void;
  onUpdatePosition?: () => void;
}

export function ChatInterface({ 
  messages, 
  onSendMessage, 
  isLoading, 
  disabled, 
  attachedQuote, 
  onAttachQuote, 
  onClearQuote,
  onUpdatePosition
}: ChatInterfaceProps) {
  const [input, setInput] = useState('');
  const [isQuoteExpanded, setIsQuoteExpanded] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  // Focus input on CMD+K
  useEffect(() => {
    const handleGlobalKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        inputRef.current?.focus();
      }
    };
    
    window.addEventListener('keydown', handleGlobalKeyDown);
    return () => window.removeEventListener('keydown', handleGlobalKeyDown);
  }, []);

  const handleSend = () => {
    // Can send if there's input OR an attached quote
    if ((!input.trim() && !attachedQuote) || isLoading || disabled) return;
    
    onSendMessage(input.trim(), attachedQuote ?? undefined);
    setInput('');
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const handlePaste = (e: React.ClipboardEvent) => {
    const pastedText = e.clipboardData.getData('text');
    
    // Only treat as quote attachment if it looks like book text (longer than a short phrase)
    // and doesn't look like a question
    if (pastedText.length > 50 && !pastedText.endsWith('?')) {
      e.preventDefault();
      onAttachQuote(pastedText);
    }
  };

  // Truncate quote for display
  const truncatedQuote = attachedQuote?.text 
    ? (attachedQuote.text.length > 200 && !isQuoteExpanded
        ? attachedQuote.text.slice(0, 200) + '...'
        : attachedQuote.text)
    : '';

  const canSend = (input.trim() || attachedQuote) && !isLoading && !disabled;

  return (
    <div className="flex flex-col h-full min-h-0 bg-[var(--color-bg-secondary)]">
      {/* Messages - takes remaining space */}
      <div 
        ref={scrollRef}
        className="flex-1 min-h-0 overflow-y-auto p-6 space-y-5"
      >
        {messages.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-center px-4">
            <div className="w-12 h-12 rounded-full bg-[var(--color-accent-muted)] flex items-center justify-center mb-4">
              <Sparkles className="w-6 h-6 text-[var(--color-accent)]" />
            </div>
            <p className="text-[var(--color-text-secondary)] text-sm">
              Paste text from your eBook reader to start analyzing
            </p>
          </div>
        ) : (
          messages.map((msg) => (
            <div
              key={msg.id}
              className={`
                flex flex-col animate-slide-up
                ${msg.role === 'user' 
                  ? 'max-w-[85%] self-end bg-[var(--color-accent)] text-white rounded-[20px] rounded-br-md px-5 py-4 ml-auto' 
                  : 'w-full text-[var(--color-text-primary)]'
                }
              `}
            >
              {msg.role === 'assistant' ? (
                <div className={`prose prose-sm max-w-none text-[15px] leading-relaxed
                  prose-p:my-4 prose-ul:my-3 prose-ol:my-3 prose-li:my-1
                  prose-strong:text-[var(--color-text-primary)] prose-strong:font-semibold
                  prose-headings:text-[var(--color-text-primary)] prose-headings:font-semibold prose-headings:mt-5 prose-headings:mb-3
                  [&>*:first-child]:mt-0 [&>*:last-child]:mb-0
                  text-[var(--color-text-primary)]
                  ${isLoading && msg.id === messages[messages.length - 1]?.id ? 'streaming-text' : ''}
                `}>
                  <ReactMarkdown>{msg.content}</ReactMarkdown>
                </div>
              ) : (
                <div className="text-[15px] leading-relaxed">
                  {msg.content}
                </div>
              )}
              {msg.contextPosition && (
                <span className={`
                  text-[11px] mt-2 opacity-60
                  ${msg.role === 'user' ? 'text-white/70' : 'text-[var(--color-text-muted)]'}
                `}>
                  Chapter {msg.contextPosition.chapterIndex + 1} · {Math.round(msg.contextPosition.percentComplete * 100)}%
                </span>
              )}
            </div>
          ))
        )}
        
        {isLoading && (
          <div className="flex items-center gap-2 text-[var(--color-text-muted)] text-sm animate-fade-in">
            <Loader2 className="w-4 h-4 animate-spin" />
            <span>Thinking...</span>
          </div>
        )}
      </div>

      {/* Input area - always anchored to bottom */}
      <div className="flex-shrink-0 p-4 border-t border-[var(--color-border)] bg-[var(--color-bg-secondary)]">
        
        {/* Attached Quote Chip */}
        {attachedQuote && (
          <div className="mb-3 p-3 bg-[var(--color-bg-tertiary)] border border-[var(--color-border)] rounded-xl animate-slide-up">
            <div className="flex items-start justify-between gap-2">
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2 mb-1.5">
                  <span className="text-[11px] font-medium text-[var(--color-accent)]">📎 Attached Quote</span>
                  {attachedQuote.isLocating && (
                    <Loader2 className="w-3 h-3 animate-spin text-[var(--color-text-muted)]" />
                  )}
                  {attachedQuote.position && (
                    <span className="flex items-center gap-1 text-[10px] text-[var(--color-text-muted)]">
                      <MapPin className="w-3 h-3" />
                      {attachedQuote.chapterTitle} · {Math.round(attachedQuote.position.percentComplete * 100)}%
                    </span>
                  )}
                </div>
                <p className="text-[13px] text-[var(--color-text-secondary)] leading-relaxed italic">
                  "{truncatedQuote}"
                </p>
                {attachedQuote.text.length > 200 && (
                  <button 
                    onClick={() => setIsQuoteExpanded(!isQuoteExpanded)}
                    className="flex items-center gap-1 mt-1 text-[11px] text-[var(--color-accent)] hover:underline"
                  >
                    {isQuoteExpanded ? (
                      <>Show less <ChevronUp className="w-3 h-3" /></>
                    ) : (
                      <>Show more <ChevronDown className="w-3 h-3" /></>
                    )}
                  </button>
                )}
                {/* Update Position Button - show when quote is located */}
                {attachedQuote.position && onUpdatePosition && (
                  <button 
                    onClick={onUpdatePosition}
                    className="flex items-center gap-1.5 mt-2 px-2.5 py-1.5 text-[11px] font-medium
                      bg-[var(--color-accent)] text-white rounded-lg
                      hover:bg-[var(--color-accent-hover)] transition-colors"
                  >
                    <MapPin className="w-3 h-3" />
                    Set as current position
                  </button>
                )}
              </div>
              <button 
                onClick={onClearQuote}
                className="p-1 hover:bg-[var(--color-bg-elevated)] rounded-md transition-colors"
              >
                <X className="w-4 h-4 text-[var(--color-text-muted)]" />
              </button>
            </div>
          </div>
        )}

        {/* Text Input */}
        <div className="relative flex items-center">
          <input
            ref={inputRef}
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            onPaste={handlePaste}
            placeholder={
              disabled 
                ? "Load a book to start chatting" 
                : attachedQuote 
                  ? "Ask about this quote... (or press Enter to analyze)" 
                  : "Paste a quote or ask a question..."
            }
            disabled={disabled}
            className={`
              w-full bg-[var(--color-bg-tertiary)] border border-[var(--color-border)]
              rounded-full px-4 py-3 pr-12 text-sm
              focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30 focus:border-[var(--color-accent)]
              transition-all placeholder:text-[var(--color-text-muted)]
              disabled:opacity-50 disabled:cursor-not-allowed
            `}
          />
          <button 
            onClick={handleSend}
            disabled={!canSend}
            className={`
              absolute right-2 p-2 rounded-full transition-all
              ${canSend
                ? 'bg-[var(--color-accent)] hover:bg-[var(--color-accent-hover)] text-white' 
                : 'bg-[var(--color-bg-elevated)] text-[var(--color-text-muted)]'
              }
              disabled:opacity-50 disabled:cursor-not-allowed
            `}
          >
            <Send className="w-4 h-4" />
          </button>
        </div>
        {!disabled && (
          <p className="text-[10px] text-[var(--color-text-muted)] mt-2 text-center">
            {attachedQuote 
              ? "Position auto-detected from your quote" 
              : "Paste text to attach as context"}
          </p>
        )}
      </div>
    </div>
  );
}
