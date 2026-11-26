import type * as MonacoApi from 'monaco-editor/editor/editor.api';

type Monaco = typeof MonacoApi;

/**
 * Monaco colors JSON only through its JSON language service, which runs a Worker this editor does
 * not start, so JSON gets a small colorizer of its own: keys, strings, numbers, literals and
 * comments, the last for the JSONC files many tools keep.
 */
export function registerJsonColorizer(monaco: Monaco): void {
	monaco.languages.register({
		aliases: ['JSON', 'json'],
		extensions: ['.json', '.jsonc', '.json5', '.jsonl', '.code-workspace', '.webmanifest'],
		filenames: ['.babelrc', '.eslintrc', '.prettierrc', 'tsconfig.json', 'jsconfig.json'],
		id: 'json',
		mimetypes: ['application/json'],
	});
	monaco.languages.setLanguageConfiguration('json', {
		autoClosingPairs: [
			{ close: '}', open: '{' },
			{ close: ']', open: '[' },
			{ close: '"', notIn: ['string'], open: '"' },
		],
		brackets: [
			['{', '}'],
			['[', ']'],
		],
		comments: { blockComment: ['/*', '*/'], lineComment: '//' },
	});
	monaco.languages.setMonarchTokensProvider('json', {
		unicode: true,
		tokenizer: {
			comment: [
				[/[^/*]+/u, 'comment'],
				[/\*\//u, 'comment', '@pop'],
				[/[/*]/u, 'comment'],
			],
			root: [
				[/\s+/u, 'white'],
				[/\/\/.*$/u, 'comment'],
				[/\/\*/u, 'comment', '@comment'],
				[/"(?:[^"\\]|\\.)*"(?=\s*:)/u, 'key'],
				[/"(?:[^"\\]|\\.)*"/u, 'string'],
				[/-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?/u, 'number'],
				[/\b(?:true|false|null)\b/u, 'keyword'],
				[/[{}[\],:]/u, 'delimiter'],
			],
		},
	});
}
