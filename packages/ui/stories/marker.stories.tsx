import { Marker, MarkerContent, MarkerIcon } from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'AI Interface/Marker',
	component: Marker,
} satisfies Meta<typeof Marker>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Variants: Story = {
	render: () => (
		<>
			<Marker>
				<MarkerIcon>✓</MarkerIcon>
				<MarkerContent>Command completed</MarkerContent>
			</Marker>
			<Marker variant='separator'>
				<MarkerContent>New messages</MarkerContent>
			</Marker>
			<Marker variant='border'>
				<MarkerContent>Execution details</MarkerContent>
			</Marker>
		</>
	),
};
