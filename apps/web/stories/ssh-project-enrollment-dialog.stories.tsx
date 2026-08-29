import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

import {
	DesktopSshProjectEnrollmentDialog,
	type DesktopSshProjectEnrollmentDialogController,
	type DesktopSshProjectEnrollmentDialogState,
} from '../src/desktop-client/desktop-ssh-project-enrollment-dialog';

class StoryController implements DesktopSshProjectEnrollmentDialogController {
	readonly #state: DesktopSshProjectEnrollmentDialogState;

	constructor(state: DesktopSshProjectEnrollmentDialogState) {
		this.#state = state;
	}

	close(): void {}
	getDialogSnapshot = (): DesktopSshProjectEnrollmentDialogState => this.#state;
	resolvePrompt(): void {}
	retry(): void {}
	submit(): void {}
	subscribeDialog = (): (() => void) => () => undefined;
	updateForm(): void {}
}

const enrollmentForm = {
	configurationFilePath: '/Users/me/.ssh/config',
	destination: 'build-host',
	name: 'Build machine',
	projectPath: '/srv/orchbee',
};

const meta = {
	title: 'Desktop/SSH Project Enrollment Dialog',
	component: DesktopSshProjectEnrollmentDialog,
	parameters: { layout: 'centered' },
} satisfies Meta<typeof DesktopSshProjectEnrollmentDialog>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Enrollment: Story = story(
	'en-US',
	new StoryController({
		canSubmit: true,
		failure: null,
		form: enrollmentForm,
		kind: 'enrollment',
		phase: 'editing',
		progress: null,
		prompt: null,
	}),
);

export const HostVerification: Story = story(
	'en-US',
	new StoryController({
		canSubmit: true,
		failure: null,
		form: enrollmentForm,
		kind: 'enrollment',
		phase: 'prompt',
		progress: 'connecting',
		prompt: {
			generation: 1,
			handle: 'story-host-verification',
			kind: 'confirm',
			prompt:
				'The authenticity of host cannot be established. ED25519 key fingerprint is SHA256:example.',
		},
	}),
);

export const Authentication: Story = story(
	'en-US',
	new StoryController({
		canSubmit: true,
		failure: null,
		form: enrollmentForm,
		kind: 'enrollment',
		phase: 'prompt',
		progress: 'connecting',
		prompt: {
			generation: 1,
			handle: 'story-authentication',
			kind: 'none',
			prompt: "Enter passphrase for key '/Users/me/.ssh/id_ed25519':",
		},
	}),
);

export const ConnectionFailure: Story = story(
	'en-US',
	new StoryController({
		failure: 'connection',
		kind: 'connection',
		phase: 'failed',
		progress: null,
		projectId: 'project-remote',
		prompt: null,
	}),
);

export const SimplifiedChineseEnrollment: Story = story(
	'zh-CN',
	new StoryController({
		canSubmit: true,
		failure: null,
		form: enrollmentForm,
		kind: 'enrollment',
		phase: 'editing',
		progress: null,
		prompt: null,
	}),
);

function story(locale: OrchbeeLocale, controller: StoryController): Story {
	return {
		args: { controller },
		decorators: [
			(StoryComponent) => (
				<OrchbeeI18nProvider locale={locale}>
					<StoryComponent />
				</OrchbeeI18nProvider>
			),
		],
	};
}
