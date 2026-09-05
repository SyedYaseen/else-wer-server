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

  // Non-finite guard, not just > 0: WebKit reports Infinity for a streamed VBR
  // MP3 with no Xing header, which React renders as max="Infinity" — the browser
  // rejects it and the slider becomes undraggable.
  const ready = Number.isFinite(duration) && duration > 0;
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
        // Without this, dragging before the duration is known commits a seek to
        // somewhere in 0..1 — i.e. jumps the book back to its start.
        disabled={!ready}
        onChange={(e) => {
          setSeeking(true);
          setLocalTime(Number(e.target.value));
        }}
        // Capturing the pointer guarantees pointerup/pointercancel land back on
        // the slider even if the finger lifts outside it. Otherwise `seeking`
        // latches true and the handle stops tracking playback entirely.
        onPointerDown={(e) => e.currentTarget.setPointerCapture(e.pointerId)}
        onPointerUp={(e) => commit(Number((e.target as HTMLInputElement).value))}
        onPointerCancel={() => setSeeking(false)}
        onLostPointerCapture={() => setSeeking(false)}
        onKeyUp={(e) => commit(Number((e.target as HTMLInputElement).value))}
      />
    </div>
  );
}
