import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type { ProjectedTerminalSearchEvent } from '@orchbee/terminal';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

import {
	TerminalFindController,
	TerminalFindSurface,
} from '../src/workbench-client/terminal-find-surface';

const completedController = createController('agent', 'complete');
const searchingController = createController('orchestration', 'searching');
const invalidController = createController('[agent', 'error');

const meta = {
	title: 'Projected Terminal/Find Surface',
	component: TerminalFindSurface,
	decorators: [
		(Story) => (
			<OrchbeeI18nProvider locale='en-US'>
				<Story />
			</OrchbeeI18nProvider>
		),
	],
	parameters: { layout: 'centered' },
} satisfies Meta<typeof TerminalFindSurface>;

export default meta;

type Story = StoryObj<typeof meta>;

export const MatchResults: Story = {
	args: {
		controller: completedController,
		onNext: () => undefined,
		onPrevious: () => undefined,
		restoreFocus: () => undefined,
	},
};

export const Searching: Story = {
	args: {
		...MatchResults.args,
		controller: searchingController,
	},
};

export const InvalidExpression: Story = {
	args: {
		...MatchResults.args,
		controller: invalidController,
	},
};

function createController(
	query: string,
	state: ProjectedTerminalSearchEvent['state'],
): TerminalFindController {
	const controller = new TerminalFindController({
		clearSearch: () => undefined,
		updateSearch: () => undefined,
	});
	controller.open();
	controller.setQuery(query);
	controller.applySearchEvent({
		activeIndex: state === 'complete' ? 1 : -1,
		errorKind: state === 'error' ? 'invalid-query' : null,
		generation: 1n,
		matches:
			state === 'complete'
				? [
						{
							end: { column: 9, row: 42 },
							start: { column: 5, row: 42 },
							text: query,
						},
						{
							end: { column: 16, row: 71 },
							start: { column: 12, row: 71 },
							text: query,
						},
					]
				: [],
		query,
		regexError: state === 'error' ? 'invalid' : null,
		resultLimitReached: false,
		revision: 4n,
		state,
	});
	return controller;
}
