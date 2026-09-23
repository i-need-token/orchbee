import { execFile } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { constants as fileSystemConstants } from 'node:fs';
import { lstat, open, readdir, unlink } from 'node:fs/promises';
import { createConnection } from 'node:net';
import path from 'node:path';
import { promisify } from 'node:util';

const executeFile = promisify(execFile);
const MAX_CORRELATIONS = 32;
const MAX_SOCKET_CANDIDATES = 32;
const MAX_COMMAND_OUTPUT_BYTES = 64 * 1024;
const SOCKET_TIMEOUT_MILLISECONDS = 250;
const IDENTITY_PATTERN = /^[A-Za-z0-9._:-]{1,128}$/u;
const PRIVATE_SOCKET_NAME_PATTERN = /^orchbee-agent-v3-[A-Za-z0-9_-]{16,64}\.sock$/u;
const OBSERVATION_SOCKET_ENVIRONMENT_NAME = 'ORCHBEE_AGENT_OBSERVATION_SOCKET_V3';
const WINDOWS_OBSERVATION_PIPE_PATTERN =
	/^\\\\\.\\pipe\\orchbee-[a-f0-9]{32}-observation$(?![\s\S])/u;
const WINDOWS_CONSOLE_TARGET = String.raw`\\.\CONOUT$`;
const PRIVATE_APC_PREFIX = 'orchbee.agent.v3;';
const ACCEPTED_ENVELOPE_ACK = '{"accepted":true,"version":3}\n';
const MAX_ACK_BYTES = Buffer.byteLength(ACCEPTED_ENVELOPE_ACK);
const ADAPTER_IDS = new Set([
	'claude-code',
	'codex',
	'grok',
	'opencode',
	'pi',
	'omp',
	'gemini',
	'github-copilot',
	'cline',
	'qwen-code',
]);
const ACTIVITIES = new Set([
	'thinking',
	'tool',
	'editing',
	'command',
	'subagent',
	'compacting',
	'retrying',
	'other',
]);
const EVENTS = new Set([
	'session-started',
	'session-idle',
	'turn-started',
	'tool-started',
	'tool-finished',
	'tool-failed',
	'question-requested',
	'approval-requested',
	'attention-resolved',
	'compaction-started',
	'compaction-finished',
	'subagent-started',
	'subagent-stopped',
	'turn-stopped',
	'turn-failed',
	'turn-canceled',
	'turn-interrupted',
	'session-ended',
]);

export async function forwardSanitizedObservation(adapterId, value, options = {}) {
	const observation = sanitizeNormalizedObservation(value);
	if (!ADAPTER_IDS.has(adapterId)) return;
	if (observation === null) return;
	const environment = options.environment ?? process.env;
	const allocateIdentity = options.allocateIdentity ?? allocateTransportIdentity;
	const resolveTargets = options.resolveTargets ?? resolveObservationTargets;
	const writeHint = options.writeHint ?? writeCorrelationHint;
	const discoverSockets = options.discoverSockets ?? discoverObservationSockets;
	const sendEnvelope = options.sendEnvelope ?? sendObservationEnvelope;
	let sockets;
	try {
		sockets = await discoverSockets(environment);
	} catch {
		return;
	}
	if (sockets.length === 0) return;
	let targets;
	try {
		targets = deduplicateTargets(await resolveTargets(environment)).slice(0, MAX_CORRELATIONS);
	} catch {
		return;
	}
	if (targets.length === 0) return;

	const eventId = allocateIdentity();
	const correlations = targets.map((target) => ({
		correlationId: allocateIdentity(),
		target,
	}));
	const envelope = {
		adapterId,
		correlations: correlations.map(({ correlationId, target }) => ({
			correlationId,
			visibility: target.visibility,
		})),
		eventId,
		observation,
		version: 3,
	};
	const deliveries = await Promise.allSettled(
		sockets.slice(0, MAX_SOCKET_CANDIDATES).map((socketPath) => sendEnvelope(socketPath, envelope)),
	);
	if (!deliveries.some((delivery) => delivery.status === 'fulfilled' && delivery.value === true)) {
		return;
	}
	await Promise.allSettled(
		correlations.map(({ correlationId, target }) =>
			writeHint(target.tty, {
				correlationId,
				eventId,
				kind: 'correlation',
				version: 3,
			}),
		),
	);
}

