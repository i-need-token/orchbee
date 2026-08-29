import { forwardSanitizedObservation } from '../scripts/observation-transport.mjs';

export function orchbeeAgentObservation(pi) {
	registerPiAgentObservation(pi, orderedDelivery('pi'));
}

export function registerPiAgentObservation(pi, deliver) {
	const activeTurns = new Map();
	const prompts = new Map();
	let identitySequence = 0;

	const nextKey = (prefix) => `${prefix}:${++identitySequence}`;
	const send = (context, event, fields = {}) => {
		const sessionKey = context.sessionManager.getSessionId();
		if (!identity(sessionKey)) return;
		deliver(observation(sessionKey, event, fields));
	};

	pi.on('session_start', (_event, context) => send(context, 'session-started'));
	pi.on('agent_start', (_event, context) => {
		const sessionKey = context.sessionManager.getSessionId();
		const turnKey = nextKey('turn');
		activeTurns.set(sessionKey, turnKey);
		send(context, 'turn-started', { activity: 'thinking', turnKey });
	});
	pi.on('agent_settled', (_event, context) => {
		const sessionKey = context.sessionManager.getSessionId();
		send(context, 'session-idle', { turnKey: activeTurns.get(sessionKey) ?? null });
		activeTurns.delete(sessionKey);
		prompts.delete(sessionKey);
	});
	pi.on('ui_prompt_start', (_event, context) => {
		const sessionKey = context.sessionManager.getSessionId();
		const operationKey = nextKey('prompt');
		prompts.set(sessionKey, operationKey);
		send(context, 'question-requested', {
			operationKey,
			turnKey: activeTurns.get(sessionKey) ?? null,
		});
	});
	pi.on('ui_prompt_end', (_event, context) => {
		const sessionKey = context.sessionManager.getSessionId();
		const operationKey = prompts.get(sessionKey) ?? null;
		prompts.delete(sessionKey);
		send(context, 'attention-resolved', {
			operationKey,
			turnKey: activeTurns.get(sessionKey) ?? null,
		});
	});
	pi.on('tool_execution_start', (event, context) => {
		const sessionKey = context.sessionManager.getSessionId();
		send(context, 'tool-started', {
			activity: piToolActivity(event.toolName),
			operationKey: event.toolCallId,
			turnKey: activeTurns.get(sessionKey) ?? null,
		});
	});
	pi.on('tool_execution_end', (event, context) => {
		const sessionKey = context.sessionManager.getSessionId();
		send(context, event.isError ? 'tool-failed' : 'tool-finished', {
			operationKey: event.toolCallId,
			turnKey: activeTurns.get(sessionKey) ?? null,
		});
	});
	pi.on('session_before_compact', (_event, context) => {
		const sessionKey = context.sessionManager.getSessionId();
		send(context, 'compaction-started', { turnKey: activeTurns.get(sessionKey) ?? null });
	});
	pi.on('session_compact', (_event, context) => {
		const sessionKey = context.sessionManager.getSessionId();
		send(context, 'compaction-finished', { turnKey: activeTurns.get(sessionKey) ?? null });
	});
	pi.on('session_compact_failed', (_event, context) => {
		const sessionKey = context.sessionManager.getSessionId();
		send(context, 'compaction-finished', { turnKey: activeTurns.get(sessionKey) ?? null });
	});
	pi.on('session_shutdown', (_event, context) => {
		const sessionKey = context.sessionManager.getSessionId();
		send(context, 'session-ended', { turnKey: activeTurns.get(sessionKey) ?? null });
		activeTurns.delete(sessionKey);
		prompts.delete(sessionKey);
	});
}

export function piToolActivity(toolName) {
	if (toolName === 'bash' || toolName === 'powershell') return 'command';
	if (toolName === 'edit' || toolName === 'write') return 'editing';
	return 'tool';
}

function observation(sessionKey, event, fields = {}) {
	return {
		activity: fields.activity ?? null,
		agentKey: null,
		event,
		operationKey: fields.operationKey ?? null,
		sessionKey,
		turnKey: fields.turnKey ?? null,
		version: 1,
	};
}

function orderedDelivery(adapterId) {
	let pending = Promise.resolve();
	return (value) => {
		pending = pending.then(() => forwardSanitizedObservation(adapterId, value));
	};
}

function identity(value) {
	return typeof value === 'string' && /^[A-Za-z0-9._:-]{1,128}$/u.test(value);
}
