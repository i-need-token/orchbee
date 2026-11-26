import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { WorkbenchAgentObservation, WorkbenchAgentSource } from '../../workbench';

type AgentState = WorkbenchAgentObservation['state'];

/** An Agent that stops working has likely just written files: `working` becoming idle or done. */
function settled(previous: AgentState | undefined, current: AgentState): boolean {
	return previous === 'working' && (current === 'idle' || current === 'done');
}

/**
 * Calls `onSettled` each time a terminal Agent stops working, so the files it changed show at once
 * instead of waiting for a file event. A first observation, or a replacement terminal, is only a
 * baseline.
 */
export function useAgentSettled(
	source: WorkbenchAgentSource | undefined,
	onSettled: () => void,
): void {
	const settle = useMemoizedFn(onSettled);
	React.useEffect(() => {
		if (source === undefined) return;
		const states = new Map<string, { readonly binding: string; readonly state: AgentState }>();
		const observe = (): void => {
			const { presentations } = source.getSnapshot();
			let any = false;
			for (const id of states.keys()) if (!presentations.has(id)) states.delete(id);
			for (const [id, observation] of presentations) {
				const previous = states.get(id);
				if (
					previous?.binding === observation.binding &&
					settled(previous.state, observation.state)
				) {
					any = true;
				}
				states.set(id, { binding: observation.binding, state: observation.state });
			}
			if (any) settle();
		};
		observe();
		return source.subscribe(observe);
	}, [settle, source]);
}