export async function resolveObservationTargets(environment, options = {}) {
	if ((options.platform ?? process.platform) === 'win32') {
		return [{ tty: WINDOWS_CONSOLE_TARGET, visibility: 'unknown' }];
	}
	const execute = options.execute ?? executeBounded;
	const targetGroups = [];
	if (validEnvironmentValue(environment.TMUX_PANE)) {
		targetGroups.push(resolveTmuxTargets(environment.TMUX_PANE, execute));
	}
	if (
		validEnvironmentValue(environment.ZELLIJ_SESSION_NAME) &&
		validEnvironmentValue(environment.ZELLIJ_PANE_ID)
	) {
		targetGroups.push(
			resolveZellijTargets(environment.ZELLIJ_SESSION_NAME, environment.ZELLIJ_PANE_ID, execute),
		);
	}
	const settled = await Promise.allSettled(targetGroups);
	const muxTargets = settled.flatMap((result) =>
		result.status === 'fulfilled' ? result.value : [],
	);
	if (muxTargets.length > 0) return deduplicateTargets(muxTargets);
	const directTty = normalizeTty(environment.SSH_TTY) ?? '/dev/tty';
	return [{ tty: directTty, visibility: 'unknown' }];
}

export function parseTmuxClientOutput(value, sessionName, paneId) {
	const targets = [];
	for (const line of value.split(/\r?\n/u)) {
		const [tty, clientSession, activePane] = line.split('\t');
		const normalizedTty = normalizeTty(tty);
		if (normalizedTty === null || clientSession !== sessionName) continue;
		targets.push({
			tty: normalizedTty,
			visibility: activePane === paneId ? 'unknown' : 'hidden',
		});
	}
	return deduplicateTargets(targets);
}

export function parseZellijClientOutput(value) {
	const clients = [];
	for (const line of value.split(/\r?\n/u)) {
		const trimmed = line.trim();
		if (trimmed === '' || trimmed.startsWith('CLIENT_ID ')) continue;
		const [clientId, paneId] = trimmed.split(/\s+/u);
		if (clientId === undefined || paneId === undefined) continue;
		clients.push({ clientId, paneId });
	}
	return clients;
}

export function parseZellijClientProcesses(value, userId, sessionName) {
	const targets = [];
	for (const line of value.split(/\r?\n/u)) {
		const match = /^\s*(\d+)\s+(\d+)\s+(\S+)\s+(.+)$/u.exec(line);
		if (match === null) continue;
		const [, uidText, , ttyText, command] = match;
		if (
			Number(uidText) !== userId ||
			command === undefined ||
			!zellijClientCommandForSession(command, sessionName)
		) {
			continue;
		}
		const tty = normalizeTty(ttyText);
		if (tty !== null) targets.push(tty);
	}
	return [...new Set(targets)];
}

export async function discoverObservationSockets(environment, options = {}) {
	const localSocket = environment[OBSERVATION_SOCKET_ENVIRONMENT_NAME];
	if ((options.platform ?? process.platform) === 'win32') {
		return typeof localSocket === 'string' && WINDOWS_OBSERVATION_PIPE_PATTERN.test(localSocket)
			? [localSocket]
			: [];
	}
	if (typeof localSocket === 'string') {
		if (!path.isAbsolute(localSocket)) return [];
		return (await isOwnedSocket(localSocket)) ? [localSocket] : [];
	}
	// Missing local routing is not proof of SSH and must not couple another terminal to Orchbee.
	if (!isRemoteSshEnvironment(environment)) return [];
	const candidates = [];
	let entries = [];
	try {
		entries = await readdir('/tmp');
	} catch {
		// A missing shared temporary directory simply disables remote discovery.
	}
	for (const entry of entries) {
		if (PRIVATE_SOCKET_NAME_PATTERN.test(entry)) candidates.push(path.join('/tmp', entry));
	}
	const admitted = [];
	for (const candidate of [...new Set(candidates)].slice(0, MAX_SOCKET_CANDIDATES * 2)) {
		if (await isOwnedSocket(candidate)) admitted.push(candidate);
	}
	return admitted.slice(0, MAX_SOCKET_CANDIDATES);
}

