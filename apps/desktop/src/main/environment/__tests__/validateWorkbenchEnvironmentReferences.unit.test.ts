import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
} from '@orchbee/workbench/document';
import { describe, expect, it } from 'vitest';

import { EnvironmentIdSchema } from '../environment-registry-contract';
import { validateWorkbenchEnvironmentReferences } from '../validateWorkbenchEnvironmentReferences';

const registeredEnvironmentId = EnvironmentIdSchema.parse(
	'environment:11111111111111111111111111111111',
);

function documentWithPlacement(
	placement:
		| { readonly kind: 'local' }
		| { readonly environmentId: string; readonly kind: 'environment' },
): WorkbenchSharedDocument {
	return createWorkbenchSharedDocument({
		projectWorkspace: {
			nextProjectSequence: 2,
			nextTerminalContextSequence: 2,
			nextWorkspaceSequence: 1,
			projects: [
				{
					directories: [
						{
							available: false,
							git: null,
							id: 'directory-1',
							kind: 'folder',
							name: 'orchbee',
							path: '/srv/orchbee',
							placement,
							source: 'selected',
						},
					],
					id: 'project-1',
					launchConfigurations: [],
					name: 'orchbee',
					primaryDirectoryId: 'directory-1',
					workspaces: [],
				},
			],
			standaloneTerminalContextId: 'terminal-context-1',
		},
		revision: workbenchSharedDocumentRevision('0'),
		terminalContexts: [
			{
				nextTabSequence: 1,
				panelWorkspace: {
					nextPanelSequence: 1,
					nextPanelSlotSequence: 1,
					nextSplitSequence: 1,
					nextTerminalSequence: 1,
					pendingRemovals: [],
					tabs: [],
				},
				tabCollection: { groups: [], tabs: [] },
				terminalContextId: 'terminal-context-1',
			},
		],
	});
}

describe('validateWorkbenchEnvironmentReferences', () => {
	it('accepts local placement without consulting the Environment registry', () => {
		expect(
			validateWorkbenchEnvironmentReferences(documentWithPlacement({ kind: 'local' }), new Set()),
		).toBe(true);
	});

	it('accepts only a registered, schema-valid Environment reference', () => {
		const registered = new Set([registeredEnvironmentId]);
		expect(
			validateWorkbenchEnvironmentReferences(
				documentWithPlacement({
					environmentId: registeredEnvironmentId,
					kind: 'environment',
				}),
				registered,
			),
		).toBe(true);
		expect(
			validateWorkbenchEnvironmentReferences(
				documentWithPlacement({
					environmentId: 'environment:22222222222222222222222222222222',
					kind: 'environment',
				}),
				registered,
			),
		).toBe(false);
		expect(
			validateWorkbenchEnvironmentReferences(
				documentWithPlacement({ environmentId: 'not-an-environment-id', kind: 'environment' }),
				registered,
			),
		).toBe(false);
	});
});
