import { describe, expect, it } from 'vitest';

import { parseJsonWithoutDuplicateMembers } from '../parseJsonWithoutDuplicateMembers';

describe('parseJsonWithoutDuplicateMembers', () => {
	it('parses nested JSON when member names are unique within each object', () => {
		expect(parseJsonWithoutDuplicateMembers('{"items":[{"id":1},{"id":2}],"ok":true}')).toEqual({
			items: [{ id: 1 }, { id: 2 }],
			ok: true,
		});
	});

	it('rejects exact and escaped-equivalent duplicates at any object depth', () => {
		for (const text of ['{"id":1,"id":2}', '{"items":[{"id":1,"\\u0069d":2}]}']) {
			expect(() => parseJsonWithoutDuplicateMembers(text)).toThrow(
				'JSON contains a duplicate object member',
			);
		}
	});

	it('delegates malformed JSON rejection to the platform parser', () => {
		expect(() => parseJsonWithoutDuplicateMembers('{"id":')).toThrow(SyntaxError);
	});
});
