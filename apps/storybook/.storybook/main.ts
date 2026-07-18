import type { StorybookConfig } from 'storybook-react-rsbuild';

const config: StorybookConfig = {
	stories: [
		{
			directory: '../../../packages/ui/stories',
			files: '**/*.stories.@(ts|tsx)',
			titlePrefix: 'UI',
		},
	],
	addons: ['@storybook/addon-docs', '@storybook/addon-a11y'],
	framework: 'storybook-react-rsbuild',
	docs: {
		defaultName: 'Documentation',
	},
};

export default config;
