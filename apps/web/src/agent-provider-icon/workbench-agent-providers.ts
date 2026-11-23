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

const MARKED_AGENTS: ReadonlySet<string> = new Set([
	...WORKBENCH_AGENT_PROVIDER_ORDER,
	'claude-code',
	'github-copilot',
	'qwen-code',
]);

/** Whether a reported Agent label or Agent start provider id draws its own product mark. */
export function hasAgentProductMark(agent: string): boolean {
	return MARKED_AGENTS.has(agent);
}
