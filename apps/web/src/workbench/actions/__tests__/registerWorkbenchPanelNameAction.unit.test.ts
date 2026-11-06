import { TerminalPresentationIdSchema } from '@orchbee/workbench-client-protocol';
import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import { expect, it, vi } from 'vitest';

import {
	activeTerminalContext,
	createSnapshotRuntime,
	fixturePanelId,
	fixturePresentationId,
	fixtureTabId,
	oneWorkspaceSnapshot,
	publishSnapshot,
	snapshotPanel,
	snapshotPresentation,
	snapshotTab,
} from '../../../__tests__/server-workbench-snapshot';
import { inertWorkbenchAuthority } from '../../../__tests__/inertWorkbenchAuthority';
import type { WorkbenchAuthority } from '../../types/workbench-authority';
import {
	registerWorkbenchPanelNameAction,
	type WorkbenchPanelNameEditorRequest,
} from '../registerWorkbenchPanelNameAction';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	type WorkbenchPanelNameInput,
} from '../workbench-action-ids';

it('captures the primary name while a child is displayed, supports clear, and refuses a stale target', async () => {
	const runtime = new ActionRuntime();
	const scope = runtime.createScope('workbench');
	const projectWorkspaceRuntime = createSnapshotRuntime(
		oneWorkspaceSnapshot(1, [
			snapshotTab(
				1,
				snapshotPanel(1, snapshotPresentation(1, { customLabel: 'Primary name' }), [
					snapshotPresentation(2, { customLabel: 'Child name' }),
				]),
			),
			snapshotTab(2, snapshotPanel(3)),
		]),
	);
	const terminalContext = activeTerminalContext(projectWorkspaceRuntime);
	terminalContext.panelWorkspaceStore.transition({
		type: 'child-terminal-activated',
		childPresentationId: TerminalPresentationIdSchema.parse(fixturePresentationId(2)),
		panelId: fixturePanelId(1),
		tabId: fixtureTabId(1),
	});
	const openEditor = vi.fn<(request: WorkbenchPanelNameEditorRequest) => void>();
	const change = vi.fn<WorkbenchAuthority['change']>(async () => ({ kind: 'completed' }));
	registerWorkbenchPanelNameAction({
		authority: inertWorkbenchAuthority({ change }),
		openEditor,
		projectWorkspaceRuntime,
		scope,
		typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(runtime),
	});
	const execute = (input: WorkbenchPanelNameInput) =>
		runtime.execute(WORKBENCH_ACTION_IDS.renamePanel, input, { scope, source: 'context-menu' })
			.completion;
	await execute(undefined);
	const captured = openEditor.mock.calls[0]?.[0];
	expect(captured).toMatchObject({
		panelId: fixturePanelId(1),
		presentationId: fixturePresentationId(1),
		title: 'Primary name',
		unnamed: false,
	});
	if (captured === undefined) throw new Error('Expected the captured primary name.');
	terminalContext.tabModel.selectTab(fixtureTabId(2));
	for (const title of ['Review 雪', '']) {
		await expect(execute({ ...captured, kind: 'commit', title })).resolves.toEqual({
			kind: 'completed',
		});
		expect(change).toHaveBeenLastCalledWith(expect.anything(), {
			type: 'panel-renamed',
			panelId: captured.panelId,
			tabId: captured.tabId,
			terminalContextId: captured.terminalContextId,
			label: title,
		});
	}
	await expect(
		execute({
			...captured,
			kind: 'commit',
			presentationId: fixturePresentationId(2),
			title: 'Wrong child',
		}),
	).resolves.toMatchObject({ kind: 'blocked' });
	publishSnapshot(
		projectWorkspaceRuntime,
		oneWorkspaceSnapshot(2, [snapshotTab(2, snapshotPanel(3))]),
	);
	await expect(execute({ ...captured, kind: 'commit', title: 'Gone' })).resolves.toMatchObject({
		kind: 'blocked',
	});
	expect(change).toHaveBeenCalledTimes(2);
});
