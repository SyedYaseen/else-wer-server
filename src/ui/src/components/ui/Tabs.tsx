import './ui.css';

export interface TabDef {
  id: string;
  label: string;
}

interface TabsProps {
  tabs: TabDef[];
  activeId: string;
  onChange: (id: string) => void;
}

export function Tabs({ tabs, activeId, onChange }: TabsProps) {
  return (
    <div className="tabs" role="tablist">
      {tabs.map((tab) => (
        <button
          key={tab.id}
          role="tab"
          aria-selected={tab.id === activeId}
          className={`tab ${tab.id === activeId ? 'tab-active' : ''}`}
          onClick={() => onChange(tab.id)}
        >
          {tab.label}
        </button>
      ))}
    </div>
  );
}
