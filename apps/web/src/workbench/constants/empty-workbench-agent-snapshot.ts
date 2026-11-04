import type { WorkbenchAgentSnapshot } from '../types/workbench-agent-observation';

export const EMPTY_WORKBENCH_AGENT_SNAPSHOT: WorkbenchAgentSnapshot = Object.freeze({
	complete: false,
	presentations: new Map(),
});
