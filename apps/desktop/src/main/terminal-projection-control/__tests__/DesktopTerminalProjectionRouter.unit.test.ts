import {
	createEmptyWorkbenchSharedTerminalContext,
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
} from '@orchbee/workbench/document';
import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it, vi } from 'vitest';

import type { DesktopTerminalProjectionLease } from '../DesktopTerminalProjectionService';
import { DesktopTerminalProjectionRouter } from '../DesktopTerminalProjectionRouter';

const presentationId = terminalPresentationId('presentation-1');
const geometry = { cellPixelHeight: 18, cellPixelWidth: 9, columns: 80, rows: 24 };
type RouterOptions = ConstructorParameters<typeof DesktopTerminalProjectionRouter>[0];

describe('DesktopTerminalProjectionRouter', () => {
	it('routes a standalone Presentation only to the local runtime', async () => {
		const projection = projectionLease();
		const localOpen = vi.fn<RouterOptions['localRuntime']['open']>(async () => projection.lease);
		const remoteOpen = vi.fn<RouterOptions['remoteRuntimes']['open']>();
		const router = new DesktopTerminalProjectionRouter({
			getDocument: () => terminalDocument(),
			localRuntime: { open: localOpen },
			remoteRuntimes: { open: remoteOpen },
		});

		await expect(router.open(presentationId, geometry)).resolves.toBe(projection.lease);
		expect(localOpen).toHaveBeenCalledWith(presentationId, geometry, {
			brokerEffectTarget: 'desktop',
		});
		expect(remoteOpen).not.toHaveBeenCalled();
		expect(projection.dispose).not.toHaveBeenCalled();
	});

	it('routes a remote Presentation only to its exact prepared Environment runtime', async () => {
		const projection = projectionLease();
		const localOpen = vi.fn<RouterOptions['localRuntime']['open']>();
		const remoteOpen = vi.fn<RouterOptions['remoteRuntimes']['open']>(async () => projection.lease);
		const document = terminalDocument('environment:00000000000000000000000000000001');
		const router = new DesktopTerminalProjectionRouter({
			getDocument: () => document,
			localRuntime: { open: localOpen },
			remoteRuntimes: { open: remoteOpen },
		});

		await expect(
			router.open(presentationId, geometry, { brokerEffectTarget: 'client' }),
		).resolves.toBe(projection.lease);
		expect(remoteOpen).toHaveBeenCalledWith(
			'environment:00000000000000000000000000000001',
			presentationId,
			geometry,
			{ brokerEffectTarget: 'client' },
		);
		expect(localOpen).not.toHaveBeenCalled();
	});

	it('fails closed without a canonical target and never probes either runtime', async () => {
		const localOpen = vi.fn<RouterOptions['localRuntime']['open']>();
		const remoteOpen = vi.fn<RouterOptions['remoteRuntimes']['open']>();
		const router = new DesktopTerminalProjectionRouter({
			getDocument: () => null,
			localRuntime: { open: localOpen },
			remoteRuntimes: { open: remoteOpen },
		});

		await expect(router.open(presentationId, geometry)).rejects.toMatchObject({
			cause: { code: 'terminal-presentation-not-ready', retryable: true },
		});
		expect(localOpen).not.toHaveBeenCalled();
		expect(remoteOpen).not.toHaveBeenCalled();
	});

	it('does not fall back locally when the exact remote runtime is unavailable', async () => {
		const localOpen = vi.fn<RouterOptions['localRuntime']['open']>();
		const unavailable = new Error('unavailable', {
			cause: { code: 'terminal-projection-unavailable', retryable: true },
		});
		const remoteOpen = vi
			.fn<RouterOptions['remoteRuntimes']['open']>()
			.mockRejectedValue(unavailable);
		const router = new DesktopTerminalProjectionRouter({
			getDocument: () => terminalDocument('environment:00000000000000000000000000000001'),
			localRuntime: { open: localOpen },
			remoteRuntimes: { open: remoteOpen },
		});

		await expect(router.open(presentationId, geometry)).rejects.toBe(unavailable);
		expect(localOpen).not.toHaveBeenCalled();
	});

	it('disposes an admitted lease when canonical authority changes before publication', async () => {
		const projection = projectionLease();
		let document = terminalDocument('environment:00000000000000000000000000000001');
		const localOpen = vi.fn<RouterOptions['localRuntime']['open']>();
		const remoteOpen = vi.fn<RouterOptions['remoteRuntimes']['open']>(async () => {
			document = terminalDocument();
			return projection.lease;
		});
		const router = new DesktopTerminalProjectionRouter({
			getDocument: () => document,
			localRuntime: { open: localOpen },
			remoteRuntimes: { open: remoteOpen },
		});

		await expect(router.open(presentationId, geometry)).rejects.toMatchObject({
			cause: { code: 'terminal-projection-retired', retryable: false },
		});
		expect(projection.dispose).toHaveBeenCalledOnce();
		expect(localOpen).not.toHaveBeenCalled();
	});

	it('fences a canonical target change in the final synchronous publication check', async () => {
		const projection = projectionLease();
		let document = terminalDocument('environment:00000000000000000000000000000001');
		const router = new DesktopTerminalProjectionRouter({
			getDocument: () => document,
			localRuntime: { open: vi.fn<RouterOptions['localRuntime']['open']>() },
			remoteRuntimes: {
				open: vi.fn<RouterOptions['remoteRuntimes']['open']>(async () => projection.lease),
			},
		});
		const lease = await router.open(presentationId, geometry);

		document = terminalDocument();
		expect(router.isPublicationTargetCurrent(presentationId, lease)).toBe(false);
		expect(router.isPublicationTargetCurrent(presentationId, lease)).toBe(false);
	});

	it('surfaces stale-lease cleanup failure instead of publishing an unowned attachment', async () => {
		const projection = projectionLease(new Error('detach failed'));
		let document = terminalDocument('environment:00000000000000000000000000000001');
		const remoteOpen = vi.fn<RouterOptions['remoteRuntimes']['open']>(async () => {
			document = terminalDocument();
			return projection.lease;
		});
		const router = new DesktopTerminalProjectionRouter({
			getDocument: () => document,
			localRuntime: { open: vi.fn<RouterOptions['localRuntime']['open']>() },
			remoteRuntimes: { open: remoteOpen },
		});

		await expect(router.open(presentationId, geometry)).rejects.toMatchObject({
			cause: { code: 'terminal-projection-cleanup-failed', retryable: true },
		});
		expect(projection.dispose).toHaveBeenCalledOnce();
	});
});

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
			projects: remote
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
							id: 'project-1',
							launchConfigurations: [],
							name: 'workspace',
							primaryDirectoryId: 'directory-1',
							workspaces: [
								{
									id: 'workspace-1',
									launchConfigurationId: null,
									name: 'workspace',
									primaryDirectoryId: 'directory-1',
									terminalContextId: 'terminal-context-1',
								},
							],
						},
					]
				: [],
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
			...(remote ? [createEmptyWorkbenchSharedTerminalContext('terminal-context-2')] : []),
		],
	});
}
