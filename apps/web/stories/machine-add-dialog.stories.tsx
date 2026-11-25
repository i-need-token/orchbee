import { OrchbeeI18nProvider } from '@orchbee/i18n';
import * as React from 'react';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

// The form is private to Settings › Machines, so the Story reads it from the same source.
import { MachineAddDialog } from '../src/terminal-settings-surface/components/machine-settings/components/machine-add-dialog/MachineAddDialog';
import type {
	TerminalSettingsMachines,
	TerminalSettingsMachinesView,
} from '../src/terminal-settings-surface/types/terminal-settings-surface';

type Form = NonNullable<TerminalSettingsMachinesView['add']['form']>;

const LABELS: TerminalSettingsMachinesView['add']['labels'] = {
	add: 'Add',
	addMachine: 'Add machine…',
	address: 'Address',
	cancel: 'Cancel',
	hosts: 'Hosts in SSH config',
	title: 'Add machine',
};

const OPEN: Form = {
	blocked: false,
	color: 'purple',
	hosts: [
		{ host: 'devbox', label: 'devbox' },
		{ host: 'nas', label: 'nas' },
		{ host: 'mac-mini', label: 'mac-mini' },
	],
	lines: [],
	problem: null,
	running: false,
	saved: null,
	status: null,
	submit: 'Add',
};

interface HarnessProps {
	/** The add as the next view reports it once Add is pressed. */
	readonly after: Form;
}

/** The form as it opens, held as the page holds the machines view; Add moves it to `after`. */
function Harness({ after }: HarnessProps): React.ReactNode {
	const [form, setForm] = React.useState(OPEN);
	const machines = React.useMemo<TerminalSettingsMachines>(
		() => ({
			getSnapshot: () => null,
			request: (intent) => {
				if (intent.act === 'add') setForm(after);
			},
			showCapacity: () => () => undefined,
			subscribe: () => () => undefined,
		}),
		[after],
	);
	return (
		<OrchbeeI18nProvider locale='en-US'>
			<MachineAddDialog
				add={{ form, labels: LABELS }}
				machines={machines}
				onClose={() => undefined}
			/>
		</OrchbeeI18nProvider>
	);
}

/** Types an address into the form and presses Add, as the person does. */
async function addAddress({
	canvasElement,
}: {
	readonly canvasElement: HTMLElement;
}): Promise<void> {
	const document = canvasElement.ownerDocument;
	const view = document.defaultView;
	if (view === null) throw new Error('Expected the Story to render in a window.');
	await new Promise((resolve) => view.requestAnimationFrame(resolve));
	const dialog = document.querySelector('[data-slot="machine-add-dialog"]');
	const field = dialog?.querySelector('input');
	if (!(field instanceof view.HTMLInputElement)) throw new Error('Expected the address field.');
	// React reads the native value setter, so the typed address reaches the form's state.
	Object.getOwnPropertyDescriptor(view.HTMLInputElement.prototype, 'value')?.set?.call(
		field,
		'dev@10.0.0.8',
	);
	field.dispatchEvent(new view.Event('input', { bubbles: true }));
	await new Promise((resolve) => view.requestAnimationFrame(resolve));
	field.form?.requestSubmit();
}

const meta = {
	title: 'Add Machine',
	component: Harness,
	args: { after: OPEN },
} satisfies Meta<typeof Harness>;

export default meta;

type Story = StoryObj<typeof meta>;

/** As it opens: the SSH config hosts under the address, and the color the fewest machines use. */
export const Open: Story = {};

/** Add runs the checks: the fields wait, and the check running is named. */
export const Checking: Story = {
	args: {
		after: {
			...OPEN,
			running: true,
			status: { text: 'Checking 3 of 8: host key…', tone: 'pending' },
		},
	},
	play: addAddress,
};

/** A failed check names itself and its lines; the primary button takes that state's action. */
export const Failed: Story = {
	args: {
		after: {
			...OPEN,
			lines: ['ssh could not reach dev@10.0.0.8.', 'Check that the host is on and reachable.'],
			status: { text: 'Check 2 of 8 failed: reaching the host.', tone: 'error' },
			submit: 'Retry',
		},
	},
	play: addAddress,
};
