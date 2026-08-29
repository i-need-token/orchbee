import { chmod, lstat, mkdtemp, rm, stat, symlink, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import path from 'node:path';

const addonPath = process.argv[2];
if (addonPath === undefined || !path.isAbsolute(addonPath)) {
	throw new Error('An absolute semantic command endpoint addon path is required.');
}
const candidate = createRequire(import.meta.url)(addonPath);
const requiredFunctions = [
	'acceptBefore',
	'closeServer',
	'createServer',
	'disconnectServer',
	'exchangeAuthenticated',
	'prepareRuntimeDirectory',
	'publishLocator',
	'readBefore',
	'readLocator',
	'removeLocator',
	'writeBefore',
];
for (const name of requiredFunctions) {
	if (typeof candidate?.[name] !== 'function') {
		throw new Error(`Semantic command endpoint addon is missing ${name}.`);
	}
}

const testRoot = await mkdtemp(path.join(tmpdir(), 'ob-sc-'));
const runtimeDirectory = path.join(testRoot, 'r');
const endpointId = 'ab'.repeat(16);
const endpointPath = path.join(runtimeDirectory, `${endpointId}.s`);
const locatorPath = path.join(runtimeDirectory, `${endpointId}.c.json`);
const request = Buffer.from('request', 'utf8');
const response = Buffer.from('response', 'utf8');
let server;
try {
	candidate.prepareRuntimeDirectory(runtimeDirectory);
	server = candidate.createServer(runtimeDirectory, endpointId);
	const locator = Buffer.from(
		JSON.stringify({
			locatorVersion: 1,
			protocolVersion: 1,
			coordinatorGeneration: 'desktop:generation:native-test',
			endpointId,
		}),
		'utf8',
	);
	candidate.publishLocator(runtimeDirectory, endpointId, locator);
	if (!Buffer.from(candidate.readLocator(runtimeDirectory, endpointId)).equals(locator)) {
		throw new Error('Semantic command locator did not round-trip.');
	}
	if (process.platform !== 'win32') {
		const [directoryStatus, endpointStatus, locatorStatus] = await Promise.all([
			stat(runtimeDirectory),
			stat(endpointPath),
			stat(locatorPath),
		]);
		if ((directoryStatus.mode & 0o777) !== 0o700) {
			throw new Error('Semantic command runtime directory is not private.');
		}
		if ((endpointStatus.mode & 0o777) !== 0o600 || !endpointStatus.isSocket()) {
			throw new Error('Semantic command socket is not private.');
		}
		if ((locatorStatus.mode & 0o777) !== 0o600 || !locatorStatus.isFile()) {
			throw new Error('Semantic command locator is not private.');
		}
	}

	const exchange = candidate.exchangeAuthenticated(
		runtimeDirectory,
		endpointId,
		request,
		1024,
		2_000,
	);
	if (candidate.acceptBefore(server, 2_000) !== true) {
		throw new Error('Semantic command authenticated client was not accepted.');
	}
	const received = Buffer.from(candidate.readBefore(server, 1024, 2_000));
	if (!received.equals(request)) throw new Error('Semantic command request bytes differ.');
	let written = 0;
	while (written < response.byteLength) {
		written += candidate.writeBefore(server, response.subarray(written), 2_000);
	}
	candidate.disconnectServer(server);
	const exchanged = Buffer.from(await exchange);
	if (!exchanged.equals(response)) throw new Error('Semantic command response bytes differ.');

	candidate.removeLocator(runtimeDirectory, endpointId);
	await writeFile(path.join(testRoot, 'target'), 'not a locator', { mode: 0o600 });
	if (process.platform !== 'win32') {
		await symlink(path.join(testRoot, 'target'), locatorPath);
		let rejected = false;
		try {
			candidate.readLocator(runtimeDirectory, endpointId);
		} catch (error) {
			rejected = error?.code === 'ERR_ORCHBEE_SEMANTIC_ENDPOINT_INSECURE';
		}
		if (!rejected) throw new Error('A locator symlink was not rejected.');
		await rm(locatorPath, { force: true });
	}

	candidate.closeServer(server);
	server = undefined;
	if (process.platform !== 'win32' && (await lstat(endpointPath).catch(() => null)) !== null) {
		throw new Error('Semantic command endpoint survived server close.');
	}

	if (process.platform !== 'win32') {
		const insecureRuntime = path.join(testRoot, 'insecure-runtime');
		await symlink(runtimeDirectory, insecureRuntime);
		let insecureRejected = false;
		try {
			candidate.prepareRuntimeDirectory(insecureRuntime);
		} catch (error) {
			insecureRejected = error?.code === 'ERR_ORCHBEE_SEMANTIC_ENDPOINT_INSECURE';
		}
		if (!insecureRejected) throw new Error('A runtime-directory symlink was not rejected.');

		await chmod(runtimeDirectory, 0o755);
		let publicDirectoryRejected = false;
		try {
			candidate.readLocator(runtimeDirectory, endpointId);
		} catch (error) {
			publicDirectoryRejected = error?.code === 'ERR_ORCHBEE_SEMANTIC_ENDPOINT_INSECURE';
		}
		if (!publicDirectoryRejected) {
			throw new Error('A non-private runtime directory was not rejected.');
		}
	}

	console.info('Desktop semantic command endpoint runtime verified.');
} finally {
	if (server !== undefined) candidate.closeServer(server);
	await rm(testRoot, { force: true, recursive: true });
}
