import type * as MonacoApi from 'monaco-editor/editor/editor.api';

type Monaco = typeof MonacoApi;

/** The Monaco theme name the float window's editors use; redefined whenever the document theme changes. */
export const EDITOR_THEME = 'orchbee';

/** The attributes that select the document's theme: its polarity and its named color theme. */
const THEME_ATTRIBUTES = ['data-theme', 'data-color-theme'];
/** A canvas keeps its previous fill when it cannot parse a color, so this fill marks a failed parse. */
const UNPARSED_FILL = '#010203';

/**
 * The system color each editor role takes. Syntax categories are categorical, so they take the five
 * categorical data series; comments read as muted text and punctuation as secondary text, so the
 * editor follows every named theme without an editor-only palette.
 */
const ROLE_TOKENS = {
	background: '--ob-sys-color-surface-canvas',
	border: '--ob-sys-color-border-default',
	comment: '--ob-sys-color-text-muted',
	cursor: '--ob-sys-color-text-primary',
	delimiter: '--ob-sys-color-text-secondary',
	focus: '--ob-sys-color-border-focus',
	foreground: '--ob-sys-color-text-primary',
	function: '--ob-sys-color-data-5',
	inserted: '--ob-sys-color-status-success',
	keyword: '--ob-sys-color-data-1',
	lineHighlight: '--ob-sys-color-surface-subtle',
	lineNumber: '--ob-sys-color-text-disabled',
	lineNumberActive: '--ob-sys-color-text-secondary',
	number: '--ob-sys-color-data-3',
	overlay: '--ob-sys-color-surface-overlay',
	removed: '--ob-sys-color-status-danger',
	scrollbar: '--ob-sys-color-scrollbar-thumb',
	scrollbarHover: '--ob-sys-color-scrollbar-thumb-hover',
	selection: '--ob-sys-color-surface-selected',
	string: '--ob-sys-color-data-2',
	type: '--ob-sys-color-data-4',
	warning: '--ob-sys-color-status-warning',
} as const;

type Role = keyof typeof ROLE_TOKENS;

function isRole(key: string): key is Role {
	return Object.hasOwn(ROLE_TOKENS, key);
}

const ROLES = Object.keys(ROLE_TOKENS).filter(isRole);

function hexByte(value: number): string {
	return value.toString(16).padStart(2, '0');
}

/**
 * Each role's color under the document's current theme as `#rrggbbaa`, painted over the canvas
 * color so translucent tokens keep their look. Tokens may be `color-mix()` or `oklch()`, which
 * Monaco's color parser does not read, so the canvas converts them.
 */
function resolveRoles(targetWindow: Window, context: CanvasRenderingContext2D): Map<Role, string> {
	const style = targetWindow.getComputedStyle(targetWindow.document.documentElement);
	const resolved = new Map<Role, string>();
	for (const role of ROLES) {
		const value = style.getPropertyValue(ROLE_TOKENS[role]).trim();
		context.fillStyle = UNPARSED_FILL;
		context.fillStyle = value;
		if (value === '' || context.fillStyle === UNPARSED_FILL) continue;
		context.clearRect(0, 0, 1, 1);
		context.fillRect(0, 0, 1, 1);
		const [red, green, blue, alpha] = context.getImageData(0, 0, 1, 1).data;
		if (red === undefined || green === undefined || blue === undefined || alpha === undefined)
			continue;
		resolved.set(role, `#${hexByte(red)}${hexByte(green)}${hexByte(blue)}${hexByte(alpha)}`);
	}
	return resolved;
}

/** Drops the alpha byte, for the token rules, which take `rrggbb` only. */
function opaque(color: string | undefined): string | undefined {
	return color?.slice(1, 7);
}

/** Lowers a color's alpha to the given byte, for the diff's inserted and removed backgrounds. */
function translucent(color: string | undefined, alpha: number): string | undefined {
	return color === undefined ? undefined : `${color.slice(0, 7)}${hexByte(alpha)}`;
}

