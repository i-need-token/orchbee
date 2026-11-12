import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

// The form is private to this package, so the Story reads it from the same source as the Workbench.
import { WorkbenchWorktreeFormDialog } from '../src/workbench/components/workbench-worktree-form-dialog/WorkbenchWorktreeFormDialog';

const meta = {
	title: 'New Worktree',
	component: WorkbenchWorktreeFormDialog,
	args: {
		branch: '',
		finalFocus: () => null,
		onCancel: () => undefined,
		onSubmit: () => undefined,
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<WorkbenchWorktreeFormDialog {...arguments_} />
		</OrchbeeI18nProvider>
	),
} satisfies Meta<typeof WorkbenchWorktreeFormDialog>;

export default meta;

type Story = StoryObj<typeof meta>;

/** From a menu or Search: both fields blank, so Create takes every default. */
export const Blank: Story = {};

/** From Start Agent's New Worktree… place: the branch proposed for the Agent, still editable. */
export const AgentBranch: Story = {
	args: { branch: 'agent/codex' },
};

export const BlankChinese: Story = {
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='zh-CN'>
			<WorkbenchWorktreeFormDialog {...arguments_} />
		</OrchbeeI18nProvider>
	),
};
