import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { renderDesignTokenInventory } from './design-token-inventory.mjs';

const repositoryRoot = resolve(import.meta.dirname, '..');
const registryPath = resolve(repositoryRoot, 'packages/design-tokens/src/tokens.css');
const outputPath = resolve(
	repositoryRoot,
	'packages/design-tokens/src/design-token-names.generated.ts',
);
const supportedArguments = new Set(['--check']);
const unexpectedArguments = process.argv
	.slice(2)
	.filter((argument) => !supportedArguments.has(argument));

if (unexpectedArguments.length > 0) {
	throw new Error(`Unsupported arguments: ${unexpectedArguments.join(', ')}`);
}

const registrySource = await readFile(registryPath, 'utf8');
const generatedSource = renderDesignTokenInventory(registrySource);

if (process.argv.includes('--check')) {
	const currentSource = await readFile(outputPath, 'utf8');
	if (currentSource !== generatedSource) {
		throw new Error(
			'packages/design-tokens/src/design-token-names.generated.ts is stale. Run pnpm generate:design-token-inventory.',
		);
	}
} else {
	await writeFile(outputPath, generatedSource);
}
