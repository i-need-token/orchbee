import { createRequire } from 'node:module';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import {
	designTokenNamesFromRegistry,
	renderDesignTokenInventory,
	responsiveBreakpointsFromRegistry,
} from '../../design-token-inventory.mjs';
import {
	tokenDeclarations,
	validateCssSource,
	validateShadcnCompatibilitySource,
	validateScriptSource,
	validateTailwindCompatibilitySource,
} from '../../style-contract.mjs';
import {
	parseCustomPropertyDeclarations,
	renderTailwindTokenCompatibility,
	resolveTailwindThemePath,
	tailwindReferenceToken,
} from '../../tailwind-token-compat.mjs';

const registeredTokens = new Set([
	'--ob-ref-animation-bounce',
	'--ob-ref-font-size-sm',
	'--ob-ref-motion-duration-fast',
	'--ob-ref-motion-easing-standard',
	'--ob-ref-space-4',
	'--ob-sys-color-text-primary',
	'--ob-sys-color-surface-raised',
	'--ob-sys-motion-duration-fast',
	'--ob-sys-motion-easing-standard',
	'--ob-sys-text-decoration-link',
	'--ob-sys-typography-body-md',
	'--ob-comp-panel-width',
]);
const breakpointValues = new Set(['40rem']);

describe('style contract', () => {
	it('accepts tokenized CSS Modules and registered breakpoints', () => {
		const diagnostics = validateCssSource({
			path: 'packages/example/src/panel.module.css',
			source: `.root {
	display: grid;
	gap: var(--ob-ref-space-4);
	color: var(--ob-sys-color-text-primary);
	background: var(--ob-sys-color-surface-raised);
	font: var(--ob-sys-typography-body-md);
	text-decoration: var(--ob-sys-text-decoration-link);
	transition: opacity var(--ob-sys-motion-duration-fast) var(--ob-sys-motion-easing-standard);
}

@media (width >= 40rem) {
	.root {
		display: flex;
	}
}`,
			registeredTokens,
			breakpointValues,
		});

		expect(diagnostics).toEqual([]);
	});

	it('rejects raw values, reference colors, and local custom properties', () => {
		const diagnostics = validateCssSource({
			path: 'packages/example/src/panel.css',
			source: `.root {
	--panel-gap: 8px;
	padding: 8px;
	color: #fff;
	background: var(--ob-ref-color-neutral-0);
	text-decoration: underline;
	transition: opacity 180ms ease;
}`,
			registeredTokens,
			breakpointValues,
		});

		expect(diagnostics.map(({ message }) => message)).toEqual(
			expect.arrayContaining([
				expect.stringContaining('*.module.css'),
				expect.stringContaining('central token registry'),
				expect.stringContaining('raw dimension'),
				expect.stringContaining('raw color'),
				expect.stringContaining('--ob-sys-color-*'),
				expect.stringContaining('registered token'),
			]),
		);
	});

	it('rejects raw values in multiline and compact CSS declarations', () => {
		const multilineDiagnostics = validateCssSource({
			path: 'packages/example/src/multiline.module.css',
			source: `.root {
	padding:
		8px;
}`,
			registeredTokens,
			breakpointValues,
		});
		const compactDiagnostics = validateCssSource({
			path: 'packages/example/src/compact.module.css',
			source: '.root { color: #fff; padding: 8px; }',
			registeredTokens,
			breakpointValues,
		});

		expect(multilineDiagnostics.map(({ message }) => message)).toContain(
			'Replace the raw dimension in "padding" with a token.',
		);
		expect(compactDiagnostics.map(({ message }) => message)).toEqual(
			expect.arrayContaining([
				'Replace the raw color in "color" with a system token.',
				'Replace the raw dimension in "padding" with a token.',
			]),
		);
	});

	it('parses custom property declarations without treating comments as declarations', () => {
		const diagnostics = validateCssSource({
			path: 'packages/example/src/custom.module.css',
			source: '.root { --local-gap: var(--ob-ref-space-4); } /* --comment-example: 8px; */',
			registeredTokens,
			breakpointValues,
		});

		expect(diagnostics.filter(({ message }) => message.includes('central token registry'))).toEqual(
			[
				expect.objectContaining({
					message: 'Move custom property "--local-gap" into the central token registry.',
				}),
			],
		);
	});

	it('rejects unregistered breakpoints in multiline media and container conditions', () => {
		const diagnostics = validateCssSource({
			path: 'packages/example/src/responsive.module.css',
			source: `@media (
	width >= 41rem
) {}

@container panel (
	width >= 640px
) {}`,
			registeredTokens,
			breakpointValues,
		});

		expect(diagnostics.map(({ message }) => message)).toEqual(
			expect.arrayContaining([
				'Use an exported responsiveBreakpoints value instead of "41rem".',
				'Use an exported responsiveBreakpoints value instead of "640px".',
			]),
		);
	});

	it('rejects reference typography and motion tokens that bypass semantic roles', () => {
		const diagnostics = validateCssSource({
			path: 'packages/example/src/panel.module.css',
			source: `.root {
	font-size: var(--ob-ref-font-size-sm);
	animation: var(--ob-ref-animation-bounce);
	transition: opacity var(--ob-ref-motion-duration-fast) var(--ob-ref-motion-easing-standard);
}`,
			registeredTokens,
			breakpointValues,
		});

		expect(diagnostics.map(({ message }) => message)).toEqual(
			expect.arrayContaining([
				expect.stringContaining('composite --ob-sys-typography-*'),
				expect.stringContaining('font shorthand'),
				expect.stringContaining('semantic --ob-sys-motion-*'),
				expect.stringContaining('semantic duration token'),
				expect.stringContaining('semantic easing token'),
			]),
		);
	});

	it('rejects Tailwind class strings and standard inline style properties', () => {
		const diagnostics = validateScriptSource({
			path: 'packages/example/src/panel.tsx',
			source: `export function Panel() {
	return <div className='flex gap-2' style={{ width: 320 }} />;
}`,
			registeredTokens,
		});

		expect(diagnostics.map(({ message }) => message)).toEqual([
			'Use a CSS Module reference instead of a string className.',
			'Inline style must be an object containing only registered --ob-comp-* properties.',
		]);
	});

	it('rejects indirect, conditional, and spread class strings', () => {
		const diagnostics = validateScriptSource({
			path: 'packages/example/src/panel.tsx',
			source: `const rootClass = 'flex gap-2';
export function Panel() {
	return <><div className={rootClass} /><div className={1 > 0 ? 'flex' : 'grid'} /><div {...{ className: 'flex' }} /></>;
}`,
			registeredTokens,
		});

		expect(
			diagnostics.filter(
				({ message }) => message === 'Use a CSS Module reference instead of a string className.',
			),
		).toHaveLength(3);
	});

	it('rejects unresolved JSX attribute spreads that can hide styling escapes', () => {
		const diagnostics = validateScriptSource({
			path: 'packages/example/src/panel.tsx',
			source: `export function Panel(props: Record<string, unknown>) {
	return <div {...props} />;
}`,
			registeredTokens,
		});

		expect(diagnostics.map(({ message }) => message)).toContain(
			'JSX attribute spreads must resolve to a local object without styling escape hatches.',
		);
	});

	it('resolves immutable class bindings by lexical scope and rejects writable bindings', () => {
		const safeDiagnostics = validateScriptSource({
			path: 'packages/example/src/scoped.tsx',
			source: `import styles from './panel.module.css';
export function Panel() {
	return <>{(() => { const rootClass = styles.root; return <div className={rootClass} />; })()}{(() => { const rootClass = styles.active; return <div className={rootClass} />; })()}</>;
}`,
			registeredTokens,
		});
		const unsafeDiagnostics = validateScriptSource({
			path: 'packages/example/src/writable.tsx',
			source: `import styles from './panel.module.css';
let rootClass = styles.root;
rootClass = 'flex';
const props = {};
props.className = 'flex';
export function Panel() {
	return <><div className={rootClass} /><div {...props} /><div {...{ ['className']: 'flex' }} /></>;
}`,
			registeredTokens,
		});

		expect(safeDiagnostics).toEqual([]);
		expect(unsafeDiagnostics.map(({ message }) => message)).toEqual(
			expect.arrayContaining([
				'Use a CSS Module reference instead of a string className.',
				'JSX attribute spreads must resolve to a local object without styling escape hatches.',
			]),
		);
		expect(
			unsafeDiagnostics.filter(
				({ message }) => message === 'Use a CSS Module reference instead of a string className.',
			),
		).toHaveLength(2);
	});

	it('rejects unknown computed spread properties and shadowed CSS Module imports', () => {
		const diagnostics = validateScriptSource({
			path: 'packages/example/src/computed.tsx',
			source: `import styles from './panel.module.css';
const key = 'id';
export function Panel(styles: Record<string, string>) {
	return <><div {...{ [key]: 'value' }} /><div className={styles.root} /></>;
}`,
			registeredTokens,
		});

		expect(diagnostics.map(({ message }) => message)).toEqual(
			expect.arrayContaining([
				'JSX attribute spreads must use statically named properties.',
				'Use a CSS Module reference instead of a string className.',
			]),
		);
	});

	it('rejects Tailwind theme declarations outside compatibility stylesheets', () => {
		const diagnostics = validateCssSource({
			path: 'packages/example/src/theme.module.css',
			source: `@theme {
	--color-brand: var(--ob-sys-color-text-primary);
}`,
			registeredTokens,
			breakpointValues,
		});

		expect(diagnostics.map(({ message }) => message)).toContain(
			'Tailwind directives are limited to @orchbee/ui.',
		);
	});

	it('accepts CSS Module expressions and registered runtime geometry slots', () => {
		const diagnostics = validateScriptSource({
			path: 'packages/example/src/panel.tsx',
			source: `import styles from './panel.module.css';
export function Panel({ active, width }: { active: boolean; width: string }) {
	return <div className={cx(styles.root, active && styles.active)} style={{ '--ob-comp-panel-width': width }} />;
}`,
			registeredTokens,
		});

		expect(diagnostics).toEqual([]);
	});
});

