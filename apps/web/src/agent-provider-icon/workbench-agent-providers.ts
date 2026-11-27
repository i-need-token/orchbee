import { AGENT_PRODUCT_MARKS } from './agent-product-marks';

/** Reported Agent labels that have a product mark, in the order a pair of marks shows them. */
export const WORKBENCH_AGENT_PROVIDER_ORDER: readonly string[] = [
	'claude',
	'codex',
	'gemini',
	'copilot',
	'cline',
	'grok',
	'omp',
	'opencode',
	'pi',
	'qwen',
	...AGENT_PRODUCT_MARKS.keys(),
];

const MARKED_AGENTS: ReadonlySet<string> = new Set(WORKBENCH_AGENT_PROVIDER_ORDER);

/** The Agent start provider ids whose reported label differs, as the Server's `report_label` maps them. */
const REPORT_LABELS: ReadonlyMap<string, string> = new Map([
	['antigravity', 'agy'],
	['claude-code', 'claude'],
	['github-copilot', 'copilot'],
	['qwen-code', 'qwen'],
]);

/** The reported label that keys an Agent's product mark, whichever of its two names it arrives as. */
export function agentMarkLabel(agent: string): string {
	return REPORT_LABELS.get(agent) ?? agent;
}

/** Whether a reported Agent label or Agent start provider id draws its own product mark. */
export function hasAgentProductMark(agent: string): boolean {
	return MARKED_AGENTS.has(agentMarkLabel(agent));
}
