import { createRslib } from '@rslib/core';
import { fileURLToPath } from 'node:url';

// This is a development build invoked only by conformance commands. Its output
// follows tsconfig's existing src -> dist mapping and is excluded from packing.
const rslib = await createRslib({
	cwd: fileURLToPath(new URL('../', import.meta.url)),
	config: {
		lib: [{ format: 'esm', syntax: 'es2022', bundle: true, dts: false }],
		source: {
			entry: { 'agent-reference': './src/__tests__/conformance/agent-reference.ts' },
			tsconfigPath: './tsconfig.json',
		},
		output: {
			target: 'node',
			distPath: { root: './dist/__tests__/conformance' },
			filename: { js: '[name].js' },
			cleanDistPath: true,
			sourceMap: true,
		},
	},
});
await rslib.build();
