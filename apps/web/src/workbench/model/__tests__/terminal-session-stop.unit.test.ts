import { describe, expect, it } from 'vitest';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
	workbenchSharedDocumentRevision,
	type TerminalPresentationCommand,
	type TerminalPresentationId,
} from '@orchbee/workbench-client-protocol';

import { collectPanelLeaves } from '../../components/workbench-panel-workspace/model/panel-workspace-state';
import type {
	WorkbenchTerminalPresentationCommand,
	WorkbenchTerminalPresentationComposition,
} from '../createWorkbenchPreviewTerminalPresentationComposition';
import { WorkbenchProjectWorkspaceRuntime } from '../WorkbenchProjectWorkspaceRuntime';

interface ManagedFixture {
	readonly commands: WorkbenchTerminalPresentationCommand[];
	readonly runtime: WorkbenchProjectWorkspaceRuntime;
}

function createManagedFixture(
	initialDocument?: ConstructorParameters<
		typeof WorkbenchProjectWorkspaceRuntime
	>[0]['initialDocument'],
): ManagedFixture {
	let operationSequence = 1;
	let presentationSequence = 2;
	let removalSequence = 1;
	const commands: WorkbenchTerminalPresentationCommand[] = [];
	const composition: WorkbenchTerminalPresentationComposition = {
		createOperationId: () =>
			terminalPresentationOperationId(`stop:operation:${operationSequence++}`),
		createPresentationId: () =>
			terminalPresentationId(`stop:presentation:${presentationSequence++}`),
		createRemovalId: () => terminalPresentationRemovalId(`stop:removal:${removalSequence++}`),
		dispatchCommands: (nextCommands) => commands.push(...nextCommands),
		initialPresentationId: terminalPresentationId('stop:presentation:1'),
		kind: 'managed',
	};
	return {
		commands,
		runtime: new WorkbenchProjectWorkspaceRuntime({
			...(initialDocument === undefined ? {} : { initialDocument }),
			standaloneCwd: '/workspace',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: composition,
		}),
	};
}

function terminalCommands(
	commands: readonly WorkbenchTerminalPresentationCommand[],
): readonly TerminalPresentationCommand[] {
	return commands.filter(
		(command): command is TerminalPresentationCommand => command.type !== 'tab-close-ready',
	);
}

function presentationIds(
	runtime: WorkbenchProjectWorkspaceRuntime,
): readonly TerminalPresentationId[] {
	return runtime
		.getActiveTerminalContext()
		.panelWorkspaceStore.getSnapshot()
		.tabs.flatMap((tab) =>
			collectPanelLeaves(tab.root).flatMap((panel) => [
				panel.primaryPresentation.presentationId,
				...panel.childPresentations.map((terminal) => terminal.presentationId),
			]),
		);
}

