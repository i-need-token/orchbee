// Generates the file-icon lookup table from the pinned upstream inputs in src/file-icon/upstream/:
// VS Code's Seti icon theme, VS Code's built-in language associations, and the Seti palette that
// the design-token registry declares. `--check` compares instead of writing, so a stale table or an
// upstream change the lookup cannot express fails the package test.
import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';

const packageRoot = resolve(import.meta.dirname, '..');
const iconDirectory = resolve(packageRoot, 'src/file-icon');
const themePath = resolve(iconDirectory, 'upstream/vs-seti-icon-theme.json');
const languagesPath = resolve(iconDirectory, 'upstream/vscode-language-associations.json');
const tokensPath = resolve(packageRoot, '../design-tokens/src/tokens.css');
const outputPath = resolve(iconDirectory, 'file-icon-table.generated.ts');
// The repository formatter's print width, which the generated source keeps to.
const printWidth = 100;

const unexpected = process.argv.slice(2).filter((argument) => argument !== '--check');
if (unexpected.length > 0) throw new Error(`Unsupported arguments: ${unexpected.join(', ')}`);

const source = renderTable(
	JSON.parse(await readFile(themePath, 'utf8')),
	JSON.parse(await readFile(languagesPath, 'utf8')).languages,
	setiPalette(await readFile(tokensPath, 'utf8')),
);

if (process.argv.includes('--check')) {
	if ((await readFile(outputPath, 'utf8')) !== source) {
		throw new Error(
			'src/file-icon/file-icon-table.generated.ts is stale; run pnpm --filter @orchbee/ui file-icons:generate',
		);
	}
} else {
	await writeFile(outputPath, source);
}

function fail(message) {
	throw new Error(`File icons: ${message}`);
}

