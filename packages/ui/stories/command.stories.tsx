import {
	Command,
	CommandEmpty,
	CommandGroup,
	CommandInput,
	CommandItem,
	CommandList,
	CommandShortcut,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Navigation/Command',
	component: Command,
} satisfies Meta<typeof Command>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Palette: Story = {
	render: () => (
		<Command>
			<CommandInput placeholder='Type a command' />
			<CommandList>
				<CommandEmpty>No command found.</CommandEmpty>
				<CommandGroup heading='Session'>
					<CommandItem>
						New terminal <CommandShortcut>⌘N</CommandShortcut>
					</CommandItem>
					<CommandItem>
						Open settings <CommandShortcut>⌘,</CommandShortcut>
					</CommandItem>
				</CommandGroup>
			</CommandList>
		</Command>
	),
};
