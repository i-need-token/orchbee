import { forwardSanitizedObservation } from './observation-transport.mjs';

// Child sessions (subagent tasks) remembered so their events never read as root sessions.
export const OPENCODE_CHILD_SESSION_LIMIT = 256;
// Root sessions an OpenCode 2.x TUI left, remembered so a return never re-reports one.
export const OPENCODE_LEFT_SESSION_LIMIT = 256;
// How often the OpenCode 2.x TUI client reads the session it displays.
export const OPENCODE_CLIENT_POLL_MS = 1000;

// The plugin definition both loader generations accept once the installed
// bootstrap default-exports it. OpenCode 1.3.4+ and Kilo read `{ id, server }`
// and call `server`; OpenCode 2.x validates `{ id, setup }`. A 2.x plugin runs
// in a server process that never shares the Agent's console (a detached
// service, or a hidden child with its own console), so a private APC
// correlation cannot reach the Agent's terminal: setup registers nothing, and
// the client definition below observes 2.x instead.
export function openCodeFamilyPlugin(adapterId) {
	return {
		id: 'orchbee-agent-observation',
		server: async () => createOpenCodeObservation(adapterId),
		setup: async () => undefined,
	};
}

// The OpenCode 2.x client definition. 2.x loads it twice from one plugin
// directory: as a server plugin, where setup does nothing, and as a TUI plugin
// in the TUI process, which owns the Agent's console, where setup reports the
// root session the TUI displays.
export function openCodeClientPlugin() {
	return {
		id: 'orchbee-agent-observation-tui',
		setup: async (context) => createOpenCodeClientObservation(context),
	};
}

// Serial, bounded, best-effort delivery shared by every OpenCode-family owner.
// At 64 outstanding observations it stops admission until reload and discards
// the queued suffix; an in-flight send may finish.
function deliveryQueue(adapterId, send) {
	const pending = [];
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
	return {
		get stopped() {
			return stopped;
		},
		push(normalized) {
			if (stopped) return;
			if (pending.length + Number(running) >= 64) {
				stop();
				return;
			}
			pending.push(normalized);
			if (!running) void drain();
		},
		stop,
	};
}

// Both provider entries delegate normalization and bounded best-effort delivery
// to this owner. Only normalized observations are retained between callbacks.
export function createOpenCodeObservation(adapterId, { send = forwardSanitizedObservation } = {}) {
	if (adapterId !== 'opencode' && adapterId !== 'kilo') throw new Error('Unknown adapter');
	const queue = deliveryQueue(adapterId, send);
	const childSessions = new Set();
	const accept = (event) => {
		if (queue.stopped) return;
		const normalized = sanitizeOpenCodeFamilyEvent(adapterId, event);
		if (normalized === null || childSessions.has(normalized.sessionKey)) return;
		queue.push(normalized);
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
		dispose: async () => queue.stop(),
	};
}

// A child is a created or updated session whose info names a parent. The set
// is first-in, first-out; a child it never saw stays a second root candidate.
function rememberChildSession(childSessions, event) {
	if (!isRecord(event) || (event.type !== 'session.created' && event.type !== 'session.updated')) {
		return;
	}
	const info = isRecord(event.properties) ? event.properties.info : null;
	if (!isRecord(info) || !hasParent(info)) return;
	const childId = identity(info.id);
	if (childId === null || childSessions.has(childId)) return;
	remember(childSessions, childId, OPENCODE_CHILD_SESSION_LIMIT);
}

// The OpenCode 2.x TUI shares one session service with every other OpenCode
// client, so its events cover sessions other Panels show. The client reports
// only the root session its own route displays, read from the TUI plugin
// context (`ui.router`, `data.session`) once at setup and then on a timer:
//
// - a newly displayed root ends the root reported before it and starts itself;
// - returning to a root it left ends the current one and reports nothing, since
//   the Server never reopens an ended session;
// - home, a plugin page or a session whose info has not loaded changes nothing;
// - busy and idle follow the displayed root's status.
//
// At most one root is live per TUI, so the Panel's resume reference is exact or
// absent. Any other context, such as the server's, registers nothing.
export function createOpenCodeClientObservation(
	context,
	{ send = forwardSanitizedObservation, schedule = setInterval, unschedule = clearInterval } = {},
) {
	if (!isClientContext(context)) return undefined;
	const queue = deliveryQueue('opencode', send);
	const left = new Set();
	let shown = null;
	let busy = false;
	const tick = () => {
		if (queue.stopped) return;
		let root;
		let running;
		try {
			root = displayedRoot(context);
			if (root !== null && root !== shown) {
				if (shown !== null) {
					queue.push(observation(shown, 'session-ended'));
					remember(left, shown, OPENCODE_LEFT_SESSION_LIMIT);
				}
				shown = left.has(root) ? null : root;
				busy = false;
				if (shown !== null) queue.push(observation(shown, 'session-started'));
			}
			if (shown === null) return;
			running = context.data.session.status(shown) === 'running';
		} catch {
			// An unreadable TUI state reports nothing.
			return;
		}
		if (running === busy) return;
		busy = running;
		queue.push(
			running
				? observation(shown, 'turn-started', { activity: 'thinking' })
				: observation(shown, 'session-idle'),
		);
	};
	tick();
	const timer = schedule(tick, OPENCODE_CLIENT_POLL_MS);
	timer?.unref?.();
	return () => {
		unschedule(timer);
		queue.stop();
	};
}

function isClientContext(context) {
	return (
		isRecord(context) &&
		isRecord(context.ui) &&
		isRecord(context.ui.router) &&
		typeof context.ui.router.current === 'function' &&
		isRecord(context.data) &&
		isRecord(context.data.session) &&
		typeof context.data.session.get === 'function' &&
		typeof context.data.session.root === 'function' &&
		typeof context.data.session.status === 'function'
	);
}

// The root of the session the route displays, once its info and its root's
// info have loaded; `null` otherwise.
function displayedRoot(context) {
	const route = context.ui.router.current();
	if (!isRecord(route) || route.type !== 'session') return null;
	const id = identity(route.sessionID);
	if (id === null) return null;
	const info = context.data.session.get(id);
	if (!isRecord(info)) return null;
	const rootId = hasParent(info) ? identity(context.data.session.root(id)) : id;
	if (rootId === null) return null;
	const root = rootId === id ? info : context.data.session.get(rootId);
	if (!isRecord(root) || root.id !== rootId || hasParent(root)) return null;
	return rootId;
}

function hasParent(info) {
	return typeof info.parentID === 'string' && info.parentID !== '';
}

// A first-in, first-out bounded set.
function remember(set, value, limit) {
	if (set.has(value)) return;
	if (set.size >= limit) set.delete(set.values().next().value);
	set.add(value);
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