describe('design-token inventory', () => {
	it('deduplicates theme overrides and sorts token names deterministically', () => {
		expect(
			designTokenNamesFromRegistry(`:root {
	--ob-sys-color-text-primary: var(--ob-ref-color-neutral-950);
	--ob-ref-color-neutral-950: oklch(0.145 0 0);
}
[data-theme='dark'] {
	--ob-sys-color-text-primary: var(--ob-ref-color-neutral-50);
}`),
		).toEqual(['--ob-ref-color-neutral-950', '--ob-sys-color-text-primary']);
	});

	it('extracts responsive breakpoint values from their reference tokens', () => {
		expect(
			responsiveBreakpointsFromRegistry(`:root {
	--ob-ref-breakpoint-sm: 40rem;
	--ob-ref-breakpoint-2xl: 96rem;
}`),
		).toEqual(
			new Map([
				['sm', '40rem'],
				['2xl', '96rem'],
			]),
		);
	});

	it('generates the sorted public TypeScript inventory from the CSS registry', async () => {
		const repositoryRoot = resolve(import.meta.dirname, '../../..');
		const [registrySource, inventorySource] = await Promise.all([
			readFile(resolve(repositoryRoot, 'packages/design-tokens/src/tokens.css'), 'utf8'),
			readFile(
				resolve(repositoryRoot, 'packages/design-tokens/src/design-token-names.generated.ts'),
				'utf8',
			),
		]);

		expect(renderDesignTokenInventory(registrySource)).toBe(inventorySource);
	});
});

