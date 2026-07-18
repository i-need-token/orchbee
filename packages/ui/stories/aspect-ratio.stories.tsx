import { AspectRatio } from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Layout/Aspect Ratio',
	component: AspectRatio,
} satisfies Meta<typeof AspectRatio>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Widescreen: Story = {
	args: {
		ratio: 16 / 9,
	},
	render: (args) => <AspectRatio {...args}>16:9 preview surface</AspectRatio>,
};
