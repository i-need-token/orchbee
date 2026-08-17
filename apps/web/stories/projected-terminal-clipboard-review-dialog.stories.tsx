import { OrchbeeI18nProvider } from '@orchbee/i18n';
import {
	AttachmentIdSchema,
	CanonicalUint64TextSchema,
	type TerminalClipboardWriteRequest,
} from '@orchbee/terminal-protocol';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

import { ProjectedTerminalClipboardReviewDialog } from '../src/workbench-client/projected-terminal-surface/components/projected-terminal-clipboard-review-dialog/ProjectedTerminalClipboardReviewDialog';

const request: TerminalClipboardWriteRequest = {
	attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
	sequence: CanonicalUint64TextSchema.parse('7'),
	text: 'https://example.com/terminal-token\nscoped-value=demo',
};

const meta = {
	title: 'Projected Terminal/Clipboard Review Dialog',
	component: ProjectedTerminalClipboardReviewDialog,
	decorators: [
		(Story) => (
			<OrchbeeI18nProvider locale='en-US'>
				<Story />
			</OrchbeeI18nProvider>
		),
	],
	parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof ProjectedTerminalClipboardReviewDialog>;

export default meta;

type Story = StoryObj<typeof meta>;

export const PlainTextRequest: Story = {
	args: {
		onApprove: () => undefined,
		onCancel: () => undefined,
		request,
	},
};
