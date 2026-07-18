import { createRequire } from 'node:module';
import { resolve } from 'node:path';

const COMPILE_TIME_THEME_TOKEN = /^--(?:breakpoint|container)-/u;

export function parseCustomPropertyDeclarations(source) {
	return Array.from(source.matchAll(/^\s*(--[a-z0-9-]+)\s*:\s*([\s\S]*?);/gimu), (match) => ({
		name: String(match[1]),
		value: String(match[2]).replace(/\s+/gu, ' ').trim(),
		index: match.index,
	}));
}

export function isCompileTimeTailwindToken(name) {
	return COMPILE_TIME_THEME_TOKEN.test(name);
}

export function tailwindReferenceToken(name) {
	if (/^--font-(?:mono|sans|serif)$/u.test(name)) {
		return name.replace('--font-', '--ob-ref-font-family-');
	}
	if (name === '--color-black') return '--ob-ref-color-neutral-1000';
	if (name === '--color-white') return '--ob-ref-color-neutral-0';
	if (name.startsWith('--color-')) return name.replace('--color-', '--ob-ref-color-');
	if (name === '--spacing') return '--ob-ref-space-1';
	if (name.startsWith('--breakpoint-'))
		return name.replace('--breakpoint-', '--ob-ref-breakpoint-');
	if (name.startsWith('--container-')) return name.replace('--container-', '--ob-ref-container-');
	if (/^--text-.+--line-height$/u.test(name)) {
		return name.replace(/^--text-(.+)--line-height$/u, '--ob-ref-font-size-$1-line-height');
	}
	if (name.startsWith('--text-shadow-')) {
		return name.replace('--text-shadow-', '--ob-ref-text-shadow-');
	}
	if (name.startsWith('--text-')) return name.replace('--text-', '--ob-ref-font-size-');
	if (name.startsWith('--font-weight-')) {
		return name.replace('--font-weight-', '--ob-ref-font-weight-');
	}
	if (name.startsWith('--tracking-')) {
		return name.replace('--tracking-', '--ob-ref-letter-spacing-');
	}
	if (name.startsWith('--leading-')) return name.replace('--leading-', '--ob-ref-line-height-');
	if (name === '--radius') return '--ob-ref-radius-sm';
	if (name.startsWith('--radius-')) return name.replace('--radius-', '--ob-ref-radius-');
	if (name === '--shadow') return '--ob-ref-shadow-sm';
	if (name === '--shadow-inner') return '--ob-ref-inset-shadow-legacy';
	if (name.startsWith('--shadow-')) return name.replace('--shadow-', '--ob-ref-shadow-');
	if (name.startsWith('--inset-shadow-')) {
		return name.replace('--inset-shadow-', '--ob-ref-inset-shadow-');
	}
	if (name === '--drop-shadow') return '--ob-ref-drop-shadow-legacy';
	if (name.startsWith('--drop-shadow-')) {
		return name.replace('--drop-shadow-', '--ob-ref-drop-shadow-');
	}
	if (name.startsWith('--ease-')) return name.replace('--ease-', '--ob-ref-motion-easing-');
	if (name.startsWith('--animate-')) return name.replace('--animate-', '--ob-ref-animation-');
	if (name === '--blur') return '--ob-ref-filter-blur-sm';
	if (name.startsWith('--blur-')) return name.replace('--blur-', '--ob-ref-filter-blur-');
	if (name.startsWith('--perspective-')) {
		return name.replace('--perspective-', '--ob-ref-perspective-');
	}
	if (name.startsWith('--aspect-')) return name.replace('--aspect-', '--ob-ref-aspect-ratio-');

	const exceptionalTokens = {
		'--default-transition-duration': '--ob-ref-motion-duration-default',
		'--default-transition-timing-function': '--ob-ref-motion-easing-default',
		'--default-font-family': '--ob-ref-font-family-sans',
		'--default-font-feature-settings': '--ob-ref-font-feature-settings-sans',
		'--default-font-variation-settings': '--ob-ref-font-variation-settings-sans',
		'--default-mono-font-family': '--ob-ref-font-family-mono',
		'--default-mono-font-feature-settings': '--ob-ref-font-feature-settings-mono',
		'--default-mono-font-variation-settings': '--ob-ref-font-variation-settings-mono',
		'--max-width-prose': '--ob-ref-layout-prose-width',
	};
	const target = exceptionalTokens[name];
	if (target !== undefined) return target;

	throw new Error(`Tailwind theme token "${name}" has no Orchbee reference-token mapping.`);
}

export function renderTailwindTokenCompatibility(tailwindThemeSource) {
	const declarations = parseCustomPropertyDeclarations(tailwindThemeSource);
	const lines = declarations.map(({ name, value }) => {
		const target = tailwindReferenceToken(name);
		const compatibilityValue = isCompileTimeTailwindToken(name) ? value : `var(${target})`;
		return `\t${name}: ${compatibilityValue};`;
	});

	return [
		'/* Generated from the installed Tailwind theme by scripts/generate-tailwind-token-compat.mjs. */',
		'@theme inline {',
		...lines,
		'}',
		'',
	].join('\n');
}

export function resolveTailwindThemePath(repositoryRoot) {
	const requireFromUi = createRequire(resolve(repositoryRoot, 'packages/ui/package.json'));
	return requireFromUi.resolve('tailwindcss/theme.css');
}