describe('durable terminal-session stop', () => {
	it('stops visible and hidden Tabs without removing their shared product structure', () => {
		const fixture = createManagedFixture();
		const firstProvision = terminalCommands(fixture.commands)[0];
		if (firstProvision?.type !== 'terminal-provision-requested') {
			throw new Error('Expected the initial provision command.');
		}
		fixture.runtime.applyTerminalPresentationEvent({
			operationId: firstProvision.operationId,
			presentationId: firstProvision.presentationId,
			type: 'terminal-provision-completed',
		});
		const creation = fixture.runtime.createTerminal({
			groupId: null,
			source: 'button',
			title: 'Hidden Terminal',
		});
		if (creation.kind !== 'created') throw new Error('Expected a second terminal Tab.');
		const secondProvision = terminalCommands(fixture.commands)[1];
		if (secondProvision?.type !== 'terminal-provision-requested') {
			throw new Error('Expected the second provision command.');
		}
		fixture.runtime.applyTerminalPresentationEvent({
			operationId: secondProvision.operationId,
			presentationId: secondProvision.presentationId,
			type: 'terminal-provision-completed',
		});
		const documentBefore = fixture.runtime.getSharedDocument(workbenchSharedDocumentRevision('1'));
		const idsBefore = presentationIds(fixture.runtime);

		const requested = fixture.runtime.requestAllTerminalSessionsStop();
		expect(requested).toMatchObject({ commandCount: 2, kind: 'started' });
		const stopCommands = terminalCommands(fixture.commands).slice(-2);
		expect(stopCommands).toEqual([
			expect.objectContaining({
				presentationId: idsBefore[0],
				type: 'terminal-close-requested',
			}),
			expect.objectContaining({
				presentationId: idsBefore[1],
				type: 'terminal-close-requested',
			}),
		]);
		const stoppingDocument = fixture.runtime.getSharedDocument(
			workbenchSharedDocumentRevision('2'),
		);
		expect(
			stoppingDocument.terminalContexts[0]?.panelWorkspace.tabs.map(
				(tab) => tab.root.kind === 'panel' && tab.root.primaryPresentation.intent.kind,
			),
		).toEqual(['stop', 'stop']);

		for (const command of stopCommands) {
			if (command?.type !== 'terminal-close-requested') continue;
			expect(
				fixture.runtime.applyTerminalPresentationEvent({
					operationId: command.operationId,
					presentationId: command.presentationId,
					type: 'terminal-close-completed',
				}),
			).toEqual({ kind: 'applied' });
		}

		expect(fixture.runtime.getTerminalSessionStopSnapshot()).toMatchObject({
			complete: true,
			presentationCount: 2,
			stoppedCount: 2,
		});
		expect(presentationIds(fixture.runtime)).toEqual(idsBefore);
		const documentAfter = fixture.runtime.getSharedDocument(workbenchSharedDocumentRevision('3'));
		expect(documentAfter.projectWorkspace).toEqual(documentBefore.projectWorkspace);
		expect(
			documentAfter.terminalContexts[0]?.panelWorkspace.tabs.map((tab) => ({
				intent: tab.root.kind === 'panel' && tab.root.primaryPresentation.intent.kind,
				tabId: tab.tabId,
			})),
		).toEqual(
			documentBefore.terminalContexts[0]?.panelWorkspace.tabs.map((tab) => ({
				intent: 'stopped',
				tabId: tab.tabId,
			})),
		);
	});

	it('hydrates stopped shared document without commands and starts only the selected session freshly', () => {
		const source = createManagedFixture();
		const provision = terminalCommands(source.commands)[0];
		if (provision?.type !== 'terminal-provision-requested') throw new Error('Missing provision.');
		source.runtime.applyTerminalPresentationEvent({
			operationId: provision.operationId,
			presentationId: provision.presentationId,
			type: 'terminal-provision-completed',
		});
		source.runtime.requestAllTerminalSessionsStop();
		const stop = terminalCommands(source.commands).at(-1);
		if (stop?.type !== 'terminal-close-requested') throw new Error('Missing stop command.');
		source.runtime.applyTerminalPresentationEvent({
			operationId: stop.operationId,
			presentationId: stop.presentationId,
			type: 'terminal-close-completed',
		});
		const stoppedDocument = source.runtime.getSharedDocument(workbenchSharedDocumentRevision('4'));
		const restored = createManagedFixture(stoppedDocument);

		expect(restored.runtime.reconcileDurableTerminalPresentations()).toEqual({
			commandCount: 0,
			kind: 'started',
		});
		expect(restored.commands).toEqual([]);
		expect(restored.runtime.startStoppedTerminalSession(stop.presentationId)).toEqual({
			kind: 'applied',
		});
		const freshProvision = terminalCommands(restored.commands)[0];
		expect(freshProvision).toMatchObject({
			launchCwd: '/workspace',
			presentationId: 'stop:presentation:2',
			type: 'terminal-provision-requested',
		});
		expect(freshProvision?.presentationId).not.toBe(stop.presentationId);
		expect(presentationIds(restored.runtime)).toEqual([
			terminalPresentationId('stop:presentation:2'),
		]);
	});

	it('stages a stopped running session as a fresh durable provision for application restart', () => {
		const fixture = createManagedFixture();
		const provision = terminalCommands(fixture.commands)[0];
		if (provision?.type !== 'terminal-provision-requested') throw new Error('Missing provision.');
		fixture.runtime.applyTerminalPresentationEvent({
			operationId: provision.operationId,
			presentationId: provision.presentationId,
			type: 'terminal-provision-completed',
		});
		const restartPresentationIds = fixture.runtime.getRestartableTerminalPresentationIds();
		const sequenceBeforeRestart = fixture.runtime.getSharedDocument(
			workbenchSharedDocumentRevision('4'),
		).terminalContexts[0]?.panelWorkspace.nextTerminalSequence;

		fixture.runtime.requestAllTerminalSessionsStop();
		const stop = terminalCommands(fixture.commands).at(-1);
		if (stop?.type !== 'terminal-close-requested') throw new Error('Missing stop command.');
		fixture.runtime.applyTerminalPresentationEvent({
			operationId: stop.operationId,
			presentationId: stop.presentationId,
			type: 'terminal-close-completed',
		});

		expect(fixture.runtime.startStoppedTerminalSession(stop.presentationId)).toEqual({
			kind: 'rejected',
			reason: 'terminal-creation-sealed',
		});
		expect(fixture.runtime.stageStoppedTerminalSessionsForRestart(restartPresentationIds)).toEqual({
			kind: 'staged',
			presentationCount: 1,
		});
		expect(terminalCommands(fixture.commands).at(-1)).toBe(stop);
		expect(presentationIds(fixture.runtime)).not.toEqual([stop.presentationId]);
		expect(
			fixture.runtime.getSharedDocument(workbenchSharedDocumentRevision('5')).terminalContexts[0]
				?.panelWorkspace.tabs[0]?.root,
		).toMatchObject({
			kind: 'panel',
			primaryPresentation: { intent: { kind: 'provision' } },
		});
		expect(
			fixture.runtime.getSharedDocument(workbenchSharedDocumentRevision('5')).terminalContexts[0]
				?.panelWorkspace.nextTerminalSequence,
		).toBe(sequenceBeforeRestart);
	});

	it('keeps failed stop authority, retries with a fresh operation, and seals creation', () => {
		const fixture = createManagedFixture();
		const provision = terminalCommands(fixture.commands)[0];
		if (provision?.type !== 'terminal-provision-requested') throw new Error('Missing provision.');
		fixture.runtime.applyTerminalPresentationEvent({
			operationId: provision.operationId,
			presentationId: provision.presentationId,
			type: 'terminal-provision-completed',
		});
		fixture.runtime.requestAllTerminalSessionsStop();
		const firstStop = terminalCommands(fixture.commands).at(-1);
		if (firstStop?.type !== 'terminal-close-requested') throw new Error('Missing stop command.');
		fixture.runtime.applyTerminalPresentationEvent({
			failure: { code: 'terminal-transport-unavailable', retryable: true },
			operationId: firstStop.operationId,
			presentationId: firstStop.presentationId,
			type: 'terminal-close-failed',
		});

		expect(fixture.runtime.getTerminalSessionStopSnapshot().failures).toHaveLength(1);
		expect(fixture.runtime.retryFailedTerminalSessionStops()).toMatchObject({
			commandCount: 1,
			kind: 'started',
		});
		const retry = terminalCommands(fixture.commands).at(-1);
		if (retry?.type !== 'terminal-close-requested') throw new Error('Missing retry command.');
		expect(retry.operationId).not.toBe(firstStop.operationId);
		expect(
			fixture.runtime.applyTerminalPresentationEvent({
				operationId: firstStop.operationId,
				presentationId: firstStop.presentationId,
				type: 'terminal-close-completed',
			}),
		).toEqual({ kind: 'rejected', reason: 'stale-operation' });
		expect(
			fixture.runtime.createTerminal({
				groupId: null,
				source: 'button',
				title: 'Too late',
			}),
		).toEqual({ kind: 'rejected', reason: 'terminal-creation-sealed' });
	});
});
