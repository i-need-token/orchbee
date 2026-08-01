import {
	Menubar,
	MenubarContent,
	MenubarItem,
	MenubarMenu,
	MenubarSeparator,
	MenubarShortcut,
	MenubarTrigger,
	ProductIcon,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Navigation/Menubar',
	component: Menubar,
} satisfies Meta<typeof Menubar>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	render: () => (
		<Menubar>
			<MenubarMenu>
				<MenubarTrigger>Session</MenubarTrigger>
				<MenubarContent>
					<MenubarItem>
						<ProductIcon name='terminal' size='menu-leading' />
						New terminal <MenubarShortcut>⌘N</MenubarShortcut>
					</MenubarItem>
					<MenubarSeparator />
					<MenubarItem>
						<ProductIcon name='close' size='menu-leading' />
						Close session
					</MenubarItem>
				</MenubarContent>
			</MenubarMenu>
			<MenubarMenu>
				<MenubarTrigger>View</MenubarTrigger>
				<MenubarContent>
					<MenubarItem>
						<ProductIcon name='sidebar-left' size='menu-leading' />
						Toggle sidebar
					</MenubarItem>
				</MenubarContent>
			</MenubarMenu>
		</Menubar>
	),
};
