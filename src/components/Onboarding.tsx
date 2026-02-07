import { useState } from 'react';
import { ArrowRight, Key, BookOpen, Loader2, CheckCircle, ExternalLink } from 'lucide-react';

interface OnboardingProps {
  onComplete: (apiKey: string) => void;
  onUploadBook: () => void;
  isUploading: boolean;
  bookUploaded: boolean;
}

export function Onboarding({ onComplete, onUploadBook, isUploading, bookUploaded }: OnboardingProps) {
  const [step, setStep] = useState(1);
  const [apiKey, setApiKey] = useState('');
  const [isValidating, setIsValidating] = useState(false);
  const [error, setError] = useState('');

  const handleApiKeySubmit = async () => {
    if (!apiKey.trim()) {
      setError('Please enter your API key');
      return;
    }

    setIsValidating(true);
    setError('');

    // Simple validation - just check if it looks like an API key
    if (apiKey.length < 20) {
      setError('API key seems too short');
      setIsValidating(false);
      return;
    }

    // Move to next step
    setIsValidating(false);
    setStep(2);
  };

  const handleFinish = () => {
    if (bookUploaded) {
      onComplete(apiKey);
    }
  };

  return (
    <div className="h-full flex flex-col bg-[var(--color-bg-secondary)]">
      {/* Header with lamp logo */}
      <div className="flex flex-col items-center pt-12 pb-8">
        {/* Lamp icon with glow */}
        <div className="relative mb-4">
          <div className="absolute -inset-4 bg-amber-400/20 rounded-full blur-xl"></div>
          <svg width="48" height="48" viewBox="0 0 24 24" fill="none" className="relative">
            <path d="M12 2L12 4M12 4L9 7M12 4L15 7" stroke="var(--color-accent)" strokeWidth="1.5" strokeLinecap="round" opacity="0.6"/>
            <path d="M7 10C7 10 8 6 12 6C16 6 17 10 17 10L16 14H8L7 10Z" fill="var(--color-accent)" />
            <rect x="9" y="14" width="6" height="2" rx="0.5" fill="var(--color-accent)" />
            <rect x="10" y="16" width="4" height="2" rx="0.5" fill="var(--color-accent)" />
            <ellipse cx="12" cy="11" rx="3" ry="2" fill="var(--color-bg-elevated)" opacity="0.5"/>
          </svg>
        </div>
        <h1 className="text-xl font-semibold text-[var(--color-text-primary)]">
          Welcome to Reading Lamp
        </h1>
        <p className="text-sm text-[var(--color-text-muted)] mt-1">
          Your AI-powered reading companion
        </p>
      </div>

      {/* Step indicator */}
      <div className="flex justify-center gap-2 mb-8">
        {[1, 2].map((s) => (
          <div
            key={s}
            className={`
              w-2 h-2 rounded-full transition-colors
              ${s === step 
                ? 'bg-[var(--color-accent)]' 
                : s < step 
                  ? 'bg-[var(--color-success)]'
                  : 'bg-[var(--color-border)]'
              }
            `}
          />
        ))}
      </div>

      {/* Step content */}
      <div className="flex-1 px-6 overflow-y-auto">
        {step === 1 && (
          <div className="animate-fade-in">
            <div className="flex items-center gap-2 mb-4">
              <div className="w-8 h-8 rounded-full bg-[var(--color-accent-muted)] flex items-center justify-center">
                <Key className="w-4 h-4 text-[var(--color-accent)]" />
              </div>
              <h2 className="font-semibold text-[var(--color-text-primary)]">
                Set up Gemini API
              </h2>
            </div>

            <p className="text-sm text-[var(--color-text-secondary)] mb-4">
              Reading Lamp uses Google's Gemini AI to analyze your books. 
              You'll need a free API key from Google AI Studio.
            </p>

            <a
              href="https://aistudio.google.com/app/apikey"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-1.5 text-sm text-[var(--color-accent)] hover:underline mb-4"
            >
              Get your free API key
              <ExternalLink className="w-3.5 h-3.5" />
            </a>

            <div className="space-y-3">
              <input
                type="password"
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                placeholder="Paste your Gemini API key"
                className="w-full bg-[var(--color-bg-tertiary)] border border-[var(--color-border)]
                  rounded-xl px-4 py-3 text-sm
                  focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30 focus:border-[var(--color-accent)]
                  placeholder:text-[var(--color-text-muted)]"
              />

              {error && (
                <p className="text-sm text-[var(--color-error)]">{error}</p>
              )}

              <button
                onClick={handleApiKeySubmit}
                disabled={isValidating || !apiKey.trim()}
                className="w-full bg-[var(--color-accent)] hover:bg-[var(--color-accent-hover)] 
                  text-white font-medium py-3 rounded-xl flex items-center justify-center gap-2
                  disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
              >
                {isValidating ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" />
                    Validating...
                  </>
                ) : (
                  <>
                    Continue
                    <ArrowRight className="w-4 h-4" />
                  </>
                )}
              </button>
            </div>
          </div>
        )}

        {step === 2 && (
          <div className="animate-fade-in">
            <div className="flex items-center gap-2 mb-4">
              <div className="w-8 h-8 rounded-full bg-[var(--color-accent-muted)] flex items-center justify-center">
                <BookOpen className="w-4 h-4 text-[var(--color-accent)]" />
              </div>
              <h2 className="font-semibold text-[var(--color-text-primary)]">
                Add your first book
              </h2>
            </div>

            <p className="text-sm text-[var(--color-text-secondary)] mb-6">
              Upload an EPUB file to get started. You can add more books later from your library.
            </p>

            {!bookUploaded ? (
              <button
                onClick={onUploadBook}
                disabled={isUploading}
                className="w-full aspect-video rounded-2xl border-2 border-dashed border-[var(--color-border)] 
                  hover:border-[var(--color-accent)] hover:bg-[var(--color-accent-muted)]
                  flex flex-col items-center justify-center gap-3 transition-colors
                  disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {isUploading ? (
                  <>
                    <Loader2 className="w-8 h-8 animate-spin text-[var(--color-accent)]" />
                    <span className="text-sm text-[var(--color-text-muted)]">
                      Processing book...
                    </span>
                  </>
                ) : (
                  <>
                    <BookOpen className="w-8 h-8 text-[var(--color-accent)]" />
                    <span className="text-sm font-medium text-[var(--color-text-secondary)]">
                      Click to upload EPUB
                    </span>
                  </>
                )}
              </button>
            ) : (
              <div className="w-full p-4 rounded-2xl bg-[var(--color-success)]/10 border border-[var(--color-success)]/30
                flex items-center gap-3">
                <CheckCircle className="w-6 h-6 text-[var(--color-success)]" />
                <div>
                  <p className="font-medium text-[var(--color-text-primary)]">Book uploaded!</p>
                  <p className="text-sm text-[var(--color-text-muted)]">Ready to start reading</p>
                </div>
              </div>
            )}

            {bookUploaded && (
              <button
                onClick={handleFinish}
                className="w-full mt-4 bg-[var(--color-accent)] hover:bg-[var(--color-accent-hover)] 
                  text-white font-medium py-3 rounded-xl flex items-center justify-center gap-2
                  transition-colors"
              >
                Start Reading
                <ArrowRight className="w-4 h-4" />
              </button>
            )}
          </div>
        )}
      </div>

      {/* Footer */}
      <div className="p-4 text-center">
        <p className="text-[10px] text-[var(--color-text-muted)]">
          Your data stays on your device. We never see your books or conversations.
        </p>
      </div>
    </div>
  );
}
