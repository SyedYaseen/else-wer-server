import { useEffect, useRef, useState } from 'react';

interface SearchableComboboxProps<T> {
  items: T[];
  getLabel: (item: T) => string;
  value: string;
  placeholder?: string;
  disabled?: boolean;
  onSelectExisting: (item: T) => void;
  onCreateNew: (text: string) => void;
  onQueryChange?: (text: string) => void;
}

// Shared "does this label match the typed query" predicate - used by the
// combobox's own filtering below and by callers (e.g. PickBookSheet) that
// filter a list using the same case-insensitive substring rule.
export function matchesQuery(label: string, query: string): boolean {
  const q = query.trim().toLowerCase();
  return !q || label.toLowerCase().includes(q);
}

// Generic filter-as-you-type combobox: offers existing items whose label
// contains the typed text, plus a "Create new: <text>" row when the typed
// text doesn't exactly match an existing item. Used for picking/creating
// authors and books so users select the canonical existing entry instead of
// retyping it (and accidentally creating a near-duplicate).
export function SearchableCombobox<T>({
  items,
  getLabel,
  value,
  placeholder,
  disabled,
  onSelectExisting,
  onCreateNew,
  onQueryChange,
}: SearchableComboboxProps<T>) {
  const [query, setQuery] = useState(value);
  const [isOpen, setIsOpen] = useState(false);
  const [placement, setPlacement] = useState({ openUpward: false, maxHeight: 220 });
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setQuery(value);
  }, [value]);

  useEffect(() => {
    function onOutside(e: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setIsOpen(false);
      }
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === 'Escape') setIsOpen(false);
    }
    document.addEventListener('mousedown', onOutside);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onOutside);
      document.removeEventListener('keydown', onKey);
    };
  }, []);

  // Flip the dropdown above the input when there isn't enough room below in
  // the visible viewport (e.g. the on-screen keyboard has eaten the bottom
  // half) - visualViewport tracks the actually-visible area, unlike
  // window.innerHeight which some mobile browsers don't shrink for the keyboard.
  useEffect(() => {
    if (!isOpen) return;

    function updatePlacement() {
      const el = containerRef.current;
      if (!el) return;
      const rect = el.getBoundingClientRect();
      const vv = window.visualViewport;
      const viewportBottom = (vv?.offsetTop ?? 0) + (vv?.height ?? window.innerHeight);
      const spaceBelow = viewportBottom - rect.bottom;
      const spaceAbove = rect.top;
      const preferred = 220;
      const margin = 8;

      if (spaceBelow < preferred && spaceAbove > spaceBelow) {
        setPlacement({ openUpward: true, maxHeight: Math.max(120, Math.min(preferred, spaceAbove - margin)) });
      } else {
        setPlacement({ openUpward: false, maxHeight: Math.max(120, Math.min(preferred, spaceBelow - margin)) });
      }
    }

    updatePlacement();
    window.visualViewport?.addEventListener('resize', updatePlacement);
    window.addEventListener('resize', updatePlacement);
    return () => {
      window.visualViewport?.removeEventListener('resize', updatePlacement);
      window.removeEventListener('resize', updatePlacement);
    };
  }, [isOpen]);

  const q = query.trim().toLowerCase();
  const filtered = items.filter((item) => matchesQuery(getLabel(item), query));
  const exactMatch = q && items.some((item) => getLabel(item).toLowerCase() === q);

  return (
    <div className="combobox" ref={containerRef}>
      <input
        className="organize-input"
        placeholder={placeholder}
        value={query}
        disabled={disabled}
        onChange={(e) => {
          setQuery(e.target.value);
          onQueryChange?.(e.target.value);
          setIsOpen(true);
        }}
        onFocus={() => setIsOpen(true)}
      />
      {isOpen && !disabled && (
        <div
          className={`combobox-panel${placement.openUpward ? ' combobox-panel-up' : ''}`}
          style={{ maxHeight: placement.maxHeight }}
        >
          {q && !exactMatch && (
            <button
              type="button"
              className="combobox-item combobox-item-create"
              onClick={() => {
                onCreateNew(query.trim());
                setIsOpen(false);
              }}
            >
              Create new: "{query.trim()}"
            </button>
          )}
          {filtered.map((item, i) => {
            const label = getLabel(item);
            return (
              <button
                type="button"
                key={`${label}-${i}`}
                className="combobox-item"
                onClick={() => {
                  onSelectExisting(item);
                  setQuery(label);
                  setIsOpen(false);
                }}
              >
                {label}
              </button>
            );
          })}
          {filtered.length === 0 && !q && <div className="combobox-empty">No existing entries</div>}
        </div>
      )}
    </div>
  );
}
