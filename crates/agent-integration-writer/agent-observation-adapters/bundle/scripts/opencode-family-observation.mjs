import { forwardSanitizedObservation } from './observation-transport.mjs';

// Child sessions (subagent tasks) remembered so their events never read as root sessions.
export const OPENCODE_CHILD_SESSION_LIMIT = 256;

// Both provider entries delegate normalization and bounded best-effort delivery
// to this owner. Only normalized observations are retained between callbacks.
export function createOpenCodeObservation(adapterId, { send = forwardSanitizedObservation } = {}) {
	if (adapterId !== 'opencode' && adapterId !== 'kilo') throw new Error('Unknown adapter');
	const pending = [];
	const childSessions = new Set();
	let running = false;
	let stopped = false;
	const stop = () => {
		stopped = true;
		pending.length = 0;
	};
	const drain = async () => {
		running = true;
		try {
			while (!stopped && pending.length > 0) {
				const next = pending.shift();
				try {
					await send(adapterId, next);
				} catch {
					// An observation failure must never reject the provider hook.
				}
			}
		} finally {
			running = false;
		}
	};
	const accept = (event) => {
		if (stopped) return;
		const normalized = sanitizeOpenCodeFamilyEvent(adapterId, event);
		if (normalized === null || childSessions.has(normalized.sessionKey)) return;
		if (pending.length + Number(running) >= 64) {
			stop();
			return;
		}
		pending.push(normalized);
		if (!running) void drain();
	};
	return {
		event: async (input) => {
			const event = isRecord(input) ? input.event : null;
			rememberChildSession(childSessions, event);
			accept(event);
		},
		'chat.message': async (input) => accept({ type: 'chat.message', properties: input }),
		'tool.execute.before': async (input) =>
			accept({ type: 'tool.execute.before', properties: input }),
		'tool.execute.after': async (input) =>
			accept({ type: 'tool.execute.after', properties: input }),
		dispose: async () => stop(),
	};
}

// A child is a created or updated session whose info names a parent. The set
// is first-in, first-out; a child it never saw stays a second root candidate.
function rememberChildSession(childSessions, event) {
	if (!isRecord(event) || (event.type !== 'session.created' && event.type !== 'session.updated')) {
		return;
	}
	const info = isRecord(event.properties) ? event.properties.info : null;
	if (!isRecord(info) || typeof info.parentID !== 'string' || info.parentID === '') return;
	const childId = identity(info.id);
	if (childId === null || childSessions.has(childId)) return;
	if (childSessions.size >= OPENCODE_CHILD_SESSION_LIMIT) {
		childSessions.delete(childSessions.values().next().value);
	}
	childSessions.add(childId);
}

function sessionIdentity(properties, event) {
	if (event !== 'session.created' && event !== 'session.deleted')
		return identity(properties.sessionID);
	const direct = properties.sessionID === undefined ? null : identity(properties.sessionID);
	if (properties.sessionID !== undefined && direct === null) return null;
	if (properties.info === undefined) return direct;
	if (!isRecord(properties.info)) return null;
	const nested = identity(properties.info.id);
	if (nested === null || (direct !== null && direct !== nested)) return null;
	return nested;
}

function statusObservation(adapterId, sessionKey, status) {
	if (adapterId === 'kilo' && typeof status === 'string') {
		if (status === 'idle') return observation(sessionKey, 'session-idle');
		if (['active', 'busy', 'pending', 'running', 'streaming', 'working'].includes(status)) {
			return observation(sessionKey, 'turn-started', { activity: 'thinking' });
		}
		return null;
	}
	if (!isRecord(status)) return null;
	switch (status.type) {
		case 'busy':
			return observation(sessionKey, 'turn-started', { activity: 'thinking' });
		case 'retry':
			return observation(sessionKey, 'tool-started', { activity: 'retrying' });
		case 'idle':
			return observation(sessionKey, 'session-idle');
		case 'offline': {
			const operationKey = identity(status.requestID);
			return adapterId === 'kilo' && operationKey !== null
				? observation(sessionKey, 'question-requested', { operationKey })
				: null;
		}
		default:
			return null;
	}
}

export function sanitizeOpenCodeFamilyEvent(adapterId, value) {
	if (!isRecord(value) || typeof value.type !== 'string' || !isRecord(value.properties)) {
		return null;
	}
	const properties = value.properties;
	const event = value.type;
	const sessionKey = sessionIdentity(properties, event);
	if (sessionKey === null) return null;

	switch (event) {
		case 'session.created':
			return observation(sessionKey, 'session-started');
		case 'session.deleted':
			return observation(sessionKey, 'session-ended');
		case 'session.status':
			return statusObservation(adapterId, sessionKey, properties.status);
		case 'chat.message':
			return observation(sessionKey, 'turn-started', { activity: 'thinking' });
		case 'session.idle':
			return observation(sessionKey, 'session-idle');
		case 'session.error':
			return observation(sessionKey, 'turn-failed');
		case 'question.asked':
		case 'question.v2.asked':
			return observation(sessionKey, 'question-requested', {
				operationKey: identity(properties.id),
			});
		case 'question.replied':
		case 'question.rejected':
		case 'question.v2.replied':
		case 'question.v2.rejected':
			return observation(sessionKey, 'attention-resolved', {
				operationKey: identity(properties.requestID),
			});
		case 'permission.asked':
		case 'permission.v2.asked':
			return observation(sessionKey, 'approval-requested', {
				operationKey: identity(properties.id),
			});
		case 'permission.replied':
		case 'permission.v2.replied':
			return observation(sessionKey, 'attention-resolved', {
				operationKey: identity(properties.requestID),
			});
		case 'tool.execute.before':
		case 'session.next.tool.called':
			return observation(sessionKey, 'tool-started', {
				activity: openCodeToolActivity(properties.tool),
				operationKey: identity(properties.callID),
			});
		case 'tool.execute.after':
		case 'session.next.tool.success':
			return observation(sessionKey, 'tool-finished', {
				operationKey: identity(properties.callID),
			});
		case 'session.next.tool.failed':
			return observation(sessionKey, 'tool-failed', {
				operationKey: identity(properties.callID),
			});
		case 'session.next.retried':
			return observation(sessionKey, 'tool-started', { activity: 'retrying' });
		default:
			return null;
	}
}

function openCodeToolActivity(toolName) {
	if (toolName === 'bash' || toolName === 'shell') return 'command';
	if (toolName === 'edit' || toolName === 'write' || toolName === 'apply_patch') return 'editing';
	if (toolName === 'task' || toolName === 'agent') return 'subagent';
	return 'tool';
}

function observation(sessionKey, event, fields = {}) {
	return {
		activity: fields.activity ?? null,
		agentKey: null,
		event,
		operationKey: fields.operationKey ?? null,
		sessionKey,
		turnKey: null,
		version: 1,
	};
}

function identity(value) {
	return typeof value === 'string' && /^[A-Za-z0-9._:-]{1,128}$/u.test(value) ? value : null;
}

function isRecord(value) {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}
