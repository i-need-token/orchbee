import { forwardSanitizedObservation } from '../scripts/observation-transport.mjs';

export const OrchbeeAgentObservation = async () => {
	const deliver = orderedDelivery('opencode');
	return {
		event: async ({ event }) => {
			const normalized = sanitizeOpenCodeEvent(event);
			if (normalized !== null) deliver(normalized);
		},
	};
};

export function sanitizeOpenCodeEvent(value) {
	if (!isRecord(value) || typeof value.type !== 'string' || !isRecord(value.properties)) {
		return null;
	}
	const properties = value.properties;
	const sessionKey = identity(properties.sessionID);
	const event = value.type;
	if (sessionKey === null) return null;

	switch (event) {
		case 'session.created':
			return observation(sessionKey, 'session-started');
		case 'session.deleted':
			return observation(sessionKey, 'session-ended');
		case 'session.status':
			if (!isRecord(properties.status)) return null;
			switch (properties.status.type) {
				case 'busy':
					return observation(sessionKey, 'turn-started', { activity: 'thinking' });
				case 'retry':
					return observation(sessionKey, 'tool-started', { activity: 'retrying' });
				case 'idle':
					return observation(sessionKey, 'session-idle');
				default:
					return null;
			}
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
		case 'session.next.tool.called':
			return observation(sessionKey, 'tool-started', {
				activity: openCodeToolActivity(properties.tool),
				operationKey: identity(properties.callID),
			});
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

function orderedDelivery(adapterId) {
	let pending = Promise.resolve();
	return (value) => {
		pending = pending.then(() => forwardSanitizedObservation(adapterId, value));
	};
}

function identity(value) {
	return typeof value === 'string' && /^[A-Za-z0-9._:-]{1,128}$/u.test(value) ? value : null;
}

function isRecord(value) {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}
