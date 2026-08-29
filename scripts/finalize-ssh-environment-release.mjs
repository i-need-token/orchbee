import { finalizeSshEnvironmentRelease } from './ssh-environment-release.mjs';

const arguments_ = parseArguments(process.argv.slice(2), [
	'--output',
	'--prepared',
	'--public-key',
	'--signature',
]);
const result = await finalizeSshEnvironmentRelease({
	outputDirectory: arguments_.get('--output'),
	preparedDirectory: arguments_.get('--prepared'),
	publicKeyFilePath: arguments_.get('--public-key'),
	signatureFilePath: arguments_.get('--signature'),
});
console.info(`Finalized signed SSH Environment release: ${result.outputDirectory}`);

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
