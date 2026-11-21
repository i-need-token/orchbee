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
	['actionPrimary', '--ob-sys-color-action-primary'],
	['actionPrimaryHover', '--ob-sys-color-action-primary-hover'],
	['actionPrimaryPress', '--ob-sys-color-action-primary-pressed'],
	['actionSecondary', '--ob-sys-color-action-secondary'],
	['actionSecondaryHover', '--ob-sys-color-action-secondary-hover'],
	['actionSecondaryPress', '--ob-sys-color-action-secondary-pressed'],
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

interface RgbColor {
	readonly r: number;
	readonly g: number;
	readonly b: number;
}

interface ParsedCssColor {
	readonly rgb: RgbColor;
	readonly alpha: number;
}

function srgbChannelToLinear(channel: number): number {
	const value = channel / 255;
	return value <= 0.040_45 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
}

function linearToSrgbChannel(channel: number): number {
	const clamped = Math.min(1, Math.max(0, channel));
	const encoded = clamped <= 0.003_130_8 ? clamped * 12.92 : 1.055 * clamped ** (1 / 2.4) - 0.055;
	return Math.round(Math.min(1, Math.max(0, encoded)) * 255);
}

function relativeLuminance(color: RgbColor): number {
	return (
		0.2126 * srgbChannelToLinear(color.r) +
		0.7152 * srgbChannelToLinear(color.g) +
		0.0722 * srgbChannelToLinear(color.b)
	);
}

function contrastRatio(left: RgbColor, right: RgbColor): number {
	const l1 = relativeLuminance(left);
	const l2 = relativeLuminance(right);
	return (Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05);
}

function rgbToOklab(color: RgbColor): readonly [number, number, number] {
	const lr = srgbChannelToLinear(color.r);
	const lg = srgbChannelToLinear(color.g);
	const lb = srgbChannelToLinear(color.b);
	const l = Math.cbrt(0.412_221_470_8 * lr + 0.536_332_536_3 * lg + 0.051_445_992_9 * lb);
	const m = Math.cbrt(0.211_903_498_2 * lr + 0.680_699_545_1 * lg + 0.107_396_956_6 * lb);
	const s = Math.cbrt(0.088_302_461_9 * lr + 0.281_718_837_6 * lg + 0.629_978_700_5 * lb);
	const lightness = 0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s;
	if (color.r === color.g && color.g === color.b) return [lightness, 0, 0];
	return [
		lightness,
		1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
		0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
	];
}

function oklabToRgb(lightness: number, a: number, b: number): RgbColor {
	const l = (lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b) ** 3;
	const m = (lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b) ** 3;
	const s = (lightness - 0.089_484_177_5 * a - 1.291_485_548 * b) ** 3;
	return {
		r: linearToSrgbChannel(4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s),
		g: linearToSrgbChannel(-1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s),
		b: linearToSrgbChannel(-0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701 * s),
	};
}

function parseHexRgb(hex: string): RgbColor {
	const digits = hex.slice(1);
	const expanded =
		digits.length === 3
			? digits
					.split('')
					.map((ch) => `${ch}${ch}`)
					.join('')
			: digits;
	return {
		r: Number.parseInt(expanded.slice(0, 2), 16),
		g: Number.parseInt(expanded.slice(2, 4), 16),
		b: Number.parseInt(expanded.slice(4, 6), 16),
	};
}

function splitTopLevelMixArgs(args: string): readonly [string, string] {
	let depth = 0;
	for (let i = 0; i < args.length; i += 1) {
		const ch = args.charAt(i);
		if (ch === '(') depth += 1;
		else if (ch === ')') depth -= 1;
		else if (ch === ',' && depth === 0) {
			return [args.slice(0, i).trim(), args.slice(i + 1).trim()];
		}
	}
	throw new Error(`Invalid color-mix arguments: ${args}`);
}

