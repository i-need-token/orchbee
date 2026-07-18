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
	],
	addons: ['@storybook/addon-docs', '@storybook/addon-a11y'],
	framework: 'storybook-react-rsbuild',
	rsbuildFinal: (rsbuildConfig, { configType }) =>
		configType === 'DEVELOPMENT'
			? mergeRsbuildConfig(rsbuildConfig, {
					plugins: [pluginTailwindcss(), pluginSourceBuild({ sourceField: '@orchbee/source' })],
				})
			: rsbuildConfig,
	docs: {
		defaultName: 'Documentation',
	},
};

export default config;
