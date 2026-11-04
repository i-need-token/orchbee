import { expect, it } from 'vitest';
import { WorkbenchRequestSchema, WorkbenchResponseSchema } from '../workbench-command-contract';
import { WorkbenchSnapshotSchema } from '../workbench-semantic-contract';
import { WORKBENCH_MAXIMUM_ENCODED_LAUNCH_COMMAND_BYTES } from '../workbench-terminal-launch-contract';
import { WORKBENCH_MAXIMUM_ENCODED_WORKSPACE_CLOSE_READ_RESULT_BYTES } from '../workbench-workspace-close-contract';
import { WORKBENCH_MAXIMUM_ENCODED_WORKSPACE_GROUP_CLOSE_RESULT_BYTES } from '../workbench-workspace-group-close-contract';

const id = (value: number) => value.toString(16).padStart(32, '0');
const encodedBytes = (value: unknown) => new TextEncoder().encode(JSON.stringify(value)).length;

function admitsNavigation(navigation: {
	readonly workspaces: readonly { readonly terminalContextId: string }[];
	readonly [field: string]: unknown;
}): boolean {
	return WorkbenchSnapshotSchema.safeParse({
		kind: 'workbench.snapshot',
		navigation,
		protocolVersion: 1,
		revision: '1',
		sessionId: id(0xffff),
		terminalContexts: navigation.workspaces
			.map(({ terminalContextId }) => terminalContextId)
			.sort()
			.map((terminalContextId) => ({ groups: [], tabs: [], terminalContextId })),
	}).success;
}

it('bounds grouped and ungrouped membership independently and preserves display order', () => {
	const workspaces = Array.from({ length: 1024 }, (_, index) => ({
		workspaceId: id(index + 1),
		terminalContextId: id(index + 2),
		label: 'Workspace',
		projectId: index < 256 ? id(1) : null,
		primaryDirectoryId: null,
	}));
	const navigation = {
		directories: [],
		projects: [{ projectId: id(1), label: 'Group' }],
		workspaces,
		workspaceOrder: workspaces.map((_workspace, index) => id(workspaces.length - index)),
	};
	expect(admitsNavigation(navigation)).toBe(true);
	expect(
		admitsNavigation({
			...navigation,
			projects: [],
			workspaces: workspaces.map((workspace) => ({ ...workspace, projectId: null })),
		}),
	).toBe(true);
	expect(
		admitsNavigation({
			...navigation,
			workspaces: workspaces.map((workspace, index) => ({
				...workspace,
				projectId: index < 257 ? id(1) : null,
			})),
		}),
	).toBe(false);
	expect(
		admitsNavigation({
			...navigation,
			workspaces: [
				...workspaces,
				{
					workspaceId: id(1025),
					terminalContextId: id(1026),
					projectId: null,
					primaryDirectoryId: null,
					label: 'Overflow',
				},
			],
			workspaceOrder: [...navigation.workspaceOrder, id(1025)],
		}),
	).toBe(false);
	expect(
		admitsNavigation({
			...navigation,
			workspaceOrder: navigation.workspaceOrder.slice(1),
		}),
	).toBe(false);
});

it('admits the worst escaped Workspace launch under the dedicated 96 KiB class', () => {
	const label = '\u0001'.repeat(512);
	const environment = Array.from({ length: 32 }, (_, index) => [`K${index}`, '']);
	const names = environment.reduce((total, entry) => total + (entry[0]?.length ?? 0), 0);
	environment[0] = ['K0', '\u0001'.repeat(1024 - names)];
	const action = {
		kind: 'workspace.launch',
		label,
		placement: { kind: 'alongside', workspaceId: id(1), projectLabel: label },
		geometry: { columns: 80, rows: 24, cellPixelWidth: 0, cellPixelHeight: 0 },
		launch: {
			exec: '/',
			args: [
				'\u0001'.repeat(Math.floor((30 * 1024 - 1) / 6)) + 'x'.repeat((30 * 1024 - 1) % 6),
				...Array.from({ length: 127 }, () => ''),
			],
		},
		options: { cwd: '/' + '\u0001'.repeat(4095), environment },
	};
	const request = {
		kind: 'workbench.command',
		protocolVersion: 1,
		requestId: id(1),
		sessionId: id(2),
		expectedRevision: '18446744073709551615',
		action,
	};
	expect(encodedBytes(request)).toBeGreaterThan(60 * 1024);
	expect(encodedBytes(request)).toBeLessThanOrEqual(WORKBENCH_MAXIMUM_ENCODED_LAUNCH_COMMAND_BYTES);
	expect(WorkbenchRequestSchema.parse(request)).toEqual(request);
});

it('derives the exact stable Workspace Close result limit without Project metadata', () => {
	const status = {
		kind: 'disposed-awaiting-publication',
		reason: 'target-cleanup-pending',
		resumable: false,
	};
	const result = {
		kind: 'workbench.workspace-close.result',
		protocolVersion: 1,
		requestId: id(1),
		outcome: {
			kind: 'available',
			closeRequestId: id(2),
			workspaceId: id(3),
			terminalContextId: id(4),
			attemptGeneration: '18446744073709551615',
			status,
			counts: {
				closing: 4_294_967_295,
				cleanupBlocked: 4_294_967_295,
				outcomeUnknown: 4_294_967_295,
				disposedAwaitingPublication: 4_294_967_295,
			},
			members: Array.from({ length: 4096 }, (_, index) => ({
				tabId: id(index + 1),
				panelId: id(index + 1),
				presentationId: id(index + 1),
				status,
			})),
		},
	};
	expect(encodedBytes(result)).toBe(WORKBENCH_MAXIMUM_ENCODED_WORKSPACE_CLOSE_READ_RESULT_BYTES);
	expect(encodedBytes(result)).toBe(1_000_007);
	expect(WorkbenchResponseSchema.safeParse(result).success).toBe(true);
});

it('admits all 256 frozen Project members at maximum label escaping', () => {
	const result = {
		kind: 'workbench.workspace-group-close.result',
		protocolVersion: 1,
		requestId: id(1),
		operationId: id(2),
		outcome: {
			kind: 'available',
			projectId: id(3),
			attempt: 64,
			status: { kind: 'closed' },
			members: Array.from({ length: 256 }, (_, index) => ({
				workspaceId: id(index + 1000),
				label: '\u0001'.repeat(512),
				close: {
					requestId: id(index + 2000),
					observation: {
						attemptGeneration: '18446744073709551615',
						status: { kind: 'closed', revision: '18446744073709551615' },
					},
				},
			})),
		},
	};
	expect(encodedBytes(result)).toBeGreaterThan(512 * 1024);
	expect(encodedBytes(result)).toBeLessThan(
		WORKBENCH_MAXIMUM_ENCODED_WORKSPACE_GROUP_CLOSE_RESULT_BYTES,
	);
	expect(WorkbenchResponseSchema.safeParse(result).success).toBe(true);
});
