import { api } from './client';

export interface DailyStat {
  day: string; // "YYYY-MM-DD"
  ms_listened: number;
}

export interface FinishedBook {
  book_id: number;
  times_finished: number;
  finished_at: string;
}

export async function getDailyStats(days = 30): Promise<DailyStat[]> {
  return api.get<DailyStat[]>(`/stats/daily?days=${days}`);
}

export async function getFinishedBooks(): Promise<FinishedBook[]> {
  return api.get<FinishedBook[]>('/stats/finished');
}
