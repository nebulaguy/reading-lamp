import { invoke } from '@tauri-apps/api/core';
import type { Book, ReadingState, MatchResult, Message } from './types';

// Book management
export async function loadBook(filepath: string): Promise<Book> {
  return invoke('load_book', { filepath });
}

export async function getReadingState(bookId: string): Promise<ReadingState | null> {
  return invoke('get_reading_state', { bookId });
}

export async function updateReadingPosition(bookId: string, position: { chapterIndex: number; charOffset: number }): Promise<void> {
  return invoke('update_reading_position', { bookId, position });
}

// Clipboard and matching
export async function matchClipboardText(text: string, bookId: string): Promise<MatchResult | null> {
  return invoke('match_clipboard_text', { text, bookId });
}

export async function startClipboardMonitoring(bookId: string): Promise<void> {
  return invoke('start_clipboard_monitoring', { bookId });
}

export async function stopClipboardMonitoring(): Promise<void> {
  return invoke('stop_clipboard_monitoring');
}

// Chat
export async function sendChatMessage(
  messages: Message[],
  bookContext: {
    title: string;
    author: string;
    currentChapter: string;
    percentComplete: number;
    spoilerBoundaryChapter: number;
    passageBeingDiscussed: string;
  }
): Promise<string> {
  return invoke('send_chat_message', { messages, bookContext });
}

// Settings
export async function setApiKey(apiKey: string): Promise<void> {
  return invoke('set_api_key', { apiKey });
}

export async function hasApiKey(): Promise<boolean> {
  return invoke('has_api_key');
}
