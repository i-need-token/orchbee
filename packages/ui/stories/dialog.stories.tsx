import { useState } from 'react';
import {
	Button,
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Overlays/Dialog',
	component: Dialog,
} satisfies Meta<typeof Dialog>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	render: function DialogExample() {
		const [open, setOpen] = useState(false);
		return (
			<>
				<Button onClick={() => setOpen(true)}>Open dialog</Button>
				<Dialog open={open} onOpenChange={setOpen}>
					<DialogContent>
						<DialogHeader>
							<DialogTitle>Create a session</DialogTitle>
							<DialogDescription>Choose the workspace for the new agent session.</DialogDescription>
						</DialogHeader>
						<DialogFooter>
							<Button variant='outline' onClick={() => setOpen(false)}>
								Cancel
							</Button>
							<Button onClick={() => setOpen(false)}>Create</Button>
						</DialogFooter>
					</DialogContent>
				</Dialog>
			</>
		);
	},
};
