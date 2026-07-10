import { useNavigate } from 'react-router-dom';
import type { ContinueListeningItem } from '../../lib/continueListening';
import { coverUrl, fileMetadata } from '../../api/books';
import { getBookProgress } from '../../api/progress';
import { resolveResumePoint, getLocalProgress } from '../../lib/playerResume';
import { loadBook } from '../../player/engine';
import { getOfflineBookData } from '../../offline/storage';
import { ProgressBar } from '../ui/ProgressBar';
import './library.css';

export function ContinueListeningRow({ items }: { items: ContinueListeningItem[] }) {
  const navigate = useNavigate();
  if (!items.length) return null;

  async function resumePlaying(bookId: number) {
    const book = items.find((i) => i.book.id === bookId)?.book;
    if (!book) return;
    try {
      const [files, progress] = await Promise.all([fileMetadata(book.id), getBookProgress(book.id)]);
      const resume = resolveResumePoint(files, progress);
      loadBook(book, files, resume);
      navigate(`/player/${book.id}`);
    } catch (e) {
      // Network unreachable — fall back to the downloaded book's local
      // metadata snapshot, same as PlayerPage's loadPlayerData.
      const offline = await getOfflineBookData(bookId);
      if (!offline) {
        console.error('resumePlaying failed and no offline snapshot available', e);
        return;
      }
      const progress = getLocalProgress(bookId);
      const resume = resolveResumePoint(offline.files, progress);
      loadBook(offline.book, offline.files, resume);
      navigate(`/player/${bookId}`);
    }
  }

  return (
    <div className="continue-listening">
      <h2 className="continue-listening-title">Continue Listening</h2>
      <div className="continue-listening-scroller">
        {items.map(({ book, progressMs }) => {
          const cover = coverUrl(book.cover_art);
          const fraction = book.duration > 0 ? progressMs / book.duration : 0;
          return (
            <button
              key={book.id}
              onClick={() => resumePlaying(book.id)}
              className="continue-listening-card"
            >
              {cover ? (
                <img className="continue-listening-cover" src={cover} alt="" loading="lazy" />
              ) : (
                <div className="continue-listening-cover-placeholder">
                  {book.title?.charAt(0).toUpperCase() ?? '?'}
                </div>
              )}
              <div className="continue-listening-body">
                <div className="continue-listening-card-title">{book.title}</div>
              </div>
              <ProgressBar value={fraction} />
            </button>
          );
        })}
      </div>
    </div>
  );
}
