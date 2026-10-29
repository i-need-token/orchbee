import { describe, expect, it, vi } from 'vitest';
import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	terminalPresentationId,
	type WorkbenchSharedDocument,
} from '@orchbee/workbench-client-protocol';

import type { DesktopTerminalProjectionLease } from '../DesktopTerminalProjectionService';
import { DesktopTerminalProjectionRouter } from '../DesktopTerminalProjectionRouter';

const presentationId = terminalPresentationId('presentation-1');
const geometry = { cellPixelHeight: 18, cellPixelWidth: 9, columns: 80, rows: 24 };
type RouterOptions = ConstructorParameters<typeof DesktopTerminalProjectionRouter>[0];

describe('DesktopTerminalProjectionRouter', () => {
	it('routes a standalone Presentation to the local runtime', async () => {
		const projection = projectionLease();
		const localOpen = vi.fn<RouterOptions['localRuntime']['open']>(async () => projection.lease);
		const router = new DesktopTerminalProjectionRouter({
			getDocument: () => terminalDocument(),
			localRuntime: localRuntime(localOpen),
		});
		await expect(router.open(presentationId, geometry)).resolves.toBe(projection.lease);
		expect(localOpen).toHaveBeenCalledWith(presentationId, geometry, {
			brokerEffectTarget: 'desktop',
		});
		expect(projection.dispose).not.toHaveBeenCalled();
	});

	it('rejects an Environment placement before opening a local runtime', async () => {
		const localOpen = vi.fn<RouterOptions['localRuntime']['open']>();
		const router = new DesktopTerminalProjectionRouter({
			getDocument: () => terminalDocument('environment:00000000000000000000000000000001'),
			localRuntime: localRuntime(localOpen),
		});
		await expect(router.open(presentationId, geometry)).rejects.toMatchObject({
			cause: { code: 'terminal-presentation-not-ready', retryable: true },
		});
		expect(localOpen).not.toHaveBeenCalled();
	});

	it('fails closed without a canonical target before probing the runtime', async () => {
		const localOpen = vi.fn<RouterOptions['localRuntime']['open']>();
		const router = new DesktopTerminalProjectionRouter({
			getDocument: () => null,
			localRuntime: localRuntime(localOpen),
		});
		await expect(router.open(presentationId, geometry)).rejects.toMatchObject({
			cause: { code: 'terminal-presentation-not-ready', retryable: true },
		});
		expect(localOpen).not.toHaveBeenCalled();
	});

	it('disposes an admitted lease when canonical authority disappears before publication', async () => {
		const projection = projectionLease();
		let document: WorkbenchSharedDocument | null = terminalDocument();
		const localOpen = vi.fn<RouterOptions['localRuntime']['open']>(async () => {
			document = null;
			return projection.lease;
		});
		const router = new DesktopTerminalProjectionRouter({
			getDocument: () => document,
			localRuntime: localRuntime(localOpen),
		});
		await expect(router.open(presentationId, geometry)).rejects.toMatchObject({
			cause: { code: 'terminal-projection-retired', retryable: false },
		});
		expect(projection.dispose).toHaveBeenCalledOnce();
	});

	it('fences a canonical target change in the final synchronous publication check', async () => {
		const projection = projectionLease();
		let document: WorkbenchSharedDocument | null = terminalDocument();
		const router = new DesktopTerminalProjectionRouter({
			getDocument: () => document,
			localRuntime: localRuntime(
				vi.fn<RouterOptions['localRuntime']['open']>(async () => projection.lease),
			),
		});
		const lease = await router.open(presentationId, geometry);
		document = null;
		expect(router.isPublicationTargetCurrent(presentationId, lease)).toBe(false);
		expect(router.isPublicationTargetCurrent(presentationId, lease)).toBe(false);
	});

	it('surfaces stale-lease cleanup failure instead of publishing an unowned attachment', async () => {
		const projection = projectionLease(new Error('detach failed'));
		let document: WorkbenchSharedDocument | null = terminalDocument();
		const localOpen = vi.fn<RouterOptions['localRuntime']['open']>(async () => {
			document = null;
			return projection.lease;
		});
		const router = new DesktopTerminalProjectionRouter({
			getDocument: () => document,
			localRuntime: localRuntime(localOpen),
		});
		await expect(router.open(presentationId, geometry)).rejects.toMatchObject({
			cause: { code: 'terminal-projection-cleanup-failed', retryable: true },
		});
		expect(projection.dispose).toHaveBeenCalledOnce();
	});

	it('routes capture-only reads without geometry or interactive projection capabilities', async () => {
		const projection = projectionLease();
		const localOpen = vi.fn<RouterOptions['localRuntime']['open']>();
		const localRead = vi
			.fn<RouterOptions['localRuntime']['openReadOnly']>()
			.mockResolvedValue(projection.lease);
		const router = new DesktopTerminalProjectionRouter({
			getDocument: () => terminalDocument(),
			localRuntime: { open: localOpen, openReadOnly: localRead },
		});
		await expect(router.openReadOnly(presentationId)).resolves.toBe(projection.lease);
		expect(localRead).toHaveBeenCalledWith(presentationId);
		expect(localOpen).not.toHaveBeenCalled();
	});
});

