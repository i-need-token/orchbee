import { parse as parseCss } from 'postcss';

function registryDeclarations(registrySource) {
	const root = parseCss(registrySource, { from: 'packages/design-tokens/src/tokens.css' });
	const declarations = [];
	root.walkDecls((declaration) => {
		if (declaration.prop.startsWith('--ob-')) {
			declarations.push({ name: declaration.prop, value: declaration.value });
		}
	});
	return declarations;
}

export function designTokenNamesFromRegistry(registrySource) {
	const tokenNames = registryDeclarations(registrySource).map(({ name }) => name);
	if (tokenNames.length === 0) throw new Error('The design-token registry contains no tokens.');
	return [...new Set(tokenNames)].toSorted();
}

export function responsiveBreakpointsFromRegistry(registrySource) {
	const breakpoints = new Map();
	for (const { name, value } of registryDeclarations(registrySource)) {
		if (!name.startsWith('--ob-ref-breakpoint-')) continue;
		const breakpoint = name.replace('--ob-ref-breakpoint-', '');
		if (breakpoints.has(breakpoint)) {
			throw new Error(`The design-token registry declares breakpoint "${breakpoint}" twice.`);
		}
		breakpoints.set(breakpoint, value);
	}
	if (breakpoints.size === 0) throw new Error('The design-token registry contains no breakpoints.');
	return breakpoints;
}

export function renderDesignTokenInventory(registrySource) {
	const tokenNames = designTokenNamesFromRegistry(registrySource);
	const breakpoints = responsiveBreakpointsFromRegistry(registrySource);
	return [
		'/* Generated from tokens.css by scripts/generate-design-token-inventory.mjs. */',
		'export const designTokenNames = [',
		...tokenNames.map((tokenName) => `\t'${tokenName}',`),
		'] as const;',
		'',
		'export const responsiveBreakpoints = {',
		...Array.from(breakpoints, ([name, value]) => {
			const property = /^[a-z_$][\w$]*$/iu.test(name) ? name : `'${name}'`;
			return `\t${property}: '${value}',`;
		}),
		'} as const;',
		'',
	].join('\n');
}
