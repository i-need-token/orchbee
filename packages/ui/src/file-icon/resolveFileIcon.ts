import {
	defaultFileIcon,
	fileIcons,
	iconsByExtension,
	iconsByFileName,
	languageIconsByExtension,
	languageIconsByFileName,
	languageIconsByGlob,
	languageIconsBySuffix,
	type SetiIcon,
} from './file-icon-table.generated';

// VS Code adds no extension classes past this name length, so a long dotted name cannot explode.
const maxExtensionNameLength = 255;

const separator = '[/\\\\]';
const segmentCharacter = '[^/\\\\]';

/**
 * VS Code's regular expression for a glob in the subset the language associations use: `*` and `?`
 * stay within a segment, a whole `**` segment spans any number of them, and either slash separates
 * segments. It matches case-insensitively, as VS Code's association globs do.
 */
function globExpression(glob: string): RegExp {
	const segments = glob.split('/');
	if (segments.every((segment) => segment === '**')) return /^.*$/i;
	let source = '';
	for (const [index, segment] of segments.entries()) {
		const last = index === segments.length - 1;
		if (segment === '**') {
			if (segments[index - 1] === '**') continue;
			const trailing = last ? `|${separator}${segmentCharacter}+` : '';
			source += `(?:${separator}|${segmentCharacter}+${separator}${trailing})*?`;
			continue;
		}
		for (const character of segment) {
			if (character === '*') source += `${segmentCharacter}*?`;
			else if (character === '?') source += segmentCharacter;
			else source += character.replace(/[\\{}*+?|^$.[\]()]/, '\\$&');
		}
		if (!last && (segments[index + 1] !== '**' || index + 2 < segments.length)) {
			source += separator;
		}
	}
	return new RegExp(`^${source}$`, 'i');
}

const languageGlobs = languageIconsByGlob.map(([glob, icon]) => ({
	expression: globExpression(glob),
	icon,
	onPath: glob.includes('/'),
}));

/**
 * The icon of the language VS Code gives the file: an exact name, else the longest matching glob,
 * else the longest matching extension. Null is a language without an icon, which still outranks
 * the associations below it.
 */
function languageIcon(name: string, path: string): number | null | undefined {
	const byName = languageIconsByFileName.get(name);
	if (byName !== undefined) return byName;
	for (const { expression, icon, onPath } of languageGlobs) {
		if (expression.test(onPath ? path : name)) return icon;
	}
	let found: number | null | undefined;
	let foundLength = 0;
	for (let dot = name.indexOf('.'); dot !== -1; dot = name.indexOf('.', dot + 1)) {
		const byExtension = languageIconsByExtension.get(name.slice(dot));
		if (byExtension !== undefined) {
			found = byExtension;
			foundLength = name.length - dot;
			break;
		}
	}
	for (const [end, icon] of languageIconsBySuffix) {
		if (end.length > foundLength && name.endsWith(end)) {
			found = icon;
			foundLength = end.length;
		}
	}
	return found;
}

/**
 * The icons of the associations that apply to a lowercase name, strongest first: Seti's file
 * name, its extensions from the longest after any dot, then the language VS Code detects.
 */
function* associatedIcons(name: string, path: string): Generator<number> {
	const byName = iconsByFileName.get(name);
	if (byName !== undefined) yield byName;
	if (name.length <= maxExtensionNameLength) {
		for (let dot = name.indexOf('.'); dot !== -1; dot = name.indexOf('.', dot + 1)) {
			const byExtension = iconsByExtension.get(name.slice(dot + 1));
			if (byExtension !== undefined) yield byExtension;
		}
	}
	const byLanguage = languageIcon(name, path);
	if (byLanguage !== undefined && byLanguage !== null) yield byLanguage;
}

/**
 * The Seti icon VS Code draws for a file. The strongest association gives the glyph; VS Code's
 * stylesheet cascades the color on its own, so an icon without one takes the color of the next
 * association that has one, down to the default icon. The path matters only to the few language
 * globs that name folders; it defaults to the name.
 */
export function resolveFileIcon(name: string, path: string = name): SetiIcon {
	let glyph: string | undefined;
	for (const index of associatedIcons(name.toLowerCase(), path.toLowerCase())) {
		const icon = fileIcons[index];
		if (icon === undefined) continue;
		const [iconGlyph, hue] = icon;
		if (glyph === undefined && hue !== null) return icon;
		glyph ??= iconGlyph;
		if (hue !== null) return [glyph, hue];
	}
	return glyph === undefined ? defaultFileIcon : [glyph, defaultFileIcon[1]];
}
