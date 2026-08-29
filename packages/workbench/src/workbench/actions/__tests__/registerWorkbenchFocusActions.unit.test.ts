import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import { describe, expect, it, vi } from 'vitest';

import type {
	WorkbenchFocusIntent,
	WorkbenchFocusResolution,
} from '../../model/resolveWorkbenchFocusTarget';
import { registerWorkbenchFocusActions } from '../registerWorkbenchFocusActions';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from '../workbench-action-ids';

type FocusRouter = Parameters<typeof registerWorkbenchFocusActions>[0]['focusRouter'];

describe('registerWorkbenchFocusActions', () => {
	it('registers each focus route and commits only concrete target changes', async () => {
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('workbench');
		const resolutionFor = (intent: WorkbenchFocusIntent): WorkbenchFocusResolution => {
			if (intent.kind === 'cycle' && intent.direction === 'next') {
				return { kind: 'target', region: 'tab-strip' };
			}
			if (intent.kind === 'direct' && intent.region === 'terminal') {
				return { kind: 'unchanged', region: 'terminal' };
			}
			return { kind: 'unavailable', reason: 'target-unavailable' };
		};
		const focusRouter: FocusRouter = {
			focus: vi.fn<FocusRouter['focus']>(resolutionFor),
			getResolution: vi.fn<FocusRouter['getResolution']>(resolutionFor),
		};
		const registrations = registerWorkbenchFocusActions({
			focusRouter,
			scope,
			typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(runtime),
		});

		expect(registrations).toHaveLength(5);
		expect(
			runtime.resolveAction(WORKBENCH_ACTION_IDS.focusNextRegion, undefined, {
				scope,
				source: 'keyboard',
			})?.availability,
		).toEqual({ kind: 'enabled' });
		await expect(
			runtime.execute(WORKBENCH_ACTION_IDS.focusNextRegion, undefined, {
				scope,
				source: 'keyboard',
			}).completion,
		).resolves.toEqual({ kind: 'completed' });
		expect(focusRouter.focus).toHaveBeenCalledExactlyOnceWith({
			direction: 'next',
			kind: 'cycle',
		});

		await expect(
			runtime.execute(WORKBENCH_ACTION_IDS.focusActiveTerminal, undefined, {
				scope,
				source: 'command-surface',
			}).completion,
		).resolves.toEqual({ kind: 'unchanged' });
		expect(focusRouter.focus).toHaveBeenCalledOnce();

		expect(
			runtime.resolveAction(WORKBENCH_ACTION_IDS.focusPreviousRegion, undefined, {
				scope,
				source: 'keyboard',
			})?.availability,
		).toMatchObject({ kind: 'disabled' });
		await expect(
			runtime.execute(WORKBENCH_ACTION_IDS.focusPreviousRegion, undefined, {
				scope,
				source: 'keyboard',
			}).completion,
		).resolves.toMatchObject({ code: 'action-unavailable', kind: 'blocked' });

		for (const registration of registrations) registration.dispose();
	});
});
