import { useState, useMemo, useCallback, useRef, useEffect } from 'react';
import { formatJson, detectContentType } from '../../lib/format-utils.js';
import styles from './ResponseBody.module.css';

interface ResponseBodyProps {
  response: {
    headers: Record<string, string | string[]>;
    bodyText: string;
    bodyJson?: unknown;
  };
}

type ViewMode = 'pretty' | 'raw' | 'preview';

const LARGE_BODY_THRESHOLD = 500_000;

function escapeRegex(s: string) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

interface HighlightedTextProps {
  text: string;
  searchTerm: string;
  currentMatchIndex: number;
  onMatchCount: (count: number) => void;
}

function HighlightedText({ text, searchTerm, currentMatchIndex, onMatchCount }: HighlightedTextProps) {
  const parts = useMemo(() => {
    if (!searchTerm) {
      onMatchCount(0);
      return [{ text, match: false, index: -1 }];
    }
    const regex = new RegExp(escapeRegex(searchTerm), 'gi');
    const segments: { text: string; match: boolean; index: number }[] = [];
    let last = 0;
    let matchIndex = 0;
    let m: RegExpExecArray | null;
    while ((m = regex.exec(text)) !== null) {
      if (m.index > last) {
        segments.push({ text: text.slice(last, m.index), match: false, index: -1 });
      }
      segments.push({ text: m[0], match: true, index: matchIndex++ });
      last = m.index + m[0].length;
    }
    if (last < text.length) {
      segments.push({ text: text.slice(last), match: false, index: -1 });
    }
    onMatchCount(matchIndex);
    return segments;
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [text, searchTerm]);

  return (
    <>
      {parts.map((part, i) => {
        if (!part.match) return <span key={i}>{part.text}</span>;
        const isCurrent = part.index === currentMatchIndex;
        return (
          <mark
            key={i}
            className={isCurrent ? styles.matchCurrent : styles.match}
            data-match-index={part.index}
          >
            {part.text}
          </mark>
        );
      })}
    </>
  );
}

/**
 * Displays the response body with pretty, raw, or preview modes, optional truncation for large payloads, copy, and search.
 */
export function ResponseBody({ response }: ResponseBodyProps) {
  const [viewMode, setViewMode] = useState<ViewMode>('pretty');
  const [showFull, setShowFull] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchTerm, setSearchTerm] = useState('');
  const [matchCount, setMatchCount] = useState(0);
  const [currentMatch, setCurrentMatch] = useState(0);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);

  const contentType = detectContentType(response.headers as Record<string, string | string[]>);

  const isLarge = response.bodyText.length > LARGE_BODY_THRESHOLD;
  const displayText = isLarge && !showFull
    ? response.bodyText.slice(0, LARGE_BODY_THRESHOLD)
    : response.bodyText;

  const prettyBody = useMemo(() => {
    if (contentType !== 'json') return null;
    return formatJson(displayText);
  }, [contentType, displayText]);

  const bodyText = prettyBody?.formatted ?? displayText;

  const openSearch = useCallback(() => {
    setSearchOpen(true);
    setTimeout(() => searchInputRef.current?.focus(), 0);
  }, []);

  const closeSearch = useCallback(() => {
    setSearchOpen(false);
    setSearchTerm('');
    setMatchCount(0);
    setCurrentMatch(0);
  }, []);

  const handleMatchCount = useCallback((count: number) => {
    setMatchCount(count);
    setCurrentMatch((prev) => (count === 0 ? 0 : Math.min(prev, count - 1)));
  }, []);

  const goToNext = useCallback(() => {
    if (matchCount === 0) return;
    setCurrentMatch((prev) => (prev + 1) % matchCount);
  }, [matchCount]);

  const goToPrev = useCallback(() => {
    if (matchCount === 0) return;
    setCurrentMatch((prev) => (prev - 1 + matchCount) % matchCount);
  }, [matchCount]);

  // Scroll current match into view
  useEffect(() => {
    if (!searchOpen || matchCount === 0) return;
    const el = contentRef.current?.querySelector(`[data-match-index="${currentMatch}"]`);
    el?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  }, [currentMatch, searchOpen, matchCount]);

  // Reset current match when search term changes
  useEffect(() => {
    setCurrentMatch(0);
  }, [searchTerm]);

  // Cmd+F / Ctrl+F — global listener, only activates when the response body is mounted
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'f' && contentRef.current) {
        e.preventDefault();
        openSearch();
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [openSearch]);

  const handleSearchKeyDown = useCallback((e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.shiftKey ? goToPrev() : goToNext();
    } else if (e.key === 'Escape') {
      closeSearch();
    }
  }, [goToNext, goToPrev, closeSearch]);

  const showSearch = searchOpen && viewMode !== 'preview';

  return (
    <div className={styles.body}>
      <div className={styles.viewModeRow}>
        {(['pretty', 'raw', 'preview'] as const).map((mode) => (
          <button
            key={mode}
            className={`${styles.viewBtn} ${viewMode === mode ? styles.activeView : ''}`}
            onClick={() => setViewMode(mode)}
          >
            {mode.charAt(0).toUpperCase() + mode.slice(1)}
          </button>
        ))}
        <div className={styles.viewModeRowRight}>
          {viewMode !== 'preview' && (
            <button
              className={`${styles.iconBtn} ${searchOpen ? styles.iconBtnActive : ''}`}
              onClick={searchOpen ? closeSearch : openSearch}
              aria-label="Search response body"
              title="Search (⌘F / Ctrl+F)"
            >
              <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                <circle cx="6.5" cy="6.5" r="4.5" stroke="currentColor" strokeWidth="1.5"/>
                <line x1="10.5" y1="10.5" x2="14" y2="14" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
              </svg>
            </button>
          )}
          <button
            className={styles.copyBtn}
            onClick={() => navigator.clipboard.writeText(response.bodyText)}
            aria-label="Copy response body"
          >
            Copy
          </button>
        </div>
      </div>

      {showSearch && (
        <div className={styles.searchBar}>
          <input
            ref={searchInputRef}
            className={styles.searchInput}
            type="text"
            placeholder="Search..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            onKeyDown={handleSearchKeyDown}
            aria-label="Search in response body"
          />
          <span className={styles.matchCount}>
            {matchCount === 0 ? (searchTerm ? 'No results' : '') : `${currentMatch + 1} / ${matchCount}`}
          </span>
          <button className={styles.searchNavBtn} onClick={goToPrev} disabled={matchCount === 0} title="Previous match (Shift+Enter)">
            ↑
          </button>
          <button className={styles.searchNavBtn} onClick={goToNext} disabled={matchCount === 0} title="Next match (Enter)">
            ↓
          </button>
          <button className={styles.searchCloseBtn} onClick={closeSearch} title="Close (Esc)">
            ✕
          </button>
        </div>
      )}

      <div className={styles.content} ref={contentRef} tabIndex={-1}>
        {viewMode === 'pretty' && (
          <pre className={styles.pre}>
            <code>
              {showSearch && searchTerm ? (
                <HighlightedText
                  text={bodyText}
                  searchTerm={searchTerm}
                  currentMatchIndex={currentMatch}
                  onMatchCount={handleMatchCount}
                />
              ) : bodyText}
            </code>
          </pre>
        )}
        {viewMode === 'raw' && (
          <pre className={styles.pre}>
            <code>
              {showSearch && searchTerm ? (
                <HighlightedText
                  text={displayText}
                  searchTerm={searchTerm}
                  currentMatchIndex={currentMatch}
                  onMatchCount={handleMatchCount}
                />
              ) : displayText}
            </code>
          </pre>
        )}
        {viewMode === 'preview' && contentType === 'html' && (
          <iframe
            className={styles.preview}
            srcDoc={response.bodyText}
            sandbox=""
            title="Response preview"
          />
        )}
        {viewMode === 'preview' && contentType !== 'html' && (
          <pre className={styles.pre}>
            <code>{prettyBody?.formatted ?? displayText}</code>
          </pre>
        )}
        {isLarge && !showFull && viewMode !== 'preview' && (
          <div className={styles.truncatedNotice}>
            <span>Response truncated ({(response.bodyText.length / 1024).toFixed(0)} KB)</span>
            <button className={styles.showAllBtn} onClick={() => setShowFull(true)}>
              Show All
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
