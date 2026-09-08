import { execFile } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { promisify } from 'node:util';
import { Worker } from 'node:worker_threads';

const executeFile = promisify(execFile);
const bundledSkill = await readFile(
	new URL('../../cli/skills/orchbee/SKILL.md', import.meta.url),
	'utf8',
);
const expectedSkill = bundledSkill.endsWith('\n') ? bundledSkill : `${bundledSkill}\n`;
const WORKER_TRANSITION_TIMEOUT_MILLISECONDS = 15_000;
const [addonPath, workerPath, cliEntryPath, cliLauncherPath, electronExecutable] =
	process.argv.slice(2);
if (
	addonPath === undefined ||
	workerPath === undefined ||
	cliEntryPath === undefined ||
	cliLauncherPath === undefined ||
	electronExecutable === undefined ||
	![addonPath, workerPath, cliEntryPath, cliLauncherPath, electronExecutable].every((value) =>
		path.isAbsolute(value),
	)
) {
	throw new Error('Absolute semantic command CLI runtime paths are required.');
}

// Darwin's Unix-domain socket pathname bound requires the controlled runtime fixture to stay short.
const testRoot = await mkdtemp(path.join(tmpdir(), 'o-'));
const runtimeDirectory = path.join(testRoot, 'r');
const endpointId = 'ef'.repeat(16);
const coordinatorGeneration = 'desktop:generation:cli-runtime';
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
	worker.postMessage({
		bytes: response(200, {
			coordinatorGeneration,
			kind: 'ready',
			protocolVersion: 1,
		}),
		kind: 'response',
		requestId: message.requestId,
	});
});

try {
	await waitForWorker(worker, 'ready');
	const directEnvironment = {
		...process.env,
		ORCHBEE_CLI_NATIVE_ADDON_PATH: addonPath,
		ORCHBEE_CLI_RUNTIME_DIRECTORY: runtimeDirectory,
	};
	const launcherEnvironment = {
		...process.env,
		ORCHBEE_CLI_RUNTIME_DIRECTORY: runtimeDirectory,
	};
	const launches = [
		{
			arguments: [cliEntryPath, 'status'],
			command: process.execPath,
			environment: directEnvironment,
			name: 'Node entry',
		},
		{
			arguments: [cliEntryPath, 'status'],
			command: electronExecutable,
			environment: { ...directEnvironment, ELECTRON_RUN_AS_NODE: '1' },
			name: 'packaged Electron Node runtime',
		},
		launcherLaunch(cliLauncherPath, launcherEnvironment),
	];
	for (const launch of launches) {
		const { stderr, stdout } = await executeFile(launch.command, launch.arguments, {
			encoding: 'utf8',
			env: launch.environment,
			windowsVerbatimArguments: launch.windowsVerbatimArguments,
		});
		if (stderr !== '') throw new Error(`${launch.name} wrote a diagnostic to stderr.`);
		if (!stdout.endsWith('\n') || stdout.slice(0, -1).includes('\n')) {
			throw new Error(`${launch.name} did not emit exactly one compact JSON document.`);
		}
		const document = JSON.parse(stdout);
		if (
			document?.cliVersion !== 1 ||
			document?.command !== 'status' ||
			document?.kind !== 'result' ||
			document?.result?.kind !== 'ready' ||
			document?.result?.coordinatorGeneration !== coordinatorGeneration
		) {
			throw new Error(`${launch.name} did not preserve the coordinator status result.`);
		}
	}
	if (
		receivedPaths.length !== launches.length * 2 ||
		receivedPaths.some((requestPath) => requestPath !== '/v1/status')
	) {
		throw new Error('The CLI process used an unexpected route or request count.');
	}
	const skillLaunches = [
		{
			arguments: [cliEntryPath, '--skill'],
			command: process.execPath,
			environment: directEnvironment,
			name: 'Node entry Skill',
		},
		{
			arguments: [cliEntryPath, '--skill'],
			command: electronExecutable,
			environment: { ...directEnvironment, ELECTRON_RUN_AS_NODE: '1' },
			name: 'packaged Electron Node runtime Skill',
		},
		launcherLaunch(cliLauncherPath, launcherEnvironment, '--skill'),
	];
	for (const launch of skillLaunches) {
		const { stderr, stdout } = await executeFile(launch.command, launch.arguments, {
			encoding: 'utf8',
			env: launch.environment,
			windowsVerbatimArguments: launch.windowsVerbatimArguments,
		});
		if (stderr !== '') throw new Error(`${launch.name} wrote a diagnostic to stderr.`);
		const skillText = stdout.replaceAll('\r\n', '\n');
		if (
			stdout !== expectedSkill ||
			!skillText.startsWith('---\nname: orchbee\n') ||
			!stdout.includes('"$ORCHBEE_CLI" agent changes') ||
			!stdout.includes('"$ORCHBEE_CLI" agent wait') ||
			!stdout.endsWith('\n')
		) {
			throw new Error(`${launch.name} did not emit the bundled Orchbee Skill.`);
		}
	}
	if (receivedPaths.length !== launches.length * 2) {
		throw new Error('Printing the bundled Skill unexpectedly contacted the coordinator.');
	}

	worker.postMessage({ kind: 'stop' });
	await waitForWorker(worker, 'stopped');
	console.info('Desktop semantic command CLI process runtime verified.');
} finally {
	await worker.terminate().catch(() => undefined);
	await rm(testRoot, { force: true, recursive: true });
}

function launcherLaunch(launcherPath, environment, argument = 'status') {
	if (process.platform !== 'win32') {
		return {
			arguments: [argument],
			command: launcherPath,
			environment,
			name: `product-injected CLI launcher ${argument}`,
		};
	}
	const commandInterpreter = process.env.ComSpec ?? 'C:\\Windows\\System32\\cmd.exe';
	return {
		arguments: ['/d', '/s', '/c', `""${launcherPath}" ${argument}"`],
		// This is already a CMD command string; CRT argv escaping would add literal backslashes.
		windowsVerbatimArguments: true,
		command: commandInterpreter,
		environment,
		name: `product-injected CLI launcher ${argument}`,
	};
}

function decodeRequestPath(bytes) {
	const firstLine = Buffer.from(bytes).toString('utf8').split('\r\n', 1)[0];
	const match = /^(?:GET|POST) ([^ ]+) HTTP\/1\.1$/u.exec(firstLine ?? '');
	if (match?.[1] === undefined) throw new Error('CLI runtime forwarded malformed request bytes.');
	return match[1];
}

function response(status, body) {
	const encodedBody = Buffer.from(JSON.stringify(body), 'utf8');
	return Buffer.concat([
		Buffer.from(
			`HTTP/1.1 ${status} OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: ${encodedBody.byteLength}\r\n\r\n`,
			'utf8',
		),
		encodedBody,
	]);
}

function waitForWorker(candidate, kind) {
	return new Promise((resolve, reject) => {
		const timer = setTimeout(
			() => reject(new Error(`Semantic command worker did not report ${kind}.`)),
			WORKER_TRANSITION_TIMEOUT_MILLISECONDS,
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
