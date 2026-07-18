import {
	Field,
	FieldDescription,
	FieldError,
	FieldGroup,
	FieldLabel,
	Input,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Forms/Field',
	component: Field,
} satisfies Meta<typeof Field>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	render: () => (
		<FieldGroup>
			<Field>
				<FieldLabel htmlFor='field-name'>Session name</FieldLabel>
				<Input id='field-name' defaultValue='Agent workspace' />
				<FieldDescription>Shown in the session switcher.</FieldDescription>
			</Field>
			<Field data-invalid='true'>
				<FieldLabel htmlFor='field-path'>Workspace path</FieldLabel>
				<Input id='field-path' aria-invalid defaultValue='/missing' />
				<FieldError>The directory does not exist.</FieldError>
			</Field>
		</FieldGroup>
	),
};
