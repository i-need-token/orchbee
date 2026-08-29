import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { prepareSshRuntimeRelease } from './ssh-runtime-release.mjs';

const runtimeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const arguments_ = parseArguments(process.argv.slice(2), ['--output', '--runtime-version']);
const result = await prepareSshRuntimeRelease({
	outputDirectory: arguments_.get('--output'),
	runtimeRoot,
	runtimeVersion: arguments_.get('--runtime-version'),
});
console.info(`Prepared SSH Runtime signing payload: ${result.outputDirectory}`);

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
