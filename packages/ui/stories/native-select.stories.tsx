import { NativeSelect, NativeSelectOptGroup, NativeSelectOption } from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Forms/Native Select',
	component: NativeSelect,
} satisfies Meta<typeof NativeSelect>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	render: () => (
		<NativeSelect defaultValue='local' aria-label='Runtime'>
			<NativeSelectOptGroup label='Runtime'>
				<NativeSelectOption value='local'>Local</NativeSelectOption>
				<NativeSelectOption value='remote'>Remote</NativeSelectOption>
			</NativeSelectOptGroup>
		</NativeSelect>
	),
};
