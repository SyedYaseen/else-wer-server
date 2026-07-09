import { useState } from 'react';
import { seek } from '../../player/engine';
import { formatDuration } from '../../lib/format';
import './player.css';

interface SeekerProps {
  currentTime: number;
  duration: number;
}

export function Seeker({ currentTime, duration }: SeekerProps) {
  const [seeking, setSeeking] = useState(false);
  const [localTime, setLocalTime] = useState(0);

  const ready = duration > 0;
  const displayTime = seeking ? localTime : ready ? currentTime : 0;

  function commit(value: number) {
    seek(value);
    setSeeking(false);
  }

  return (
    <div className="seeker">
      <div className="seeker-time-row">
        <span>{formatDuration(displayTime * 1000)}</span>
        <span>{formatDuration((ready ? duration : 0) * 1000)}</span>
      </div>
      <input
        type="range"
        className="seeker-slider"
        min={0}
        max={ready ? duration : 1}
        step={0.1}
        value={displayTime}
        onChange={(e) => {
          setSeeking(true);
          setLocalTime(Number(e.target.value));
        }}
        onPointerUp={(e) => commit(Number((e.target as HTMLInputElement).value))}
        onKeyUp={(e) => commit(Number((e.target as HTMLInputElement).value))}
      />
    </div>
  );
}
