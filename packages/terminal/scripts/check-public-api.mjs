import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const expectedRuntimeExports = [
	'DEFAULT_TERMINAL_INPUT_CONFIGURATION',
	'DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION',
	'ProjectedHistoryPhase',
	'ProjectedTerminal',
	'ProjectedTerminalPhase',
	'TerminalBufferRowDetail',
	'TerminalOptionAsAltMode',
	'isTerminalOptionAsAltMode',
	'measureMountedTerminalGeometry',
	'normalizePermittedExternalTerminalUrl',
	'normalizeTerminalPresentationConfiguration',
];

const packageRoot = resolve(import.meta.dirname, '..');
const esmApi = await import(pathToFileURL(resolve(packageRoot, 'dist/index.js')).href);
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

process.stdout.write('@orchbee/terminal: public API verified\n');