function parseCssColor(value: string, vars: ReadonlyMap<string, string>): ParsedCssColor {
	const trimmed = value.trim();
	if (trimmed === 'transparent') return { rgb: { r: 0, g: 0, b: 0 }, alpha: 0 };
	if (/^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/iu.test(trimmed)) {
		return { rgb: parseHexRgb(trimmed), alpha: 1 };
	}
	const refMatch = /^var\((?<name>--ob-[a-z0-9-]+)\)$/u.exec(trimmed);
	if (refMatch?.groups?.['name'] !== undefined) {
		const target = vars.get(refMatch.groups['name']);
		if (target === undefined) throw new Error(`Missing token ${refMatch.groups['name']}.`);
		return parseCssColor(target, vars);
	}
	const oklchMatch =
		/^oklch\((?<lightness>[\d.]+)(?<pct>%?)\s+(?<chroma>[\d.]+)\s+(?<hue>none|[\d.]+)(?:\s*\/\s*(?<alpha>[\d.]+)%)?\)$/u.exec(
			trimmed,
		);
	if (oklchMatch?.groups !== undefined) {
		const rawLightness = Number(oklchMatch.groups['lightness']);
		const lightness = oklchMatch.groups['pct'] === '%' ? rawLightness / 100 : rawLightness;
		const chroma = Number(oklchMatch.groups['chroma']);
		const hueRaw = oklchMatch.groups['hue'] ?? 'none';
		const radians = hueRaw === 'none' ? 0 : (Number(hueRaw) * Math.PI) / 180;
		const a = hueRaw === 'none' ? 0 : chroma * Math.cos(radians);
		const b = hueRaw === 'none' ? 0 : chroma * Math.sin(radians);
		const alphaRaw = oklchMatch.groups['alpha'];
		return {
			rgb: oklabToRgb(lightness, a, b),
			alpha: alphaRaw === undefined ? 1 : Number(alphaRaw) / 100,
		};
	}
	const mixMatch = /^color-mix\(\s*in\s+(?<space>srgb|oklab|oklch),\s*(?<args>.+)\s*\)$/u.exec(
		trimmed,
	);
	if (mixMatch?.groups !== undefined) {
		const [leftRaw, rightRaw] = splitTopLevelMixArgs(mixMatch.groups['args'] ?? '');
		const parseStop = (stop: string): { color: ParsedCssColor; weight: number | null } => {
			const m = /^(?<expr>.+?)\s+(?<pct>\d+(?:\.\d+)?)%$/u.exec(stop);
			if (m?.groups !== undefined) {
				return {
					color: parseCssColor(m.groups['expr'] ?? '', vars),
					weight: Number(m.groups['pct']) / 100,
				};
			}
			return { color: parseCssColor(stop, vars), weight: null };
		};
		const left = parseStop(leftRaw);
		const right = parseStop(rightRaw);
		const w1 = left.weight ?? (right.weight === null ? 0.5 : 1 - right.weight);
		const w2 = right.weight ?? 1 - w1;
		if (left.color.alpha === 0) return { rgb: right.color.rgb, alpha: w2 * right.color.alpha };
		if (right.color.alpha === 0) return { rgb: left.color.rgb, alpha: w1 * left.color.alpha };
		if (mixMatch.groups['space'] === 'srgb') {
			return {
				rgb: {
					r: Math.round(left.color.rgb.r * w1 + right.color.rgb.r * w2),
					g: Math.round(left.color.rgb.g * w1 + right.color.rgb.g * w2),
					b: Math.round(left.color.rgb.b * w1 + right.color.rgb.b * w2),
				},
				alpha: 1,
			};
		}
		const [l1, a1, b1] = rgbToOklab(left.color.rgb);
		const [l2, a2, b2] = rgbToOklab(right.color.rgb);
		return {
			rgb: oklabToRgb(l1 * w1 + l2 * w2, a1 * w1 + a2 * w2, b1 * w1 + b2 * w2),
			alpha: 1,
		};
	}
	throw new Error(`Unsupported color expression: ${value}`);
}

function paintOver(foreground: ParsedCssColor, background: RgbColor): RgbColor {
	if (foreground.alpha >= 1) return foreground.rgb;
	const a = foreground.alpha;
	return {
		r: Math.round(foreground.rgb.r * a + background.r * (1 - a)),
		g: Math.round(foreground.rgb.g * a + background.g * (1 - a)),
		b: Math.round(foreground.rgb.b * a + background.b * (1 - a)),
	};
}

function compositeOpacity(foreground: RgbColor, background: RgbColor, alpha: number): RgbColor {
	return paintOver({ rgb: foreground, alpha }, background);
}

/** The 8-bit sRGB channel of an achromatic OKLCH lightness. */
function achromaticChannel(lightnessPercent: number): number {
	const linear = (lightnessPercent / 100) ** 3;
	const encoded = linear <= 0.003_130_8 ? 12.92 * linear : 1.055 * linear ** (1 / 2.4) - 0.055;
	return Math.round(encoded * 255);
}

