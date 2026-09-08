import { useNetworkStatusStore } from '../../store/networkStatus';
import './ui.css';

export function NetworkBanner() {
  const reachable = useNetworkStatusStore((s) => s.reachable);
  const mediaOk = useNetworkStatusStore((s) => s.mediaOk);

  // Mutually exclusive, unreachable first: the two banners share a fixed position,
  // and a server we can't reach can't tell us anything trustworthy about its drives.
  if (!reachable) {
    return (
      <div className="network-banner" role="status">
        Can't reach the server — retrying…
      </div>
    );
  }

  // The failure this exists for: the server is up and the library still lists fine
  // from the DB, but its media drive is unmounted, so every stream and cover 404s.
  // Without this the app looks perfectly healthy and simply refuses to play anything.
  if (mediaOk === false) {
    return (
      <div className="network-banner network-banner-error" role="alert">
        ⚠ Media drive not mounted — playback and covers unavailable
      </div>
    );
  }

  return null;
}
