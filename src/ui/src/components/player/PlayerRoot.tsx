import { useLocation } from 'react-router-dom';
import { usePlayerStore } from '../../store/player';
import { MiniPlayer } from './MiniPlayer';

// Mounted once outside <Routes> (see App.tsx) so it — and the underlying
// <audio> element in player/engine.ts — survives route navigation.
export function PlayerRoot() {
  const book = usePlayerStore((s) => s.book);
  const location = useLocation();

  if (!book) return null;
  if (location.pathname === '/login' || location.pathname.startsWith('/player/')) return null;

  return <MiniPlayer />;
}
