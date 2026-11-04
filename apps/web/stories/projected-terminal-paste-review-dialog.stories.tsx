import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type { ProjectedPasteReview } from '@orchbee/terminal';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

import { ProjectedTerminalPasteReviewDialog } from '../src/workbench-client/projected-terminal-surface/components/projected-terminal-paste-review-dialog/ProjectedTerminalPasteReviewDialog';

const review: ProjectedPasteReview = {
	bracketed: true,
	embeddedBracketTerminator: true,
	multiline: true,
	originalText: 'printf "first line\\n"\nprintf "second line\\n"\u001b[201~',
	replacedControlCharacters: 1,
	source: 'clipboard',
	text: 'printf "first line\\n"\nprintf "second line\\n" [201~',
};

const meta = {
	title: 'Projected Terminal/Paste Review Dialog',
	component: ProjectedTerminalPasteReviewDialog,
	decorators: [
		(Story) => (
			<OrchbeeI18nProvider locale='en-US'>
				<Story />
			</OrchbeeI18nProvider>
		),
	],
	parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof ProjectedTerminalPasteReviewDialog>;

export default meta;

type Story = StoryObj<typeof meta>;

export const SuspiciousPaste: Story = {
	args: {
		onApprove: () => undefined,
		onCancel: () => undefined,
		review,
	},
};