export async function writeCorrelationHint(tty, hint) {
	if (process.platform === 'win32') {
		if (tty !== WINDOWS_CONSOLE_TARGET) return;
		const handle = await open(WINDOWS_CONSOLE_TARGET, 'r+');
		try {
			if (!(await handle.stat()).isCharacterDevice()) return;
			const payload = Buffer.from(JSON.stringify(hint), 'utf8').toString('base64url');
			await handle.writeFile(`\u001B_${PRIVATE_APC_PREFIX}${payload}\u001B\\`);
		} finally {
			await handle.close();
		}
		return;
	}
	if (normalizeTty(tty) !== tty) return;
	const payload = Buffer.from(JSON.stringify(hint), 'utf8').toString('base64url');
	const handle = await open(
		tty,
		fileSystemConstants.O_WRONLY |
			fileSystemConstants.O_NOCTTY |
			fileSystemConstants.O_NOFOLLOW |
			fileSystemConstants.O_NONBLOCK,
	);
	try {
		const metadata = await handle.stat();
		const userId = process.getuid?.();
		if (
			!metadata.isCharacterDevice() ||
			(tty !== '/dev/tty' && userId !== undefined && metadata.uid !== userId)
		) {
			return;
		}
		await handle.write(`\u001B_${PRIVATE_APC_PREFIX}${payload}\u001B\\`);
	} finally {
		await handle.close();
	}
}

export async function sendObservationEnvelope(socketPath, envelope) {
	return new Promise((resolve) => {
		const socket = createConnection({ path: socketPath });
		const chunks = [];
		let byteLength = 0;
		let settled = false;
		const finish = (accepted) => {
			if (settled) return;
			settled = true;
			socket.destroy();
			resolve(accepted);
		};
		socket.setTimeout(SOCKET_TIMEOUT_MILLISECONDS, () => finish(false));
		socket.once('connect', () => socket.end(JSON.stringify(envelope)));
		socket.on('data', (chunk) => {
			byteLength += chunk.byteLength;
			if (byteLength > MAX_ACK_BYTES) {
				finish(false);
				return;
			}
			chunks.push(chunk);
		});
		socket.once('end', () => {
			const response = Buffer.concat(chunks, byteLength).toString('utf8');
			finish(response === ACCEPTED_ENVELOPE_ACK);
		});
		socket.once('close', () => finish(false));
		socket.once('error', async (error) => {
			if (process.platform !== 'win32' && hasErrorCode(error, 'ECONNREFUSED')) {
				await removeStaleForwardSocket(socketPath);
			}
			finish(false);
		});
	});
}

async function resolveTmuxTargets(paneId, execute) {
	const sessionName = (
		await execute('tmux', ['display-message', '-p', '-t', paneId, '#{session_name}'])
	).trim();
	if (sessionName === '') return [];
	const clients = await execute('tmux', [
		'list-clients',
		'-F',
		'#{client_tty}\t#{client_session}\t#{pane_id}',
	]);
	return parseTmuxClientOutput(clients, sessionName, paneId);
}

async function resolveZellijTargets(sessionName, paneId, execute) {
	const [clientOutput, processOutput] = await Promise.all([
		execute('zellij', ['--session', sessionName, 'action', 'list-clients']),
		execute('ps', ['-axo', 'uid=,pid=,tty=,command=']),
	]);
	const clients = parseZellijClientOutput(clientOutput);
	const userId = process.getuid?.();
	if (userId === undefined || clients.length === 0) return [];
	const ttys = parseZellijClientProcesses(processOutput, userId, sessionName);
	const visibleCount = clients.filter((client) => client.paneId === paneId).length;
	const visibility = visibleCount === 0 ? 'hidden' : 'unknown';
	return ttys.map((tty) => ({ tty, visibility }));
}

async function executeBounded(file, arguments_) {
	const result = await executeFile(file, arguments_, {
		encoding: 'utf8',
		maxBuffer: MAX_COMMAND_OUTPUT_BYTES,
		timeout: SOCKET_TIMEOUT_MILLISECONDS,
	});
	return result.stdout;
}

