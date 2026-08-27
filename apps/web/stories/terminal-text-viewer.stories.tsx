import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

import {
	commandOutputViewerDocument,
	scrollbackViewerDocument,
	TerminalTextViewer,
	TerminalTextViewerController,
} from '../src/workbench-client/terminal-text-viewer';

const outputController = new TerminalTextViewerController();
outputController.open(
	commandOutputViewerDocument({
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
	}),
);
outputController.setQuery('agent');

const scrollbackText = [
	'$ pnpm verify',
	'Checking repository contracts',
	'Checking terminal projection',
	'Visible terminal starts here',
	'All checks passed',
].join('\n');
const scrollbackController = new TerminalTextViewerController();
scrollbackController.open(
	scrollbackViewerDocument({
		endRow: 28n,
		generation: 2n,
		initialTopOffset: scrollbackText.indexOf('Visible terminal starts here'),
		kind: 'scrollback',
		revision: 7n,
		startRow: 0n,
		text: scrollbackText,
	}),
);

const noOutputController = new TerminalTextViewerController();
noOutputController.open(commandOutputViewerDocument({ kind: 'empty', reason: 'no-output' }));

const unavailableController = new TerminalTextViewerController();
unavailableController.open(
	scrollbackViewerDocument({ kind: 'unavailable', reason: 'source-limit' }),
);

const meta = {
	title: 'Projected Terminal/Terminal Text Viewer',
	component: TerminalTextViewer,
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
} satisfies Meta<typeof TerminalTextViewer>;

export default meta;

type Story = StoryObj<typeof meta>;

export const CommandOutput: Story = {
	args: {
		controller: outputController,
		onCopy: () => undefined,
		platform: 'apple',
		restoreFocus: () => undefined,
	},
};

export const Scrollback: Story = {
	args: {
		...CommandOutput.args,
		controller: scrollbackController,
	},
};

export const NoOutput: Story = {
	args: {
		...CommandOutput.args,
		controller: noOutputController,
	},
};

export const Unavailable: Story = {
	args: {
		...CommandOutput.args,
		controller: unavailableController,
	},
};
