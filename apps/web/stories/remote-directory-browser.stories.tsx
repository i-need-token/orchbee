import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

import {
	RemoteDirectoryBrowser,
	type RemoteDirectoryBrowserLocation,
} from '../src/remote-directory-browser';

const defaultLocation: RemoteDirectoryBrowserLocation = {
	canonicalPath: '/home/builder',
	crumbs: [
		{ label: '/', path: '/' },
		{ label: 'home', path: '/home' },
		{ label: 'builder', path: '/home/builder' },
	],
	directories: [
		{ name: '.config', path: '/home/builder/.config' },
		{ name: 'orchbee', path: '/home/builder/orchbee' },
		{ name: 'projects', path: '/home/builder/projects' },
	],
	navigationFailure: null,
	truncated: false,
};

const meta = {
	title: 'Remote Directory/Browser',
	component: RemoteDirectoryBrowser,
	parameters: { layout: 'centered' },
} satisfies Meta<typeof RemoteDirectoryBrowser>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = story('en-US', {
	busy: false,
	canNavigate: true,
	location: defaultLocation,
	onCancel: () => undefined,
	onNavigate: () => undefined,
	onPathDraftChange: () => undefined,
	onSelect: () => undefined,
	pathDraft: '/home/builder',
	sourceLabel: 'builder@build-host',
});

export const Loading: Story = story('en-US', {
	...Default.args,
	busy: true,
});

export const NavigationFailure: Story = story('en-US', {
	...Default.args,
	location: { ...defaultLocation, navigationFailure: 'not-found' },
	pathDraft: '/home/builder/missing',
});

export const SimplifiedChinese: Story = story('zh-CN', {
	...Default.args,
	sourceLabel: '构建机',
});

function story(locale: OrchbeeLocale, args: Story['args']): Story {
	return {
		args,
		decorators: [
			(StoryComponent) => (
				<OrchbeeI18nProvider locale={locale}>
					<div
						style={{
							inlineSize: 'min(var(--ob-ref-container-2xl), calc(100vw - var(--ob-ref-space-8)))',
						}}
					>
						<StoryComponent />
					</div>
				</OrchbeeI18nProvider>
			),
		],
	};
}
