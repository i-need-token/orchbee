import { readFile, readdir } from 'node:fs/promises';
import { extname, join, relative, resolve, sep } from 'node:path';
import { parseSync, visitorKeys } from 'oxc-parser';
import { parse as parseCss } from 'postcss';
import { renderDesignTokenInventory } from './design-token-inventory.mjs';
import {
	isCompileTimeTailwindToken,
	parseCustomPropertyDeclarations,
	renderTailwindTokenCompatibility,
	resolveTailwindThemePath,
	tailwindReferenceToken,
} from './tailwind-token-compat.mjs';

const TOKEN_NAME = /^--ob-(?:ref|sys|comp)-[a-z0-9]+(?:-[a-z0-9]+)*$/u;
const CSS_SOURCE_EXTENSION = '.css';
const SCRIPT_SOURCE_EXTENSIONS = new Set(['.js', '.jsx', '.ts', '.tsx']);
const RAW_COLOR = /#[\da-f]{3,8}\b|\b(?:color|hsl|hsla|lab|lch|oklab|oklch|rgb|rgba)\(/iu;
const RAW_DIMENSION =
	/(^|[^\w.-])-?(?:\d+\.?\d*|\.\d+)(?:ch|cm|dvh|em|ex|in|lvh|mm|ms|pc|pt|px|rem|s|svh|turn|vh|vmax|vmin|vw)\b/iu;
const COLOR_PROPERTIES =
	/^(?:background(?:-color)?|border(?:-(?:block|bottom|inline|left|right|top)(?:-color)?|-color)?|caret-color|color|fill|outline-color|stroke)$/u;
const TOKEN_REQUIRED_PROPERTIES =
	/^(?:animation(?:-duration|-timing-function)?|box-shadow|font|font-family|font-size|font-weight|letter-spacing|line-height|opacity|text-decoration(?:-line)?|text-shadow|transition(?:-duration|-timing-function)?|z-index)$/u;
