import { OrchbeeI18nProvider } from '@orchbee/i18n';
import {
	Workbench,
	type WorkbenchSession,
	type WorkbenchSessionService,
	type WorkbenchSessionSnapshot,
} from '@orchbee/workbench';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

class StoryTerminalChannel {
	private readonly transcript: string;

	constructor(transcript: string) {
		this.transcript = transcript;
	}

	subscribeOutput(listener: (data: string) => void): () => void {
		listener(this.transcript);
		return () => undefined;
	}

	write(): void {}

	resize(): void {}
}

class StorySessionService implements WorkbenchSessionService {
	private readonly snapshot: WorkbenchSessionSnapshot;

	constructor() {
		const sessions: readonly WorkbenchSession[] = [
			{
				id: 'storybook-shell',
				title: 'Local shell',
				status: 'running',
				terminal: new StoryTerminalChannel('Orchbee Storybook session\r\n\r\n$ '),
			},
			{
				id: 'storybook-agent',
				title: 'Agent output',
				status: 'running',
				terminal: new StoryTerminalChannel('Codex agent · ready\r\n'),
			},
		];
		this.snapshot = { sessions };
	}

	getSnapshot(): WorkbenchSessionSnapshot {
		return this.snapshot;
	}

	subscribe(): () => void {
		return () => undefined;
	}

	createSession(): string {
		return 'storybook-shell';
	}

	closeSession(): void {}
}

const meta = {
	title: 'Workbench',
	component: Workbench,
	parameters: {
		layout: 'fullscreen',
	},
} satisfies Meta<typeof Workbench>;

export default meta;

type Story = StoryObj<typeof meta>;

export const InMemorySessions: Story = {
	args: {
		sessions: new StorySessionService(),
	},
	render: (args) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...args} />
		</OrchbeeI18nProvider>
	),
};

export const SimplifiedChinese: Story = {
	args: {
		sessions: new StorySessionService(),
	},
	render: (args) => (
		<OrchbeeI18nProvider locale='zh-CN'>
			<Workbench {...args} />
		</OrchbeeI18nProvider>
	),
};
