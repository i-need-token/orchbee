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
	navigateDirectory(): void {}
	openFolderAttachment(): void {}
	resolvePrompt(): void {}
	retry(): void {}
	selectDirectory(): void {}
	submit(): void {}
	subscribeDialog = (): (() => void) => () => undefined;
	updateDirectoryPath(): void {}
	updateForm(): void {}
}

const enrollmentForm = {
	destination: 'ssh builder@build-host',
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
		canNavigate: false,
		canSubmit: true,
		directory: null,
		failure: null,
		form: enrollmentForm,
		kind: 'enrollment',
		pathInput: '',
		phase: 'editing',
		progress: null,
		prompt: null,
	}),
);

export const HostVerification: Story = story(
	'en-US',
	new StoryController({
		canNavigate: false,
		canSubmit: true,
		directory: null,
		failure: null,
		form: enrollmentForm,
		kind: 'enrollment',
		pathInput: '',
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
		canNavigate: false,
		canSubmit: true,
		directory: null,
		failure: null,
		form: enrollmentForm,
		kind: 'enrollment',
		pathInput: '',
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

export const DirectorySelection: Story = story(
	'en-US',
	new StoryController({
		canNavigate: true,
		canSubmit: true,
		directory: {
			canonicalPath: '/home/builder',
			directories: [{ name: '.config' }, { name: 'orchbee' }, { name: 'projects' }],
			navigationFailure: null,
			parentPath: '/home',
			truncated: false,
		},
		failure: null,
		form: enrollmentForm,
		kind: 'enrollment',
		pathInput: '/home/builder',
		phase: 'directory',
		progress: 'selecting-project-directory',
		prompt: null,
	}),
);

export const FolderAttachment: Story = story(
	'en-US',
	new StoryController({
		canNavigate: false,
		canSubmit: true,
		directory: null,
		failure: null,
		form: enrollmentForm,
		kind: 'attachment',
		pathInput: '',
		phase: 'editing',
		progress: null,
		projectId: 'project-local',
		prompt: null,
	}),
);

export const ConnectionFailure: Story = story(
	'en-US',
	new StoryController({
		canNavigate: false,
		directory: null,
		failure: 'connection',
		kind: 'connection',
		pathInput: '',
		phase: 'failed',
		progress: null,
		projectId: 'project-remote',
		prompt: null,
	}),
);

export const SimplifiedChineseEnrollment: Story = story(
	'zh-CN',
	new StoryController({
		canNavigate: false,
		canSubmit: true,
		directory: null,
		failure: null,
		form: enrollmentForm,
		kind: 'enrollment',
		pathInput: '',
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
