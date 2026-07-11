import { Link } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { getDailyStats, getFinishedBooks } from '../api/stats';
import { useLibraryBooks } from '../hooks/useLibraryBooks';
import { Card } from '../components/ui/Card';
import { ChevronRightIcon } from '../components/organize/icons';
import { DailyBarChart } from '../components/stats/DailyBarChart';
import { FinishedBooksList } from '../components/stats/FinishedBooksList';
import { formatDuration } from '../lib/format';
import './stats.css';

const DAYS = 30;

export function StatsPage() {
  const { data: books = [] } = useLibraryBooks();
  const { data: daily = [], isLoading: dailyLoading } = useQuery({
    queryKey: ['stats', 'daily', DAYS],
    queryFn: () => getDailyStats(DAYS),
  });
  const { data: finished = [], isLoading: finishedLoading } = useQuery({
    queryKey: ['stats', 'finished'],
    queryFn: getFinishedBooks,
  });

  const last7 = daily.slice(-7);
  const weekMs = last7.reduce((sum, d) => sum + d.ms_listened, 0);
  const allTimeMs = daily.reduce((sum, d) => sum + d.ms_listened, 0);

  return (
    <div className="settings-page">
      <div className="settings-header">
        <div>
          <h1 className="settings-title">Listening stats</h1>
          <p className="settings-subtitle">Time listened and finished books</p>
        </div>
        <Link className="settings-back-link" to="/settings">
          <ChevronRightIcon size={16} className="rotate-180" /> Back to settings
        </Link>
      </div>

      <h2 className="settings-section-title">Last 7 days</h2>
      {dailyLoading && <div className="settings-state">Loading…</div>}
      {!dailyLoading && (
        <Card className="stats-summary-card">
          <div className="stats-summary-row">
            <div className="stats-summary-stat">
              <span className="stats-summary-value">{formatDuration(weekMs)}</span>
              <span className="stats-summary-label">this week</span>
            </div>
            <div className="stats-summary-stat">
              <span className="stats-summary-value">{formatDuration(allTimeMs)}</span>
              <span className="stats-summary-label">last {DAYS} days</span>
            </div>
          </div>
          <DailyBarChart stats={last7} />
        </Card>
      )}

      <h2 className="settings-section-title">Finished books</h2>
      {finishedLoading && <div className="settings-state">Loading…</div>}
      {!finishedLoading && <FinishedBooksList books={books} finished={finished} />}
    </div>
  );
}
