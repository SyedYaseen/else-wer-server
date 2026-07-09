import './library.css';

interface SearchBarProps {
  value: string;
  onChange: (value: string) => void;
  resultCount?: number;
  hasQuery: boolean;
}

export function SearchBar({ value, onChange, resultCount, hasQuery }: SearchBarProps) {
  return (
    <div className="search-bar">
      <div className="search-bar-input-row">
        <input
          className="search-bar-input"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder="Search titles, authors, series…"
          autoCorrect="off"
          autoCapitalize="none"
        />
        {value.length > 0 && (
          <button className="search-bar-clear" onClick={() => onChange('')} aria-label="Clear search">
            ×
          </button>
        )}
      </div>
      {hasQuery && resultCount !== undefined && (
        <div className="search-bar-count">
          {resultCount === 0 ? 'No results' : `${resultCount} ${resultCount === 1 ? 'book' : 'books'}`}
        </div>
      )}
    </div>
  );
}
