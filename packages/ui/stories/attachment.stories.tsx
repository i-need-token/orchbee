import {
	Attachment,
	AttachmentAction,
	AttachmentActions,
	AttachmentContent,
	AttachmentDescription,
	AttachmentGroup,
	AttachmentMedia,
	AttachmentTitle,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'AI Interface/Attachment',
	component: Attachment,
} satisfies Meta<typeof Attachment>;

export default meta;

type Story = StoryObj<typeof meta>;

export const States: Story = {
	render: () => (
		<AttachmentGroup>
			<Attachment>
				<AttachmentMedia>PDF</AttachmentMedia>
				<AttachmentContent>
					<AttachmentTitle>architecture.pdf</AttachmentTitle>
					<AttachmentDescription>1.4 MB</AttachmentDescription>
				</AttachmentContent>
				<AttachmentActions>
					<AttachmentAction aria-label='Remove attachment'>×</AttachmentAction>
				</AttachmentActions>
			</Attachment>
			<Attachment state='uploading'>
				<AttachmentMedia>TS</AttachmentMedia>
				<AttachmentContent>
					<AttachmentTitle>agent.ts</AttachmentTitle>
					<AttachmentDescription>Uploading</AttachmentDescription>
				</AttachmentContent>
			</Attachment>
			<Attachment state='error'>
				<AttachmentMedia>LOG</AttachmentMedia>
				<AttachmentContent>
					<AttachmentTitle>session.log</AttachmentTitle>
					<AttachmentDescription>Upload failed</AttachmentDescription>
				</AttachmentContent>
			</Attachment>
		</AttachmentGroup>
	),
};