function resolveColor(value: string, references: ReadonlyMap<string, string>): ResolvedColor {
	// An opaque OKLab mix, such as a primary action's hover, resolves to its sRGB channels.
	if (/^color-mix\(\s*in\s+oklab,/u.test(value)) {
		const opaque = parseCssColor(value, references);
		if (opaque.alpha !== 1) throw new Error(`Expected an opaque color: ${value}`);
		const { r, g, b } = opaque.rgb;
		return {
			rgb: `#${[r, g, b].map((channel) => channel.toString(16).padStart(2, '0')).join('')}`,
		};
	}
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

	it('satisfies GUI derived-state contrast floors in light and dark system themes', async () => {
		const css = await readFile(new URL('../tokens.css', import.meta.url), 'utf8');
		const references = new Map(
			[...declaredValues(css)].filter(([name]) => name.startsWith('--ob-ref-')),
		);
		const failures: string[] = [];
		const themes: readonly { readonly selector: ThemeSelector; readonly dark: boolean }[] = [
			{ selector: "[data-theme='light']", dark: false },
			{ selector: "[data-theme='dark']", dark: true },
		];
		for (const { selector, dark } of themes) {
			const system = declaredValues(ruleBody(css, selector));
			const vars = new Map<string, string>([...references, ...system]);
			const get = (role: string): ParsedCssColor => {
				const raw = vars.get(`--ob-sys-color-${role}`);
				if (raw === undefined) throw new Error(`Missing --ob-sys-color-${role} in ${selector}`);
				return parseCssColor(raw, vars);
			};
			const canvas = paintOver(get('surface-canvas'), { r: 255, g: 255, b: 255 });
			const raised = paintOver(get('surface-raised'), canvas);
			const overlay = paintOver(get('surface-overlay'), canvas);
			const bases: readonly (readonly [string, RgbColor])[] = [
				['canvas', canvas],
				['raised', raised],
				['overlay', overlay],
			];
			const surfaces: [string, RgbColor][] = [];
			for (const [baseName, base] of bases) {
				surfaces.push([baseName, base]);
				const subtle = paintOver(get('surface-subtle'), base);
				surfaces.push([`${baseName}+subtle`, subtle]);
				for (const state of ['inactive', 'hovered', 'selected', 'pressed', 'selected-hover']) {
					surfaces.push([`${baseName}+${state}`, paintOver(get(`surface-${state}`), base)]);
				}
				if (baseName !== 'overlay') {
					for (const state of ['hovered', 'selected', 'selected-hover']) {
						surfaces.push([
							`${baseName}+subtle+${state}`,
							paintOver(get(`surface-${state}`), subtle),
						]);
					}
				}
				if (dark) {
					const inputBorder = get('border-default');
					surfaces.push([
						`${baseName}+input/30`,
						compositeOpacity(inputBorder.rgb, base, inputBorder.alpha * 0.3),
					]);
				}
				for (const pct of [0.05, 0.1]) {
					const actionPrimary = get('action-primary');
					surfaces.push([
						`${baseName}+primary/${pct * 100}`,
						compositeOpacity(actionPrimary.rgb, base, actionPrimary.alpha * pct),
					]);
				}
				for (const pct of [0.05, 0.08, 0.8]) {
					const actionSecondary = get('action-secondary');
					surfaces.push([
						`${baseName}+secondary/${pct * 100}`,
						compositeOpacity(actionSecondary.rgb, base, actionSecondary.alpha * pct),
					]);
				}
			}
			for (const status of ['info', 'success', 'warning', 'danger']) {
				for (const suffix of ['-surface', '-surface-hover']) {
					const fillSpec = get(`status-${status}${suffix}`);
					for (const [baseName, base] of bases) {
						surfaces.push([`${baseName}+status-${status}${suffix}`, paintOver(fillSpec, base)]);
					}
				}
			}

			for (const [baseName, base] of bases) {
				for (const [label, fill] of [
					[baseName, base],
					[`${baseName}+subtle`, paintOver(get('surface-subtle'), base)],
				] as const) {
					const fg = paintOver(get('text-muted'), fill);
					const ratio = contrastRatio(fg, fill);
					if (ratio < 4.5) {
						failures.push(`${selector}: text-muted on ${label} = ${ratio.toFixed(3)} < 4.5`);
					}
				}
			}

			const foregrounds: readonly (readonly [string, number])[] = [
				['text-primary', 4.5],
				['text-secondary', 4.5],
				['text-link', 4.5],
				['text-link-hover', 4.5],
				['text-link-focus', 4.5],
				['text-link-pressed', 4.5],
				['status-unread', 3.0],
				...(['info', 'success', 'warning', 'danger'] as const).flatMap(
					(status): readonly (readonly [string, number])[] => [
						[`status-${status}`, 4.5],
						[`status-${status}-text`, 4.5],
						[`status-${status}-text-hover`, 4.5],
						[`status-${status}-text-pressed`, 4.5],
					],
				),
			];

			for (const [surfaceName, fill] of surfaces) {
				const primaryRatio = contrastRatio(paintOver(get('text-primary'), fill), fill);
				const secondaryRatio = contrastRatio(paintOver(get('text-secondary'), fill), fill);
				if (primaryRatio < secondaryRatio) {
					failures.push(
						`${selector}: text hierarchy inverted on ${surfaceName} (${primaryRatio.toFixed(3)} < ${secondaryRatio.toFixed(3)})`,
					);
				}
				for (const [role, floor] of foregrounds) {
					if (
						(surfaceName.includes('input/') || surfaceName.includes('primary/')) &&
						role !== 'text-primary' &&
						role !== 'text-secondary'
					) {
						continue;
					}
					const fg = paintOver(get(role), fill);
					const ratio = contrastRatio(fg, fill);
					if (ratio < floor) {
						failures.push(
							`${selector}: ${role} on ${surfaceName} = ${ratio.toFixed(3)} < ${floor}`,
						);
					}
				}
			}

			const actionText = get('action-primary-text');
			for (const variant of ['action-primary', 'action-primary-hover', 'action-primary-pressed']) {
				const variantSpec = get(variant);
				for (const [baseName, base] of bases) {
					const fill = paintOver(variantSpec, base);
					const textRatio = contrastRatio(paintOver(actionText, fill), fill);
					if (textRatio < 4.5) {
						failures.push(
							`${selector}: action-primary-text on ${baseName}+${variant} = ${textRatio.toFixed(3)} < 4.5`,
						);
					}
					const boundaryRatio = contrastRatio(fill, base);
					if (boundaryRatio < 3.0) {
						failures.push(
							`${selector}: ${variant} UI on ${baseName} = ${boundaryRatio.toFixed(3)} < 3.0`,
						);
					}
				}
			}

			const inverseBg = get('surface-inverse').rgb;
			const inverseFg = paintOver(get('text-inverse'), inverseBg);
			const invRatio = contrastRatio(inverseFg, inverseBg);
			if (invRatio < 4.5) {
				failures.push(
					`${selector}: text-inverse on surface-inverse = ${invRatio.toFixed(3)} < 4.5`,
				);
			}
			const inv80 = compositeOpacity(inverseFg, inverseBg, 0.8);
			const inv80Ratio = contrastRatio(inv80, inverseBg);
			if (inv80Ratio < 4.5) {
				failures.push(
					`${selector}: text-inverse@80% on surface-inverse = ${inv80Ratio.toFixed(3)} < 4.5`,
				);
			}
			const kbdRatio = contrastRatio(
				inverseFg,
				compositeOpacity(canvas, inverseBg, dark ? 0.1 : 0.15),
			);
			if (kbdRatio < 4.5) {
				failures.push(`${selector}: text-inverse on tooltip kbd = ${kbdRatio.toFixed(3)} < 4.5`);
			}
			const hintRatio = contrastRatio(
				paintOver(get('text-inverse'), get('status-warning').rgb),
				get('status-warning').rgb,
			);
			if (hintRatio < 4.5) {
				failures.push(
					`${selector}: text-inverse on status-warning = ${hintRatio.toFixed(3)} < 4.5`,
				);
			}
			for (const [surfaceName, bgRole] of [
				['text-muted', 'text-muted'],
				['text-secondary', 'text-secondary'],
				['border-focus', 'border-focus'],
			] as const) {
				const bgSpec = paintOver(get(bgRole), canvas);
				const fgSpec = paintOver(get('text-inverse'), bgSpec);
				const ratio = contrastRatio(fgSpec, bgSpec);
				if (ratio < 4.5) {
					failures.push(`${selector}: text-inverse on ${surfaceName} = ${ratio.toFixed(3)} < 4.5`);
				}
			}
		}
		expect(failures).toEqual([]);
	});
});
