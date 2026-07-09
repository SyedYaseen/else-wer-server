import { useNavigate } from 'react-router-dom';
import { usePlayerStore } from '../../store/player';
import { coverUrl } from '../../api/books';
import { togglePlay } from '../../player/engine';
import { PlayIcon, PauseIcon } from './icons';
import { ProgressBar } from '../ui/ProgressBar';
import './player.css';

export function MiniPlayer() {
  const navigate = useNavigate();
  const book = usePlayerStore((s) => s.book);
  const playing = usePlayerStore((s) => s.playing);
  const currentTime = usePlayerStore((s) => s.currentTime);
  const duration = usePlayerStore((s) => s.duration);

  if (!book) return null;

  const cover = coverUrl(book.cover_art);
  const progress = duration > 0 ? currentTime / duration : 0;

  return (
    <div className="mini-player" onClick={() => navigate(`/player/${book.id}`)}>
      {cover ? (
        <img className="mini-player-cover" src={cover} alt="" />
      ) : (
        <div className="mini-player-cover-placeholder">{book.title?.charAt(0).toUpperCase() ?? '?'}</div>
      )}
      <div className="mini-player-text">
        <div className="mini-player-title">{book.title}</div>
        <div className="mini-player-author">{book.author}</div>
      </div>
      <button
        className="mini-player-play"
        onClick={(e) => {
          e.stopPropagation();
          togglePlay();
        }}
        aria-label={playing ? 'Pause' : 'Play'}
      >
        {playing ? <PauseIcon size={36} /> : <PlayIcon size={36} />}
      </button>
      <ProgressBar value={progress} className="mini-player-progress" />
    </div>
  );
}
