import type { WorkbenchTabDragEvent, WorkbenchTabDragState } from '../types/workbench-tab-drag';
import { getWorkbenchTabsTopologyKey } from './workbench-tab-drag-state';

type WorkbenchTabDragReconciliationEvent = Extract<
	WorkbenchTabDragEvent,
	{ readonly kind: 'acknowledge-commit' | 'stale-input' }
>;

export function resolveWorkbenchTabDragReconciliation(
	state: WorkbenchTabDragState,
	previousTopologyKey: string,
	topologyKey: string,
): WorkbenchTabDragReconciliationEvent | undefined {
	if (
		state.kind === 'settling' &&
		topologyKey === getWorkbenchTabsTopologyKey(state.previewState)
	) {
		return { kind: 'acknowledge-commit' };
	}
	if (state.kind !== 'idle' && previousTopologyKey !== topologyKey) {
		return { kind: 'stale-input' };
	}
	return undefined;
}
