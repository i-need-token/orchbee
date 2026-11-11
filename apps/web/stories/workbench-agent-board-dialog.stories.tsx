import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

import {
	createSnapshotRuntime,
	fixturePresentationId,
	snapshotContext,
	snapshotDirectory,
	snapshotPanel,
	snapshotPresentation,
	snapshotTab,
	snapshotWorkspace,
	workbenchSnapshot,
} from '../src/__tests__/server-workbench-snapshot';
// The board is private to this package, so the Story reads it from the same source as the dialog.
import { WorkbenchAgentBoardDialog } from '../src/workbench/components/workbench-agent-board-dialog/WorkbenchAgentBoardDialog';
import type {
	WorkbenchAgentBoard,
	WorkbenchAgentBoardRow,
	WorkbenchAgentBoardSnapshot,
} from '../src/workbench/types/workbench-agent-board';

/** One Workspace whose Tabs hold the terminals the sessions below are attached to, in this order. */
const createBoardRuntime = (titles: readonly string[]) =>
	createSnapshotRuntime(
		workbenchSnapshot(
			1,
			{
				directories: [{ ...snapshotDirectory(1), path: '/workspace/orchbee' }],
				workspaces: [snapshotWorkspace(1, { label: 'orchbee' })],
			},
			[
				snapshotContext(
					1,
					titles.map((title, index) =>
						snapshotTab(
							index + 1,
							snapshotPanel(index + 1, snapshotPresentation(index + 1, { customLabel: title })),
							{ label: title },
						),
					),
				),
			],
		),
	);

const sessions: readonly WorkbenchAgentBoardRow[] = [
	{
		ended: false,
		freshness: 'fresh',
		origin: 'session',
		presentationId: fixturePresentationId(2),
		providerId: 'claude',
		sessionId: 'session-review',
		status: { count: 2, kind: 'needs-attention' },
	},
	{
		ended: false,
		freshness: 'fresh',
		origin: 'session',
		presentationId: fixturePresentationId(1),
		providerId: 'codex',
		sessionId: 'session-build',
		status: { kind: 'working' },
	},
	{
		ended: false,
		freshness: 'stale',
		origin: 'session',
		presentationId: fixturePresentationId(3),
		providerId: 'gemini',
		sessionId: 'session-docs',
		status: { kind: 'idle' },
	},
	{
		ended: false,
		freshness: 'disconnected',
		origin: 'session',
		presentationId: null,
		providerId: 'my-agent',
		sessionId: 'session-elsewhere',
		status: { kind: 'unknown' },
	},
	{
		ended: true,
		freshness: 'fresh',
		origin: 'session',
		presentationId: null,
		providerId: 'claude',
		sessionId: 'session-done',
		status: { kind: 'succeeded' },
	},
	{
		ended: true,
		freshness: 'fresh',
		origin: 'session',
		presentationId: null,
		providerId: 'opencode',
		sessionId: 'session-stopped',
		status: { kind: 'interrupted' },
	},
];

/** A board that holds one snapshot: it never reads, and every jump fails without a notice. */
const fixedBoard = (snapshot: WorkbenchAgentBoardSnapshot): WorkbenchAgentBoard => ({
	close: () => undefined,
	getSnapshot: () => snapshot,
	jump: () => Promise.resolve('failed'),
	open: () => undefined,
	refresh: () => undefined,
	subscribe: () => () => undefined,
});

const englishRuntime = createBoardRuntime(['orchbee review', 'build', 'docs']);

const meta = {
	title: 'Agent Board',
	component: WorkbenchAgentBoardDialog,
	args: {
		board: fixedBoard({
			jumping: false,
			notice: null,
			read: { complete: true, kind: 'listed' },
			rows: sessions,
		}),
		finalFocus: () => null,
		onClose: () => undefined,
		onShown: () => undefined,
		onStartAgent: () => undefined,
		projectWorkspaceRuntime: englishRuntime,
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<WorkbenchAgentBoardDialog {...arguments_} />
		</OrchbeeI18nProvider>
	),
} satisfies Meta<typeof WorkbenchAgentBoardDialog>;

export default meta;

type Story = StoryObj<typeof meta>;

/** Waiting sessions first, a stale and a disconnected one, and ended ones dimmed, each with its Tab. */
export const Sessions: Story = {};

export const SessionsChinese: Story = {
	args: { projectWorkspaceRuntime: createBoardRuntime(['orchbee 评审', '构建', '文档']) },
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='zh-CN'>
			<WorkbenchAgentBoardDialog {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

/** A cut-short read keeps the rows of the last whole one and says so. */
export const PartialRead: Story = {
	args: {
		board: fixedBoard({
			jumping: false,
			notice: null,
			read: { complete: false, kind: 'listed' },
			rows: sessions,
		}),
	},
};

/** Why the last jump stopped, until the next jump or read. */
export const JumpFailed: Story = {
	args: {
		board: fixedBoard({
			jumping: false,
			notice: 'gone',
			read: { complete: true, kind: 'listed' },
			rows: sessions,
		}),
	},
};

export const Empty: Story = {
	args: {
		board: fixedBoard({
			jumping: false,
			notice: null,
			read: { complete: true, kind: 'listed' },
			rows: [],
		}),
	},
};

export const ReadFailed: Story = {
	args: {
		board: fixedBoard({
			jumping: false,
			notice: null,
			read: { kind: 'failed', reason: 'unavailable' },
			rows: [],
		}),
	},
};
