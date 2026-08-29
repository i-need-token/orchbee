import type { AgentRuntimeSnapshot, AgentSnapshotSource } from '@orchbee/agent';
import * as React from 'react';

import { EMPTY_AGENT_RUNTIME_SNAPSHOT } from '../constants/empty-agent-runtime-snapshot';

export function useAgentSnapshot(source: AgentSnapshotSource | undefined): AgentRuntimeSnapshot {
	return React.useSyncExternalStore(
		(listener) => source?.subscribe(listener) ?? (() => undefined),
		() => source?.getSnapshot() ?? EMPTY_AGENT_RUNTIME_SNAPSHOT,
		() => source?.getSnapshot() ?? EMPTY_AGENT_RUNTIME_SNAPSHOT,
	);
}
