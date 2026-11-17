import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import type { Decorator, Meta, StoryObj } from 'storybook-react-rsbuild';

import surfaceStyles from '../src/workbench-client/projected-terminal-surface/ProjectedTerminalSurface.module.css';
import { ProjectedTerminalConnectionStatus } from '../src/workbench-client/projected-terminal-surface/components/projected-terminal-connection-status/ProjectedTerminalConnectionStatus';

/** The row sits across the top of a terminal, as the surface places it. */
function inTerminal(locale: OrchbeeLocale): Decorator {
	return (Story) => (
		<OrchbeeI18nProvider locale={locale}>
			<div style={{ blockSize: '16rem' }}>
				<div className={surfaceStyles['terminalSurface']}>
					<Story />
				</div>
			</div>
		</OrchbeeI18nProvider>
	);
}

const meta = {
	title: 'Projected Terminal/Connection Status',
	component: ProjectedTerminalConnectionStatus,
	args: { state: { kind: 'reconnecting' } },
	parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof ProjectedTerminalConnectionStatus>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Reconnecting: Story = { decorators: [inTerminal('en-US')] };

export const ReconnectingSimplifiedChinese: Story = { decorators: [inTerminal('zh-CN')] };

export const CantReconnect: Story = {
	args: { state: { kind: 'given-up', onQuitAll: () => undefined } },
	decorators: [inTerminal('en-US')],
};

export const CantReconnectSimplifiedChinese: Story = {
	args: { state: { kind: 'given-up', onQuitAll: () => undefined } },
	decorators: [inTerminal('zh-CN')],
};
