import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

// Start Agent is private to this package, so the Story reads it from the same source as the Workbench.
import { WorkbenchAgentStartDialog } from '../src/workbench/components/workbench-agent-start-dialog/WorkbenchAgentStartDialog';
import type {
	WorkbenchAgentCatalogSnapshot,
	WorkbenchAgentCatalogSource,
} from '../src/workbench/types/workbench-agent-start';

type CatalogAgent = WorkbenchAgentCatalogSnapshot['agents'][number];

const AGENTS: readonly Omit<CatalogAgent, 'state'>[] = [
	{ command: 'pi', productName: 'Pi', providerId: 'pi' },
	{ command: 'claude', productName: 'Claude Code', providerId: 'claude-code' },
	{ command: 'codex', productName: 'Codex', providerId: 'codex' },
	{ command: 'gemini', productName: 'Gemini CLI', providerId: 'gemini' },
	{ command: 'agent', productName: 'Cursor', providerId: 'cursor' },
	{ command: 'opencode', productName: 'OpenCode', providerId: 'opencode' },
	{ command: 'copilot', productName: 'GitHub Copilot CLI', providerId: 'github-copilot' },
	{ command: 'qwen', productName: 'Qwen Code', providerId: 'qwen-code' },
	{ command: 'openclaw chat', productName: 'OpenClaw', providerId: 'openclaw' },
];

const FOUND = new Set(['claude-code', 'codex', 'gemini', 'opencode']);
const GENERIC = new Set(['cursor', 'openclaw']);

const catalog = (
	lookup: WorkbenchAgentCatalogSnapshot['lookup'],
	state: (agent: Omit<CatalogAgent, 'state'>) => CatalogAgent['state'],
): WorkbenchAgentCatalogSnapshot => ({
	agents: AGENTS.map((agent) => ({ ...agent, state: state(agent) })),
	lookup,
	recent: ['codex', 'claude-code', 'pi'],
	startable: true,
});

const checkedState = ({ providerId }: Omit<CatalogAgent, 'state'>): CatalogAgent['state'] => {
	if (FOUND.has(providerId)) return 'found';
	return GENERIC.has(providerId) ? 'cant-check' : 'not-found';
};

/** A catalog that holds one snapshot and never looks again. */
const fixedCatalog = (snapshot: WorkbenchAgentCatalogSnapshot): WorkbenchAgentCatalogSource => ({
	getSnapshot: () => snapshot,
	refresh: () => undefined,
	subscribe: () => () => undefined,
});

const listed = fixedCatalog(
	catalog({ checkedAt: 1_794_300_000_000, status: 'complete' }, checkedState),
);

const meta = {
	title: 'Start Agent',
	component: WorkbenchAgentStartDialog,
	args: {
		catalog: listed,
		finalFocus: () => null,
		onClose: () => undefined,
		onStart: () => undefined,
		places: ['new-tab', 'split-right', 'split-down', 'new-worktree'],
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<WorkbenchAgentStartDialog {...arguments_} />
		</OrchbeeI18nProvider>
	),
} satisfies Meta<typeof WorkbenchAgentStartDialog>;

export default meta;

type Story = StoryObj<typeof meta>;

/** The Session's first lookup runs: only the Agents already known show, and no fold yet. */
export const Checking: Story = {
	args: {
		catalog: fixedCatalog(
			catalog({ checkedAt: null, status: 'running' }, (agent) =>
				agent.providerId === 'codex' || agent.providerId === 'claude-code' ? 'found' : 'checking',
			),
		),
	},
};

/** Recent, the rest of Installed, and one row that unfolds the Agents not found. */
export const Listed: Story = {};

export const ListedChinese: Story = {
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='zh-CN'>
			<WorkbenchAgentStartDialog {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

/** No lookup gave a result: every Agent stays startable, listed as not checked, with a warning. */
export const FailedLookup: Story = {
	args: {
		catalog: fixedCatalog(
			catalog({ checkedAt: null, status: 'failed' }, (agent) =>
				agent.providerId === 'pi' ? 'not-found' : 'unknown',
			),
		),
	},
};

/** A query no Agent matches lists nothing and says so in the status line. */
export const NoMatch: Story = {
	play: async ({ canvasElement }) => {
		const view = canvasElement.ownerDocument.defaultView;
		if (view === null) throw new Error('Expected the Story to render in a window.');
		await new Promise((resolve) => view.requestAnimationFrame(resolve));
		const search = canvasElement.ownerDocument.querySelector(
			'[data-slot="workbench-agent-start-search"]',
		);
		if (!(search instanceof view.HTMLInputElement)) {
			throw new Error('Expected the Start Agent search field.');
		}
		// React reads the native value setter, so the typed query reaches the dialog's state.
		Object.getOwnPropertyDescriptor(view.HTMLInputElement.prototype, 'value')?.set?.call(
			search,
			'zzz',
		);
		search.dispatchEvent(new view.Event('input', { bubbles: true }));
	},
};
