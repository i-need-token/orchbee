import {
	Sidebar,
	SidebarContent,
	SidebarFooter,
	SidebarGroup,
	SidebarGroupContent,
	SidebarGroupLabel,
	SidebarHeader,
	SidebarInset,
	SidebarMenu,
	SidebarMenuButton,
	SidebarMenuItem,
	SidebarProvider,
	SidebarRail,
	SidebarTrigger,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Layout/Sidebar',
	component: Sidebar,
	parameters: {
		layout: 'fullscreen',
	},
} satisfies Meta<typeof Sidebar>;

export default meta;

type Story = StoryObj<typeof meta>;

export const WorkbenchNavigation: Story = {
	render: () => (
		<SidebarProvider>
			<Sidebar collapsible='icon'>
				<SidebarHeader>Orchbee</SidebarHeader>
				<SidebarContent>
					<SidebarGroup>
						<SidebarGroupLabel>Workspace</SidebarGroupLabel>
						<SidebarGroupContent>
							<SidebarMenu>
								<SidebarMenuItem>
									<SidebarMenuButton isActive>Terminal</SidebarMenuButton>
								</SidebarMenuItem>
								<SidebarMenuItem>
									<SidebarMenuButton>Agent sessions</SidebarMenuButton>
								</SidebarMenuItem>
								<SidebarMenuItem>
									<SidebarMenuButton>Settings</SidebarMenuButton>
								</SidebarMenuItem>
							</SidebarMenu>
						</SidebarGroupContent>
					</SidebarGroup>
				</SidebarContent>
				<SidebarFooter>Connected</SidebarFooter>
				<SidebarRail />
			</Sidebar>
			<SidebarInset style={{ minHeight: '100vh', padding: 24 }}>
				<SidebarTrigger />
				<h2>Terminal workspace</h2>
				<p>Select the sidebar trigger to test its collapsed state.</p>
			</SidebarInset>
		</SidebarProvider>
	),
};
