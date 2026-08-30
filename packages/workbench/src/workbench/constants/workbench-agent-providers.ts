import { AgentProviderIdSchema } from '@orchbee/agent';

export const WORKBENCH_AGENT_PROVIDER_ORDER = [
	AgentProviderIdSchema.parse('claude-code'),
	AgentProviderIdSchema.parse('codex'),
	AgentProviderIdSchema.parse('gemini'),
	AgentProviderIdSchema.parse('github-copilot'),
	AgentProviderIdSchema.parse('cline'),
	AgentProviderIdSchema.parse('grok'),
	AgentProviderIdSchema.parse('omp'),
	AgentProviderIdSchema.parse('opencode'),
	AgentProviderIdSchema.parse('pi'),
	AgentProviderIdSchema.parse('qwen-code'),
] as const;
