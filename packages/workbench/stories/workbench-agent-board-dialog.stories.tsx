import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

// The board is private to this package, so its runtime comes from the same source as the dialog.
import { WorkbenchAgentBoardDialog } from '../src/workbench/components/workbench-agent-board-dialog/WorkbenchAgentBoardDialog';
import { createWorkbenchPreviewTerminalPresentationComposition } from '../src/workbench/model/createWorkbenchPreviewTerminalPresentationComposition';
import { WorkbenchProjectWorkspaceRuntime } from '../src/workbench/model/WorkbenchProjectWorkspaceRuntime';
import type {
	WorkbenchAgentBoard,
	WorkbenchAgentBoardRow,
	WorkbenchAgentBoardSnapshot,
} from '../src/workbench/types/workbench-agent-board';

/** Three Tabs whose terminals the sessions below are attached to: board:1, board:2 and board:3. */
const createBoardRuntime = (titles: readonly [string, ...string[]]) => {
	let sequence = 1;
	const runtime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/workspace/orchbee',
		standaloneTerminalTitle: titles[0],
		terminalPresentation: createWorkbenchPreviewTerminalPresentationComposition(
			terminalPresentationId('storybook:board:1'),
			() => terminalPresentationId(`storybook:board:${(sequence += 1)}`),
		),
	});
	for (const title of titles.slice(1)) {
		runtime.createTerminal({ groupId: null, source: 'button', title });
	}
	return runtime;
};

const sessions: readonly WorkbenchAgentBoardRow[] = [
	{
		ended: false,
		freshness: 'fresh',
		presentationId: 'storybook:board:2',
		providerId: 'claude',
		sessionId: 'session-review',
		status: { count: 2, kind: 'needs-attention' },
	},
	{
		ended: false,
		freshness: 'fresh',
		presentationId: 'storybook:board:1',
		providerId: 'codex',
		sessionId: 'session-build',
		status: { kind: 'working' },
	},
	{
		ended: false,
		freshness: 'stale',
		presentationId: 'storybook:board:3',
		providerId: 'gemini',
		sessionId: 'session-docs',
		status: { kind: 'idle' },
	},
	{
		ended: false,
		freshness: 'disconnected',
		presentationId: null,
		providerId: 'my-agent',
		sessionId: 'session-elsewhere',
		status: { kind: 'unknown' },
	},
	{
		ended: true,
		freshness: 'fresh',
		presentationId: null,
		providerId: 'claude',
		sessionId: 'session-done',
		status: { kind: 'succeeded' },
	},
	{
		ended: true,
		freshness: 'fresh',
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
