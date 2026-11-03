import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import sharedThemeColors from '../../../../schemas/workbench/v1/shared-theme-colors-v1.json';
import { designTokenNames, responsiveBreakpoints } from '../design-token-names.generated';

type ThemeSelector = "[data-theme='light']" | "[data-theme='dark']";

interface ResolvedColor {
	readonly opacity?: number;
	readonly rgb: string;
}

/** The registry color that carries each color the native theme shares. */
const SHARED_COLOR_TOKENS = new Map<string, string>([
	['action', '--ob-sys-color-action-primary'],
	['agentAttention', '--ob-sys-color-status-attention'],
	['agentDone', '--ob-sys-color-status-agent-done'],
	['agentError', '--ob-sys-color-status-agent-error'],
	['agentIdle', '--ob-sys-color-status-agent-idle'],
	['agentWorking', '--ob-sys-color-status-working'],
	['hover', '--ob-sys-color-surface-hovered'],
	['press', '--ob-sys-color-surface-pressed'],
]);

function ruleBody(css: string, selector: ThemeSelector): string {
	const escapedSelector = selector.replaceAll(/[.*+?^${}()|[\]\\]/gu, '\\$&');
	const rule = new RegExp(`${escapedSelector} \\{(?<body>[\\s\\S]*?)\\n\\}`, 'u').exec(css);
	const body = rule?.groups?.['body'];
	if (body === undefined) throw new Error(`Missing ${selector} token rule.`);
	return body;
}

function ruleTokenNames(css: string, selector: ThemeSelector) {
	return new Set(
		Array.from(ruleBody(css, selector).matchAll(/^\s*(--ob-[a-z0-9-]+)\s*:/gmu), (match) =>
			String(match[1]),
		),
	);
}

function declaredValues(css: string): Map<string, string> {
	return new Map(
		Array.from(css.matchAll(/^\s*(--ob-[a-z0-9-]+)\s*:\s*([^;]+);/gmu), (match) => [
			String(match[1]),
			String(match[2]).replaceAll(/\s+/gu, ' ').trim(),
		]),
	);
}

/** The 8-bit sRGB channel of an achromatic OKLCH lightness. */
function achromaticChannel(lightnessPercent: number): number {
	const linear = (lightnessPercent / 100) ** 3;
	const encoded = linear <= 0.003_130_8 ? 12.92 * linear : 1.055 * linear ** (1 / 2.4) - 0.055;
	return Math.round(encoded * 255);
}

