// Disable SSR — the app relies on localStorage and onMount
// Prerender root so GitHub Pages has an index.html to serve
export const ssr = false;
export const prerender = true;
