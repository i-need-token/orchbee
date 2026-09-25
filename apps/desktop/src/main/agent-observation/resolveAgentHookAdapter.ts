import { AgentProviderIdSchema, type AgentCapability, type AgentProviderId } from '@orchbee/agent';

import type {
	AgentHookAdapterId,
	AgentHookObservationEvent,
} from './agent-hook-observation-contract';

export interface AgentHookAdapterDefinition {
	readonly adapterId: AgentHookAdapterId;
	readonly capabilities: readonly AgentCapability[];
	readonly providerId: AgentProviderId;
}

interface RegisteredAgentHookAdapter extends AgentHookAdapterDefinition {
	readonly acceptedEvents: ReadonlySet<AgentHookObservationEvent>;
}

const CLAUDE_CODE_EVENTS = new Set<AgentHookObservationEvent>([
	'session-started',
	'turn-started',
	'tool-started',
	'tool-finished',
	'tool-failed',
	'question-requested',
	'approval-requested',
	'compaction-started',
	'compaction-finished',
	'subagent-started',
	'subagent-stopped',
	'turn-stopped',
	'turn-failed',
	'session-ended',
]);

const CODEX_EVENTS = new Set<AgentHookObservationEvent>([
	'session-started',
	'turn-started',
	'tool-started',
	'tool-finished',
	'question-requested',
	'approval-requested',
	'compaction-started',
	'compaction-finished',
	'subagent-started',
	'subagent-stopped',
	'turn-stopped',
	'session-ended',
]);

const REGISTRY = {
	cursor: {
		acceptedEvents: new Set<AgentHookObservationEvent>(['session-started']),
		adapterId: 'cursor',
		capabilities: ['sessionLifecycle'],
		providerId: AgentProviderIdSchema.parse('cursor'),
	},
	'claude-code': {
		acceptedEvents: CLAUDE_CODE_EVENTS,
		adapterId: 'claude-code',
		capabilities: [
			'sessionLifecycle',
			'turnLifecycle',
			'questions',
			'approvals',
			'activity',
			'subagents',
		],
		providerId: AgentProviderIdSchema.parse('claude-code'),
	},
	codex: {
		acceptedEvents: CODEX_EVENTS,
		adapterId: 'codex',
		capabilities: [
			'sessionLifecycle',
			'turnLifecycle',
			'questions',
			'approvals',
			'activity',
			'subagents',
		],
		providerId: AgentProviderIdSchema.parse('codex'),
	},
	grok: {
		acceptedEvents: new Set<AgentHookObservationEvent>([
			'session-started',
			'session-idle',
			'turn-started',
			'tool-started',
			'tool-finished',
			'tool-failed',
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
		]),
		adapterId: 'grok',
		capabilities: ['sessionLifecycle', 'turnLifecycle', 'approvals', 'activity', 'subagents'],
		providerId: AgentProviderIdSchema.parse('grok'),
	},
	opencode: {
		acceptedEvents: new Set<AgentHookObservationEvent>([
			'session-started',
			'session-idle',
			'turn-started',
			'tool-started',
			'tool-finished',
			'tool-failed',
			'question-requested',
			'approval-requested',
			'attention-resolved',
			'turn-failed',
			'session-ended',
		]),
		adapterId: 'opencode',
		capabilities: ['sessionLifecycle', 'turnLifecycle', 'questions', 'approvals', 'activity'],
		providerId: AgentProviderIdSchema.parse('opencode'),
	},
	kilo: {
		acceptedEvents: new Set<AgentHookObservationEvent>([
			'session-started',
			'session-idle',
			'turn-started',
			'tool-started',
			'tool-finished',
			'tool-failed',
			'question-requested',
			'approval-requested',
			'attention-resolved',
			'turn-failed',
			'session-ended',
		]),
		adapterId: 'kilo',
		capabilities: ['sessionLifecycle', 'turnLifecycle', 'questions', 'approvals', 'activity'],
		providerId: AgentProviderIdSchema.parse('kilo'),
	},
	pi: {
		acceptedEvents: new Set<AgentHookObservationEvent>([
			'session-started',
			'session-idle',
			'turn-started',
			'tool-started',
			'tool-finished',
			'tool-failed',
			'question-requested',
			'attention-resolved',
			'compaction-started',
			'compaction-finished',
			'session-ended',
		]),
		adapterId: 'pi',
		capabilities: ['sessionLifecycle', 'turnLifecycle', 'questions', 'activity'],
		providerId: AgentProviderIdSchema.parse('pi'),
	},
	omp: {
		acceptedEvents: new Set<AgentHookObservationEvent>([
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
			'session-ended',
		]),
		adapterId: 'omp',
		capabilities: ['sessionLifecycle', 'turnLifecycle', 'questions', 'approvals', 'activity'],
		providerId: AgentProviderIdSchema.parse('omp'),
	},
	gemini: {
		acceptedEvents: new Set<AgentHookObservationEvent>([
			'session-started',
			'turn-started',
			'tool-started',
			'tool-finished',
			'question-requested',
			'approval-requested',
			'compaction-started',
			'turn-stopped',
			'session-ended',
		]),
		adapterId: 'gemini',
		capabilities: ['sessionLifecycle', 'turnLifecycle', 'questions', 'approvals', 'activity'],
		providerId: AgentProviderIdSchema.parse('gemini'),
	},
	'github-copilot': {
		acceptedEvents: new Set<AgentHookObservationEvent>([
			'session-started',
			'turn-started',
			'tool-finished',
			'tool-failed',
			'question-requested',
			'approval-requested',
			'compaction-started',
			'turn-stopped',
			'turn-failed',
			'session-ended',
		]),
		adapterId: 'github-copilot',
		capabilities: ['sessionLifecycle', 'turnLifecycle', 'questions', 'approvals', 'activity'],
		providerId: AgentProviderIdSchema.parse('github-copilot'),
	},
	cline: {
		acceptedEvents: new Set<AgentHookObservationEvent>([
			'turn-started',
			'tool-started',
			'tool-finished',
			'tool-failed',
			'question-requested',
			'turn-stopped',
			'turn-failed',
			'turn-interrupted',
		]),
		adapterId: 'cline',
		capabilities: ['turnLifecycle', 'questions', 'activity'],
		providerId: AgentProviderIdSchema.parse('cline'),
	},
	'qwen-code': {
		acceptedEvents: new Set<AgentHookObservationEvent>([
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
			'session-ended',
		]),
		adapterId: 'qwen-code',
		capabilities: [
			'sessionLifecycle',
			'turnLifecycle',
			'questions',
			'approvals',
			'activity',
			'subagents',
		],
		providerId: AgentProviderIdSchema.parse('qwen-code'),
	},
} as const satisfies Record<AgentHookAdapterId, RegisteredAgentHookAdapter>;

export function resolveAgentHookAdapter(
	adapterId: AgentHookAdapterId,
	event: AgentHookObservationEvent,
): AgentHookAdapterDefinition | null {
	const adapter = REGISTRY[adapterId];
	if (!adapter.acceptedEvents.has(event)) return null;
	return {
		adapterId: adapter.adapterId,
		capabilities: adapter.capabilities,
		providerId: adapter.providerId,
	};
}