function resolveColor(value: string, references: ReadonlyMap<string, string>): ResolvedColor {
	const mix = /^color-mix\( ?in srgb, (?<color>\S+) (?<percent>\d+)%, transparent ?\)$/u.exec(
		value,
	)?.groups;
	if (mix !== undefined) {
		const color = resolveColor(String(mix['color']), references);
		return { opacity: Number(mix['percent']), rgb: color.rgb };
	}
	const reference = /^var\((?<name>--ob-ref-[a-z0-9-]+)\)$/u.exec(value)?.groups?.['name'];
	if (reference !== undefined) {
		const referenced = references.get(reference);
		if (referenced === undefined) throw new Error(`Missing ${reference}.`);
		return resolveColor(referenced, references);
	}
	if (/^#[0-9a-f]{6}$/u.test(value)) return { rgb: value };
	const lightness = /^oklch\((?<lightness>[\d.]+)% 0 none\)$/u.exec(value)?.groups?.['lightness'];
	if (lightness === undefined) throw new Error(`Unsupported shared color ${value}.`);
	return {
		rgb: `#${achromaticChannel(Number(lightness)).toString(16).padStart(2, '0').repeat(3)}`,
	};
}

describe('design tokens', () => {
	it('keeps the public token inventory synchronized with the CSS registry', async () => {
		const css = await readFile(new URL('../tokens.css', import.meta.url), 'utf8');
		const declarations = Array.from(css.matchAll(/^\s*(--ob-[a-z0-9-]+)\s*:/gmu), (match) =>
			String(match[1]),
		);
		const uniqueDeclarations = new Set(declarations);

		expect(uniqueDeclarations).toEqual(new Set(designTokenNames));
		expect(designTokenNames.length).toBe(uniqueDeclarations.size);
	});

	it('publishes the finite responsive breakpoint vocabulary', () => {
		expect(responsiveBreakpoints).toEqual({
			xs: '24rem',
			sm: '40rem',
			md: '48rem',
			lg: '64rem',
			xl: '80rem',
			'2xl': '96rem',
		});
	});

	it('keeps explicit light and dark theme scopes limited to equal semantic color inventories', async () => {
		const css = await readFile(new URL('../tokens.css', import.meta.url), 'utf8');
		const lightSystemColors = ruleTokenNames(css, "[data-theme='light']");
		const darkSystemColors = ruleTokenNames(css, "[data-theme='dark']");

		expect(darkSystemColors).toEqual(lightSystemColors);
		expect([...lightSystemColors].every((name) => name.startsWith('--ob-sys-color-'))).toBe(true);
		expect(css).toContain(":root,\n[data-theme='light'] {\n\tcolor-scheme: light;");
		expect(css).toContain("[data-theme='dark'] {\n\tcolor-scheme: dark;");
	});

	it('resolves the colors the native theme shares to the language-neutral palette', async () => {
		const css = await readFile(new URL('../tokens.css', import.meta.url), 'utf8');
		const references = new Map(
			[...declaredValues(css)].filter(([name]) => name.startsWith('--ob-ref-')),
		);
		expect(sharedThemeColors.contractVersion).toBe(1);
		const themes: readonly {
			readonly colors: Readonly<Record<string, ResolvedColor>>;
			readonly selector: ThemeSelector;
		}[] = [
			{ colors: sharedThemeColors.light, selector: "[data-theme='light']" },
			{ colors: sharedThemeColors.dark, selector: "[data-theme='dark']" },
		];
		for (const { colors, selector } of themes) {
			const system = declaredValues(ruleBody(css, selector));
			for (const [name, color] of Object.entries({ ...sharedThemeColors.everyTheme, ...colors })) {
				const token = SHARED_COLOR_TOKENS.get(name);
				if (token === undefined) throw new Error(`${name} has no registry color.`);
				const value = system.get(token);
				if (value === undefined) throw new Error(`${selector} lacks ${token}.`);
				expect(resolveColor(value, references), `${selector} ${name}`).toEqual(color);
			}
		}
	});

	it('collapses semantic motion durations for reduced-motion preferences', async () => {
		const css = await readFile(new URL('../tokens.css', import.meta.url), 'utf8');
		const reducedMotion = /@media \(prefers-reduced-motion: reduce\) \{(?<body>[\s\S]*?)\n\}/u.exec(
			css,
		)?.groups?.['body'];
		if (reducedMotion === undefined) throw new Error('Missing reduced-motion token overrides.');

		for (const duration of ['fast', 'normal', 'slow']) {
			expect(reducedMotion).toContain(
				`--ob-sys-motion-duration-${duration}: var(--ob-ref-motion-duration-instant);`,
			);
		}
	});

	it('publishes complete cross-component semantic role matrices', () => {
		const inventory = new Set<string>(designTokenNames);
		for (const token of [
			'--ob-sys-color-surface-canvas',
			'--ob-sys-color-surface-inactive',
			'--ob-sys-color-surface-subtle',
			'--ob-sys-color-surface-raised',
			'--ob-sys-color-surface-overlay',
			'--ob-sys-color-surface-inverse',
			'--ob-sys-color-surface-hovered',
			'--ob-sys-color-surface-pressed',
			'--ob-sys-color-surface-selected',
			'--ob-sys-color-surface-selected-hover',
			'--ob-sys-color-surface-disabled',
			'--ob-sys-color-surface-scrim',
			'--ob-sys-color-text-heading',
			'--ob-sys-color-text-label',
			'--ob-sys-color-text-placeholder',
			'--ob-sys-color-text-disabled',
			'--ob-sys-color-text-link-focus',
			'--ob-sys-color-text-link-pressed',
			'--ob-sys-color-icon-hover',
			'--ob-sys-color-icon-disabled',
			'--ob-sys-color-border-selected',
			'--ob-sys-color-border-disabled',
			'--ob-sys-color-scrollbar-track',
			'--ob-sys-color-scrollbar-thumb',
			'--ob-sys-color-scrollbar-thumb-hover',
			'--ob-sys-color-scrollbar-thumb-pressed',
			'--ob-sys-color-action-disabled',
			'--ob-sys-color-action-disabled-text',
			'--ob-sys-color-action-disabled-border',
			'--ob-sys-text-decoration-link',
			'--ob-sys-text-decoration-link-hover',
			'--ob-sys-text-decoration-link-focus',
			'--ob-sys-motion-duration-instant',
			'--ob-sys-motion-duration-fast',
			'--ob-sys-motion-duration-normal',
			'--ob-sys-motion-duration-slow',
			'--ob-sys-motion-easing-standard',
			'--ob-sys-motion-easing-enter',
			'--ob-sys-motion-easing-exit',
		]) {
			expect(inventory).toContain(token);
		}

		for (const status of ['danger', 'info', 'success', 'warning']) {
			for (const role of [
				'',
				'-hover',
				'-pressed',
				'-surface',
				'-surface-hover',
				'-border',
				'-border-hover',
				'-text',
				'-text-hover',
				'-text-pressed',
			]) {
				expect(inventory).toContain(`--ob-sys-color-status-${status}${role}`);
			}
		}

		for (const role of [
			'body-sm',
			'body-md',
			'body-lg',
			'control-sm',
			'label-xs',
			'label-sm',
			'label-md',
			'label-lg',
			'heading-xs',
			'heading-sm',
			'heading-md',
			'heading-lg',
			'heading-xl',
			'code-sm',
			'code-md',
		]) {
			expect(inventory).toContain(`--ob-sys-typography-${role}`);
		}
	});
});
