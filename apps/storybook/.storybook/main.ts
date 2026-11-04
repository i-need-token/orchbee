import { mergeRsbuildConfig } from '@rsbuild/core';
import { pluginSourceBuild } from '@rsbuild/plugin-source-build';
import { pluginTailwindcss } from '@rsbuild/plugin-tailwindcss';
import type { StorybookConfig } from 'storybook-react-rsbuild';

const DEVELOPMENT_FONT_INLINE_LIMIT_BYTES = 1_048_576;

const config: StorybookConfig = {
	stories: [
		{
			directory: '../../../packages/ui/stories',
			files: '**/*.stories.@(ts|tsx)',
			titlePrefix: 'UI',
		},
		{
			directory: '../../../packages/tab-strip/stories',
			files: '**/*.stories.@(ts|tsx)',
			titlePrefix: 'Packages/TabStrip',
		},
		{
			directory: '../../../apps/web/stories',
			files: '**/*.stories.@(ts|tsx)',
			titlePrefix: 'Applications/Web',
		},
	],
	addons: ['@storybook/addon-docs', '@storybook/addon-a11y'],
	framework: 'storybook-react-rsbuild',
	rsbuildFinal: (rsbuildConfig, { configType }) =>
		mergeRsbuildConfig(
			rsbuildConfig,
			configType === 'DEVELOPMENT'
				? {
						// Storybook's dev CSS base cannot serve dependency-relative font assets reliably.
						output: {
							dataUriLimit: { font: DEVELOPMENT_FONT_INLINE_LIMIT_BYTES },
						},
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
