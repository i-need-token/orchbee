import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

import {
	TerminalCommandOutputViewer,
	TerminalCommandOutputViewerController,
} from '../src/workbench-client/terminal-command-output-viewer';

const outputController = new TerminalCommandOutputViewerController();
outputController.open({
	endRow: 18n,
	generation: 2n,
	kind: 'output',
	revision: 7n,
	startRow: 12n,
	text: [
		'Agent orchestration ready',
		'Running 3 tasks in parallel',
		'Agent worker-1 completed in 1.8s',
		'Agent worker-2 completed in 2.1s',
		'All tasks completed successfully',
	].join('\n'),
	truncatedStart: true,
});
outputController.setQuery('agent');

const noOutputController = new TerminalCommandOutputViewerController();
noOutputController.open({ kind: 'empty', reason: 'no-output' });

const unavailableController = new TerminalCommandOutputViewerController();
unavailableController.open({ kind: 'unavailable', reason: 'source-limit' });

const meta = {
	title: 'Projected Terminal/Command Output Viewer',
	component: TerminalCommandOutputViewer,
	decorators: [
		(Story) => (
			<OrchbeeI18nProvider locale='en-US'>
				<div style={{ height: '32rem', position: 'relative' }}>
					<Story />
				</div>
			</OrchbeeI18nProvider>
		),
	],
	parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof TerminalCommandOutputViewer>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Output: Story = {
	args: {
		controller: outputController,
		onCopy: () => undefined,
		platform: 'apple',
		restoreFocus: () => undefined,
	},
};

export const NoOutput: Story = {
	args: {
		...Output.args,
		controller: noOutputController,
	},
};

export const Unavailable: Story = {
	args: {
		...Output.args,
		controller: unavailableController,
	},
};
