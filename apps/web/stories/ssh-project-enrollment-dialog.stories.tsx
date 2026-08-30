import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import {
	DesktopSshEnvironmentCatalogSchema,
	DesktopSshProjectEnrollmentInputSchema,
	type DesktopSshEnvironmentCatalog,
} from '@orchbee/desktop-client-protocol';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

import {
	DesktopSshProjectEnrollmentDialog,
	type DesktopSshProjectEnrollmentDialogController,
	type DesktopSshProjectEnrollmentDialogState,
} from '../src/desktop-client/desktop-ssh-project-enrollment-dialog';

class StoryController implements DesktopSshProjectEnrollmentDialogController {
	readonly #catalog: DesktopSshEnvironmentCatalog;
	readonly #listeners = new Set<() => void>();
	#state: DesktopSshProjectEnrollmentDialogState;

	constructor(
		state: DesktopSshProjectEnrollmentDialogState,
		catalog: DesktopSshEnvironmentCatalog = EMPTY_CATALOG,
	) {
		this.#state = state;
		this.#catalog = catalog;
	}

	cancel(): void {
		const state = this.#state;
		if ((state.kind === 'attachment' || state.kind === 'enrollment') && state.phase !== 'editing') {
			this.#state = {
				...state,
				canNavigate: false,
				directory: null,
				failure: null,
				pathInput: '',
				phase: 'editing',
				progress: null,
				prompt: null,
			};
			this.#publish();
			return;
		}
		this.close();
	}

	close(): void {
		this.#state = { kind: 'closed' };
		this.#publish();
	}
	getDestinationCatalogSnapshot = (): DesktopSshEnvironmentCatalog => this.#catalog;
	getDialogSnapshot = (): DesktopSshProjectEnrollmentDialogState => this.#state;
	navigateDirectory(): void {}
	openFolderAttachment(): void {}
	resolvePrompt(): void {}
	retry(): void {}
	selectDirectory(): void {}
	submit(): void {}
	subscribeDestinationCatalog = (): (() => void) => () => undefined;
	subscribeDialog = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};
	updateDirectoryPath(): void {}
	updateForm(update: { readonly destination?: string }): void {
		const state = this.#state;
		if ((state.kind !== 'attachment' && state.kind !== 'enrollment') || state.phase !== 'editing') {
			return;
		}
		const form = { ...state.form, ...update };
		this.#state = {
			...state,
			canSubmit: DesktopSshProjectEnrollmentInputSchema.safeParse({
				destination: form.destination.trim(),
			}).success,
			form,
		};
		this.#publish();
	}

	#publish(): void {
		for (const listener of this.#listeners) listener();
	}
}

const EMPTY_CATALOG = DesktopSshEnvironmentCatalogSchema.parse({
	discoveryIssue: null,
	managed: [],
	registryIssue: null,
	suggestions: [],
});

const CONFIGURED_HOSTS_CATALOG = DesktopSshEnvironmentCatalogSchema.parse({
	discoveryIssue: null,
	managed: [
		{
			blockedReason: null,
			destination: 'github.com',
			environmentId: `environment:${'a'.repeat(32)}`,
			name: 'github.com',
			projectCount: 0,
			state: 'disconnected',
		},
	],
	registryIssue: null,
	suggestions: [{ destination: 'gpu-lab' }],
});

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

export const ConfiguredHosts: Story = {
	...story(
		'en-US',
		new StoryController(
			{
				canNavigate: false,
				canSubmit: false,
				directory: null,
				failure: null,
				form: { destination: '' },
				kind: 'enrollment',
				pathInput: '',
				phase: 'editing',
				progress: null,
				prompt: null,
			},
			CONFIGURED_HOSTS_CATALOG,
		),
	),
};

export const Connecting: Story = story(
	'en-US',
	new StoryController({
		canNavigate: false,
		canSubmit: true,
		directory: null,
		failure: null,
		form: enrollmentForm,
		kind: 'enrollment',
		pathInput: '',
		phase: 'working',
		progress: 'connecting',
		prompt: null,
	}),
);

export const InvalidAddress: Story = story(
	'en-US',
	new StoryController({
		canNavigate: false,
		canSubmit: false,
		directory: null,
		failure: null,
		form: { destination: '' },
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
		canSubmit: true,
		directory: null,
		failure: 'connection',
		form: enrollmentForm,
		kind: 'enrollment',
		pathInput: '',
		phase: 'failed',
		progress: null,
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

export const SimplifiedChineseAuthentication: Story = story(
	'zh-CN',
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
			handle: 'story-authentication-zh-cn',
			kind: 'none',
			prompt: '(builder@build-host) Password:',
		},
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
