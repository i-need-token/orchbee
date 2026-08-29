import { forwardSanitizedObservation } from '../scripts/observation-transport.mjs';

export function sanitizeClineRuntimeEvent(value) {
	if (!isRecord(value) || !isRecord(value.snapshot)) return null;
	const sessionKey = identity(value.snapshot.conversationId);
	const turnKey = identity(value.snapshot.runId);
	if (sessionKey === null || turnKey === null) return null;

	switch (value.type) {
		case 'run-started':
		case 'turn-started':
			return observation(sessionKey, 'turn-started', { activity: 'thinking', turnKey });
		case 'tool-started': {
			if (!isRecord(value.toolCall)) return null;
			const operationKey = identity(value.toolCall.toolCallId);
			const toolName = identity(value.toolCall.toolName);
			if (operationKey === null) return null;
			return observation(
				sessionKey,
				toolName === 'ask_question' ? 'question-requested' : 'tool-started',
				{
					activity: toolName === 'ask_question' ? null : clineToolActivity(toolName),
					operationKey,
					turnKey,
				},
			);
		}
		case 'tool-finished': {
			if (!isRecord(value.toolCall)) return null;
			const operationKey = identity(value.toolCall.toolCallId);
			if (operationKey === null) return null;
			return observation(
				sessionKey,
				toolResultFailed(value.message, operationKey) ? 'tool-failed' : 'tool-finished',
				{ operationKey, turnKey },
			);
		}
		case 'run-finished':
			if (!isRecord(value.result)) return null;
			switch (value.result.status) {
				case 'completed':
					return observation(sessionKey, 'turn-stopped', { turnKey });
				case 'aborted':
					return observation(sessionKey, 'turn-interrupted', { turnKey });
				case 'failed':
					return observation(sessionKey, 'turn-failed', { turnKey });
				default:
					return null;
			}
		case 'run-failed':
			return observation(sessionKey, 'turn-failed', { turnKey });
		default:
			return null;
	}
}

export function deliverClineRuntimeEvent(value, deliver = deliverObservation) {
	const normalized = sanitizeClineRuntimeEvent(value);
	if (normalized !== null) deliver(normalized);
}

export const plugin = {
	hooks: {
		onEvent(value) {
			deliverClineRuntimeEvent(value);
		},
	},
	manifest: { capabilities: ['hooks'] },
	name: 'orchbee-agent-observation',
};

function deliverObservation(value) {
	void forwardSanitizedObservation('cline', value).catch(() => undefined);
}

function toolResultFailed(value, operationKey) {
	if (!isRecord(value) || !Array.isArray(value.content)) return false;
	for (const part of value.content) {
		if (
			isRecord(part) &&
			part.type === 'tool-result' &&
			part.toolCallId === operationKey &&
			part.isError === true
		) {
			return true;
		}
	}
	return false;
}

function clineToolActivity(toolName) {
	if (toolName === 'bash' || toolName === 'execute_command') return 'command';
	if (toolName === 'edit_file' || toolName === 'replace_in_file' || toolName === 'write_to_file') {
		return 'editing';
	}
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

function identity(value) {
	return typeof value === 'string' && /^[A-Za-z0-9._:-]{1,128}$/u.test(value) ? value : null;
}

function isRecord(value) {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}