async function removeStaleForwardSocket(socketPath) {
	if (
		path.dirname(socketPath) !== '/tmp' ||
		!PRIVATE_SOCKET_NAME_PATTERN.test(path.basename(socketPath))
	) {
		return;
	}
	try {
		const metadata = await lstat(socketPath);
		const userId = process.getuid?.();
		if (metadata.isSocket() && (userId === undefined || metadata.uid === userId)) {
			await unlink(socketPath);
		}
	} catch {
		// Another hook or OpenSSH may already have removed the stale socket.
	}
}

async function isOwnedSocket(socketPath) {
	try {
		const metadata = await lstat(socketPath);
		const userId = process.getuid?.();
		return metadata.isSocket() && (userId === undefined || metadata.uid === userId);
	} catch {
		// Candidate discovery is intentionally best effort.
		return false;
	}
}

function sanitizeNormalizedObservation(value) {
	if (!isRecord(value)) return null;
	const sessionKey = identity(value.sessionKey);
	const event = typeof value.event === 'string' && EVENTS.has(value.event) ? value.event : null;
	const activity =
		value.activity === null || ACTIVITIES.has(value.activity) ? value.activity : undefined;
	const agentKey = nullableIdentity(value.agentKey);
	const operationKey = nullableIdentity(value.operationKey);
	const turnKey = nullableIdentity(value.turnKey);
	if (
		sessionKey === null ||
		event === null ||
		activity === undefined ||
		agentKey === undefined ||
		operationKey === undefined ||
		turnKey === undefined ||
		value.version !== 1
	) {
		return null;
	}
	if (
		(event === 'question-requested' ||
			event === 'approval-requested' ||
			event === 'attention-resolved') &&
		operationKey === null
	) {
		return null;
	}
	if ((event === 'subagent-started' || event === 'subagent-stopped') && agentKey === null) {
		return null;
	}
	return { activity, agentKey, event, operationKey, sessionKey, turnKey, version: 1 };
}

function allocateTransportIdentity() {
	return randomBytes(18).toString('base64url');
}

function identity(value) {
	return typeof value === 'string' && IDENTITY_PATTERN.test(value) ? value : null;
}

function nullableIdentity(value) {
	if (value === null) return null;
	const result = identity(value);
	return result === null ? undefined : result;
}

function validEnvironmentValue(value) {
	return typeof value === 'string' && value.length > 0 && value.length <= 256;
}

function isRemoteSshEnvironment(environment) {
	return (
		validEnvironmentValue(environment.SSH_CONNECTION) ||
		validEnvironmentValue(environment.SSH_CLIENT) ||
		validEnvironmentValue(environment.SSH_TTY)
	);
}

function normalizeTty(value) {
	if (typeof value !== 'string' || value === '' || value.includes('\0')) return null;
	const candidate = value.startsWith('/dev/') ? value : `/dev/${value}`;
	if (!/^\/dev\/[A-Za-z0-9._/-]{1,240}$/u.test(candidate)) return null;
	return path.posix.normalize(candidate) === candidate ? candidate : null;
}

function deduplicateTargets(targets) {
	const byTty = new Map();
	for (const target of targets) {
		const tty =
			target.tty === WINDOWS_CONSOLE_TARGET ? WINDOWS_CONSOLE_TARGET : normalizeTty(target.tty);
		if (tty === null) continue;
		const existing = byTty.get(tty);
		if (existing === undefined) byTty.set(tty, { tty, visibility: target.visibility });
		else existing.visibility = combineVisibility(existing.visibility, target.visibility);
	}
	return [...byTty.values()];
}

function combineVisibility(left, right) {
	if (left === 'hidden' || right === 'hidden') return 'hidden';
	return left === 'visible' && right === 'visible' ? 'visible' : 'unknown';
}

function zellijClientCommandForSession(command, sessionName) {
	const arguments_ = command.trim().split(/\s+/u);
	const executable = arguments_[0];
	if (executable === undefined || path.basename(executable) !== 'zellij') return false;
	for (let index = 1; index < arguments_.length; index += 1) {
		const argument = arguments_[index];
		const next = arguments_[index + 1];
		if ((argument === '--session' || argument === '-s') && next === sessionName) return true;
		if (argument === `--session=${sessionName}`) return true;
		if ((argument === 'attach' || argument === 'a') && next === sessionName) return true;
	}
	return false;
}

function hasErrorCode(error, code) {
	return typeof error === 'object' && error !== null && error.code === code;
}

function isRecord(value) {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}
