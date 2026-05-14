import adapter from '@sveltejs/adapter-static';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	compilerOptions: {
		// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
		runes: ({ filename }) => (filename.split(/[/\\]/).includes('node_modules') ? undefined : true)
	},
	kit: {
		adapter: adapter({
			// For GitHub Pages SPA: fallback 404.html serves the app for any unknown route
			// See https://github.com/sveltejs/kit/tree/main/packages/adapter-static#spa-mode
			fallback: '404.html'
		})
	}
};

export default config;
