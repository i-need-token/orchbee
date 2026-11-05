import { mergeRsbuildConfig } from '@rsbuild/core';
import { pluginSourceBuild } from '@rsbuild/plugin-source-build';
import { pluginTailwindcss } from '@rsbuild/plugin-tailwindcss';
import type { StorybookConfig } from 'storybook-react-rsbuild';

/**
 * Fonts referenced from CSS are inlined in both builds: the builder's empty asset prefix resolves a
 * stylesheet's font URL against the page, not the stylesheet, so a separate font file is not found.
 */
const FONT_INLINE_LIMIT_BYTES = 1_048_576;

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
			{ output: { dataUriLimit: { font: FONT_INLINE_LIMIT_BYTES } } },
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
