// Disable SSR — the app relies on localStorage and onMount
// With adapter-cloudflare + fallback:'spa', everything is client-side rendered
export const ssr = false;
export const prerender = false;