/** The Seti hues by their dark and light values, as the registry's reference tokens declare them. */
function setiPalette(tokens) {
	const dark = new Map();
	const light = new Map();
	for (const [, name, value] of tokens.matchAll(
		/--ob-ref-color-seti-([a-z-]+):\s*(#[0-9a-f]{6});/gu,
	)) {
		if (name.endsWith('-on-light')) light.set(name.slice(0, -'-on-light'.length), value);
		else dark.set(name, value);
	}
	if (dark.size === 0) fail('the registry declares no --ob-ref-color-seti-* palette');
	for (const name of dark.keys()) {
		if (!light.has(name)) fail(`--ob-ref-color-seti-${name} has no -on-light shade`);
	}
	return { dark, light };
}

function hueOf(color, shades, definition) {
	for (const [name, value] of shades) if (value === color) return name;
	return fail(`${definition} uses ${color}, which no --ob-ref-color-seti-* token declares`);
}

function glyphOf(definition, id) {
	const match = /^\\([0-9a-f]{1,6})$/iu.exec(definition.fontCharacter ?? '');
	if (match === null) fail(`${id} has no font character`);
	return String.fromCodePoint(Number.parseInt(match[1], 16));
}

/**
 * Checks the shape the lookup relies on: one font at 150%, a default file icon, no folder icons, and
 * a light section that repeats every colored association with its `_light` shade. FileIcon draws
 * the light shades through theme tokens, so a light section that diverged would need a new design.
 */
function checkTheme(theme, palette) {
	const allowed = new Set([
		'information_for_contributors',
		'fonts',
		'iconDefinitions',
		'file',
		'fileExtensions',
		'fileNames',
		'languageIds',
		'light',
		'version',
	]);
	for (const key of Object.keys(theme)) if (!allowed.has(key)) fail(`unexpected theme key ${key}`);
	const [font, ...otherFonts] = theme.fonts;
	if (
		otherFonts.length > 0 ||
		font?.size !== '150%' ||
		font.src?.length !== 1 ||
		font.src[0].path !== './seti.woff' ||
		font.src[0].format !== 'woff'
	) {
		fail('the theme no longer draws one seti.woff font at 150%');
	}
	const definitions = theme.iconDefinitions;
	if (definitions[theme.file] === undefined) fail('the default file icon is undefined');
	const lightKeys = new Set(['file', 'fileExtensions', 'fileNames', 'languageIds']);
	for (const key of Object.keys(theme.light)) {
		if (!lightKeys.has(key)) fail(`unexpected light key ${key}`);
	}
	const lightShade = (id) => {
		const base = definitions[id];
		if (base.fontColor === undefined) return undefined;
		const shade = definitions[`${id}_light`];
		if (shade?.fontCharacter !== base.fontCharacter) fail(`${id}_light changes the glyph`);
		const hue = hueOf(base.fontColor, palette.dark, id);
		if (shade.fontColor !== palette.light.get(hue)) fail(`${id}_light is not the ${hue} shade`);
		return `${id}_light`;
	};
	if (theme.light.file !== lightShade(theme.file)) fail('the light default diverges');
	for (const section of ['fileExtensions', 'fileNames', 'languageIds']) {
		const base = theme[section];
		const shades = theme.light[section];
		for (const [key, id] of Object.entries(base)) {
			if (definitions[id] === undefined) fail(`${section}.${key} names undefined ${id}`);
			if (shades[key] !== lightShade(id)) fail(`light ${section}.${key} diverges`);
			if (section !== 'languageIds' && (key !== key.toLowerCase() || /[\s/]/u.test(key))) {
				fail(`${section}.${key} needs folder or case handling the lookup lacks`);
			}
		}
		for (const key of Object.keys(shades)) {
			if (!(key in base)) fail(`light ${section}.${key} has no dark association`);
		}
	}
}

/**
 * Checks that a language-association glob stays within what resolveFileIcon compiles: `*`, `?`
 * and whole `**` segments. Braces and brackets have no use in the associations yet.
 */
function checkGlob(pattern) {
	if (/[{}[\]]/u.test(pattern) || pattern !== pattern.trim()) {
		fail(`glob ${JSON.stringify(pattern)} needs syntax resolveFileIcon lacks`);
	}
	return pattern;
}

/**
 * VS Code's built-in language of a file, resolved as its registry does: an exact name, else the
 * longest matching glob, else the longest matching extension, each tie going to the later
 * registration. Each association keeps the Seti icon of its language, or null for a language
 * without one, which still hides an association it outranks.
 */
function languageTables(languages, iconOfLanguage) {
	const byName = new Map();
	const byExtension = new Map();
	const globs = [];
	for (const language of languages) {
		const icon = iconOfLanguage(language.id);
		for (const name of language.filenames ?? []) byName.set(name.toLowerCase(), icon);
		for (const extension of language.extensions ?? []) {
			byExtension.set(extension.toLowerCase(), icon);
		}
		for (const glob of language.filenamePatterns ?? []) {
			globs.push({ glob: checkGlob(glob), icon, order: globs.length });
		}
	}
	globs.sort((a, b) => b.glob.length - a.glob.length || b.order - a.order);
	const dotted = [...byExtension].filter(([extension]) => extension.startsWith('.'));
	const suffixes = [...byExtension].filter(([extension]) => !extension.startsWith('.'));
	return { byName, dotted, globs, suffixes };
}

function stringLiteral(value) {
	let text = "'";
	for (const character of value) {
		const code = character.codePointAt(0);
		if (character === "'" || character === '\\') text += `\\${character}`;
		else if (code < 0x20 || code > 0x7e) {
			text += `\\u${code.toString(16).toUpperCase().padStart(4, '0')}`;
		} else text += character;
	}
	return `${text}'`;
}

function byKey([a], [b]) {
	if (a === b) return 0;
	return a < b ? -1 : 1;
}

function renderEntries(entries, render) {
	return entries.map(([key, value]) => `\t[${stringLiteral(key)}, ${render(value)}],\n`).join('');
}

function renderMap(map, render) {
	if (map.size === 0) return 'new Map()';
	return `new Map([\n${renderEntries([...map].toSorted(byKey), render)}])`;
}

function renderLanguageIcon(icon) {
	return icon === null ? 'null' : String(icon);
}

/** A string union as the formatter lays it out: on one line while it fits, else one per line. */
function renderUnion(head, members) {
	const line = `${head} ${members.join(' | ')};`;
	if (line.length <= printWidth) return line;
	return `${head}\n${members.map((member) => `\t| ${member}`).join('\n')};`;
}

function renderTable(theme, languages, palette) {
	checkTheme(theme, palette);
	const definitions = theme.iconDefinitions;
	const used = new Set([
		theme.file,
		...Object.values(theme.fileNames),
		...Object.values(theme.fileExtensions),
		...Object.values(theme.languageIds),
	]);
	const ids = [...used].toSorted();
	const index = new Map(ids.map((id, position) => [id, position]));
	const hues = new Set();
	const icons = ids.map((id) => {
		const definition = definitions[id];
		const hue =
			definition.fontColor === undefined ? null : hueOf(definition.fontColor, palette.dark, id);
		if (hue !== null) hues.add(hue);
		return { glyph: glyphOf(definition, id), hue, id };
	});
	// VS Code lends JSON's icon to JSON with Comments when a theme names only the first.
	const iconOfLanguage = (id) => {
		const icon = theme.languageIds[id] ?? (id === 'jsonc' ? theme.languageIds.json : undefined);
		return icon === undefined ? null : index.get(icon);
	};
	const language = languageTables(languages, iconOfLanguage);
	// An icon without a color takes one from a weaker association. Under the light theme's extra
	// qualifier that cascade matches the dark one only while such icons are named by file names.
	const uncolored = new Set(icons.flatMap(({ hue }, position) => (hue === null ? [position] : [])));
	const reachable = [
		...Object.values(theme.fileExtensions).map((id) => index.get(id)),
		...language.byName.values(),
		...language.globs.map(({ icon }) => icon),
		...language.dotted.map(([, icon]) => icon),
		...language.suffixes.map(([, icon]) => icon),
	];
	if (reachable.some((icon) => uncolored.has(icon))) {
		fail('an extension or language reaches an icon without a color');
	}
	const renderIcon = (id) => String(index.get(id));
	const hueType = renderUnion(
		'export type FileIconHue =',
		[...hues].toSorted().map((hue) => stringLiteral(hue)),
	);
	const renderIconTuple = ({ glyph, hue }) =>
		`[${stringLiteral(glyph)}, ${hue === null ? 'null' : stringLiteral(hue)}]`;
	const iconRows = icons.map((icon) => `\t${renderIconTuple(icon)}, // ${icon.id}\n`).join('');
	const globRows = renderEntries(
		language.globs.map(({ glob, icon }) => [glob, icon]),
		renderLanguageIcon,
	);
	const suffixRows = renderEntries(language.suffixes.toSorted(byKey), renderLanguageIcon);

	return `// Generated by scripts/generate-file-icon-table.mjs from upstream/; do not edit. Regenerate with
// \`pnpm --filter @orchbee/ui file-icons:generate\`.

/** A Seti hue, drawn through its \`--ob-sys-color-file-type-*\` token. */
${hueType}

/** A Seti icon: its glyph, and its hue or null to draw in the surrounding text color. */
export type SetiIcon = readonly [glyph: string, hue: FileIconHue | null];

/** Every icon an association names, at the index the tables below hold. */
export const fileIcons: readonly SetiIcon[] = [
${iconRows}];

/** The icon of a file nothing else names. */
export const defaultFileIcon: SetiIcon = ${renderIconTuple(icons[index.get(theme.file)])};

/** Seti's icons by lowercase file name. */
export const iconsByFileName: ReadonlyMap<string, number> = ${renderMap(new Map(Object.entries(theme.fileNames)), renderIcon)};

/** Seti's icons by lowercase extension, without its leading dot. */
export const iconsByExtension: ReadonlyMap<string, number> = ${renderMap(new Map(Object.entries(theme.fileExtensions)), renderIcon)};

// The language tables hold the Seti icon of the language VS Code gives a file, or null where Seti
// has none for that language.

/** Languages by lowercase file name. */
export const languageIconsByFileName: ReadonlyMap<string, number | null> = ${renderMap(language.byName, renderLanguageIcon)};

/** Language globs, longest first; one that holds a slash matches the whole path. */
export const languageIconsByGlob: readonly (readonly [glob: string, icon: number | null])[] = [
${globRows}];

/** Languages by lowercase extension, leading dot included. */
export const languageIconsByExtension: ReadonlyMap<string, number | null> = ${renderMap(new Map(language.dotted), renderLanguageIcon)};

/** Languages by lowercase name ending, for the extensions declared without a dot. */
export const languageIconsBySuffix: readonly (readonly [end: string, icon: number | null])[] = [
${suffixRows}];
`;
}
