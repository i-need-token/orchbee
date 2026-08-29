import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { prepareSshEnvironmentRelease } from './ssh-environment-release.mjs';

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const arguments_ = parseArguments(process.argv.slice(2), [
	'--output',
	'--runtime-version',
	'--terminal-host-version',
]);
const result = await prepareSshEnvironmentRelease({
	outputDirectory: arguments_.get('--output'),
	runtimeRoot: path.join(repositoryRoot, 'apps', 'ssh-environment-runtime'),
	runtimeVersion: arguments_.get('--runtime-version'),
	terminalHostRoot: path.join(repositoryRoot, 'apps', 'terminal-host'),
	terminalHostVersion: arguments_.get('--terminal-host-version'),
});
console.info(`Prepared SSH Environment signing payload: ${result.outputDirectory}`);

function parseArguments(values, requiredNames) {
	if (values.length % 2 !== 0) throw new Error('Release arguments must be name/value pairs.');
	const parsed = new Map();
	for (let index = 0; index < values.length; index += 2) {
		const name = values[index];
		const value = values[index + 1];
		if (!requiredNames.includes(name) || value === undefined || parsed.has(name)) {
			throw new Error(`Invalid release argument: ${String(name)}`);
		}
		parsed.set(name, value);
	}
	for (const name of requiredNames) {
		if (!parsed.has(name)) throw new Error(`Missing release argument: ${name}`);
	}
	return parsed;
}
