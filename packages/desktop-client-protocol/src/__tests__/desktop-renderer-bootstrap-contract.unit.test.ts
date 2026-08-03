import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';
import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it } from 'vitest';

import {
	DESKTOP_RENDERER_BOOTSTRAP_IPC_CHANNEL,
	DESKTOP_RENDERER_BOOTSTRAP_WINDOW_MESSAGE,
	DESKTOP_RENDERER_PORT_ROLES,
	DesktopRendererBootstrapSchema,
	DesktopRendererBootstrapWindowMessageSchema,
} from '../desktop-renderer-bootstrap-contract';

describe('Desktop renderer bootstrap contract', () => {
	it('parses one atomic client bootstrap with a fixed three-capability port order', () => {
		const initialPresentationId = terminalPresentationId('presentation-initial');
		const initializationDocument = createWorkbenchSharedDocument({
			projectWorkspace: {
				nextProjectSequence: 1,
				nextTerminalContextSequence: 2,
				nextWorkspaceSequence: 1,
				projects: [],
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
		const bootstrap = DesktopRendererBootstrapSchema.parse({
			clientSession: {
				clientSessionId: 'client-1',
				defaultCwd: '/workspace',
				documentState: { kind: 'missing' },
				initialPresentationId,
				initializationDocument,
				sequence: '0',
				terminalEvents: [],
				type: 'orchbee.desktop.client-session-bootstrap',
			},
			type: DESKTOP_RENDERER_BOOTSTRAP_IPC_CHANNEL,
		});

		expect(DESKTOP_RENDERER_PORT_ROLES).toEqual([
			'client-session',
			'terminal-projection',
			'application-lifetime',
		]);
		expect(
			DesktopRendererBootstrapWindowMessageSchema.parse({
				bootstrap,
				security: { contextIsolated: true, sandboxed: true },
				type: DESKTOP_RENDERER_BOOTSTRAP_WINDOW_MESSAGE,
			}),
		).toMatchObject({ bootstrap, security: { contextIsolated: true, sandboxed: true } });
	});

	it('rejects partial bootstrap data and unknown window fields', () => {
		expect(
			DesktopRendererBootstrapSchema.safeParse({
				type: DESKTOP_RENDERER_BOOTSTRAP_IPC_CHANNEL,
			}).success,
		).toBe(false);
		expect(
			DesktopRendererBootstrapWindowMessageSchema.safeParse({
				bootstrap: {},
				security: { contextIsolated: true, sandboxed: true },
				type: DESKTOP_RENDERER_BOOTSTRAP_WINDOW_MESSAGE,
				unknown: true,
			}).success,
		).toBe(false);
	});
});
