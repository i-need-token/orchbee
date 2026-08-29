import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

import { ProjectedTerminalMobileInputBar } from '../src/workbench-client/projected-terminal-surface/components/projected-terminal-mobile-input-bar/ProjectedTerminalMobileInputBar';

const meta = {
	title: 'Projected Terminal/Mobile Input Bar',
	component: ProjectedTerminalMobileInputBar,
	decorators: [
		(Story) => (
			<OrchbeeI18nProvider locale='en-US'>
				<style>{`[data-slot="projected-terminal-mobile-input-bar"] { display: flex !important; position: relative !important; width: min(100vw, 32rem); }`}</style>
				<Story />
			</OrchbeeI18nProvider>
		),
	],
	parameters: { layout: 'centered' },
} satisfies Meta<typeof ProjectedTerminalMobileInputBar>;

export default meta;

type Story = StoryObj<typeof meta>;

export const ReadyWithModifiers: Story = {
	args: {
		onKey: () => undefined,
		onKeyboardToggle: () => undefined,
		onModifierToggle: () => undefined,
		state: {
			alt: true,
			control: false,
			focused: true,
			readiness: { kind: 'ready', revision: 'storybook:1' },
		},
	},
};

export const ReadOnlyChinese: Story = {
	args: {
		...ReadyWithModifiers.args,
		state: {
			alt: false,
			control: false,
			focused: false,
			readiness: { kind: 'unavailable' },
		},
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='zh-CN'>
			<ProjectedTerminalMobileInputBar {...arguments_} />
		</OrchbeeI18nProvider>
	),
};
