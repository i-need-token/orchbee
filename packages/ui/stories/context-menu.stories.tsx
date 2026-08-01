import {
	ContextMenu,
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuSeparator,
	ContextMenuShortcut,
	ContextMenuTrigger,
	ProductIcon,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Overlays/Context Menu',
	component: ContextMenu,
} satisfies Meta<typeof ContextMenu>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	render: () => (
		<ContextMenu>
			<ContextMenuTrigger>Right-click this terminal surface</ContextMenuTrigger>
			<ContextMenuContent>
				<ContextMenuItem>
					<ProductIcon name='copy' size='menu-leading' />
					Copy <ContextMenuShortcut>⌘C</ContextMenuShortcut>
				</ContextMenuItem>
				<ContextMenuItem>
					<ProductIcon name='clipboard' size='menu-leading' />
					Paste
				</ContextMenuItem>
				<ContextMenuSeparator />
				<ContextMenuItem variant='destructive'>
					<ProductIcon name='close' size='menu-leading' />
					Close terminal
				</ContextMenuItem>
			</ContextMenuContent>
		</ContextMenu>
	),
};
