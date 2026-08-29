import { createRequire } from 'node:module';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { Worker } from 'node:worker_threads';

const addonPath = process.argv[2];
const workerPath = process.argv[3];
if (
	addonPath === undefined ||
	workerPath === undefined ||
	!path.isAbsolute(addonPath) ||
	!path.isAbsolute(workerPath)
) {
	throw new Error('Absolute addon and local HTTP worker paths are required.');
}
const native = createRequire(import.meta.url)(addonPath);
if (
	typeof native?.exchangeAuthenticated !== 'function' ||
	typeof native?.readLocator !== 'function'
) {
	throw new Error('The semantic command endpoint native client is unavailable.');
}

const testRoot = await mkdtemp(path.join(tmpdir(), 'o-'));
const runtimeDirectory = path.join(testRoot, 'r');
const endpointId = 'cd'.repeat(16);
const coordinatorGeneration = 'desktop:generation:local-http-runtime';
const locatorJson = JSON.stringify({
	locatorVersion: 1,
	protocolVersion: 1,
	coordinatorGeneration,
	endpointId,
});
const worker = new Worker(workerPath, {
	workerData: { addonPath, endpointId, locatorJson, runtimeDirectory },
});
const receivedPaths = [];
worker.on('message', (message) => {
	if (message?.kind !== 'request') return;
	const requestPath = decodeRequestPath(message.bytes);
	receivedPaths.push(requestPath);
	if (requestPath === '/v1/disconnect-proof') {
		setTimeout(() => {
			worker.postMessage({
				bytes: response(200, { kind: 'accepted-before-disconnect' }),
				kind: 'response',
				requestId: message.requestId,
			});
		}, 250);
		return;
	}
	worker.postMessage({
		bytes:
			requestPath === '/v1/oversized-response'
				? new Uint8Array(1_064_961)
				: response(200, { kind: 'runtime-response', requestPath }),
		kind: 'response',
		requestId: message.requestId,
	});
});

try {
	await waitForWorker(worker, 'ready');
	const locator = Buffer.from(native.readLocator(runtimeDirectory, endpointId)).toString('utf8');
	if (locator !== locatorJson) throw new Error('The live local HTTP locator differs.');

	const valid = await native.exchangeAuthenticated(
		runtimeDirectory,
		endpointId,
		request('/v1/status'),
		1_064_960,
		2_000,
	);
	if (decodeStatus(valid) !== 200) throw new Error('A valid local HTTP exchange failed.');

	for (const [candidate, expectedStatus] of [
		[Buffer.concat([request('/v1/status'), Buffer.from('extra', 'utf8')]), 400],
		[
			Buffer.from(
				'POST /v1/executions HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 65537\r\n\r\n',
				'utf8',
			),
			413,
		],
	]) {
		const rejected = await native.exchangeAuthenticated(
			runtimeDirectory,
			endpointId,
			candidate,
			1_064_960,
			2_000,
		);
		if (decodeStatus(rejected) !== expectedStatus) {
			throw new Error(`Local HTTP rejection status ${expectedStatus} was not preserved.`);
		}
	}

	const oversized = await native.exchangeAuthenticated(
		runtimeDirectory,
		endpointId,
		request('/v1/oversized-response'),
		1_064_960,
		2_000,
	);
	if (decodeStatus(oversized) !== 500) {
		throw new Error('An oversized parent response was not replaced atomically.');
	}

	let disconnected = false;
	try {
		await native.exchangeAuthenticated(
			runtimeDirectory,
			endpointId,
			request('/v1/disconnect-proof'),
			1_064_960,
			75,
		);
	} catch (error) {
		disconnected = error?.code === 'ERR_ORCHBEE_SEMANTIC_ENDPOINT_TIMEOUT';
	}
	if (!disconnected) throw new Error('The disconnect proof client did not time out.');
	await delay(400);
	const afterDisconnect = await native.exchangeAuthenticated(
		runtimeDirectory,
		endpointId,
		request('/v1/after-disconnect'),
		1_064_960,
		2_000,
	);
	if (decodeStatus(afterDisconnect) !== 200) {
		throw new Error('The endpoint did not recover after a client disconnect.');
	}
	if (
		receivedPaths.filter((value) => value === '/v1/disconnect-proof').length !== 1 ||
		receivedPaths.filter((value) => value === '/v1/after-disconnect').length !== 1
	) {
		throw new Error('A transport disconnect replayed or dropped an accepted request.');
	}

	worker.postMessage({ kind: 'stop' });
	await waitForWorker(worker, 'stopped');
	console.info('Desktop semantic command local HTTP runtime verified.');
} finally {
	await worker.terminate().catch(() => undefined);
	await rm(testRoot, { force: true, recursive: true });
}

function request(requestPath) {
	return Buffer.from(
		`GET ${requestPath} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 0\r\n\r\n`,
		'utf8',
	);
}

function response(status, body) {
	const encodedBody = Buffer.from(JSON.stringify(body), 'utf8');
	const reason = status === 200 ? 'OK' : 'Internal Server Error';
	return Buffer.concat([
		Buffer.from(
			`HTTP/1.1 ${status} ${reason}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: ${encodedBody.byteLength}\r\n\r\n`,
			'utf8',
		),
		encodedBody,
	]);
}

function decodeRequestPath(bytes) {
	const firstLine = Buffer.from(bytes).toString('utf8').split('\r\n', 1)[0];
	const match = /^GET ([^ ]+) HTTP\/1\.1$/u.exec(firstLine ?? '');
	if (match?.[1] === undefined) throw new Error('Worker forwarded malformed request bytes.');
	return match[1];
}

function decodeStatus(bytes) {
	const firstLine = Buffer.from(bytes).toString('utf8').split('\r\n', 1)[0];
	const match = /^HTTP\/1\.1 ([1-5][0-9]{2}) /u.exec(firstLine ?? '');
	if (match?.[1] === undefined) throw new Error('Local HTTP response status is malformed.');
	return Number(match[1]);
}

function waitForWorker(candidate, kind) {
	return new Promise((resolve, reject) => {
		const timer = setTimeout(
			() => reject(new Error(`Semantic command worker did not report ${kind}.`)),
			5_000,
		);
		const onMessage = (message) => {
			if (message?.kind === 'failed') {
				clearTimeout(timer);
				reject(new Error(`Semantic command worker failed with ${message.code}.`));
			} else if (message?.kind === kind) {
				clearTimeout(timer);
				resolve();
			}
		};
		candidate.once('error', (error) => {
			clearTimeout(timer);
			reject(error);
		});
		candidate.on('message', onMessage);
	});
}

function delay(milliseconds) {
	return new Promise((resolve) => setTimeout(resolve, milliseconds));
}