const TYPOGRAPHY_LONGHAND = /^(?:font-family|font-size|font-weight|letter-spacing|line-height)$/u;
const TYPOGRAPHY_REFERENCE_TOKEN = /^--ob-ref-(?:font|letter-spacing|line-height)-/u;
const MOTION_REFERENCE_TOKEN = /^--ob-ref-(?:animation|motion)-/u;
const SYSTEM_TYPOGRAPHY_USAGE = /var\(\s*--ob-sys-typography-[a-z0-9-]+\s*\)/iu;
const SYSTEM_MOTION_DURATION_USAGE = /var\(\s*--ob-sys-motion-duration-[a-z0-9-]+\s*\)/iu;
const SYSTEM_MOTION_EASING_USAGE = /var\(\s*--ob-sys-motion-easing-[a-z0-9-]+\s*\)/iu;
const TAILWIND_DIRECTIVE =
	/@(apply|custom-variant|theme|utility)\b|@import\s+['"](?:shadcn\/tailwind\.css|tailwindcss|tw-animate-css)['"]/iu;
const TAILWIND_PACKAGE =
	/^(?:@tailwindcss\/|tailwind-merge(?:\/|$)|tailwindcss(?:\/|$)|tw-animate-css(?:\/|$))/u;
const CSS_VARIABLE_USAGE = /var\(\s*(--[a-z0-9-]+)/giu;
const CSS_VARIABLE_FALLBACK = /var\(\s*(--ob-[a-z0-9-]+)\s*,/giu;
const SHADCN_COLOR_ALIAS_TARGETS = new Map([
	['accent', '--ob-sys-color-action-secondary-hover'],
	['accent-foreground', '--ob-sys-color-text-primary'],
	['background', '--ob-sys-color-surface-canvas'],
	['border', '--ob-sys-color-border-default'],
	['card', '--ob-sys-color-surface-raised'],
	['card-foreground', '--ob-sys-color-text-primary'],
	['chart-1', '--ob-sys-color-data-1'],
	['chart-2', '--ob-sys-color-data-2'],
	['chart-3', '--ob-sys-color-data-3'],
	['chart-4', '--ob-sys-color-data-4'],
	['chart-5', '--ob-sys-color-data-5'],
	['destructive', '--ob-sys-color-status-danger'],
	['foreground', '--ob-sys-color-text-primary'],
	['input', '--ob-sys-color-border-default'],
	['muted', '--ob-sys-color-surface-subtle'],
	['muted-foreground', '--ob-sys-color-text-muted'],
	['popover', '--ob-sys-color-surface-overlay'],
	['popover-foreground', '--ob-sys-color-text-primary'],
	['primary', '--ob-sys-color-action-primary'],
	['primary-foreground', '--ob-sys-color-action-primary-text'],
	['ring', '--ob-sys-color-border-focus'],
	['secondary', '--ob-sys-color-action-secondary'],
	['secondary-foreground', '--ob-sys-color-action-secondary-text'],
	['sidebar', '--ob-sys-color-surface-subtle'],
	['sidebar-accent', '--ob-sys-color-action-secondary-hover'],
	['sidebar-accent-foreground', '--ob-sys-color-text-primary'],
	['sidebar-border', '--ob-sys-color-border-default'],
	['sidebar-foreground', '--ob-sys-color-text-primary'],
	['sidebar-primary', '--ob-sys-color-action-primary'],
	['sidebar-primary-foreground', '--ob-sys-color-action-primary-text'],
	['sidebar-ring', '--ob-sys-color-border-focus'],
]);
const SHADCN_RADIUS_TARGETS = new Map([
	['2xl', '--ob-ref-radius-ui-2xl'],
	['3xl', '--ob-ref-radius-ui-3xl'],
	['4xl', '--ob-ref-radius-ui-4xl'],
	['lg', '--ob-ref-radius-ui-lg'],
	['md', '--ob-ref-radius-ui-md'],
	['sm', '--ob-ref-radius-ui-sm'],
	['xl', '--ob-ref-radius-ui-xl'],
]);

function lineNumber(source, index) {
	return source.slice(0, index).split('\n').length;
}

function diagnostic(path, source, index, message) {
	return { path, line: lineNumber(source, index), message };
}

function tokenTier(token) {
	return token.split('-')[3];
}

function isAllowedColorKeyword(value) {
	return /^(?:currentcolor|inherit|initial|none|revert|revert-layer|transparent|unset)$/iu.test(
		value.trim(),
	);
}

function isZeroOrVariable(value) {
	return /var\(\s*--ob-[a-z0-9-]+\s*\)/iu.test(value) || /^0(?:\.0+)?$/u.test(value.trim());
}

function cssDeclarations(path, source) {
	const root = parseCss(source, { from: path });
	const declarations = [];
	root.walkDecls((declaration) => {
		declarations.push({
			property: declaration.prop.toLowerCase(),
			value: declaration.value,
			index: declaration.source?.start?.offset ?? 0,
		});
	});
	return declarations;
}

function validateTokenReferences(path, source, registeredTokens) {
	const diagnostics = [];
	for (const match of source.matchAll(CSS_VARIABLE_USAGE)) {
		const token = String(match[1]);
		if (token.startsWith('--ob-') && !registeredTokens.has(token)) {
			diagnostics.push(
				diagnostic(path, source, match.index, `Register unknown design token "${token}".`),
			);
		}
	}
	for (const match of source.matchAll(CSS_VARIABLE_FALLBACK)) {
		diagnostics.push(
			diagnostic(
				path,
				source,
				match.index,
				`Remove the fallback from "${String(match[1])}"; registered tokens are total.`,
			),
		);
	}
	return diagnostics;
}

export function tokenDeclarations(source) {
	const declarations = [];
	parseCss(source).walkDecls((declaration) => {
		if (declaration.prop.startsWith('--')) declarations.push(declaration.prop);
	});
	return declarations;
}

export function validateRegistrySource({ path, source, publicTokens }) {
	const diagnostics = [];
	const declarations = tokenDeclarations(source);
	const registeredTokens = new Set(declarations);
	const referenceTokenCounts = new Map();

	for (const token of declarations) {
		if (!token.startsWith('--ob-ref-')) continue;
		referenceTokenCounts.set(token, (referenceTokenCounts.get(token) ?? 0) + 1);
	}
	for (const [token, count] of referenceTokenCounts) {
		if (count > 1) {
			diagnostics.push(
				diagnostic(
					path,
					source,
					source.indexOf(token),
					`Reference token "${token}" is declared twice.`,
				),
			);
		}
	}

	for (const token of registeredTokens) {
		if (!TOKEN_NAME.test(token)) {
			diagnostics.push(
				diagnostic(path, source, source.indexOf(token), `Rename invalid token "${token}".`),
			);
		}
	}

	for (const match of source.matchAll(CSS_VARIABLE_USAGE)) {
		const ownerLineStart = source.lastIndexOf('\n', match.index) + 1;
		const ownerMatch = /^\s*(--ob-[a-z0-9-]+)\s*:/iu.exec(source.slice(ownerLineStart));
		const referencedToken = String(match[1]);
		if (!registeredTokens.has(referencedToken)) {
			diagnostics.push(
				diagnostic(
					path,
					source,
					match.index,
					`Register unknown design token "${referencedToken}".`,
				),
			);
			continue;
		}
		const ownerToken = ownerMatch?.[1];
		if (ownerToken !== undefined && tokenTier(ownerToken) === 'ref') {
			diagnostics.push(
				diagnostic(path, source, match.index, `Reference token "${ownerToken}" must be concrete.`),
			);
		}
	}

	const registered = [...registeredTokens].toSorted();
	const published = [...publicTokens].toSorted();
	if (JSON.stringify(registered) !== JSON.stringify(published)) {
		diagnostics.push(
			diagnostic(
				path,
				source,
				0,
				'The CSS registry and designTokenNames public inventory must contain the same tokens.',
			),
		);
	}

	diagnostics.push(...validateTokenReferences(path, source, registeredTokens));
	return diagnostics;
}

function declarationMap(declarations) {
	return new Map(declarations.map(({ name, value }) => [name, value]));
}

function duplicateDeclarationNames(declarations) {
	const seen = new Set();
	const duplicates = new Set();
	for (const { name } of declarations) {
		if (seen.has(name)) duplicates.add(name);
		seen.add(name);
	}
	return duplicates;
}

export function validateTailwindCompatibilitySource({
	path,
	source,
	tailwindThemeSource,
	registrySource,
}) {
	const diagnostics = [];
	const tailwindDeclarations = parseCustomPropertyDeclarations(tailwindThemeSource);
	const compatibilityDeclarations = parseCustomPropertyDeclarations(source);
	const registryDeclarations = parseCustomPropertyDeclarations(registrySource);
	const compatibilityByName = declarationMap(compatibilityDeclarations);
	const registryByName = declarationMap(registryDeclarations);

	for (const name of duplicateDeclarationNames(tailwindDeclarations)) {
		diagnostics.push(
			diagnostic(path, source, 0, `Installed Tailwind theme token "${name}" is declared twice.`),
		);
	}
	for (const name of duplicateDeclarationNames(compatibilityDeclarations)) {
		diagnostics.push(
			diagnostic(
				path,
				source,
				source.indexOf(name),
				`Tailwind compatibility token "${name}" is declared twice.`,
			),
		);
	}

	const tailwindNames = tailwindDeclarations.map(({ name }) => name).toSorted();
	const compatibilityNames = compatibilityDeclarations.map(({ name }) => name).toSorted();
	if (JSON.stringify(tailwindNames) !== JSON.stringify(compatibilityNames)) {
		diagnostics.push(
			diagnostic(
				path,
				source,
				0,
				'Tailwind compatibility must enumerate exactly the installed Tailwind theme tokens.',
			),
		);
	}

	for (const { name, value } of tailwindDeclarations) {
		let target;
		try {
			target = tailwindReferenceToken(name);
		} catch (error) {
			diagnostics.push(diagnostic(path, source, 0, error.message));
			continue;
		}

		const compatibilityValue = compatibilityByName.get(name);
		const targetValue = registryByName.get(target);
		if (targetValue === undefined) {
			diagnostics.push(
				diagnostic(path, source, 0, `Register missing Tailwind target token "${target}".`),
			);
			continue;
		}

		if (isCompileTimeTailwindToken(name)) {
			if (compatibilityValue !== value) {
				diagnostics.push(
					diagnostic(
						path,
						source,
						Math.max(source.indexOf(name), 0),
						`Compile-time Tailwind token "${name}" must preserve the installed literal "${value}".`,
					),
				);
			}
			if (targetValue !== value) {
				diagnostics.push(
					diagnostic(
						path,
						source,
						0,
						`Reference token "${target}" must match compile-time Tailwind value "${value}".`,
					),
				);
			}
			continue;
		}

		const expectedValue = `var(${target})`;
		if (compatibilityValue !== expectedValue) {
			diagnostics.push(
				diagnostic(
					path,
					source,
					Math.max(source.indexOf(name), 0),
					`Tailwind token "${name}" must map to "${expectedValue}".`,
				),
			);
		}
	}

	if (source !== renderTailwindTokenCompatibility(tailwindThemeSource)) {
		diagnostics.push(
			diagnostic(
				path,
				source,
				0,
				'Tailwind compatibility output is stale; run pnpm generate:tailwind-token-compat.',
			),
		);
	}

	return diagnostics;
}

function containerDeclarations(path, source, container, diagnostics, label) {
	const declarations = new Map();
	for (const node of container.nodes ?? []) {
		if (node.type !== 'decl') continue;
		if (declarations.has(node.prop)) {
			diagnostics.push(
				diagnostic(
					path,
					source,
					node.source?.start?.offset ?? 0,
					`${label} declares "${node.prop}" more than once.`,
				),
			);
		}
		declarations.set(node.prop, node);
	}
	return declarations;
}

function validateExactMappings({ path, source, container, expected, diagnostics, label }) {
	const declarations = containerDeclarations(path, source, container, diagnostics, label);
	for (const [property, target] of expected) {
		const declaration = declarations.get(property);
		if (declaration === undefined) {
			diagnostics.push(
				diagnostic(
					path,
					source,
					container.source?.start?.offset ?? 0,
					`${label} must declare "${property}" as "var(${target})".`,
				),
			);
			continue;
		}
		if (declaration.value !== `var(${target})`) {
			diagnostics.push(
				diagnostic(
					path,
					source,
					declaration.source?.start?.offset ?? 0,
					`${label} must map "${property}" to "var(${target})".`,
				),
			);
		}
	}
	for (const [property, declaration] of declarations) {
		if (expected.has(property)) continue;
		diagnostics.push(
			diagnostic(
				path,
				source,
				declaration.source?.start?.offset ?? 0,
				`${label} contains unexpected mapping "${property}".`,
			),
		);
	}
}

export function validateShadcnCompatibilitySource({ path, source, registeredTokens }) {
	const diagnostics = validateTokenReferences(path, source, registeredTokens);
	const root = parseCss(source, { from: path });

	const semanticRules = [];
	root.walkRules((rule) => {
		const selectors = rule.selector
			.split(',')
			.map((selector) => selector.trim())
			.toSorted();
		if (JSON.stringify(selectors) === JSON.stringify(['.dark', ':root'])) semanticRules.push(rule);
	});
	if (semanticRules.length !== 1) {
		diagnostics.push(
			diagnostic(
				path,
				source,
				0,
				'Shadcn semantic aliases require exactly one shared ":root, .dark" mapping rule.',
			),
		);
	} else {
		const expectedAliases = new Map(
			[...SHADCN_COLOR_ALIAS_TARGETS].map(([alias, target]) => [`--${alias}`, target]),
		);
		expectedAliases.set('--radius', '--ob-ref-radius-ui-lg');
		validateExactMappings({
			path,
			source,
			container: semanticRules[0],
			expected: expectedAliases,
			diagnostics,
			label: 'Shadcn semantic aliases',
		});
	}

	const themeRules = [];
	root.walkAtRules('theme', (atRule) => {
		themeRules.push(atRule);
	});
	if (themeRules.length !== 1 || themeRules[0]?.params.trim() !== 'inline') {
		diagnostics.push(
			diagnostic(
				path,
				source,
				0,
				'Shadcn compatibility requires exactly one "@theme inline" rule.',
			),
		);
	} else {
		const expectedThemeMappings = new Map(
			[...SHADCN_COLOR_ALIAS_TARGETS.keys()].map((alias) => [`--color-${alias}`, `--${alias}`]),
		);
		for (const [radius, target] of SHADCN_RADIUS_TARGETS) {
			expectedThemeMappings.set(`--radius-${radius}`, target);
		}
		validateExactMappings({
			path,
			source,
			container: themeRules[0],
			expected: expectedThemeMappings,
			diagnostics,
			label: 'Shadcn Tailwind theme mappings',
		});
	}

	const expectedAliases = new Set([
		...Array.from(SHADCN_COLOR_ALIAS_TARGETS.keys(), (alias) => `--${alias}`),
		'--radius',
	]);
	const expectedThemeProperties = new Set([
		...Array.from(SHADCN_COLOR_ALIAS_TARGETS.keys(), (alias) => `--color-${alias}`),
		...Array.from(SHADCN_RADIUS_TARGETS.keys(), (radius) => `--radius-${radius}`),
	]);
	const semanticRule = semanticRules.length === 1 ? semanticRules[0] : undefined;
	const themeRule =
		themeRules.length === 1 && themeRules[0]?.params.trim() === 'inline'
			? themeRules[0]
			: undefined;
	root.walkDecls((declaration) => {
		if (expectedAliases.has(declaration.prop) && declaration.parent !== semanticRule) {
			diagnostics.push(
				diagnostic(
					path,
					source,
					declaration.source?.start?.offset ?? 0,
					`Shadcn semantic alias "${declaration.prop}" may only be declared in the shared mapping rule.`,
				),
			);
		}
		if (expectedThemeProperties.has(declaration.prop) && declaration.parent !== themeRule) {
			diagnostics.push(
				diagnostic(
					path,
					source,
					declaration.source?.start?.offset ?? 0,
					`Shadcn Tailwind theme property "${declaration.prop}" may only be declared in "@theme inline".`,
				),
			);
		}
	});

	return diagnostics;
}

export function validateCssSource({
	path,
	source,
	registeredTokens,
	breakpointValues,
	isRegistry = false,
	isShadcnCompatibility = false,
	isTailwindCompatibility = false,
	publicTokens = registeredTokens,
}) {
	if (isRegistry) return validateRegistrySource({ path, source, publicTokens });
	if (isShadcnCompatibility) {
		return validateShadcnCompatibilitySource({ path, source, registeredTokens });
	}

	const diagnostics = validateTokenReferences(path, source, registeredTokens);
	if (isTailwindCompatibility) return diagnostics;
	const root = parseCss(source, { from: path });

	if (!path.endsWith('.module.css')) {
		diagnostics.push(
			diagnostic(path, source, 0, 'Rename authored component styles to "*.module.css".'),
		);
	}
	const tailwindMatch = TAILWIND_DIRECTIVE.exec(source);
	if (tailwindMatch !== null) {
		diagnostics.push(
			diagnostic(
				path,
				source,
				tailwindMatch.index,
				'Tailwind directives are limited to @orchbee/ui.',
			),
		);
	}

	root.walkDecls((declaration) => {
		if (!declaration.prop.startsWith('--')) return;
		diagnostics.push(
			diagnostic(
				path,
				source,
				declaration.source?.start?.offset ?? 0,
				`Move custom property "${declaration.prop}" into the central token registry.`,
			),
		);
	});
	for (const match of source.matchAll(CSS_VARIABLE_USAGE)) {
		const token = String(match[1]);
		if (!token.startsWith('--ob-')) {
			diagnostics.push(
				diagnostic(
					path,
					source,
					match.index,
					`Use a registered --ob-* token instead of "${token}".`,
				),
			);
		}
		if (token.startsWith('--ob-ref-color-')) {
			diagnostics.push(
				diagnostic(
					path,
					source,
					match.index,
					`Use a theme-aware --ob-sys-color-* token instead of "${token}".`,
				),
			);
		}
		if (TYPOGRAPHY_REFERENCE_TOKEN.test(token)) {
			diagnostics.push(
				diagnostic(
					path,
					source,
					match.index,
					`Use a composite --ob-sys-typography-* token instead of "${token}".`,
				),
			);
		}
		if (MOTION_REFERENCE_TOKEN.test(token)) {
			diagnostics.push(
				diagnostic(
					path,
					source,
					match.index,
					`Use semantic --ob-sys-motion-* tokens instead of "${token}".`,
				),
			);
		}
	}

	for (const declaration of cssDeclarations(path, source)) {
		const { property, value, index } = declaration;
		if (RAW_COLOR.test(value)) {
			diagnostics.push(
				diagnostic(
					path,
					source,
					index,
					`Replace the raw color in "${property}" with a system token.`,
				),
			);
		}
		if (RAW_DIMENSION.test(value)) {
			diagnostics.push(
				diagnostic(path, source, index, `Replace the raw dimension in "${property}" with a token.`),
			);
		}
		if (
			COLOR_PROPERTIES.test(property) &&
			!isAllowedColorKeyword(value) &&
			!value.includes('var(--ob-')
		) {
			diagnostics.push(
				diagnostic(path, source, index, `Property "${property}" must use a system color token.`),
			);
		}
		if (TOKEN_REQUIRED_PROPERTIES.test(property) && !isZeroOrVariable(value) && value !== 'none') {
			diagnostics.push(
				diagnostic(path, source, index, `Property "${property}" must use a registered token.`),
			);
		}
		if (property === 'font' && !SYSTEM_TYPOGRAPHY_USAGE.test(value)) {
			diagnostics.push(
				diagnostic(path, source, index, 'Property "font" must use a --ob-sys-typography-* token.'),
			);
		}
		if (TYPOGRAPHY_LONGHAND.test(property)) {
			diagnostics.push(
				diagnostic(
					path,
					source,
					index,
					`Replace "${property}" with the font shorthand and a --ob-sys-typography-* token.`,
				),
			);
		}
		if (
			(property === 'animation-duration' || property === 'transition-duration') &&
			!SYSTEM_MOTION_DURATION_USAGE.test(value)
		) {
			diagnostics.push(
				diagnostic(
					path,
					source,
					index,
					`Property "${property}" must use a semantic duration token.`,
				),
			);
		}
		if (
			(property === 'animation-timing-function' || property === 'transition-timing-function') &&
			!SYSTEM_MOTION_EASING_USAGE.test(value)
		) {
			diagnostics.push(
				diagnostic(path, source, index, `Property "${property}" must use a semantic easing token.`),
			);
		}
		if ((property === 'animation' || property === 'transition') && value !== 'none') {
			if (!SYSTEM_MOTION_DURATION_USAGE.test(value)) {
				diagnostics.push(
					diagnostic(
						path,
						source,
						index,
						`Property "${property}" must use a semantic duration token.`,
					),
				);
			}
			if (!SYSTEM_MOTION_EASING_USAGE.test(value)) {
				diagnostics.push(
					diagnostic(
						path,
						source,
						index,
						`Property "${property}" must use a semantic easing token.`,
					),
				);
			}
		}
	}

	root.walkAtRules((atRule) => {
		if (atRule.name !== 'media' && atRule.name !== 'container') return;
		for (const dimension of atRule.params.matchAll(/(?:\d+\.?\d*|\.\d+)(?:px|rem)/giu)) {
			if (!breakpointValues.has(String(dimension[0]))) {
				diagnostics.push(
					diagnostic(
						path,
						source,
						atRule.source?.start?.offset ?? 0,
						`Use an exported responsiveBreakpoints value instead of "${String(dimension[0])}".`,
					),
				);
			}
		}
	});

	return diagnostics;
}

function walkScriptAst(node, visit) {
	if (node === null || typeof node !== 'object' || typeof node.type !== 'string') return;
	visit(node);
	for (const key of visitorKeys[node.type] ?? []) {
		const value = node[key];
		if (Array.isArray(value)) {
			for (const child of value) walkScriptAst(child, visit);
		} else {
			walkScriptAst(value, visit);
		}
	}
}

function scriptLanguage(path) {
	if (path.endsWith('.tsx')) return 'tsx';
	if (path.endsWith('.jsx')) return 'jsx';
	if (path.endsWith('.ts')) return 'ts';
	return 'js';
}

function unwrapExpression(expression) {
	let current = expression;
	while (
		current?.type === 'ChainExpression' ||
		current?.type === 'ParenthesizedExpression' ||
		current?.type === 'TSAsExpression' ||
		current?.type === 'TSInstantiationExpression' ||
		current?.type === 'TSNonNullExpression' ||
		current?.type === 'TSSatisfiesExpression' ||
		current?.type === 'TSTypeAssertion'
	) {
		current = current.expression;
	}
	return current;
}

function staticPropertyName(property) {
	if (property?.key?.type === 'Literal' && typeof property.key.value === 'string') {
		return property.key.value;
	}
	if (!property?.computed && property?.key?.type === 'Identifier') return property.key.name;
	return undefined;
}

const SCRIPT_FUNCTION_NODES = new Set([
	'ArrowFunctionExpression',
	'FunctionDeclaration',
	'FunctionExpression',
]);
const SCRIPT_LEXICAL_SCOPE_NODES = new Set([
	'BlockStatement',
	'CatchClause',
	'ClassBody',
	'ForInStatement',
	'ForOfStatement',
	'ForStatement',
	'StaticBlock',
	'SwitchStatement',
]);

function bindingIdentifiers(pattern, identifiers = []) {
	const candidate = unwrapExpression(pattern);
	if (candidate?.type === 'Identifier') {
		identifiers.push(candidate);
		return identifiers;
	}
	if (candidate?.type === 'AssignmentPattern') {
		return bindingIdentifiers(candidate.left, identifiers);
	}
	if (candidate?.type === 'RestElement') {
		return bindingIdentifiers(candidate.argument, identifiers);
	}
	if (candidate?.type === 'ArrayPattern') {
		for (const element of candidate.elements) bindingIdentifiers(element, identifiers);
		return identifiers;
	}
	if (candidate?.type === 'ObjectPattern') {
		for (const property of candidate.properties) {
			if (property.type === 'Property') bindingIdentifiers(property.value, identifiers);
			if (property.type === 'RestElement') bindingIdentifiers(property.argument, identifiers);
		}
	}
	return identifiers;
}

function createScriptScope(node, parent, kind) {
	return { node, parent, kind, bindings: new Map() };
}

function addScriptBinding(scope, identifier, kind, initializer) {
	const binding = {
		name: identifier.name,
		kind,
		initializer,
		declarationStart: identifier.start,
		written: false,
	};
	const existing = scope.bindings.get(identifier.name) ?? [];
	existing.push(binding);
	scope.bindings.set(identifier.name, existing);
	return binding;
}

function nearestScriptVariableScope(scope) {
	let candidate = scope;
	while (candidate.kind !== 'function' && candidate.kind !== 'program') {
		candidate = candidate.parent;
	}
	return candidate;
}

function scriptAnalysisContext(program) {
	const nodeScopes = new WeakMap();

	function visit(node, currentScope) {
		if (node === null || typeof node !== 'object' || typeof node.type !== 'string') return;
		let scope = currentScope;
		if (node.type === 'Program') {
			scope = createScriptScope(node, undefined, 'program');
		} else {
			if (node.type === 'FunctionDeclaration' && node.id?.type === 'Identifier') {
				addScriptBinding(currentScope, node.id, 'other', undefined);
			}
			if (node.type === 'ClassDeclaration' && node.id?.type === 'Identifier') {
				addScriptBinding(currentScope, node.id, 'other', undefined);
			}
			if (SCRIPT_FUNCTION_NODES.has(node.type)) {
				scope = createScriptScope(node, currentScope, 'function');
			} else if (SCRIPT_LEXICAL_SCOPE_NODES.has(node.type)) {
				scope = createScriptScope(node, currentScope, 'lexical');
			}
		}
		nodeScopes.set(node, scope);

		if (node.type === 'ImportDeclaration') {
			const kind =
				typeof node.source?.value === 'string' && node.source.value.endsWith('.module.css')
					? 'css-module'
					: 'other';
			for (const specifier of node.specifiers) {
				addScriptBinding(scope, specifier.local, kind, undefined);
			}
		}
		if (SCRIPT_FUNCTION_NODES.has(node.type)) {
			if (node.type === 'FunctionExpression' && node.id?.type === 'Identifier') {
				addScriptBinding(scope, node.id, 'other', undefined);
			}
			for (const parameter of node.params) {
				for (const identifier of bindingIdentifiers(parameter)) {
					addScriptBinding(scope, identifier, 'other', undefined);
				}
			}
		}
		if (node.type === 'CatchClause') {
			for (const identifier of bindingIdentifiers(node.param)) {
				addScriptBinding(scope, identifier, 'other', undefined);
			}
		}
		if (node.type === 'VariableDeclaration') {
			const declarationScope = node.kind === 'var' ? nearestScriptVariableScope(scope) : scope;
			for (const declaration of node.declarations) {
				for (const identifier of bindingIdentifiers(declaration.id)) {
					addScriptBinding(
						declarationScope,
						identifier,
						node.kind === 'const' && declaration.id.type === 'Identifier' ? 'const' : 'other',
						declaration.init ?? undefined,
					);
				}
			}
		}

		for (const key of visitorKeys[node.type] ?? []) {
			const value = node[key];
			if (Array.isArray(value)) {
				for (const child of value) visit(child, scope);
			} else {
				visit(value, scope);
			}
		}
	}

	visit(program, undefined);

	function resolveBinding(identifier) {
		let scope = nodeScopes.get(identifier);
		while (scope !== undefined) {
			const bindings = scope.bindings.get(identifier.name);
			if (bindings !== undefined) {
				if (bindings.length !== 1) return undefined;
				const binding = bindings[0];
				if (
					binding.kind !== 'css-module' &&
					binding.kind !== 'other' &&
					binding.declarationStart > identifier.start
				) {
					return undefined;
				}
				return binding;
			}
			scope = scope.parent;
		}
		return undefined;
	}

	walkScriptAst(program, (node) => {
		let assigned = [];
		if (node.type === 'AssignmentExpression') assigned = bindingIdentifiers(node.left);
		if (node.type === 'UpdateExpression') assigned = bindingIdentifiers(node.argument);
		if (
			(node.type === 'ForInStatement' || node.type === 'ForOfStatement') &&
			node.left?.type !== 'VariableDeclaration'
		) {
			assigned = bindingIdentifiers(node.left);
		}
		for (const identifier of assigned) {
			const binding = resolveBinding(identifier);
			if (binding !== undefined) binding.written = true;
		}
	});

	return { resolveBinding };
}

function isCssModuleReference(expression, context) {
	const candidate = unwrapExpression(expression);
	return (
		candidate?.type === 'MemberExpression' &&
		candidate.object?.type === 'Identifier' &&
		context.resolveBinding(candidate.object)?.kind === 'css-module'
	);
}

function isEmptyClassValue(expression) {
	const candidate = unwrapExpression(expression);
	return (
		(candidate?.type === 'Literal' && (candidate.value === false || candidate.value === null)) ||
		(candidate?.type === 'Identifier' && candidate.name === 'undefined')
	);
}

function isAllowedClassArgument(expression, context, seenVariables) {
	const candidate = unwrapExpression(expression);
	if (candidate?.type === 'ObjectExpression') {
		return candidate.properties.every((property) => {
			if (property.type !== 'Property' || !property.computed) return false;
			return isAllowedClassExpression(property.key, context, seenVariables);
		});
	}
	if (candidate?.type === 'ArrayExpression') {
		return candidate.elements.every(
			(element) => element === null || isAllowedClassArgument(element, context, seenVariables),
		);
	}
	if (candidate?.type === 'SpreadElement') {
		return isAllowedClassArgument(candidate.argument, context, seenVariables);
	}
	return (
		isEmptyClassValue(candidate) || isAllowedClassExpression(candidate, context, seenVariables)
	);
}

function isAllowedClassExpression(expression, context, seenBindings = new Set()) {
	const candidate = unwrapExpression(expression);
	if (isCssModuleReference(candidate, context)) return true;
	if (candidate?.type === 'Identifier') {
		const binding = context.resolveBinding(candidate);
		if (
			binding?.kind !== 'const' ||
			binding.initializer === undefined ||
			binding.written ||
			seenBindings.has(binding)
		) {
			return false;
		}
		return isAllowedClassExpression(
			binding.initializer,
			context,
			new Set([...seenBindings, binding]),
		);
	}
	if (candidate?.type === 'ConditionalExpression') {
		return (
			isAllowedClassArgument(candidate.consequent, context, seenBindings) &&
			isAllowedClassArgument(candidate.alternate, context, seenBindings)
		);
	}
	if (candidate?.type === 'LogicalExpression') {
		if (candidate.operator === '&&') {
			return isAllowedClassArgument(candidate.right, context, seenBindings);
		}
		return (
			isAllowedClassArgument(candidate.left, context, seenBindings) &&
			isAllowedClassArgument(candidate.right, context, seenBindings)
		);
	}
	if (candidate?.type === 'CallExpression' && candidate.arguments.length > 0) {
		return candidate.arguments.every((argument) =>
			isAllowedClassArgument(argument, context, seenBindings),
		);
	}
	return false;
}

function isAllowedInlineStyle(expression, registeredTokens) {
	const candidate = unwrapExpression(expression);
	if (candidate?.type !== 'ObjectExpression') return false;
	return candidate.properties.every((property) => {
		if (property.type !== 'Property') return false;
		const propertyName = staticPropertyName(property);
		return propertyName?.startsWith('--ob-comp-') === true && registeredTokens.has(propertyName);
	});
}

function resolvedObjectExpression(expression) {
	const candidate = unwrapExpression(expression);
	if (candidate?.type === 'ObjectExpression') return candidate;
	return undefined;
}

function tailwindModuleRequest(node) {
	if (
		(node.type === 'ImportDeclaration' ||
			node.type === 'ExportAllDeclaration' ||
			node.type === 'ExportNamedDeclaration') &&
		typeof node.source?.value === 'string'
	) {
		return node.source.value;
	}
	if (node.type === 'ImportExpression' && typeof node.source?.value === 'string') {
		return node.source.value;
	}
	if (
		node.type === 'CallExpression' &&
		node.callee?.type === 'Identifier' &&
		node.callee.name === 'require' &&
		typeof node.arguments[0]?.value === 'string'
	) {
		return node.arguments[0].value;
	}
	return undefined;
}

export function validateScriptSource({
	path,
	source,
	registeredTokens,
	isTailwindCompatibility = false,
}) {
	const diagnostics = [];
	if (isTailwindCompatibility) return diagnostics;
	const parseResult = parseSync(path, source, {
		lang: scriptLanguage(path),
		range: true,
		sourceType: 'unambiguous',
	});
	for (const error of parseResult.errors) {
		if (error.severity !== 'Error') continue;
		const index = error.labels[0]?.start ?? 0;
		diagnostics.push(diagnostic(path, source, index, `Parse the source: ${error.message}`));
	}
	if (parseResult.errors.some((error) => error.severity === 'Error')) return diagnostics;

	const context = scriptAnalysisContext(parseResult.program);
	walkScriptAst(parseResult.program, (node) => {
		const moduleRequest = tailwindModuleRequest(node);
		if (moduleRequest !== undefined && TAILWIND_PACKAGE.test(moduleRequest)) {
			diagnostics.push(
				diagnostic(
					path,
					source,
					node.start,
					'Tailwind dependencies are limited to the checked-in shadcn compatibility boundary.',
				),
			);
		}
		if (node.type === 'JSXAttribute' && node.name?.name === 'className') {
			const expression =
				node.value?.type === 'JSXExpressionContainer' ? node.value.expression : node.value;
			if (!isAllowedClassExpression(expression, context)) {
				diagnostics.push(
					diagnostic(
						path,
						source,
						node.start,
						'Use a CSS Module reference instead of a string className.',
					),
				);
			}
		}
		if (node.type === 'JSXAttribute' && node.name?.name === 'style') {
			const expression =
				node.value?.type === 'JSXExpressionContainer' ? node.value.expression : undefined;
			if (!isAllowedInlineStyle(expression, registeredTokens)) {
				diagnostics.push(
					diagnostic(
						path,
						source,
						node.start,
						'Inline style must be an object containing only registered --ob-comp-* properties.',
					),
				);
			}
		}
		if (node.type === 'JSXSpreadAttribute') {
			const object = resolvedObjectExpression(node.argument);
			if (object === undefined) {
				diagnostics.push(
					diagnostic(
						path,
						source,
						node.start,
						'JSX attribute spreads must resolve to a local object without styling escape hatches.',
					),
				);
				return;
			}
			for (const property of object.properties) {
				if (property.type !== 'Property') {
					diagnostics.push(
						diagnostic(
							path,
							source,
							node.start,
							'JSX attribute spreads must resolve to a local object without styling escape hatches.',
						),
					);
					continue;
				}
				const propertyName = staticPropertyName(property);
				if (propertyName === undefined) {
					diagnostics.push(
						diagnostic(
							path,
							source,
							node.start,
							'JSX attribute spreads must use statically named properties.',
						),
					);
					continue;
				}
				if (propertyName === 'className' && !isAllowedClassExpression(property.value, context)) {
					diagnostics.push(
						diagnostic(
							path,
							source,
							node.start,
							'Use a CSS Module reference instead of a string className.',
						),
					);
				}
				if (propertyName === 'style' && !isAllowedInlineStyle(property.value, registeredTokens)) {
					diagnostics.push(
						diagnostic(
							path,
							source,
							node.start,
							'Inline style must be an object containing only registered --ob-comp-* properties.',
						),
					);
				}
			}
		}
	});

	return diagnostics;
}

async function collectSourceFiles(directory) {
	const entries = await readdir(directory, { withFileTypes: true });
	const files = await Promise.all(
		entries.map(async (entry) => {
			const path = join(directory, entry.name);
			return entry.isDirectory() ? collectSourceFiles(path) : [path];
		}),
	);
	return files.flat();
}

async function workspaceSourceFiles(repositoryRoot) {
	const collections = ['apps', 'packages'];
	const collectionFiles = await Promise.all(
		collections.map(async (collection) => {
			const collectionRoot = join(repositoryRoot, collection);
			const workspaces = await readdir(collectionRoot, { withFileTypes: true });
			const workspaceFiles = await Promise.all(
				workspaces.map(async (workspace) => {
					if (!workspace.isDirectory()) return [];
					const sourceRoot = join(collectionRoot, workspace.name, 'src');
					try {
						return await collectSourceFiles(sourceRoot);
					} catch (error) {
						if (error?.code !== 'ENOENT') throw error;
						return [];
					}
				}),
			);
			return workspaceFiles.flat();
		}),
	);
	return collectionFiles.flat();
}

function parsePublicTokens(source) {
	return new Set(Array.from(source.matchAll(/'(--ob-[a-z0-9-]+)'/gu), (match) => String(match[1])));
}

function parseBreakpointValues(source) {
	const block = /export const responsiveBreakpoints = \{(?<body>[\s\S]*?)\} as const;/u.exec(
		source,
	);
	if (block?.groups?.body === undefined) return new Set();
	return new Set(
		Array.from(block.groups.body.matchAll(/:\s*'([^']+)'/gu), (match) => String(match[1])),
	);
}

function normalizedRelative(repositoryRoot, path) {
	return relative(repositoryRoot, path).split(sep).join('/');
}

export async function checkStyleContract(repositoryRoot) {
	const registryPath = join(repositoryRoot, 'packages/design-tokens/src/tokens.css');
	const tokenInventoryPath = join(
		repositoryRoot,
		'packages/design-tokens/src/design-token-names.generated.ts',
	);
	const tailwindCompatibilityPath = join(
		repositoryRoot,
		'packages/design-tokens/src/tailwind-compat.css',
	);
	const tailwindThemePath = resolveTailwindThemePath(repositoryRoot);
	const [
		registrySource,
		tokenInventorySource,
		tailwindCompatibilitySource,
		tailwindThemeSource,
		sourceFiles,
	] = await Promise.all([
		readFile(registryPath, 'utf8'),
		readFile(tokenInventoryPath, 'utf8'),
		readFile(tailwindCompatibilityPath, 'utf8'),
		readFile(tailwindThemePath, 'utf8'),
		workspaceSourceFiles(repositoryRoot),
	]);
	const publicTokens = parsePublicTokens(tokenInventorySource);
	const registeredTokens = new Set(tokenDeclarations(registrySource));
	const breakpointValues = parseBreakpointValues(tokenInventorySource);
	const diagnostics = validateRegistrySource({
		path: normalizedRelative(repositoryRoot, registryPath),
		source: registrySource,
		publicTokens,
	});
	if (tokenInventorySource !== renderDesignTokenInventory(registrySource)) {
		diagnostics.push(
			diagnostic(
				normalizedRelative(repositoryRoot, tokenInventoryPath),
				tokenInventorySource,
				0,
				'The generated design-token inventory is stale; run pnpm generate:design-token-inventory.',
			),
		);
	}
	diagnostics.push(
		...validateTailwindCompatibilitySource({
			path: normalizedRelative(repositoryRoot, tailwindCompatibilityPath),
			source: tailwindCompatibilitySource,
			tailwindThemeSource,
			registrySource,
		}),
	);

	const sourceDiagnostics = await Promise.all(
		sourceFiles.map(async (sourcePath) => {
			const extension = extname(sourcePath);
			if (extension !== CSS_SOURCE_EXTENSION && !SCRIPT_SOURCE_EXTENSIONS.has(extension)) return [];
			const path = normalizedRelative(repositoryRoot, sourcePath);
			const source = await readFile(sourcePath, 'utf8');
			if (extension === CSS_SOURCE_EXTENSION) {
				return validateCssSource({
					path,
					source,
					registeredTokens,
					breakpointValues,
					isRegistry: sourcePath === registryPath,
					isShadcnCompatibility: path === 'packages/ui/src/styles.css',
					isTailwindCompatibility: path === 'packages/design-tokens/src/tailwind-compat.css',
					publicTokens,
				});
			}
			const isGeneratedShadcn = path.startsWith('packages/ui/src/components/ui/');
			const isShadcnUtility = path === 'packages/ui/src/lib/utils.ts';
			return validateScriptSource({
				path,
				source,
				registeredTokens,
				isTailwindCompatibility: isGeneratedShadcn || isShadcnUtility,
			});
		}),
	);
	diagnostics.push(...sourceDiagnostics.flat());

	return diagnostics;
}

export function formatStyleDiagnostics(diagnostics) {
	return diagnostics.map(({ path, line, message }) => `${path}:${line}: ${message}`).join('\n');
}

export function repositoryRootFrom(importMetaDirectory) {
	return resolve(importMetaDirectory, '..');
}