function localRuntime(open: RouterOptions['localRuntime']['open']): RouterOptions['localRuntime'] {
	return { open, openReadOnly: vi.fn<RouterOptions['localRuntime']['openReadOnly']>() };
}

function projectionLease(disposalFailure?: Error): {
	readonly dispose: ReturnType<typeof vi.fn<() => Promise<void>>>;
	readonly lease: DesktopTerminalProjectionLease;
} {
	const dispose = vi.fn<() => Promise<void>>(async () => {
		if (disposalFailure !== undefined) throw disposalFailure;
	});
	return {
		dispose,
		lease: { dispose } as unknown as DesktopTerminalProjectionLease,
	};
}

function terminalDocument(environmentId?: string): WorkbenchSharedDocument {
	const remote = environmentId !== undefined;
	return createWorkbenchSharedDocument({
		projectWorkspace: {
			nextProjectSequence: remote ? 2 : 1,
			nextTerminalContextSequence: remote ? 3 : 2,
			nextWorkspaceSequence: remote ? 2 : 1,
			projects: remote ? [{ id: 'project-1', name: 'workspace' }] : [],
			resourceScopes: remote
				? [
						{
							directories: [
								{
									available: false,
									git: null,
									id: 'directory-1',
									kind: 'folder',
									name: 'workspace',
									path: '/workspace',
									placement: { environmentId, kind: 'environment' },
									source: 'selected',
								},
							],
							id: 'resource-scope-1',
							launchConfigurations: [],
							name: 'workspace',
							primaryDirectoryId: 'directory-1',
						},
					]
				: [],
			workspaces: remote
				? [
						{
							projectId: 'project-1',
							resourceScopeId: 'resource-scope-1',
							id: 'workspace-1',
							launchConfigurationId: null,
							name: 'workspace',
							primaryDirectoryId: 'directory-1',
							terminalContextId: 'terminal-context-1',
						},
					]
				: [],
			nextResourceScopeSequence: 2,
			standaloneTerminalContextId: remote ? 'terminal-context-2' : 'terminal-context-1',
		},
		revision: workbenchSharedDocumentRevision('0'),
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
									intent: { kind: 'retain' },
									launchCwd: '/workspace',
									presentationId,
									title: 'Terminal',
								},
							},
							tabId: 'tab-1',
						},
					],
				},
				tabCollection: {
					groups: [],
					tabs: [
						{
							customTitle: null,
							group: null,
							id: 'tab-1',
							pinned: false,
							title: 'Terminal',
						},
					],
				},
				terminalContextId: 'terminal-context-1',
			},
			...(remote
				? [
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
							terminalContextId: 'terminal-context-2',
						} satisfies WorkbenchSharedDocument['terminalContexts'][number],
					]
				: []),
		],
	});
}