describe('Tailwind token compatibility', () => {
	it('maps every Tailwind theme namespace to an Orchbee reference-token category', () => {
		expect([
			tailwindReferenceToken('--color-mauve-500'),
			tailwindReferenceToken('--text-9xl--line-height'),
			tailwindReferenceToken('--tracking-widest'),
			tailwindReferenceToken('--animate-bounce'),
			tailwindReferenceToken('--default-transition-duration'),
			tailwindReferenceToken('--max-width-prose'),
		]).toEqual([
			'--ob-ref-color-mauve-500',
			'--ob-ref-font-size-9xl-line-height',
			'--ob-ref-letter-spacing-widest',
			'--ob-ref-animation-bounce',
			'--ob-ref-motion-duration-default',
			'--ob-ref-layout-prose-width',
		]);
		expect(() => tailwindReferenceToken('--future-unknown-token')).toThrow(
			'has no Orchbee reference-token mapping',
		);
	});

	it('enumerates the complete installed Tailwind theme with a current compatibility file', async () => {
		const repositoryRoot = resolve(import.meta.dirname, '../../..');
		const [tailwindThemeSource, compatibilitySource, registrySource] = await Promise.all([
			readFile(resolveTailwindThemePath(repositoryRoot), 'utf8'),
			readFile(resolve(repositoryRoot, 'packages/design-tokens/src/tailwind-compat.css'), 'utf8'),
			readFile(resolve(repositoryRoot, 'packages/design-tokens/src/tokens.css'), 'utf8'),
		]);

		expect(parseCustomPropertyDeclarations(tailwindThemeSource)).toHaveLength(419);
		expect(parseCustomPropertyDeclarations(compatibilitySource)).toHaveLength(419);
		expect(renderTailwindTokenCompatibility(tailwindThemeSource)).toBe(compatibilitySource);
		expect(
			validateTailwindCompatibilitySource({
				path: 'packages/design-tokens/src/tailwind-compat.css',
				source: compatibilitySource,
				tailwindThemeSource,
				registrySource,
			}),
		).toEqual([]);
	});

	it('compiles representative utilities from every Tailwind theme namespace', async () => {
		type TailwindCompiler = { build: (candidates: string[]) => string };
		type TailwindModule = { compile: (source: string) => Promise<TailwindCompiler> };

		const repositoryRoot = resolve(import.meta.dirname, '../../..');
		const requireFromUi = createRequire(resolve(repositoryRoot, 'packages/ui/package.json'));
		const { compile } = requireFromUi('tailwindcss') as TailwindModule;
		const compatibilitySource = await readFile(
			resolve(repositoryRoot, 'packages/design-tokens/src/tailwind-compat.css'),
			'utf8',
		);
		const compiler = await compile(`${compatibilitySource}\n@tailwind utilities;`);
		const output = compiler.build([
			'font-serif',
			'font-black',
			'bg-mauve-500',
			'p-4',
			'2xl:flex',
			'max-w-7xl',
			'@7xl:flex',
			'text-9xl',
			'text-shadow-lg',
			'tracking-widest',
			'leading-loose',
			'rounded-4xl',
			'shadow-2xl',
			'inset-shadow-sm',
			'drop-shadow-2xl',
			'ease-in-out',
			'animate-bounce',
			'blur-3xl',
			'perspective-distant',
			'aspect-video',
			'transition',
			'max-w-prose',
		]);

		for (const expectedFragment of [
			'aspect-ratio: var(--ob-ref-aspect-ratio-video)',
			'max-width: 80rem',
			'max-width: var(--ob-ref-layout-prose-width)',
			'animation: var(--ob-ref-animation-bounce)',
			'border-radius: var(--ob-ref-radius-4xl)',
			'background-color: var(--ob-ref-color-mauve-500)',
			'padding: calc(var(--ob-ref-space-1) * 4)',
			'font-family: var(--ob-ref-font-family-serif)',
			'font-size: var(--ob-ref-font-size-9xl)',
			'line-height: var(--ob-ref-line-height-loose)',
			'font-weight: var(--ob-ref-font-weight-black)',
			'letter-spacing: var(--ob-ref-letter-spacing-widest)',
			'--tw-shadow: var(--ob-ref-shadow-2xl)',
			'--tw-inset-shadow: var(--ob-ref-inset-shadow-sm)',
			'--tw-drop-shadow: drop-shadow(var(--ob-ref-drop-shadow-2xl))',
			'transition-duration: var(--tw-duration, var(--ob-ref-motion-duration-default))',
			'transition-timing-function: var(--ob-ref-motion-easing-in-out)',
			'--tw-blur: blur(var(--ob-ref-filter-blur-3xl))',
			'perspective: var(--ob-ref-perspective-distant)',
			'text-shadow: var(--ob-ref-text-shadow-lg)',
			'@media (width >= 96rem)',
			'@container (width >= 80rem)',
		]) {
			expect(output).toContain(expectedFragment);
		}
	});
});

