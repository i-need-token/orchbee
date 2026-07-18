import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Layout/Resizable',
	component: ResizablePanelGroup,
} satisfies Meta<typeof ResizablePanelGroup>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Horizontal: Story = {
	render: () => (
		<ResizablePanelGroup orientation='horizontal' style={{ width: 640, height: 320 }}>
			<ResizablePanel defaultSize={35}>Sessions</ResizablePanel>
			<ResizableHandle withHandle />
			<ResizablePanel defaultSize={65}>Terminal</ResizablePanel>
		</ResizablePanelGroup>
	),
};
