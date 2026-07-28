import { useNetworkStatusStore } from '../../store/networkStatus';
import './ui.css';

export function NetworkBanner() {
  const reachable = useNetworkStatusStore((s) => s.reachable);

  if (reachable) return null;

  return (
    <div className="network-banner" role="status">
      Can't reach the server — retrying…
    </div>
  );
}
