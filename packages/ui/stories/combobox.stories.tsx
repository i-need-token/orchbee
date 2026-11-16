import {
	Combobox,
	ComboboxCollection,
	ComboboxContent,
	ComboboxGroup,
	ComboboxItem,
	ComboboxLabel,
	ComboboxList,
	ComboboxSearch,
	ComboboxTrigger,
	ComboboxValue,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const GROUPS = [
	{ label: 'Local', value: 'local', items: ['Default shell', 'PowerShell', 'Command Prompt'] },
	{ label: 'Remote', value: 'remote', items: ['Build server', 'Staging'] },
] as const;

const meta = {
	title: 'Forms/Combobox',
	component: Combobox,
} satisfies Meta<typeof Combobox>;

export default meta;

type Story = StoryObj<typeof meta>;

function SearchableChoice({ open }: { readonly open: boolean }) {
	return (
		<Combobox defaultOpen={open} defaultValue='PowerShell' items={GROUPS}>
			<ComboboxTrigger aria-label='Profile' size='sm'>
				<ComboboxValue />
			</ComboboxTrigger>
			<ComboboxContent align='end'>
				<ComboboxSearch aria-label='Search profiles' placeholder='Search profiles' />
				<ComboboxList>
					{(group: (typeof GROUPS)[number]) => (
						<ComboboxGroup key={group.value} items={group.items}>
							<ComboboxLabel>{group.label}</ComboboxLabel>
							<ComboboxCollection>
								{(item: string) => (
									<ComboboxItem key={item} value={item}>
										{item}
									</ComboboxItem>
								)}
							</ComboboxCollection>
						</ComboboxGroup>
					)}
				</ComboboxList>
			</ComboboxContent>
		</Combobox>
	);
}

/** A choice whose list carries its own search field, opened from a select-style trigger. */
export const Default: Story = {
	render: () => <SearchableChoice open={false} />,
};

export const Open: Story = {
	render: () => <SearchableChoice open />,
};
