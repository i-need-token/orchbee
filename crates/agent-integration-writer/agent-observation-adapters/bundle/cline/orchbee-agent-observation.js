import { forwardSanitizedObservation } from '../scripts/observation-transport.mjs';

export const CLINE_COMPLETION_RETENTION = Object.freeze({
	completedRunsPerConversation: 64,
	conversations: 128,
});

// `rootSessionKey` is the root session id Cline hands to `setup`; it is the id
// `cline --id` resumes. Without it the conversation id stands in, which no
// resume accepts.
export function sanitizeClineRuntimeEvent(value, rootSessionKey = null) {
	if (!isRecord(value) || !isRecord(value.snapshot)) return null;
	// Subagent runs carry their parent Agent; they would otherwise read as extra root sessions.
	const parentAgentId = value.snapshot.parentAgentId;
	if (typeof parentAgentId === 'string' && parentAgentId !== '') return null;
	const sessionKey = rootSessionKey ?? identity(value.snapshot.conversationId);
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

export function createClineRuntimeEventDelivery() {
	const completedRunsByConversation = new Map();
	return (value, deliver = deliverObservation, rootSessionKey = null) => {
		const normalized = sanitizeClineRuntimeEvent(value, rootSessionKey);
		if (normalized === null) return;
		if (
			completedRunIsRetained(completedRunsByConversation, normalized.sessionKey, normalized.turnKey)
		) {
			return;
		}
		if (isClineCompletionEvent(normalized.event)) {
			// A synchronous delivery sink may reenter with an older event for this run.
			rememberCompletedRun(completedRunsByConversation, normalized.sessionKey, normalized.turnKey);
		}
		deliver(normalized);
	};
}

// Cline loads each sandboxed plugin once per session, so one plugin instance
// serves one root session. If `setup` ever reports a missing or different id,
// the instance is shared and no event can be attributed: fall back for good.
export function createClinePlugin(deliver = deliverObservation) {
	const deliverRuntimeEvent = createClineRuntimeEventDelivery();
	let rootSessionKey = null;
	let setupCount = 0;
	return {
		hooks: {
			onEvent(value) {
				deliverRuntimeEvent(value, deliver, rootSessionKey);
			},
		},
		manifest: { capabilities: ['hooks'] },
		name: 'orchbee-agent-observation',
		setup(_api, ctx) {
			const reported =
				isRecord(ctx) && isRecord(ctx.session) ? identity(ctx.session.sessionId) : null;
			setupCount += 1;
			rootSessionKey = setupCount === 1 || reported === rootSessionKey ? reported : null;
		},
	};
}

export const plugin = createClinePlugin();

function deliverObservation(value) {
	void forwardSanitizedObservation('cline', value).catch(() => undefined);
}

function completedRunIsRetained(completedRunsByConversation, conversationId, runId) {
	const completedRuns = completedRunsByConversation.get(conversationId);
	return completedRuns?.has(runId) === true;
}

function isClineCompletionEvent(event) {
	return event === 'turn-stopped' || event === 'turn-failed' || event === 'turn-interrupted';
}

function rememberCompletedRun(completedRunsByConversation, conversationId, runId) {
	let completedRuns = completedRunsByConversation.get(conversationId);
	if (completedRuns === undefined) {
		if (completedRunsByConversation.size >= CLINE_COMPLETION_RETENTION.conversations) {
			const oldestConversationId = completedRunsByConversation.keys().next().value;
			if (oldestConversationId !== undefined) {
				completedRunsByConversation.delete(oldestConversationId);
			}
		}
		completedRuns = new Set();
	} else {
		completedRunsByConversation.delete(conversationId);
	}
	if (completedRuns.size >= CLINE_COMPLETION_RETENTION.completedRunsPerConversation) {
		const oldestRunId = completedRuns.values().next().value;
		if (oldestRunId !== undefined) completedRuns.delete(oldestRunId);
	}
	completedRuns.add(runId);
	completedRunsByConversation.set(conversationId, completedRuns);
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
