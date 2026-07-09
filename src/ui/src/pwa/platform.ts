declare global {
  interface Navigator {
    standalone?: boolean;
  }
}

export function isStandalone(): boolean {
  return (
    window.matchMedia('(display-mode: standalone)').matches ||
    window.navigator.standalone === true
  );
}

export function isIOSSafari(): boolean {
  const ua = window.navigator.userAgent;
  const isIOSDevice =
    /iPad|iPhone|iPod/.test(ua) || (ua.includes('Macintosh') && 'ontouchend' in document);
  const isSafari = /Safari/.test(ua) && !/CriOS|FxiOS|EdgiOS|OPiOS/.test(ua);
  return isIOSDevice && isSafari;
}
