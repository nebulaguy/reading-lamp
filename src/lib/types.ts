// Book and reading state types

export interface Book {
  id: string;
  title: string;
  author: string;
  filepath: string;
  contentHash: string;
  chapters: Chapter[];
  totalCharacters: number;
  chunkCount: number;
  fromCache: boolean;
}

export interface Chapter {
  index: number;
  title: string;
  startOffset: number;
  endOffset: number;
}

export interface Position {
  chapterIndex: number;
  charOffset: number;
  percentComplete: number;
}

export interface ReadingState {
  bookId: string;
  highWaterMark: Position;
  currentContext: Position;
  updatedAt: string;
}

export interface MatchResult {
  position: Position;
  matchedText: string;
  confidence: number;
  isRetrospective: boolean;
  chapterTitle: string;
}

export interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: string;
  contextPosition?: Position;
}

export interface AppState {
  book: Book | null;
  readingState: ReadingState | null;
  messages: Message[];
  pendingMatch: MatchResult | null;
  isMonitoring: boolean;
  isLoading: boolean;
}

export type ContextMode = 'retrospective' | 'current' | 'full';

export interface AttachedQuote {
  text: string;
  position: Position | null;
  chapterTitle: string | null;
  isLocating: boolean;
}

export interface CachedBook {
  id: string;
  title: string;
  author: string;
  totalChapters: number;
  readingProgress: number;
  hasEmbeddings: boolean;
}

