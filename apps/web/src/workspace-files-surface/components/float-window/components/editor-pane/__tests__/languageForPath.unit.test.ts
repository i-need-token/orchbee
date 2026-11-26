import { describe, expect, it } from 'vitest';

import { type LanguageEntry, languageForPath } from '../languageForPath';

const LANGUAGES: readonly LanguageEntry[] = [
	{ extensions: ['.ts', '.mts'], id: 'typescript' },
	{ extensions: ['.d.ts'], id: 'typescript-declaration' },
	{ extensions: ['.JSON', '.jsonc'], id: 'json' },
	{ extensions: ['.dockerfile'], filenames: ['Dockerfile'], id: 'dockerfile' },
	{ id: 'no-extensions' },
];

describe('languageForPath', () => {
	it('prefers an exact file name over any extension, ignoring case and folders', () => {
		expect(languageForPath('deploy/dockerfile', LANGUAGES)).toBe('dockerfile');
	});

	it('takes the longest matching extension', () => {
		expect(languageForPath('src/types/index.d.ts', LANGUAGES)).toBe('typescript-declaration');
		expect(languageForPath('src/index.ts', LANGUAGES)).toBe('typescript');
	});

	it('matches extensions without regard to case', () => {
		expect(languageForPath('Package.Json', LANGUAGES)).toBe('json');
	});

	it('falls back to plain text when nothing matches', () => {
		expect(languageForPath('README', LANGUAGES)).toBe('plaintext');
		expect(languageForPath('notes.txt', [])).toBe('plaintext');
	});
});
