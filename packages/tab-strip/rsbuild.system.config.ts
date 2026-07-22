import { defineConfig } from '@rsbuild/core';
import { pluginReact } from '@rsbuild/plugin-react';

export default defineConfig({
	plugins: [pluginReact()],
	source: {
		entry: {
			index: './system-tests/fixture/main.tsx',
		},
	},
	html: {
		template: './system-tests/fixture/index.html',
	},
	server: {
		host: '127.0.0.1',
		port: 5198,
		strictPort: true,
	},
});
