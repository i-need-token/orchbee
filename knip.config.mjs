// Knip passes raw CLI arguments; --strict implies production by default.
export default function createKnipConfiguration({ production, strict }) {
	const isProduction = production ?? strict ?? false;
	return {
		$schema: './node_modules/knip/schema.json',
		treatConfigHintsAsErrors: true,
		workspaces: {
			'.': {
				entry: [
					'scripts/*.mjs',
					'scripts/style-contract/__tests__/*.unit.test.ts',
					'scripts/tla/check-inventory.mjs',
				],
				project: ['scripts/**/*.{js,mjs,ts}', '*.{js,mjs,ts}'],
				ignoreBinaries: isProduction ? [] : ['rustc', 'zig'],
			},
			'apps/*': {
				entry: [
					'src/index.ts',
					'src/composition/entry.tsx',
					'src/**/*.unit.test.{ts,tsx}',
					'rsbuild.config.ts',
					'vitest.config.ts',
				],
				project: ['src/**/*.{ts,tsx}', '*.ts'],
			},
			'packages/*': {
				entry: ['src/**/*.unit.test.{ts,tsx}'],
				project: ['src/**/*.{ts,tsx}', '*.ts'],
			},
			'packages/agent': {
				paths: {
					'../dist/__tests__/conformance/agent-reference.js': [
						'./src/__tests__/conformance/agent-reference.ts',
					],
				},
				entry: ['src/**/*.unit.test.{ts,tsx}!'],
				project: [
					'src/**/*.{ts,tsx}!',
					'*.ts',
					'!src/__tests__/conformance/agent-reference.ts!',
					'!src/__tests__/conformance/agent-reference-contracts.ts!',
				],
				includeEntryExports: true,
			},
			'packages/foundation': {
				entry: ['src/**/*.unit.test.{ts,tsx}!'],
				project: ['src/**/*.{ts,tsx}!', '*.ts'],
				includeEntryExports: true,
			},
			'packages/ui': {
				entry: ['stories/**/*.stories.tsx'],
				ignoreDependencies: ['shadcn!', 'tailwindcss!', 'tw-animate-css!'],
				project: ['src/**/*.{ts,tsx}!', 'stories/**/*.tsx', '*.ts'],
				includeEntryExports: true,
			},
			'packages/workbench': {
				entry: ['stories/**/*.stories.tsx'],
				project: ['src/**/*.{ts,tsx}!', 'stories/**/*.tsx', '*.ts', '!src/**/__tests__/**!'],
				includeEntryExports: true,
			},
			'packages/terminal-protocol': {
				entry: ['src/**/*.unit.test.{ts,tsx}!', 'scripts/*.mjs!'],
				project: ['src/**/*.{ts,tsx}!', 'scripts/*.mjs!', '*.ts', '!src/__tests__/conformance/**!'],
				includeEntryExports: true,
			},
			'packages/desktop-client-protocol': {
				entry: ['src/**/*.unit.test.{ts,tsx}!'],
				project: ['src/**/*.{ts,tsx}!', '*.ts'],
				includeEntryExports: true,
			},
			'packages/workbench-client-protocol': {
				entry: ['src/**/*.unit.test.{ts,tsx}!'],
				project: ['src/**/*.{ts,tsx}!', '*.ts'],
				includeEntryExports: true,
			},
			'packages/terminal': {
				entry: ['src/projected/projected-terminal.ts!', 'src/**/*.unit.test.{ts,tsx}'],
				project: ['src/**/*.{ts,tsx}!', '*.ts'],
			},
			'packages/tab-strip': {
				entry: [
					'src/tab-strip/hooks/useTabStripActions.ts!',
					'src/**/*.unit.test.{ts,tsx}',
					'stories/**/*.stories.tsx',
				],
				project: [
					'src/**/*.{ts,tsx}!',
					'stories/**/*.tsx',
					'*.ts',
					'!src/tab-strip/dragging/__tests__/horizontal-group-drag-invariants.ts!',
				],
				includeEntryExports: true,
			},
			'apps/web': {
				entry: [
					'src/index.ts!',
					'src/composition/entry.ts!',
					'src/**/*.unit.test.{ts,tsx}!',
					'src/**/*.system-fixture.ts!',
					'stories/**/*.stories.tsx',
					'system-tests/*.spec.ts',
				],
				project: ['src/**/*.{ts,tsx}!', 'stories/**/*.tsx', 'system-tests/*.ts', '*.ts'],
			},
			'apps/desktop': {
				entry: [
					'scripts/*.mjs',
					'src/index.ts!',
					'src/main/composition/entry.ts!',
					'src/preload/entry.ts!',
					'src/preload/index.ts!',
					'src/**/*.unit.test.{ts,tsx}!',
				],
				project: ['scripts/*.mjs', 'src/**/*.{ts,tsx}!', '*.ts!', '!src/distribution/tooling/**!'],
			},
			'apps/storybook': {
				entry: ['src/**/*.unit.test.{ts,tsx}!'],
				ignoreDependencies: ['@orchbee/tab-strip!', '@orchbee/ui!', '@orchbee/workbench!'],
				project: ['.storybook/*.ts!', 'src/**/*.{ts,tsx}!', '*.ts'],
			},
			'apps/terminal-host': {
				entry: ['scripts/*.mjs!'],
				ignoreBinaries: ['zig'],
				project: ['scripts/*.mjs!'],
			},
			'apps/website': {
				entry: [
					'src/index.ts!',
					'src/composition/entry.tsx!',
					'src/**/*.unit.test.{ts,tsx}!',
					'scripts/*.mjs!',
				],
				project: ['src/**/*.{ts,tsx}!', 'scripts/*.mjs!', '*.ts!'],
			},
			'packages/actions': {
				includeEntryExports: true,
				entry: ['src/**/*.unit.test.{ts,tsx}'],
				project: ['src/**/*.{ts,tsx}!', '*.ts'],
			},
			'packages/design-tokens': {
				includeEntryExports: true,
				entry: ['src/**/*.unit.test.{ts,tsx}'],
				project: ['src/**/*.{ts,tsx}', '*.ts'],
			},
			'packages/i18n': {
				includeEntryExports: true,
				entry: ['src/**/*.unit.test.{ts,tsx}'],
				project: ['src/**/*.{ts,tsx}!', '*.ts'],
			},
			'packages/keyboard-shortcuts': {
				includeEntryExports: true,
				entry: ['src/**/*.unit.test.{ts,tsx}'],
				project: ['src/**/*.{ts,tsx}!', '*.ts'],
			},
		},
	};
}
