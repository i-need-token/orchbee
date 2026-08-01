import { mergeRsbuildConfig } from '@rsbuild/core';
import { pluginSourceBuild } from '@rsbuild/plugin-source-build';
import { pluginTailwindcss } from '@rsbuild/plugin-tailwindcss';
import type { StorybookConfig } from 'storybook-react-rsbuild';

const config: StorybookConfig = {
	stories: [
		{
			directory: '../../../packages/ui/stories',
			files: '**/*.stories.@(ts|tsx)',
			titlePrefix: 'UI',
		},
		{
			directory: '../../../packages/workbench/stories',
			files: '**/*.stories.@(ts|tsx)',
			titlePrefix: 'Packages/Workbench',
		},
		{
			directory: '../../../packages/tab-strip/stories',
			files: '**/*.stories.@(ts|tsx)',
			titlePrefix: 'Packages/TabStrip',
		},
	],
	addons: ['@storybook/addon-docs', '@storybook/addon-a11y'],
	framework: 'storybook-react-rsbuild',
	rsbuildFinal: (rsbuildConfig, { configType }) =>
		mergeRsbuildConfig(
			rsbuildConfig,
			configType === 'DEVELOPMENT'
				? {
						plugins: [pluginTailwindcss(), pluginSourceBuild({ sourceField: '@orchbee/source' })],
					}
				: {},
			{ performance: { printFileSize: false } },
		),
	docs: {
		defaultName: 'Documentation',
	},
};

export default config;
