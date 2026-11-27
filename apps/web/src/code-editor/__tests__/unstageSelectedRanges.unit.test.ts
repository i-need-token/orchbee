import { describe, expect, it } from 'vitest';

import { unstageSelectedRanges } from '../unstageSelectedRanges';

describe('unstageSelectedRanges', () => {
	it('returns only the selected staged lines to HEAD', () => {
		const request = {
			changes: [
				{
					modifiedEndLineNumber: 2,
					modifiedStartLineNumber: 2,
					originalEndLineNumber: 2,
					originalStartLineNumber: 2,
				},
				{
					modifiedEndLineNumber: 4,
					modifiedStartLineNumber: 4,
					originalEndLineNumber: 4,
					originalStartLineNumber: 4,
				},
			],
			head: 'a\nb\nc\nd\n',
			index: 'a\nB\nc\nD\n',
		};
		expect(
			unstageSelectedRanges({ ...request, selections: [{ endLineNumber: 2, startLineNumber: 2 }] }),
		).toBe('a\nb\nc\nD\n');
		expect(
			unstageSelectedRanges({ ...request, selections: [{ endLineNumber: 3, startLineNumber: 3 }] }),
		).toBeNull();
	});

	it('keeps the line breaks right around blocks at the end of the file', () => {
		expect(
			unstageSelectedRanges({
				changes: [
					{
						modifiedEndLineNumber: 3,
						modifiedStartLineNumber: 3,
						originalEndLineNumber: 0,
						originalStartLineNumber: 2,
					},
				],
				head: 'a\nb',
				index: 'a\nb\nc',
				selections: [{ endLineNumber: 3, startLineNumber: 3 }],
			}),
		).toBe('a\nb');
		expect(
			unstageSelectedRanges({
				changes: [
					{
						modifiedEndLineNumber: 0,
						modifiedStartLineNumber: 2,
						originalEndLineNumber: 3,
						originalStartLineNumber: 3,
					},
				],
				head: 'a\r\nb\r\nc',
				index: 'a\r\nb',
				selections: [{ endLineNumber: 2, startLineNumber: 2 }],
			}),
		).toBe('a\r\nb\r\nc');
	});
});
