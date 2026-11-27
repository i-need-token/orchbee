import type { languages } from './monaco-entry';

type LanguageRegistration = Pick<
	typeof languages,
	'register' | 'setLanguageConfiguration' | 'setMonarchTokensProvider'
>;

/**
 * VS Code's JSON languages with the file names and extensions its JSON extension associates, plus
 * the `tsconfig.json` and `jsconfig.json` its TypeScript extension adds. Monaco declares JSON only
 * in its language service, which the editor leaves out.
 */
const JSON_LANGUAGES: readonly languages.ILanguageExtensionPoint[] = [
	{
		aliases: ['JSON', 'json'],
		extensions: [
			'.json',
			'.bowerrc',
			'.jscsrc',
			'.webmanifest',
			'.js.map',
			'.css.map',
			'.ts.map',
			'.har',
			'.jslintrc',
			'.jsonld',
			'.geojson',
			'.ipynb',
			'.vuerc',
		],
		filenames: ['composer.lock', '.watchmanconfig'],
		id: 'json',
		mimetypes: ['application/json'],
	},
	{
		aliases: ['JSON with Comments'],
		extensions: [
			'.jsonc',
			'.eslintrc',
			'.eslintrc.json',
			'.jsfmtrc',
			'.jshintrc',
			'.swcrc',
			'.hintrc',
			'.babelrc',
		],
		filenames: [
			'babel.config.json',
			'bun.lock',
			'.babelrc.json',
			'.ember-cli',
			'typedoc.json',
			'tsconfig.json',
			'jsconfig.json',
		],
		id: 'jsonc',
	},
	{ aliases: ['JSON Lines'], extensions: ['.jsonl', '.ndjson'], id: 'jsonl' },
];

/**
 * Token types as Monaco's JSON language service names them, so the theme colors keys, values and
 * literals alike. A string is a key when a colon follows it on its line.
 */
const JSON_TOKENS: languages.IMonarchLanguage = {
	// Without `start`, Monarch starts in the state declared first, which sorted keys make `comment`.
	start: 'root',
	tokenizer: {
		comment: [
			[/[^*]+/u, 'comment.block'],
			[/\*\//u, 'comment.block', '@pop'],
			[/\*/u, 'comment.block'],
		],
		root: [
			[/\s+/u, ''],
			[/\/\*/u, 'comment.block', '@comment'],
			[/\/\/.*$/u, 'comment.line'],
			[/"(?:[^"\\]|\\.)*"(?=\s*:)/u, 'string.key'],
			[/"(?:[^"\\]|\\.)*"?/u, 'string.value'],
			[/-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/u, 'number'],
			[/\b(?:true|false|null)\b/u, 'keyword'],
			[/[{}]/u, 'delimiter.bracket'],
			[/[[\]]/u, 'delimiter.array'],
			[/:/u, 'delimiter.colon'],
			[/,/u, 'delimiter.comma'],
		],
	},
	tokenPostfix: '.json',
	unicode: true,
};

/** Monaco's JSON language configuration: brackets, comments and the word pattern. */
const JSON_CONFIGURATION: languages.LanguageConfiguration = {
	autoClosingPairs: [
		{ close: '}', notIn: ['string'], open: '{' },
		{ close: ']', notIn: ['string'], open: '[' },
		{ close: '"', notIn: ['string'], open: '"' },
	],
	brackets: [
		['{', '}'],
		['[', ']'],
	],
	comments: { blockComment: ['/*', '*/'], lineComment: '//' },
	wordPattern: /(-?\d*\.\d\w*)|([^[{\]}:",\s]+)/gu,
};

/** Declares the JSON languages with a Monarch tokenizer of their own. */
export function registerJsonLanguages(target: LanguageRegistration): void {
	for (const language of JSON_LANGUAGES) {
		target.register(language);
		target.setMonarchTokensProvider(language.id, JSON_TOKENS);
		target.setLanguageConfiguration(language.id, JSON_CONFIGURATION);
	}
}
