import { access, readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const expectedRuntimeExports = [
	'DEFAULT_TERMINAL_INPUT_CONFIGURATION',
	'DEFAULT_TERMINAL_INTERACTION_CONFIGURATION',
	'DEFAULT_TERMINAL_INTERACTION_CONFIGURATION_SOURCE',
	'DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION',
	'ProjectedHistoryPhase',
	'ProjectedTerminal',
	'ProjectedTerminalPhase',
	'TerminalBufferRowDetail',
	'TerminalOptionAsAltMode',
	'isTerminalOptionAsAltMode',
	'measureMountedTerminalGeometry',
	'normalizePermittedExternalTerminalUrl',
	'normalizeTerminalInteractionConfiguration',
	'normalizeTerminalPresentationConfiguration',
	'orchbeeTerminalColorTheme',
	'resolveTerminalColorTheme',
	'terminalColorThemeAnsi',
	'terminalColorThemes',
];

const packageRoot = resolve(import.meta.dirname, '..');
const esmEntryPath = resolve(packageRoot, 'dist/index.js');
const esmApi = await import(pathToFileURL(esmEntryPath).href);
const cjsApi = createRequire(import.meta.url)(resolve(packageRoot, 'dist/index.cjs'));

for (const [format, api] of [
	['ESM', esmApi],
	['CommonJS', cjsApi],
]) {
	const actual = Object.keys(api).sort();
	if (JSON.stringify(actual) !== JSON.stringify(expectedRuntimeExports)) {
		throw new Error(
			`@orchbee/terminal ${format} runtime exports differ\nExpected: ${expectedRuntimeExports.join(', ')}\nActual: ${actual.join(', ')}`,
		);
	}
}

const esmEntrySource = await readFile(esmEntryPath, 'utf8');
const projectedSearchWorkerUrl = './projected-search-worker.js';
if (!esmEntrySource.includes(`new URL('${projectedSearchWorkerUrl}', import.meta.url)`)) {
	throw new Error(`@orchbee/terminal ESM entry does not reference ${projectedSearchWorkerUrl}`);
}
await access(resolve(packageRoot, 'dist', projectedSearchWorkerUrl.slice(2)));

process.stdout.write('@orchbee/terminal: public API verified\n');
