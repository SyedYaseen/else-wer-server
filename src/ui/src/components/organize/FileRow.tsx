import type { FileInfo } from '../../types/scan';
import { PencilIcon } from './icons';

interface FileRowProps {
  file: FileInfo;
  selected: boolean;
  onToggleSelect: (id: number) => void;
  onRename: (file: FileInfo) => void;
}

export function FileRow({ file, selected, onToggleSelect, onRename }: FileRowProps) {
  return (
    <div className="file-row">
      <input
        type="checkbox"
        className="file-checkbox"
        checked={selected}
        onChange={() => onToggleSelect(file.id)}
      />
      <span className="file-row-name">{file.file_name}</span>
      <button className="icon-btn" aria-label="Rename file" onClick={() => onRename(file)}>
        <PencilIcon size={14} />
      </button>
    </div>
  );
}
