import { forwardSanitizedObservation } from '../scripts/observation-transport.mjs';

export function orchbeeAgentObservation(omp) {
	registerOmpAgentObservation(omp, orderedDelivery('omp'));
}

export function registerOmpAgentObservation(omp, deliver) {
	const activeTurns = new Map();
	let identitySequence = 0;

	const send = (context, event, fields = {}) => {
		const sessionKey = context.sessionManager.getSessionId();
		if (!identity(sessionKey)) return;
		deliver(observation(sessionKey, event, fields));
	};

	omp.on('session_start', (_event, context) => send(context, 'session-started'));
	omp.on('agent_start', (_event, context) => {
		const sessionKey = context.sessionManager.getSessionId();
		const turnKey = `turn:${++identitySequence}`;
		activeTurns.set(sessionKey, turnKey);
		send(context, 'turn-started', { activity: 'thinking', turnKey });
	});
	omp.on('agent_end', (event, context) => {
		if (event.willContinue === true) return;
		const sessionKey = context.sessionManager.getSessionId();
		send(context, 'session-idle', { turnKey: activeTurns.get(sessionKey) ?? null });
		activeTurns.delete(sessionKey);
	});
	omp.on('tool_approval_requested', (event, context) => {
		const sessionKey = context.sessionManager.getSessionId();
		send(context, 'approval-requested', {
			operationKey: event.toolCallId,
			turnKey: activeTurns.get(sessionKey) ?? null,
		});
	});
	omp.on('tool_approval_resolved', (event, context) => {
		const sessionKey = context.sessionManager.getSessionId();
		send(context, 'attention-resolved', {
			operationKey: event.toolCallId,
			turnKey: activeTurns.get(sessionKey) ?? null,
		});
	});
	omp.on('tool_execution_start', (event, context) => {
		const sessionKey = context.sessionManager.getSessionId();
		const question = event.toolName.toLowerCase() === 'ask';
		send(context, question ? 'question-requested' : 'tool-started', {
			activity: question ? null : ompToolActivity(event.toolName),
			operationKey: event.toolCallId,
			turnKey: activeTurns.get(sessionKey) ?? null,
		});
	});
	omp.on('tool_execution_end', (event, context) => {
		const sessionKey = context.sessionManager.getSessionId();
		const question = event.toolName.toLowerCase() === 'ask';
		const toolEvent = event.isError ? 'tool-failed' : 'tool-finished';
		send(context, question ? 'attention-resolved' : toolEvent, {
			operationKey: event.toolCallId,
			turnKey: activeTurns.get(sessionKey) ?? null,
		});
	});
	omp.on('auto_compaction_start', (_event, context) => {
		const sessionKey = context.sessionManager.getSessionId();
		send(context, 'compaction-started', { turnKey: activeTurns.get(sessionKey) ?? null });
	});
	omp.on('auto_compaction_end', (_event, context) => {
		const sessionKey = context.sessionManager.getSessionId();
		send(context, 'compaction-finished', { turnKey: activeTurns.get(sessionKey) ?? null });
	});
	omp.on('auto_retry_start', (_event, context) => {
		const sessionKey = context.sessionManager.getSessionId();
		send(context, 'tool-started', {
			activity: 'retrying',
			turnKey: activeTurns.get(sessionKey) ?? null,
		});
	});
	omp.on('auto_retry_end', (event, context) => {
		const sessionKey = context.sessionManager.getSessionId();
		send(context, event.success ? 'tool-finished' : 'tool-failed', {
			turnKey: activeTurns.get(sessionKey) ?? null,
		});
	});
	omp.on('session_shutdown', (_event, context) => {
		const sessionKey = context.sessionManager.getSessionId();
		send(context, 'session-ended', { turnKey: activeTurns.get(sessionKey) ?? null });
		activeTurns.delete(sessionKey);
	});
}

export function ompToolActivity(toolName) {
	if (toolName === 'bash' || toolName === 'powershell') return 'command';
	if (toolName === 'edit' || toolName === 'write' || toolName === 'apply_patch') return 'editing';
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
