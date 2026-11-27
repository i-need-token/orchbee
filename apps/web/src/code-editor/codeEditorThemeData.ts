import type { editor } from './monaco-entry';

/** The document's current theme colors, in the hex forms Monaco parses. */
export interface CodeEditorThemeColors {
	/** The token's color: `#rrggbb`, or `#rrggbbaa` when translucent; null when it does not parse. */
	color(token: string): string | null;
	/** The token painted over the editor's canvas, as opaque `#rrggbb`; null when either does not parse. */
	onCanvas(token: string): string | null;
}

/**
 * How Monaco takes a token: `as-is` keeps its alpha, so fills layer over the canvas and over one
 * another; `on-canvas` paints it over the canvas first, for text, whose token colors Monaco reads
 * as opaque.
 */
type ColorUse = 'as-is' | 'on-canvas';

/**
 * Monaco's colors by ID. The editor paints the canvas, as the terminal does; its widgets float on
 * the overlay surface; additions and removals take the success and danger roles. Every color left
 * out keeps Monaco's default for the polarity, such as find matches, which it draws translucent.
 */
const EDITOR_COLORS: readonly (readonly [id: string, token: string, use: ColorUse])[] = [
	['editor.background', '--ob-sys-color-surface-canvas', 'as-is'],
	['editor.foreground', '--ob-sys-color-text-primary', 'on-canvas'],
	['editorLineNumber.foreground', '--ob-sys-color-text-muted', 'on-canvas'],
	['editorLineNumber.activeForeground', '--ob-sys-color-text-primary', 'on-canvas'],
	['editorCursor.foreground', '--ob-sys-color-text-primary', 'on-canvas'],
	['editor.lineHighlightBackground', '--ob-sys-color-surface-subtle', 'as-is'],
	['editor.selectionBackground', '--ob-sys-color-surface-selected', 'as-is'],
	['editorIndentGuide.background1', '--ob-sys-color-border-subtle', 'as-is'],
	['editorIndentGuide.activeBackground1', '--ob-sys-color-border-strong', 'as-is'],
	['editorRuler.foreground', '--ob-sys-color-border-subtle', 'as-is'],
	['editorOverviewRuler.border', '--ob-sys-color-border-subtle', 'as-is'],
	['editorLink.activeForeground', '--ob-sys-color-text-link', 'on-canvas'],
	['editorError.foreground', '--ob-sys-color-status-danger', 'on-canvas'],
	['editorWarning.foreground', '--ob-sys-color-status-warning', 'on-canvas'],
	['editorInfo.foreground', '--ob-sys-color-status-info', 'on-canvas'],
	// Bracket pairs cycle through three syntax colors, as VS Code's gold, orchid and blue do.
	['editorBracketHighlight.foreground1', '--ob-sys-color-syntax-type', 'on-canvas'],
	['editorBracketHighlight.foreground2', '--ob-sys-color-syntax-keyword', 'on-canvas'],
	['editorBracketHighlight.foreground3', '--ob-sys-color-syntax-variable', 'on-canvas'],
	[
		'editorBracketHighlight.unexpectedBracket.foreground',
		'--ob-sys-color-status-danger',
		'on-canvas',
	],
	['foreground', '--ob-sys-color-text-primary', 'as-is'],
	['descriptionForeground', '--ob-sys-color-text-secondary', 'as-is'],
	['errorForeground', '--ob-sys-color-status-danger-text', 'as-is'],
	['icon.foreground', '--ob-sys-color-icon-secondary', 'as-is'],
	['focusBorder', '--ob-sys-color-border-focus', 'as-is'],
	['editorWidget.background', '--ob-sys-color-surface-overlay', 'as-is'],
	['editorWidget.foreground', '--ob-sys-color-text-primary', 'as-is'],
	['editorWidget.border', '--ob-sys-color-border-default', 'as-is'],
	['input.background', '--ob-sys-color-surface-canvas', 'as-is'],
	['input.foreground', '--ob-sys-color-text-primary', 'as-is'],
	['input.border', '--ob-sys-color-border-default', 'as-is'],
	['input.placeholderForeground', '--ob-sys-color-text-placeholder', 'as-is'],
	['inputOption.activeBackground', '--ob-sys-color-surface-selected', 'as-is'],
	['inputOption.activeBorder', '--ob-sys-color-border-focus', 'as-is'],
	['inputOption.activeForeground', '--ob-sys-color-text-primary', 'as-is'],
	['toolbar.hoverBackground', '--ob-sys-color-surface-hovered', 'as-is'],
	['toolbar.activeBackground', '--ob-sys-color-surface-pressed', 'as-is'],
	['scrollbarSlider.background', '--ob-sys-color-scrollbar-thumb', 'as-is'],
	['scrollbarSlider.hoverBackground', '--ob-sys-color-scrollbar-thumb-hover', 'as-is'],
	['scrollbarSlider.activeBackground', '--ob-sys-color-scrollbar-thumb-pressed', 'as-is'],
	['diffEditor.insertedLineBackground', '--ob-sys-color-status-success-surface', 'as-is'],
	['diffEditor.insertedTextBackground', '--ob-sys-color-status-success-surface-hover', 'as-is'],
	['diffEditor.removedLineBackground', '--ob-sys-color-status-danger-surface', 'as-is'],
	['diffEditor.removedTextBackground', '--ob-sys-color-status-danger-surface-hover', 'as-is'],
	['diffEditorGutter.insertedLineBackground', '--ob-sys-color-status-success-surface', 'as-is'],
	['diffEditorGutter.removedLineBackground', '--ob-sys-color-status-danger-surface', 'as-is'],
	['diffEditorOverview.insertedForeground', '--ob-sys-color-status-success', 'as-is'],
	['diffEditorOverview.removedForeground', '--ob-sys-color-status-danger', 'as-is'],
	['diffEditor.border', '--ob-sys-color-border-default', 'as-is'],
	['diffEditor.diagonalFill', '--ob-sys-color-border-subtle', 'as-is'],
	['diffEditor.unchangedRegionBackground', '--ob-sys-color-surface-raised', 'as-is'],
	['diffEditor.unchangedRegionForeground', '--ob-sys-color-text-secondary', 'as-is'],
];

