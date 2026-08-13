import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
	type WorkbenchSharedPresentationIntent,
} from '@orchbee/workbench/document';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
} from '@orchbee/workbench/terminal-presentation';
import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { decideDesktopClientDocumentProposal } from '../decideDesktopClientDocumentProposal';

describe('desktop client document proposal decision', () => {
	it('admits only an exact initial or exact-next proposal', () => {
		const initial = document('0', 'Terminal A');
		expect(
			decideDesktopClientDocumentProposal(
				null,
				{ clientSessionId: 'client-a', document: initial, expectedRevision: null },
				{ kind: 'accepting' },
			),
		).toEqual({ kind: 'commit' });
		expect(
			decideDesktopClientDocumentProposal(
				null,
				{
					clientSessionId: 'client-a',
					document: document('1', 'Terminal A'),
					expectedRevision: null,
				},
				{ kind: 'accepting' },
			),
		).toEqual({ kind: 'rejected', reason: 'revision-sequence' });

		const current = document('7', 'Terminal A');
		expect(
			decideDesktopClientDocumentProposal(
				current,
				{
					clientSessionId: 'client-a',
					document: document('8', 'Terminal B'),
					expectedRevision: current.revision,
				},
				{ kind: 'accepting' },
			),
		).toEqual({ kind: 'commit' });
	});

	it('converges an equivalent loser and returns canonical state to a divergent loser', () => {
		const base = document('0', 'Terminal A');
		const winner = document('1', 'Terminal B');
		const equivalent = decideDesktopClientDocumentProposal(
			winner,
			{
				clientSessionId: 'client-b',
				document: winner,
				expectedRevision: base.revision,
			},
			{ kind: 'accepting' },
		);
		const divergent = decideDesktopClientDocumentProposal(
			winner,
			{
				clientSessionId: 'client-b',
				document: document('1', 'Terminal C'),
				expectedRevision: base.revision,
			},
			{ kind: 'accepting' },
		);

		expect(equivalent).toEqual({ document: winner, kind: 'converged' });
		expect(divergent).toEqual({ document: winner, kind: 'stale' });
	});

	it('admits only the exit owner stop progression while quiescing', () => {
		const current = document('3', 'Terminal A');
		const stopping = document('4', 'Terminal A', {
			kind: 'stop',
			operationId: terminalPresentationOperationId('stop-operation'),
		});
		const phase = { exitOwner: 'client-a', kind: 'quiescing', mode: 'stop' } as const;

		expect(
			decideDesktopClientDocumentProposal(
				current,
				{
					clientSessionId: 'client-a',
					document: stopping,
					expectedRevision: current.revision,
				},
				phase,
			),
		).toEqual({ kind: 'commit' });
		expect(
			decideDesktopClientDocumentProposal(
				current,
				{
					clientSessionId: 'client-b',
					document: stopping,
					expectedRevision: current.revision,
				},
				phase,
			),
		).toEqual({ kind: 'rejected', reason: 'application-quiescing' });
		expect(
			decideDesktopClientDocumentProposal(
				current,
				{
					clientSessionId: 'client-a',
					document: document('4', 'Renamed'),
					expectedRevision: current.revision,
				},
				phase,
			),
		).toEqual({ kind: 'rejected', reason: 'exit-transition' });
		expect(
			decideDesktopClientDocumentProposal(
				current,
				{
					clientSessionId: 'client-a',
					document: stopping,
					expectedRevision: current.revision,
				},
				{ ...phase, mode: 'preserve' },
			),
		).toEqual({ kind: 'rejected', reason: 'application-quiescing' });
	});

	it('classifies generated stale proposals solely by canonical content equivalence', () => {
		fc.assert(
			fc.property(fc.bigInt({ max: 10_000n, min: 1n }), fc.boolean(), (revision, equivalent) => {
				const canonical = document(revision.toString(), 'Canonical');
				const expected = workbenchSharedDocumentRevision((revision - 1n).toString());
				const candidate = document(revision.toString(), equivalent ? 'Canonical' : 'Divergent');
				const decision = decideDesktopClientDocumentProposal(
					canonical,
					{ clientSessionId: 'client-a', document: candidate, expectedRevision: expected },
					{ kind: 'accepting' },
				);
				expect(decision.kind).toBe(equivalent ? 'converged' : 'stale');
				expect(canonical).toEqual(document(revision.toString(), 'Canonical'));
			}),
			{ numRuns: 200, seed: 0x0b_ee_02 },
		);
	});
});

function document(
	revision: string,
	title: string,
	intent: WorkbenchSharedPresentationIntent = {
		kind: 'provision',
		operationId: terminalPresentationOperationId('provision-operation'),
	},
): WorkbenchSharedDocument {
	return createWorkbenchSharedDocument({
		projectWorkspace: {
			nextProjectSequence: 1,
			nextTerminalContextSequence: 2,
			nextWorkspaceSequence: 1,
			projects: [],
			standaloneTerminalContextId: 'terminal-context-1',
		},
		revision: workbenchSharedDocumentRevision(revision),
		terminalContexts: [
			{
				nextTabSequence: 2,
				panelWorkspace: {
					nextPanelSequence: 2,
					nextPanelSlotSequence: 1,
					nextSplitSequence: 1,
					nextTerminalSequence: 2,
					pendingRemovals: [],
					tabs: [
						{
							root: {
								childPresentations: [],
								id: 'panel-1',
								kind: 'panel',
								primaryPresentation: {
									intent,
									launchCwd: '/workspace',
									presentationId: terminalPresentationId('presentation-1'),
									title,
								},
							},
							tabId: 'tab-1',
						},
					],
				},
				tabCollection: {
					groups: [],
					tabs: [{ customTitle: null, group: null, id: 'tab-1', pinned: false, title }],
				},
				terminalContextId: 'terminal-context-1',
			},
		],
	});
}
