import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

import { RouteUnavailable } from '../src/route-unavailable';

const meta = {
	title: 'Routing/Route Unavailable',
	component: RouteUnavailable,
	parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof RouteUnavailable>;

export default meta;

type Story = StoryObj<typeof meta>;

export const English: Story = {
	decorators: [
		(StoryComponent) => (
			<OrchbeeI18nProvider locale='en-US'>
				<StoryComponent />
			</OrchbeeI18nProvider>
		),
	],
};

export const SimplifiedChinese: Story = {
	decorators: [
		(StoryComponent) => (
			<OrchbeeI18nProvider locale='zh-CN'>
				<StoryComponent />
			</OrchbeeI18nProvider>
		),
	],
};
