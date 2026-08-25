import { chmod, mkdtemp, rm } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { Socket, createServer } from 'node:net';
import path from 'node:path';

const addonPath = process.argv[2];
if (addonPath === undefined || !path.isAbsolute(addonPath)) {
	throw new Error('An absolute POSIX Host endpoint addon path is required.');
}
const candidate = createRequire(import.meta.url)(addonPath);
if (
	typeof candidate?.connectAuthenticated !== 'function' ||
	typeof candidate?.prepareRuntimeDirectory !== 'function'
) {
	throw new Error('The POSIX Host endpoint addon exports are unavailable.');
}

const testRoot = await mkdtemp('/tmp/ob-endpoint-runtime-');
const runtimeDirectory = path.join(testRoot, 'r');
const endpointId = 'ab'.repeat(16);
const endpointPath = path.join(runtimeDirectory, `${endpointId}.s`);
const expected = `authenticated-${process.versions.electron === undefined ? 'node' : 'electron'}`;
const server = createServer((socket) => socket.end(expected));
try {
	candidate.prepareRuntimeDirectory(runtimeDirectory);
	await new Promise((resolve, reject) => {
		server.once('error', reject);
		server.listen(endpointPath, resolve);
	});
	await chmod(endpointPath, 0o600);
	const descriptor = candidate.connectAuthenticated(runtimeDirectory, endpointId);
	if (!Number.isInteger(descriptor) || descriptor < 0) {
		throw new Error('The native adapter returned an invalid descriptor.');
	}
	const socket = new Socket({ fd: descriptor, readable: true, writable: true });
	socket.setEncoding('utf8');
	let received = '';
	for await (const chunk of socket) received += chunk;
	if (received !== expected) {
		throw new Error(`Authenticated runtime roundtrip mismatch: ${received}`);
	}
	console.info(
		`Desktop POSIX Host endpoint ${process.versions.electron === undefined ? 'Node' : 'Electron'} runtime verification passed.`,
	);
} finally {
	await new Promise((resolve) => server.close(resolve));
	await rm(testRoot, { force: true, recursive: true });
}
