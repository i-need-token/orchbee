import { describe, expect, it } from 'vitest';

import type { FileResult } from '../../file-error';
import type { FileRootEvent } from '../../file-session-port';
import type { GitMutation, GitRelayEvent, GitSummary, GitSummaryEvent } from '../../git-port';
import { MemoryFileSessionPort } from '../MemoryFileSessionPort';

const WORKSPACE = 'workspace-1';
const LIMIT = 1024 * 1024;
const COMMIT: GitMutation = {
	all: 'none',
	amend: false,
	message: { kind: 'text', text: 'Fix' },
	noVerify: false,
	op: 'commit',
	repository: '/w',
	signoff: false,
};

function signal(): AbortSignal {
	return new AbortController().signal;
}

function value<Value>(result: FileResult<Value>): Value {
	if (result.kind === 'error') throw new Error(`The request failed with ${result.error.code}.`);
	return result.value;
}

function code(result: FileResult<unknown>): string | null {
	return result.kind === 'error' ? result.error.code : null;
}

/** Records a stream's events after its first records arrived. */
async function follow<Event>(subscribe: (listener: (event: Event) => void) => () => void) {
	const events: Event[] = [];
	const stop = subscribe((event) => events.push(event));
	await Promise.resolve();
	return { events, stop };
}

function machine(options: ConstructorParameters<typeof MemoryFileSessionPort>[0] = {}) {
	const port = new MemoryFileSessionPort(options);
	port.makeDirectory('/w');
	port.setRoot(WORKSPACE, '/w');
	return { port, session: port.open(WORKSPACE) };
}

