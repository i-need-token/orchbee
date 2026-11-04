import { createOrchbeeIntl } from '@orchbee/i18n';
import { parseWorkbenchResponse, type WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';

import {
	createDefaultWorkbenchClientView,
	createWorkbenchClientView,
	WorkbenchProjectWorkspaceRuntime,
	type WorkbenchClientView,
	type WorkbenchPublication,
} from '../workbench';
import { projectServerSnapshot } from '../workbench-client/server-workbench';

export const FIXTURE_INTL = createOrchbeeIntl('en-US');
export const FIXTURE_WORKER_ID = fixtureId(0x2);

/** A snapshot piece as the Server sends it, before the Client parses it. */
export type FixtureRecord = Readonly<Record<string, unknown>>;

export interface FixtureBinding {
	readonly terminalId: string;
	readonly workerId: string;
}

export interface FixturePresentation extends FixtureRecord {
	readonly binding: FixtureBinding | null;
	readonly customLabel: string | null;
	readonly label: string;
	readonly presentationId: string;
}

export interface FixturePanel extends FixtureRecord {
	readonly childPresentations: readonly FixtureRecord[];
	readonly kind: 'panel';
	readonly panelId: string;
	readonly primaryPresentation: FixtureRecord;
}

export interface FixtureTab extends FixtureRecord {
	readonly label: string;
	readonly root: FixtureRecord;
	readonly tabId: string;
}

export interface FixtureWorkspace extends FixtureRecord {
	readonly label: string;
	readonly projectId: string | null;
	readonly terminalContextId: string;
	readonly workspaceId: string;
}

export interface FixtureProject extends FixtureRecord {
	readonly label: string;
	readonly projectId: string;
}

/** A protocol identifier: 32 lowercase hexadecimal digits. */
export function fixtureId(value: number): string {
	return value.toString(16).padStart(32, '0');
}

/** The Worker terminal that Presentation `sequence` shows unless the fixture overrides it. */
export function fixtureBinding(sequence: number): FixtureBinding {
	return { terminalId: fixtureId(0x900 + sequence), workerId: FIXTURE_WORKER_ID };
}

export function fixturePresentationId(sequence: number): string {
	return fixtureId(0x800 + sequence);
}

export function fixturePanelId(sequence: number): string {
	return fixtureId(0x700 + sequence);
}

export function fixtureTabId(sequence: number): string {
	return fixtureId(0x600 + sequence);
}

export function fixtureContextId(sequence: number): string {
	return fixtureId(0x500 + sequence);
}

export function fixtureWorkspaceId(sequence: number): string {
	return fixtureId(0x400 + sequence);
}

export function fixtureDirectoryId(sequence: number): string {
	return fixtureId(0x300 + sequence);
}

export function fixtureProjectId(sequence: number): string {
	return fixtureId(0x200 + sequence);
}

export function snapshotPresentation(
	sequence: number,
	overrides: FixtureRecord = {},
): FixturePresentation {
	return {
		binding: fixtureBinding(sequence),
		customLabel: null,
		label: `Terminal ${sequence}`,
		presentationId: fixturePresentationId(sequence),
		...overrides,
	};
}

export function snapshotPanel(
	sequence: number,
	primary: FixtureRecord = snapshotPresentation(sequence),
	children: readonly FixtureRecord[] = [],
): FixturePanel {
	return {
		childPresentations: children,
		kind: 'panel',
		panelId: fixturePanelId(sequence),
		primaryPresentation: primary,
	};
}

export function snapshotSplit(
	sequence: number,
	orientation: 'horizontal' | 'vertical',
	first: FixtureRecord,
	second: FixtureRecord,
	ratio?: number,
): FixtureRecord {
	return {
		children: [
			{ node: first, slotId: fixtureId(0xa00 + sequence * 2) },
			{ node: second, slotId: fixtureId(0xa01 + sequence * 2) },
		],
		kind: 'split',
		orientation,
		splitId: fixtureId(0xb00 + sequence),
		...(ratio === undefined ? {} : { ratio }),
	};
}

export function snapshotTab(
	sequence: number,
	root: FixtureRecord = snapshotPanel(sequence),
	overrides: FixtureRecord = {},
): FixtureTab {
	return {
		groupId: null,
		label: `Tab ${sequence}`,
		pinned: false,
		root,
		tabId: fixtureTabId(sequence),
		...overrides,
	};
}

export function snapshotContext(
	sequence: number,
	tabs: readonly FixtureRecord[],
	groups: readonly FixtureRecord[] = [],
): FixtureRecord {
	return { groups, tabs, terminalContextId: fixtureContextId(sequence) };
}

export function snapshotWorkspace(
	sequence: number,
	overrides: FixtureRecord = {},
): FixtureWorkspace {
	return {
		label: `Workspace ${sequence}`,
		primaryDirectoryId: fixtureDirectoryId(sequence),
		projectId: null,
		terminalContextId: fixtureContextId(sequence),
		workspaceId: fixtureWorkspaceId(sequence),
		...overrides,
	};
}

export function snapshotDirectory(sequence: number): FixtureRecord {
	return {
		directoryId: fixtureDirectoryId(sequence),
		path: `C:\\work\\directory-${sequence}`,
		workerId: FIXTURE_WORKER_ID,
	};
}

export function snapshotProject(sequence: number, label = `Project ${sequence}`): FixtureProject {
	return { label, projectId: fixtureProjectId(sequence) };
}

/** Parses the pieces as a Client receives them, so a fixture can only describe a valid snapshot. */
export function workbenchSnapshot(
	revision: number,
	navigation: { readonly workspaces: readonly FixtureRecord[] } & FixtureRecord,
	terminalContexts: readonly FixtureRecord[],
): WorkbenchSnapshot {
	const response = parseWorkbenchResponse({
		kind: 'workbench.snapshot.result',
		outcome: {
			kind: 'available',
			snapshot: {
				kind: 'workbench.snapshot',
				navigation: { directories: [], projects: [], ...navigation },
				protocolVersion: 1,
				revision: `${revision}`,
				sessionId: fixtureId(0x1),
				terminalContexts,
			},
		},
		protocolVersion: 1,
		requestId: fixtureId(0xf00),
	});
	if (response.kind !== 'workbench.snapshot.result' || response.outcome.kind !== 'available') {
		throw new Error('The fixture is not an available snapshot.');
	}
	return response.outcome.snapshot;
}

/** Workspace 1 in Directory 1, holding `tabs`. */
export function oneWorkspaceSnapshot(
	revision: number,
	tabs: readonly FixtureRecord[],
): WorkbenchSnapshot {
	return workbenchSnapshot(
		revision,
		{ directories: [snapshotDirectory(1)], workspaces: [snapshotWorkspace(1)] },
		[snapshotContext(1, tabs)],
	);
}

interface SnapshotRuntimeOptions {
	/** The view a reopened window recovers; overrides `expandedProjectIds`. */
	readonly clientView?: WorkbenchClientView | undefined;
	/** Projects a reopened window recovers as expanded. */
	readonly expandedProjectIds?: readonly string[] | undefined;
}

/** The runtime a window builds when it connects and the Server publishes `snapshot`. */
export function createSnapshotRuntime(
	snapshot: WorkbenchSnapshot,
	options: SnapshotRuntimeOptions = {},
): WorkbenchProjectWorkspaceRuntime {
	const initialPublication = snapshotPublication(snapshot);
	const initialClientView =
		options.clientView ??
		(options.expandedProjectIds === undefined
			? undefined
			: createWorkbenchClientView({
					...createDefaultWorkbenchClientView(initialPublication),
					expandedProjectIds: [...options.expandedProjectIds],
				}));
	return new WorkbenchProjectWorkspaceRuntime({ initialClientView, initialPublication });
}

/**
 * Adopts what the Server publishes next, as the window does, leaving out `withheldTabIds` as a
 * window leaves out a Tab whose close it follows.
 */
export function publishSnapshot(
	runtime: WorkbenchProjectWorkspaceRuntime,
	snapshot: WorkbenchSnapshot,
	withheldTabIds: readonly string[] = [],
): void {
	runtime.applyPublishedWorkbench(snapshotPublication(snapshot, withheldTabIds));
}

/** What a window projects from `snapshot` for its stores to hold, without `withheldTabIds`. */
export function snapshotPublication(
	snapshot: WorkbenchSnapshot,
	withheldTabIds: readonly string[] = [],
): WorkbenchPublication {
	return projectServerSnapshot(snapshot, FIXTURE_INTL, undefined, (_context, tabId) =>
		withheldTabIds.includes(tabId),
	);
}

/** The open Workspace's context, which a test that opened one takes as present. */
export function activeTerminalContext(
	runtime: WorkbenchProjectWorkspaceRuntime,
): NonNullable<ReturnType<WorkbenchProjectWorkspaceRuntime['getActiveTerminalContext']>> {
	const context = runtime.getActiveTerminalContext();
	if (context === null) throw new Error('No Workspace is open.');
	return context;
}
