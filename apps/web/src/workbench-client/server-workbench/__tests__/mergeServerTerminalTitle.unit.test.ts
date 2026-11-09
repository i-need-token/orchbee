import { describe, expect, it } from 'vitest';

import {
	mergeServerTerminalTitle,
	type ServerTerminalTitleArrival,
	type ServerTerminalTitleCursor,
} from '../mergeServerTerminalTitle';

// Each step names the snapshot's terminal when it arrives, so a case reads as the order things
// reached this Client. The result is the title shown after the last step: text it came from, null
// for the program's name.
type Step = readonly [terminal: string | null, arrival: ServerTerminalTitleArrival];

const DIRECTORY = { currentCwd: 'C:\\work', kind: 'reported' } as const;

function surface(title: string | null): ServerTerminalTitleArrival {
	return { currentDirectory: DIRECTORY, kind: 'surface', title };
}

function row(binding: string, title: string | null | 'unavailable'): ServerTerminalTitleArrival {
	return {
		binding,
		kind: 'stream',
		title: title === 'unavailable' ? { kind: 'unavailable' } : { kind: 'observed', title },
	};
}

const PUBLICATION: ServerTerminalTitleArrival = { kind: 'terminal' };

function shown(steps: readonly Step[]): string | null {
	let cursor: ServerTerminalTitleCursor | undefined;
	for (const [terminal, arrival] of steps) {
		cursor = mergeServerTerminalTitle(cursor, terminal, arrival).cursor;
	}
	return cursor?.shown?.text ?? null;
}

describe('mergeServerTerminalTitle', () => {
	it('shows whichever source changed the title last', () => {
		expect(
			shown([
				['A', surface('vim')],
				['A', row('A', 'build')],
			]),
		).toBe('build');
		expect(
			shown([
				['A', row('A', 'build')],
				['A', surface('vim')],
			]),
		).toBe('vim');
		// A row repeated because another Presentation in its stream changed is not a new title.
		expect(
			shown([
				['A', row('A', 'build')],
				['A', surface('vim')],
				['A', row('A', 'build')],
			]),
		).toBe('vim');
		// A cleared title shows the program's name; an unreadable one keeps what was shown.
		expect(
			shown([
				['A', surface('vim')],
				['A', row('A', null)],
			]),
		).toBeNull();
		expect(
			shown([
				['A', row('A', 'build')],
				['A', row('A', 'unavailable')],
			]),
		).toBe('build');
		expect(
			shown([
				['A', surface('vim')],
				['A', row('A', 'unavailable')],
			]),
		).toBe('vim');
	});

	it('applies a row only while the snapshot holds its binding', () => {
		// The snapshot moved first: the previous binding's row is ignored, and the new terminal shows
		// the program's name until a title arrives for it.
		const moved: Step[] = [
			['A', row('A', 'old')],
			['B', PUBLICATION],
			['B', row('A', 'stale')],
		];
		expect(shown(moved)).toBeNull();
		expect(shown([...moved, ['B', row('B', 'new')]])).toBe('new');
		expect(shown([...moved, ['B', row('B', 'unavailable')]])).toBeNull();
		// The row arrived first: it applies once the snapshot holds its binding.
		const early: Step[] = [
			['A', row('A', 'old')],
			['A', row('B', 'new')],
		];
		expect(shown(early)).toBe('old');
		expect(shown([...early, ['B', PUBLICATION]])).toBe('new');
		// A row for no terminal says nothing.
		expect(
			mergeServerTerminalTitle(undefined, 'A', {
				binding: null,
				kind: 'stream',
				title: { kind: 'unavailable' },
			}).applied,
		).toBeNull();
	});

	it('changes the directory only from the surface', () => {
		const first = mergeServerTerminalTitle(undefined, 'A', surface('vim'));
		expect(first.applied).toEqual({
			currentDirectory: DIRECTORY,
			title: { published: false, text: 'vim' },
		});
		expect(mergeServerTerminalTitle(first.cursor, 'A', row('A', 'build')).applied).toEqual({
			currentDirectory: { kind: 'ignored' },
			title: { published: true, text: 'build' },
		});
	});
});
