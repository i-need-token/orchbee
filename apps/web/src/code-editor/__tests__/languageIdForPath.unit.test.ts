import { describe, expect, it } from 'vitest';

import { languageIdForPath } from '../languageIdForPath';

const DECLARED = [
	{ extensions: ['.json'], id: 'json' },
	{ extensions: ['.jsonc'], filenames: ['tsconfig.json'], id: 'jsonc' },
	{ extensions: ['.map'], id: 'map' },
	{ extensions: ['.js.map'], id: 'json-map' },
	{ extensions: ['.json'], id: 'later-json' },
];

describe('languageIdForPath', () => {
	it('prefers an exact name, then the longest extension, then the language declared last', () => {
		expect(languageIdForPath(DECLARED, 'C:\\project\\TSConfig.JSON')).toBe('jsonc');
		expect(languageIdForPath(DECLARED, '/project/out/app.js.map')).toBe('json-map');
		expect(languageIdForPath(DECLARED, '/project/package.json')).toBe('later-json');
		expect(languageIdForPath(DECLARED, '/project/Makefile')).toBe('plaintext');
	});
});
