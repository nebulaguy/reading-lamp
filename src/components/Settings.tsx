import { useState, useEffect } from 'react';
import { X, Key, Check, AlertCircle, Loader2, ChevronDown, Sun, Moon, Shield, ShieldOff } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

interface SettingsProps {
  isOpen: boolean;
  onClose: () => void;
  theme: 'light' | 'dark';
  onThemeChange: (theme: 'light' | 'dark') => void;
  spoilerMode: boolean;
  onSpoilerModeChange: (enabled: boolean) => void;
}

const GEMINI_MODELS = [
  { id: 'gemini-2.5-flash-lite', name: 'Gemini 2.5 Flash Lite', description: 'Smallest and most cost effective model' },
  { id: 'gemini-2.5-flash', name: 'Gemini 2.5 Flash', description: 'Recommended - hybrid reasoning, 1M context' },
  { id: 'gemini-3-pro-preview', name: 'Gemini 3 Pro (Preview)', description: 'Most powerful, best quality' },
];

export function Settings({ isOpen, onClose, theme, onThemeChange, spoilerMode, onSpoilerModeChange }: SettingsProps) {
  const [apiKey, setApiKey] = useState('');
  const [hasExistingKey, setHasExistingKey] = useState(false);
  const [selectedModel, setSelectedModel] = useState('gemini-2.5-flash');
  const [isSaving, setIsSaving] = useState(false);
  const [saveStatus, setSaveStatus] = useState<'idle' | 'success' | 'error'>('idle');
  const [errorMessage, setErrorMessage] = useState('');

  useEffect(() => {
    if (isOpen) {
      checkExistingKey();
      loadCurrentModel();
    }
  }, [isOpen]);

  const checkExistingKey = async () => {
    try {
      const hasKey = await invoke<boolean>('has_api_key');
      setHasExistingKey(hasKey);
    } catch (err) {
      console.error('Failed to check API key:', err);
    }
  };

  const loadCurrentModel = async () => {
    try {
      const model = await invoke<string>('get_gemini_model');
      setSelectedModel(model);
    } catch (err) {
      console.error('Failed to load model:', err);
    }
  };

  const handleSaveApiKey = async () => {
    if (!apiKey.trim()) {
      setErrorMessage('Please enter an API key');
      setSaveStatus('error');
      return;
    }

    setIsSaving(true);
    setSaveStatus('idle');
    setErrorMessage('');

    try {
      await invoke('set_api_key', { apiKey: apiKey.trim() });
      setSaveStatus('success');
      setHasExistingKey(true);
      setApiKey('');
      
      setTimeout(() => {
        setSaveStatus('idle');
      }, 2000);
    } catch (err) {
      console.error('Failed to save API key:', err);
      setErrorMessage(`Failed to save: ${err}`);
      setSaveStatus('error');
    } finally {
      setIsSaving(false);
    }
  };

  const handleModelChange = async (e: React.ChangeEvent<HTMLSelectElement>) => {
    const modelId = e.target.value;
    setSelectedModel(modelId);
    try {
      await invoke('set_gemini_model', { model: modelId });
    } catch (err) {
      console.error('Failed to save model:', err);
    }
  };

  const selectedModelInfo = GEMINI_MODELS.find(m => m.id === selectedModel);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div 
        className="absolute inset-0 bg-black/40 backdrop-blur-sm"
        onClick={onClose}
      />
      
      {/* Modal */}
      <div className="relative w-[90%] max-w-md bg-[var(--color-bg-elevated)] rounded-2xl shadow-2xl border border-[var(--color-border)] animate-slide-up">
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-[var(--color-border)]">
          <h2 className="text-lg font-semibold text-[var(--color-text-primary)]">Settings</h2>
          <button 
            onClick={onClose}
            className="p-1.5 hover:bg-[var(--color-bg-tertiary)] rounded-lg transition-colors"
          >
            <X className="w-5 h-5 text-[var(--color-text-muted)]" />
          </button>
        </div>

        {/* Content */}
        <div className="p-4 space-y-5">
          
          {/* Theme Toggle */}
          <div>
            <label className="block text-sm font-medium text-[var(--color-text-primary)] mb-3">
              Appearance
            </label>
            <div className="flex gap-2">
              <button
                onClick={() => onThemeChange('light')}
                className={`flex-1 flex items-center justify-center gap-2 p-3 rounded-xl border transition-all ${
                  theme === 'light'
                    ? 'border-[var(--color-accent)] bg-[var(--color-accent-muted)]'
                    : 'border-[var(--color-border)] hover:border-[var(--color-border-hover)]'
                }`}
              >
                <Sun className={`w-4 h-4 ${theme === 'light' ? 'text-[var(--color-accent)]' : 'text-[var(--color-text-muted)]'}`} />
                <span className={`text-sm ${theme === 'light' ? 'text-[var(--color-accent)]' : 'text-[var(--color-text-secondary)]'}`}>
                  Light
                </span>
              </button>
              <button
                onClick={() => onThemeChange('dark')}
                className={`flex-1 flex items-center justify-center gap-2 p-3 rounded-xl border transition-all ${
                  theme === 'dark'
                    ? 'border-[var(--color-accent)] bg-[var(--color-accent-muted)]'
                    : 'border-[var(--color-border)] hover:border-[var(--color-border-hover)]'
                }`}
              >
                <Moon className={`w-4 h-4 ${theme === 'dark' ? 'text-[var(--color-accent)]' : 'text-[var(--color-text-muted)]'}`} />
                <span className={`text-sm ${theme === 'dark' ? 'text-[var(--color-accent)]' : 'text-[var(--color-text-secondary)]'}`}>
                  Dark
                </span>
              </button>
            </div>
          </div>

          {/* Spoiler Protection Toggle */}
          <div>
            <label className="block text-sm font-medium text-[var(--color-text-primary)] mb-3">
              Spoiler Protection
            </label>
            <div className="flex gap-2">
              <button
                onClick={() => onSpoilerModeChange(true)}
                className={`flex-1 flex items-center justify-center gap-2 p-3 rounded-xl border transition-all ${
                  spoilerMode
                    ? 'border-[var(--color-accent)] bg-[var(--color-accent-muted)]'
                    : 'border-[var(--color-border)] hover:border-[var(--color-border-hover)]'
                }`}
              >
                <Shield className={`w-4 h-4 ${spoilerMode ? 'text-[var(--color-accent)]' : 'text-[var(--color-text-muted)]'}`} />
                <span className={`text-sm ${spoilerMode ? 'text-[var(--color-accent)]' : 'text-[var(--color-text-secondary)]'}`}>
                  On
                </span>
              </button>
              <button
                onClick={() => onSpoilerModeChange(false)}
                className={`flex-1 flex items-center justify-center gap-2 p-3 rounded-xl border transition-all ${
                  !spoilerMode
                    ? 'border-[var(--color-accent)] bg-[var(--color-accent-muted)]'
                    : 'border-[var(--color-border)] hover:border-[var(--color-border-hover)]'
                }`}
              >
                <ShieldOff className={`w-4 h-4 ${!spoilerMode ? 'text-[var(--color-accent)]' : 'text-[var(--color-text-muted)]'}`} />
                <span className={`text-sm ${!spoilerMode ? 'text-[var(--color-accent)]' : 'text-[var(--color-text-secondary)]'}`}>
                  Off
                </span>
              </button>
            </div>
            <p className="text-xs text-[var(--color-text-muted)] mt-2">
              {spoilerMode 
                ? "AI won't reveal events after your current reading position"
                : "AI can discuss the entire book (for re-reads)"
              }
            </p>
          </div>

          {/* Model Selection */}
          <div>
            <label className="block text-sm font-medium text-[var(--color-text-primary)] mb-2">
              AI Model
            </label>
            
            <div className="relative">
              <select
                value={selectedModel}
                onChange={handleModelChange}
                className="w-full appearance-none bg-[var(--color-bg-tertiary)] border border-[var(--color-border)] rounded-xl px-4 py-2.5 pr-10 text-sm text-[var(--color-text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30 focus:border-[var(--color-accent)] transition-all cursor-pointer"
              >
                {GEMINI_MODELS.map((model) => (
                  <option key={model.id} value={model.id}>
                    {model.name}
                  </option>
                ))}
              </select>
              <ChevronDown className="absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 text-[var(--color-text-muted)] pointer-events-none" />
            </div>
            
            {selectedModelInfo && (
              <p className="text-xs text-[var(--color-text-muted)] mt-2">
                {selectedModelInfo.description}
              </p>
            )}
          </div>

          {/* API Key Section */}
          <div>
            <label className="flex items-center gap-2 text-sm font-medium text-[var(--color-text-primary)] mb-2">
              <Key className="w-4 h-4 text-[var(--color-accent)]" />
              Gemini API Key
            </label>
            
            {hasExistingKey && (
              <div className="flex items-center gap-2 mb-3 p-2 bg-[var(--color-success)]/10 border border-[var(--color-success)]/20 rounded-lg">
                <Check className="w-4 h-4 text-[var(--color-success)]" />
                <span className="text-xs text-[var(--color-success)]">API key configured</span>
              </div>
            )}

            <div className="flex gap-2">
              <input
                type="password"
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                placeholder={hasExistingKey ? "Enter new key to replace" : "Enter your Gemini API key"}
                className="flex-1 bg-[var(--color-bg-tertiary)] border border-[var(--color-border)] rounded-xl px-4 py-2.5 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-muted)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30 focus:border-[var(--color-accent)] transition-all"
              />
              <button
                onClick={handleSaveApiKey}
                disabled={isSaving || !apiKey.trim()}
                className="px-4 py-2 text-sm bg-[var(--color-accent)] hover:bg-[var(--color-accent-hover)] text-[var(--color-bg-primary)] rounded-xl transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
              >
                {isSaving ? <Loader2 className="w-4 h-4 animate-spin" /> : 'Save'}
              </button>
            </div>

            <p className="text-xs text-[var(--color-text-muted)] mt-2">
              Get your key from{' '}
              <a 
                href="https://aistudio.google.com/apikey" 
                target="_blank" 
                rel="noopener noreferrer"
                className="text-[var(--color-accent)] hover:underline"
              >
                Google AI Studio
              </a>
            </p>
          </div>

          {/* Error Message */}
          {saveStatus === 'error' && errorMessage && (
            <div className="flex items-center gap-2 p-3 bg-[var(--color-error)]/10 border border-[var(--color-error)]/20 rounded-lg animate-fade-in">
              <AlertCircle className="w-4 h-4 text-[var(--color-error)]" />
              <span className="text-xs text-[var(--color-error)]">{errorMessage}</span>
            </div>
          )}

          {/* Success Message */}
          {saveStatus === 'success' && (
            <div className="flex items-center gap-2 p-3 bg-[var(--color-success)]/10 border border-[var(--color-success)]/20 rounded-lg animate-fade-in">
              <Check className="w-4 h-4 text-[var(--color-success)]" />
              <span className="text-xs text-[var(--color-success)]">API key saved!</span>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="p-4 border-t border-[var(--color-border)] flex justify-end">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm bg-[var(--color-bg-tertiary)] hover:bg-[var(--color-bg-elevated)] text-[var(--color-text-primary)] rounded-xl transition-colors"
          >
            Done
          </button>
        </div>
      </div>
    </div>
  );
}
