/** Extract `desktop-token` from a location hash like "#desktop-token=XYZ" or "#a=1&desktop-token=XYZ". */
export function readDesktopToken(hash: string): string | null {
  const h = hash.startsWith('#') ? hash.slice(1) : hash;
  if (!h) return null;
  const token = new URLSearchParams(h).get('desktop-token');
  return token && token.trim() ? token.trim() : null;
}

/** Remove the hash (token) from the address bar without reloading or adding history. */
export function stripHash() {
  const { pathname, search } = window.location;
  window.history.replaceState(window.history.state, '', pathname + search);
}
