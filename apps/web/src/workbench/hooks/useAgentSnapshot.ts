import * as React from 'react';

import { EMPTY_WORKBENCH_AGENT_SNAPSHOT } from '../constants/empty-workbench-agent-snapshot';
import type {
	WorkbenchAgentSnapshot,
	WorkbenchAgentSource,
} from '../types/workbench-agent-observation';

export function useAgentSnapshot(source: WorkbenchAgentSource | undefined): WorkbenchAgentSnapshot {
	return React.useSyncExternalStore(
		(listener) => source?.subscribe(listener) ?? (() => undefined),
		() => source?.getSnapshot() ?? EMPTY_WORKBENCH_AGENT_SNAPSHOT,
		() => source?.getSnapshot() ?? EMPTY_WORKBENCH_AGENT_SNAPSHOT,
	);
}