function themeData(dark: boolean, roles: Map<Role, string>): MonacoApi.editor.IStandaloneThemeData {
	const colors: Record<string, string> = {};
	const assign = (key: string, color: string | undefined): void => {
		if (color !== undefined) colors[key] = color;
	};
	assign('editor.background', roles.get('background'));
	assign('editor.foreground', roles.get('foreground'));
	assign('editorCursor.foreground', roles.get('cursor'));
	assign('editor.lineHighlightBackground', roles.get('lineHighlight'));
	assign('editor.lineHighlightBorder', '#00000000');
	assign('editorLineNumber.foreground', roles.get('lineNumber'));
	assign('editorLineNumber.activeForeground', roles.get('lineNumberActive'));
	assign('editor.selectionBackground', roles.get('selection'));
	assign('editor.inactiveSelectionBackground', roles.get('lineHighlight'));
	assign('editorWidget.background', roles.get('overlay'));
	assign('editorWidget.border', roles.get('border'));
	assign('focusBorder', roles.get('focus'));
	assign('scrollbarSlider.background', roles.get('scrollbar'));
	assign('scrollbarSlider.hoverBackground', roles.get('scrollbarHover'));
	assign('scrollbarSlider.activeBackground', roles.get('scrollbarHover'));
	assign('editorGutter.background', roles.get('background'));
	assign('diffEditor.insertedTextBackground', translucent(roles.get('inserted'), 0x33));
	assign('diffEditor.removedTextBackground', translucent(roles.get('removed'), 0x33));
	assign('diffEditor.insertedLineBackground', translucent(roles.get('inserted'), 0x1a));
	assign('diffEditor.removedLineBackground', translucent(roles.get('removed'), 0x1a));
	assign('editorOverviewRuler.border', '#00000000');
	assign('editorWarning.foreground', roles.get('warning'));
	const rule = (
		token: string,
		role: Role,
		fontStyle?: string,
	): MonacoApi.editor.ITokenThemeRule => {
		const foreground = opaque(roles.get(role));
		return {
			token,
			...(foreground === undefined ? {} : { foreground }),
			...(fontStyle === undefined ? {} : { fontStyle }),
		};
	};
	return {
		base: dark ? 'vs-dark' : 'vs',
		colors,
		inherit: true,
		rules: [
			rule('', 'foreground'),
			rule('comment', 'comment', 'italic'),
			rule('keyword', 'keyword'),
			rule('storage', 'keyword'),
			rule('string', 'string'),
			rule('string.escape', 'number'),
			rule('regexp', 'string'),
			rule('number', 'number'),
			rule('constant', 'number'),
			rule('type', 'type'),
			rule('type.identifier', 'type'),
			rule('tag', 'keyword'),
			rule('attribute.name', 'type'),
			rule('attribute.value', 'string'),
			rule('key', 'type'),
			rule('variable.predefined', 'function'),
			rule('predefined', 'function'),
			rule('identifier.function', 'function'),
			rule('delimiter', 'delimiter'),
			rule('operator', 'delimiter'),
		],
	};
}

/**
 * Defines the editor theme from the document's current theme, and defines it again whenever the
 * theme attributes change, so open editors follow a theme switch without reopening.
 */
export function followEditorTheme(targetWindow: Window, monaco: Monaco): void {
	const { document } = targetWindow;
	const canvas = document.createElement('canvas');
	canvas.width = 1;
	canvas.height = 1;
	const context = canvas.getContext('2d', { willReadFrequently: true });
	const apply = (): void => {
		const dark = document.documentElement.dataset['theme'] === 'dark';
		const roles = context === null ? new Map<Role, string>() : resolveRoles(targetWindow, context);
		monaco.editor.defineTheme(EDITOR_THEME, themeData(dark, roles));
		monaco.editor.setTheme(EDITOR_THEME);
	};
	new MutationObserver(apply).observe(document.documentElement, {
		attributeFilter: THEME_ATTRIBUTES,
	});
	apply();
}