describe('the in-memory file session', () => {
	it('lists a folder in pages that its cursor continues, and loading it routes its changes', async () => {
		const { port, session } = machine({ pageSize: 2 });
		for (const name of ['a', 'b', 'c']) port.writeFile(`/w/src/${name}.ts`, name);
		const stream = await follow<FileRootEvent>((listener) => session.subscribe(listener));
		expect(stream.events).toEqual([
			{
				kind: 'snapshot',
				root: { path: '/w', state: 'present' },
				rules: { caseSensitive: true, os: 'linux', separator: '/' },
				sequence: 1,
			},
		]);

		const first = value(await session.tree.list({ path: '/w/src' }, signal()));
		expect(first.cursor).not.toBeNull();
		const rest = value(
			await session.tree.list({ cursor: first.cursor ?? undefined, path: '/w/src' }, signal()),
		);
		expect(rest.cursor).toBeNull();
		// Entries arrive unsorted, and each appears once across the pages.
		const names = [...first.entries, ...rest.entries].map((entry) => entry.name);
		expect(names).toHaveLength(3);
		expect(new Set(names)).toEqual(new Set(['a.ts', 'b.ts', 'c.ts']));
		// A cursor serves one continuation.
		expect(
			code(
				await session.tree.list({ cursor: first.cursor ?? undefined, path: '/w/src' }, signal()),
			),
		).toBe('invalid-request');

		port.writeFile('/w/src/d.ts', 'd');
		port.writeFile('/w/other/e.ts', 'e');
		expect(stream.events.slice(1)).toEqual([
			{
				changes: [{ directory: false, path: '/w/src/d.ts', type: 'added' }],
				kind: 'update',
				rescan: [],
				root: null,
				sequence: 2,
				watch: null,
			},
		]);

		session.tree.release(['/w/src']);
		port.writeFile('/w/src/f.ts', 'f');
		expect(stream.events).toHaveLength(2);
	});

	it('reports a deleted loaded folder once and the root going missing', async () => {
		const { port, session } = machine();
		port.writeFile('/w/src/a.ts', 'a');
		value(await session.tree.list({ path: '/w' }, signal()));
		value(await session.tree.list({ path: '/w/src' }, signal()));
		const stream = await follow<FileRootEvent>((listener) => session.subscribe(listener));

		port.remove('/w/src');
		port.remove('/w');
		expect(stream.events.slice(1)).toEqual([
			{
				changes: [{ directory: true, path: '/w/src', type: 'deleted' }],
				kind: 'update',
				rescan: [],
				root: null,
				sequence: 2,
				watch: null,
			},
			{
				changes: [],
				kind: 'update',
				rescan: [],
				root: { path: '/w', state: 'missing' },
				sequence: 3,
				watch: null,
			},
		]);
	});

	it('serves only the root, consented repositories and opened files', async () => {
		const { port, session } = machine();
		port.writeFile('/etc/hosts', '127.0.0.1 localhost\n');
		port.writeFile('/repo/README.md', 'readme');
		const read = { limit: LIMIT, mode: 'text', path: '/etc/hosts' } as const;

		expect(code(await session.content.read(read, signal()))).toBe('outside-scope');
		expect(value(await session.tree.stat(['/etc/hosts'], signal()))).toEqual([
			{ error: { code: 'outside-scope' }, kind: 'error', path: '/etc/hosts' },
		]);
		expect(code(await session.tree.list({ path: '/repo' }, signal()))).toBe('outside-scope');

		value(await session.grants.grant('/etc/hosts', 'terminal-link', signal()));
		expect(value(await session.content.read(read, signal()))).toMatchObject({
			kind: 'text',
			text: '127.0.0.1 localhost\n',
		});
		expect(port.grants()).toEqual([
			{ path: '/etc/hosts', source: 'terminal-link', workspaceId: WORKSPACE },
		]);

		value(await session.grants.consentRepository('/repo', signal()));
		expect(value(await session.tree.list({ path: '/repo' }, signal())).entries).toHaveLength(1);
		value(await session.grants.closeRepository('/repo', signal()));
		expect(code(await session.tree.list({ path: '/repo' }, signal()))).toBe('outside-scope');

		session.grants.ungrant(['/etc/hosts']);
		expect(code(await session.content.read(read, signal()))).toBe('outside-scope');
	});

	it('compares paths caselessly on a machine that ignores case', async () => {
		const port = new MemoryFileSessionPort({
			rules: { caseSensitive: false, os: 'windows', separator: '\\' },
		});
		port.writeFile('C:\\Work\\Notes.txt', 'notes');
		port.setRoot(WORKSPACE, 'C:\\Work');
		const session = port.open(WORKSPACE);

		expect(
			value(
				await session.content.read(
					{ limit: LIMIT, mode: 'text', path: 'c:\\work\\NOTES.TXT' },
					signal(),
				),
			),
		).toMatchObject({ kind: 'text', text: 'notes' });
	});

	it('reads with VS Code’s line ending, binary and size facts', async () => {
		const { port, session } = machine();
		const read = async (path: string, options: { forceText?: boolean; limit?: number } = {}) =>
			session.content.read({ limit: LIMIT, mode: 'text', path, ...options }, signal());
		port.writeFile('/w/crlf.txt', 'a\r\nb\r\nc\n');
		port.writeFile('/w/even.txt', 'a\r\nb\n');
		port.writeFile('/w/flat.txt', 'abc');
		port.writeFile('/w/image.bin', new Uint8Array([0x89, 0x50, 0x00, 0x47]));

		expect(value(await read('/w/crlf.txt'))).toMatchObject({ lineEnding: 'crlf' });
		// CRLF wins only when carriage returns exceed half of the breaks.
		expect(value(await read('/w/even.txt'))).toMatchObject({ lineEnding: 'lf' });
		expect(value(await read('/w/flat.txt'))).toMatchObject({ lineEnding: 'none' });
		expect(value(await read('/w/image.bin'))).toMatchObject({ kind: 'binary' });
		expect(value(await read('/w/image.bin', { forceText: true }))).toMatchObject({ kind: 'text' });
		expect(code(await read('/w/flat.txt', { limit: 2 }))).toBe('too-large');
		expect(code(await read('/w'))).toBe('is-directory');
		expect(code(await read('/w/missing.txt'))).toBe('not-found');
	});

	it('holds manual answers until flushed, cancels on abort and reports content changed meanwhile', async () => {
		const { port, session } = machine({ answering: 'manual' });
		port.writeFile('/w/a.txt', 'one');
		const controller = new AbortController();
		const aborted = session.content.read(
			{ limit: LIMIT, mode: 'text', path: '/w/a.txt' },
			controller.signal,
		);
		const held = session.content.read({ limit: LIMIT, mode: 'text', path: '/w/a.txt' }, signal());
		expect(port.waiting).toBe(2);

		controller.abort();
		expect(code(await aborted)).toBe('cancelled');
		port.writeFile('/w/a.txt', 'two');
		expect(port.flush()).toBe(1);
		expect(value(await held)).toMatchObject({ changed: true, kind: 'text', text: 'two' });

		session.close();
		expect(
			code(await session.content.read({ limit: LIMIT, mode: 'text', path: '/w/a.txt' }, signal())),
		).toBe('cancelled');
		expect(port.openWorkspaces()).toEqual([]);
	});

	it('releases what the link held when it drops, and resynchronizes every stream when it returns', async () => {
		const { port, session } = machine({ answering: 'manual' });
		port.writeFile('/w/src/a.ts', 'a');
		port.writeFile('/etc/hosts', 'hosts');
		const summary: GitSummary = {
			candidates: [],
			git: { state: 'ok', version: '2.47.1' },
			repositories: [],
			repositoriesTruncated: false,
			unsafe: ['/w'],
		};
		port.git.setSummary(WORKSPACE, summary);
		const root = await follow<FileRootEvent>((listener) => session.subscribe(listener));
		const scm = await follow<GitSummaryEvent>((listener) => session.git.subscribe(listener));
		const relay = await follow<GitRelayEvent>((listener) => session.git.subscribeRelay(listener));
		const listed = session.tree.list({ path: '/w/src' }, signal());
		const granted = session.grants.grant('/etc/hosts', 'tree', signal());
		port.flush();
		value(await listed);
		value(await granted);
		port.git.prompt(WORKSPACE, {
			id: 'prompt-1',
			kind: 'password',
			path: null,
			repository: '/w',
			text: 'Password:',
		});
		const waiting = session.tree.stat(['/w/src/a.ts'], signal());

		port.disconnect();
		expect(code(await waiting)).toBe('remote-disconnected');
		const whileDown = session.tree.stat(['/w/src/a.ts'], signal());
		expect(code(await whileDown)).toBe('remote-disconnected');
		expect(root.events.at(-1)).toEqual({ kind: 'gap' });
		expect(scm.events.at(-1)).toEqual({ kind: 'gap' });
		expect(relay.events.at(-1)).toEqual({ id: 'prompt-1', kind: 'closed' });

		port.reconnect();
		expect(root.events.at(-1)).toMatchObject({
			kind: 'snapshot',
			root: { path: '/w', state: 'present' },
		});
		expect(scm.events.at(-1)).toMatchObject({ kind: 'summary', summary });
		// The new link holds no loaded folder and no opened file until the Client asks again.
		port.writeFile('/w/src/b.ts', 'b');
		expect(root.events.at(-1)).toMatchObject({ kind: 'snapshot' });
		const read = session.content.read({ limit: LIMIT, mode: 'text', path: '/etc/hosts' }, signal());
		port.flush();
		expect(code(await read)).toBe('outside-scope');
	});

	it('runs a Git change once per request ID and leaves a change the link lost before it ran unknown', async () => {
		const { port, session } = machine({ answering: 'manual' });
		const run = (requestId: string) => session.git.run(requestId, COMMIT, signal());
		const outcome = async (requestId: string) => {
			const answer = session.git.outcome(requestId, signal());
			port.flush();
			return value(await answer);
		};

		const first = run('a');
		port.flush();
		const committed = value(await first);
		expect(value(await run('a'))).toEqual(committed);
		expect(await outcome('a')).toEqual({
			result: { kind: 'ok', value: committed },
			state: 'completed',
		});

		const lost = run('b');
		port.disconnect();
		expect(code(await lost)).toBe('remote-disconnected');
		port.reconnect();
		expect(await outcome('b')).toEqual({ state: 'unknown' });

		const unanswered = run('c');
		port.disconnect({ delivered: true });
		expect(code(await unanswered)).toBe('remote-disconnected');
		port.reconnect();
		expect(await outcome('c')).toMatchObject({ state: 'completed' });
		const resent = run('c');
		expect(code(await resent)).toBeNull();

		expect(port.git.runs().map((change) => change.requestId)).toEqual(['a', 'c']);
	});

	it('pages a status by cursor and refuses a page of an older revision', async () => {
		const { port, session } = machine({ pageSize: 2 });
		const entries = ['a', 'b', 'c'].map((path) => ({
			original: null,
			path,
			submodule: false,
			x: '.',
			y: 'M',
		}));
		port.git.setStatus('/w', 2, entries);

		const first = value(await session.git.statusPage({ repository: '/w', revision: 2 }, signal()));
		const rest = value(
			await session.git.statusPage(
				{ cursor: first.cursor ?? undefined, repository: '/w', revision: 2 },
				signal(),
			),
		);
		expect([...first.entries, ...rest.entries]).toEqual(entries);
		expect(rest.cursor).toBeNull();

		port.git.setStatus('/w', 3, entries);
		expect(code(await session.git.statusPage({ repository: '/w', revision: 2 }, signal()))).toBe(
			'stale-revision',
		);
	});
});
