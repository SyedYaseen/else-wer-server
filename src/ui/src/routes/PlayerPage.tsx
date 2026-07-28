import { useEffect } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { usePlayerStore } from '../store/player';
import { listBooks, fileMetadata, coverUrl } from '../api/books';
import { getBookProgress } from '../api/progress';
import { resolveResumePoint, reconcileProgress, getLocalProgress } from '../lib/playerResume';
import { loadBook, togglePlay, skip, saveProgressNow } from '../player/engine';
import { getOfflineBookData } from '../offline/storage';
import { Seeker } from '../components/player/Seeker';
import { PlaybackSpeedMenu } from '../components/player/PlaybackSpeedMenu';
import { SleepTimerSheet } from '../components/player/SleepTimerSheet';
import { ChaptersSheet } from '../components/player/ChaptersSheet';
import { BookmarksSheet } from '../components/player/BookmarksSheet';
import { PlayIcon, PauseIcon, RewindIcon, ForwardIcon, ChevronDownIcon } from '../components/player/icons';
import { CheckIcon } from '../components/ui/icons';
import { showToast } from '../lib/toast';
import '../components/player/player.css';

async function loadPlayerData(bookId: number) {
  try {
    const [books, files, progress] = await Promise.all([
      listBooks(),
      fileMetadata(bookId),
      getBookProgress(bookId),
    ]);
    const book = books.find((b) => b.id === bookId);
    if (!book) throw new Error('Book not found');
    const local = getLocalProgress(bookId);
    return { book, files, resume: reconcileProgress(files, progress, local) };
  } catch (e) {
    // Network unreachable (or book not found live) — fall back to a
    // downloaded book's local metadata snapshot so cold-starting the app
    // offline can still reach playback instead of dead-ending here.
    const offline = await getOfflineBookData(bookId);
    if (!offline) throw e;
    const progress = getLocalProgress(bookId);
    return { book: offline.book, files: offline.files, resume: resolveResumePoint(offline.files, progress) };
  }
}

export function PlayerPage() {
  const { id } = useParams<{ id: string }>();
  const bookId = Number(id);
  const navigate = useNavigate();

  const storeBook = usePlayerStore((s) => s.book);
  const files = usePlayerStore((s) => s.files);
  const index = usePlayerStore((s) => s.index);
  const playing = usePlayerStore((s) => s.playing);
  const currentTime = usePlayerStore((s) => s.currentTime);
  const duration = usePlayerStore((s) => s.duration);
  const ffwdSec = usePlayerStore((s) => s.ffwdSec);
  const rewindSec = usePlayerStore((s) => s.rewindSec);

  const alreadyLoaded = storeBook?.id === bookId && files.length > 0;

  const { data, isLoading, isError } = useQuery({
    queryKey: ['player-load', bookId],
    queryFn: () => loadPlayerData(bookId),
    enabled: Number.isFinite(bookId) && !alreadyLoaded,
  });

  useEffect(() => {
    if (data) loadBook(data.book, data.files, data.resume);
  }, [data]);

  const book = alreadyLoaded ? storeBook : data?.book;
  const fileList = alreadyLoaded ? files : (data?.files ?? []);

  if (!alreadyLoaded && isLoading) {
    return <div className="player-page player-state">Loading…</div>;
  }
  if (!alreadyLoaded && isError) {
    return <div className="player-page player-state">Couldn't load this book.</div>;
  }
  if (!book) {
    return <div className="player-page player-state">Book not found.</div>;
  }

  const cover = coverUrl(book.cover_art);

  return (
    <div className="player-page">
      <button className="player-back" onClick={() => navigate(-1)} aria-label="Back" title="Back">
        <ChevronDownIcon />
      </button>

      <div className="player-cover-wrap">
        {cover ? (
          <img className="player-cover" src={cover} alt="" />
        ) : (
          <div className="player-cover-placeholder">{book.title?.charAt(0).toUpperCase() ?? '?'}</div>
        )}
      </div>

      <div className="player-info">
        <h1 className="player-title">{book.title}</h1>
        <p className="player-author">{book.author}</p>
        {fileList.length > 0 && (
          <p className="player-chapter">
            Chapter {index + 1} of {fileList.length}
          </p>
        )}
      </div>

      <Seeker currentTime={currentTime} duration={duration} />

      <div className="player-transport">
        <button
          className="player-transport-btn"
          onClick={() => skip(-rewindSec)}
          aria-label={`Back ${rewindSec} seconds`}
          title={`Back ${rewindSec} seconds`}
        >
          <RewindIcon size={36} />
        </button>
        <button
          className="player-transport-play"
          onClick={togglePlay}
          aria-label={playing ? 'Pause' : 'Play'}
          title={playing ? 'Pause' : 'Play'}
        >
          {playing ? <PauseIcon size={56} /> : <PlayIcon size={56} />}
        </button>
        <button
          className="player-transport-btn"
          onClick={() => skip(ffwdSec)}
          aria-label={`Forward ${ffwdSec} seconds`}
          title={`Forward ${ffwdSec} seconds`}
        >
          <ForwardIcon size={36} />
        </button>
      </div>

      <div className="player-secondary">
        <PlaybackSpeedMenu />
        <SleepTimerSheet />
        <ChaptersSheet />
        <BookmarksSheet />
        <button
          className="player-secondary-btn"
          onClick={() => {
            saveProgressNow();
            showToast('Progress saved', 'success');
          }}
          aria-label="Save progress"
          title="Save progress"
        >
          <CheckIcon />
        </button>
      </div>
    </div>
  );
}
