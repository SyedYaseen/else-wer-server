import type { DailyStat } from '../../api/stats';
import { formatDuration } from '../../lib/format';

interface DailyBarChartProps {
  stats: DailyStat[];
}

// Div-based bar chart — no charting dependency, matches ProgressBar.tsx's plain-div style.
export function DailyBarChart({ stats }: DailyBarChartProps) {
  const maxMs = Math.max(1, ...stats.map((s) => s.ms_listened));

  return (
    <div className="stats-bar-chart">
      {stats.map((s) => {
        const pct = Math.round((s.ms_listened / maxMs) * 100);
        const label = new Date(`${s.day}T00:00:00Z`).toLocaleDateString(undefined, {
          weekday: 'short',
          timeZone: 'UTC',
        });
        return (
          <div key={s.day} className="stats-bar-col" title={`${s.day}: ${formatDuration(s.ms_listened)}`}>
            <div className="stats-bar-track">
              <div className="stats-bar-fill" style={{ height: `${pct}%` }} />
            </div>
            <span className="stats-bar-label">{label}</span>
          </div>
        );
      })}
    </div>
  );
}