/**
 * Monarch token types by their leading segments; each also colors every longer type, so `string`
 * colors `string.quote.js`. Types left out, such as delimiters and identifiers, take the editor
 * foreground.
 */
const TOKEN_COLORS: readonly (readonly [token: string, color: string])[] = [
	['comment', '--ob-sys-color-syntax-comment'],
	['keyword', '--ob-sys-color-syntax-keyword'],
	['string', '--ob-sys-color-syntax-string'],
	['regexp', '--ob-sys-color-syntax-string'],
	['string.escape', '--ob-sys-color-syntax-constant'],
	['regexp.escape', '--ob-sys-color-syntax-constant'],
	['number', '--ob-sys-color-syntax-constant'],
	['constant', '--ob-sys-color-syntax-constant'],
	['type', '--ob-sys-color-syntax-type'],
	['annotation', '--ob-sys-color-syntax-type'],
	['namespace', '--ob-sys-color-syntax-type'],
	['tag', '--ob-sys-color-syntax-tag'],
	['metatag', '--ob-sys-color-syntax-tag'],
	['metatag.content', '--ob-sys-color-syntax-string'],
	['attribute.name', '--ob-sys-color-syntax-attribute'],
	['attribute.value', '--ob-sys-color-syntax-string'],
	['attribute.value.number', '--ob-sys-color-syntax-constant'],
	['attribute.value.unit', '--ob-sys-color-syntax-constant'],
	['attribute.value.hex', '--ob-sys-color-syntax-constant'],
	['key', '--ob-sys-color-syntax-attribute'],
	['string.key', '--ob-sys-color-syntax-attribute'],
	['variable', '--ob-sys-color-syntax-variable'],
	['predefined', '--ob-sys-color-syntax-variable'],
	['invalid', '--ob-sys-color-status-danger'],
];

/**
 * The editor theme for the document's current colors. It inherits no rules from Monaco's base
 * theme, whose language-specific colors would show through every named theme; the base supplies
 * only Monaco's defaults, by polarity, for the colors left out.
 */
export function codeEditorThemeData(
	colors: CodeEditorThemeColors,
	dark: boolean,
): editor.IStandaloneThemeData {
	const themeColors: Record<string, string> = {};
	for (const [id, token, use] of EDITOR_COLORS) {
		const value = use === 'as-is' ? colors.color(token) : colors.onCanvas(token);
		if (value !== null) themeColors[id] = value;
	}
	const rules: editor.ITokenThemeRule[] = [
		{ fontStyle: 'italic', token: 'emphasis' },
		{ fontStyle: 'bold', token: 'strong' },
	];
	for (const [token, color] of TOKEN_COLORS) {
		const foreground = colors.onCanvas(color);
		if (foreground !== null) rules.push({ foreground, token });
	}
	return { base: dark ? 'vs-dark' : 'vs', colors: themeColors, inherit: false, rules };
}