describe('shadcn token compatibility', () => {
	it('keeps the complete shadcn alias and Tailwind theme maps on Orchbee tokens', async () => {
		const repositoryRoot = resolve(import.meta.dirname, '../../..');
		const [registrySource, compatibilitySource, styleEntrySource] = await Promise.all([
			readFile(resolve(repositoryRoot, 'packages/design-tokens/src/tokens.css'), 'utf8'),
			readFile(resolve(repositoryRoot, 'packages/ui/src/styles.css'), 'utf8'),
			readFile(resolve(repositoryRoot, 'packages/ui/src/styles.ts'), 'utf8'),
		]);
		expect(styleEntrySource).toBe(
			[
				"import * as designTokenStyles from '@orchbee/design-tokens/tokens.css';",
				"import * as shadcnCompatibilityStyles from './styles.css';",
				'',
				'void designTokenStyles;',
				'void shadcnCompatibilityStyles;',
				'',
			].join('\n'),
		);
		const actualRegisteredTokens = new Set(tokenDeclarations(registrySource));
		const validationInput = {
			path: 'packages/ui/src/styles.css',
			registeredTokens: actualRegisteredTokens,
		};

		expect(
			validateShadcnCompatibilitySource({
				...validationInput,
				source: compatibilitySource,
			}),
		).toEqual([]);

		const brokenSource = compatibilitySource.replace(
			'--background: var(--ob-sys-color-surface-canvas);',
			'--background: oklch(1 0 0);',
		);
		expect(
			validateShadcnCompatibilitySource({ ...validationInput, source: brokenSource }).map(
				({ message }) => message,
			),
		).toContain(
			'Shadcn semantic aliases must map "--background" to "var(--ob-sys-color-surface-canvas)".',
		);

		const overrideSource = `${compatibilitySource}\n[data-theme='dark'] { --background: oklch(1 0 0); }`;
		expect(
			validateShadcnCompatibilitySource({ ...validationInput, source: overrideSource }).map(
				({ message }) => message,
			),
		).toContain(
			'Shadcn semantic alias "--background" may only be declared in the shared mapping rule.',
		);

		const legacyDarkVariantSource = compatibilitySource.replace(
			"@custom-variant dark (&:where([data-theme='dark'], [data-theme='dark'] *));",
			'@custom-variant dark (&:is(.dark *));',
		);
		expect(
			validateShadcnCompatibilitySource({
				...validationInput,
				source: legacyDarkVariantSource,
			}).map(({ message }) => message),
		).toContain(
			"Shadcn dark variants must use \"dark (&:where([data-theme='dark'], [data-theme='dark'] *))\".",
		);

		const extraThemeSource = `${compatibilitySource}\n@theme { --color-background: oklch(1 0 0); }`;
		expect(
			validateShadcnCompatibilitySource({ ...validationInput, source: extraThemeSource }).map(
				({ message }) => message,
			),
		).toContain('Shadcn compatibility requires exactly one "@theme inline" rule.');

		const missingSourceRegistration = compatibilitySource.replace("@source './components';\n", '');
		expect(
			validateShadcnCompatibilitySource({
				...validationInput,
				source: missingSourceRegistration,
			}).map(({ message }) => message),
		).toContain(
			'Shadcn compatibility requires exactly one "@source \'./components\'" registration.',
		);
	});
});
