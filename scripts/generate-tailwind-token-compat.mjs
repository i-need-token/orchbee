import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import {
	renderTailwindTokenCompatibility,
	resolveTailwindThemePath,
} from './tailwind-token-compat.mjs';

const repositoryRoot = resolve(import.meta.dirname, '..');
const outputPath = resolve(repositoryRoot, 'packages/design-tokens/src/tailwind-compat.css');
const supportedArguments = new Set(['--check']);
const unexpectedArguments = process.argv
	.slice(2)
	.filter((argument) => !supportedArguments.has(argument));

if (unexpectedArguments.length > 0) {
	throw new Error(`Unsupported arguments: ${unexpectedArguments.join(', ')}`);
}

const tailwindThemePath = resolveTailwindThemePath(repositoryRoot);
const tailwindThemeSource = await readFile(tailwindThemePath, 'utf8');
const generatedSource = renderTailwindTokenCompatibility(tailwindThemeSource);

if (process.argv.includes('--check')) {
	const currentSource = await readFile(outputPath, 'utf8');
	if (currentSource !== generatedSource) {
		throw new Error(
			'packages/design-tokens/src/tailwind-compat.css is stale. Run pnpm generate:tailwind-token-compat.',
		);
	}
} else {
	await writeFile(outputPath, generatedSource);
}
