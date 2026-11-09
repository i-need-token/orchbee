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

function row(
	incarnation: string,
	title: string | null | 'unavailable',
): ServerTerminalTitleArrival {
	return {
		incarnation,
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
				['A', row('1', 'build')],
			]),
		).toBe('build');
		expect(
			shown([
				['A', row('1', 'build')],
				['A', surface('vim')],
			]),
		).toBe('vim');
		// A row repeated because another Presentation in its stream changed is not a new title.
		expect(
			shown([
				['A', row('1', 'build')],
				['A', surface('vim')],
				['A', row('1', 'build')],
			]),
		).toBe('vim');
		// A cleared title shows the program's name; an unreadable one keeps what was shown.
		expect(
			shown([
				['A', surface('vim')],
				['A', row('1', null)],
			]),
		).toBeNull();
		expect(
			shown([
				['A', row('1', 'build')],
				['A', row('1', 'unavailable')],
			]),
		).toBe('build');
		expect(
			shown([
				['A', surface('vim')],
				['A', row('1', 'unavailable')],
			]),
		).toBe('vim');
	});

	it('applies a row only to the terminal the Presentation shows', () => {
		// The snapshot moved first: the previous terminal's row is ignored, and the new terminal shows
		// the program's name until its own row arrives.
		const moved: Step[] = [
			['A', row('1', 'old')],
			['B', PUBLICATION],
			['B', row('1', 'stale')],
		];
		expect(shown(moved)).toBeNull();
		expect(shown([...moved, ['B', row('2', 'new')]])).toBe('new');
		expect(shown([...moved, ['B', row('2', 'unavailable')]])).toBeNull();
		// The Server moved first: its row waits for the snapshot to show that terminal.
		const ahead: Step[] = [
			['A', row('1', 'old')],
			['A', row('2', 'new')],
		];
		expect(shown(ahead)).toBe('old');
		expect(shown([...ahead, ['B', PUBLICATION]])).toBe('new');
		// A row naming no terminal says nothing.
		expect(
			mergeServerTerminalTitle(undefined, 'A', {
				incarnation: null,
				kind: 'stream',
				title: { kind: 'unavailable' },
			}),
		).toEqual({ applied: null, cursor: undefined });
	});

	it('changes the directory only from the surface', () => {
		const first = mergeServerTerminalTitle(undefined, 'A', surface('vim'));
		expect(first.applied).toEqual({
			currentDirectory: DIRECTORY,
			title: { published: false, text: 'vim' },
		});
		expect(mergeServerTerminalTitle(first.cursor, 'A', row('1', 'build')).applied).toEqual({
			currentDirectory: { kind: 'ignored' },
			title: { published: true, text: 'build' },
		});
	});
});
