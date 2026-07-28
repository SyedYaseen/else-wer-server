import { refreshToken } from '../api/auth';
import { useAuthStore } from '../store/auth';
import { useNetworkStatusStore } from '../store/networkStatus';

let refreshInFlight = false;

function attemptRefresh() {
  if (refreshInFlight || !useAuthStore.getState().token) return;
  refreshInFlight = true;
  refreshToken()
    .catch(() => {
      // Server unreachable — fine, the next reconnect transition retries. A real
      // 401 has already cleared the token via client.ts before this rejection.
    })
    .finally(() => {
      refreshInFlight = false;
    });
}

// Keeps the stored 30-day JWT perpetually fresh: silently re-issues it on app
// start and on every offline→online transition, so RequireAuth can guard on
// token presence alone and an offline session is never bounced to /login by a
// stale exp. Same reconnect-subscription pattern as startProgressSyncOnReconnect.
export function startTokenRefresh(): void {
  useNetworkStatusStore.subscribe((state, prevState) => {
    if (state.reachable && !prevState.reachable) attemptRefresh();
  });
  attemptRefresh();
}
