import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import {
	createFileOverlayState,
	transitionFileOverlay,
	type FileOverlayEffect,
	type FileOverlayEvent,
	type FileOverlayState,
} from '../file-overlay-state';

type FileTabInput = Extract<FileOverlayEvent, { readonly type: 'file-opened' }>['input'];
type FileTabCloseTarget = Extract<FileOverlayEvent, { readonly type: 'close-requested' }>['target'];

interface ReferenceTab {
	readonly id: string;
	readonly input: FileTabInput;
	preview: boolean;
	sticky: boolean;
	unsaved: boolean;
}

interface ReferenceClosed {
	readonly batch: number;
	readonly id: string;
	readonly index: number;
	readonly input: FileTabInput;
	readonly sticky: boolean;
}

type Tier = 'default' | 'full' | 'left-expand';

/**
 * An independent reading of VS Code's single editor group, its close commands and Reopen Closed
 * Editor, with the overlay's modes: each tab carries its own sticky and preview flags, the recent
 * order is a plain list and closed tabs a flat list of batch-numbered entries.
 */
class ReferenceOverlay {
	private active: string | null = null;
	private batches = 0;
	private closed: ReferenceClosed[] = [];
	private kind: 'closed' | 'open' | 'pip' = 'closed';
	private leftExpandOffered: boolean;
	private pending: { readonly ids: readonly string[]; saving: boolean } | null = null;
	private recent: string[] = [];
	private restoreTier: Exclude<Tier, 'full'> = 'default';
	private row: ReferenceTab[] = [];
	private tier: Tier = 'default';

	public constructor(leftExpandOffered: boolean) {
		this.leftExpandOffered = leftExpandOffered;
	}

	public apply(event: FileOverlayEvent): FileOverlayEffect[] {
		switch (event.type) {
			case 'dismissed':
				if (this.kind !== 'open') return [];
				this.kind = 'pip';
				return [{ kind: 'focus-terminal' }];
			case 'terminal-replaced':
				if (this.kind === 'open') this.kind = 'pip';
				return [];
			case 'pip-restored':
				if (this.kind === 'pip') this.kind = 'open';
				return [];
			case 'full-toggled':
				if (this.kind !== 'open') return [];
				if (this.tier === 'full') {
					this.tier = this.restoreTier;
				} else {
					this.restoreTier = this.tier;
					this.tier = 'full';
				}
				return [];
			case 'tier-chosen':
				if (this.kind !== 'open' || this.tier === 'full') return [];
				if (event.tier === 'left-expand' && !this.leftExpandOffered) return [];
				this.tier = event.tier;
				return [];
			case 'layout-changed':
				this.leftExpandOffered = event.leftExpandOffered;
				if (!event.leftExpandOffered) {
					if (this.tier === 'left-expand') this.tier = 'default';
					this.restoreTier = 'default';
				}
				return [];
			default: {
				const count = this.row.length;
				const effects = this.applyToTabs(event);
				if (this.row.length === 0) this.kind = 'closed';
				else if (
					event.type === 'file-opened' ||
					(event.type === 'tab-activated' && this.tab(event.id) !== undefined) ||
					(event.type === 'reopen-requested' && this.row.length > count)
				) {
					this.kind = 'open';
				}
				return effects;
			}
		}
	}

	/** The overlay as the production state reports it. */
	public view(): unknown {
		const batches = new Map<number, unknown[]>();
		for (const entry of this.closed) {
			const batch = batches.get(entry.batch) ?? [];
			batch.push({ id: entry.id, index: entry.index, sticky: entry.sticky });
			batches.set(entry.batch, batch);
		}
		return {
			active: this.active,
			closed: [...batches.values()],
			mode: {
				kind: this.kind,
				leftExpandOffered: this.leftExpandOffered,
				restoreTier: this.restoreTier,
				tier: this.tier,
			},
			pending: this.pending === null ? null : { ...this.pending },
			recent: this.recent,
			row: this.row.map(({ id, preview, sticky, unsaved }) => ({ id, preview, sticky, unsaved })),
		};
	}

	private applyToTabs(
		event: Exclude<FileOverlayEvent, { readonly type: ModeEventType }>,
	): FileOverlayEffect[] {
		switch (event.type) {
			case 'file-opened':
				this.open(event.input, { index: null, pinned: !event.preview, sticky: false });
				return [];
			case 'tab-activated':
				if (this.tab(event.id) !== undefined) this.activate(event.id);
				return [];
			case 'tab-kept': {
				const tab = this.tab(event.id);
				if (tab !== undefined) tab.preview = false;
				return [];
			}
			case 'tab-moved': {
				const tab = this.tab(event.id);
				if (tab !== undefined && this.moveTo(tab, event.index)) tab.preview = false;
				return [];
			}
			case 'tab-stuck': {
				const tab = this.tab(event.id);
				if (tab !== undefined && !tab.sticky) this.stick(tab);
				return [];
			}
			case 'tab-unstuck': {
				const tab = this.tab(event.id);
				if (tab?.sticky === true) {
					const stickyCount = this.stickyCount();
					this.row.splice(this.row.indexOf(tab), 1);
					this.row.splice(stickyCount - 1, 0, tab);
					tab.sticky = false;
				}
				return [];
			}
			case 'unsaved-changed': {
				const tab = this.tab(event.id);
				if (tab === undefined) return [];
				tab.unsaved = event.unsaved;
				if (event.unsaved) tab.preview = false;
				return [];
			}
			case 'reopen-requested':
				this.reopen();
				return [];
			case 'close-requested':
				if (this.pending === null) this.requestClose(event.target);
				return [];
			case 'close-decided': {
				const pending = this.pending;
				if (pending === null || pending.saving) return [];
				const unsaved = this.row
					.filter((tab) => tab.unsaved && pending.ids.includes(tab.id))
					.map(({ id, input, unsaved: isUnsaved }) => ({ id, input, unsaved: isUnsaved }));
				if (event.decision === 'cancel') {
					this.pending = null;
					return [];
				}
				if (event.decision === 'save' && unsaved.length > 0) {
					pending.saving = true;
					return [{ kind: 'save', tabs: unsaved }];
				}
				this.pending = null;
				this.close(pending.ids);
				return unsaved.length > 0 ? [{ kind: 'revert', tabs: unsaved }] : [];
			}
			case 'close-saved': {
				const pending = this.pending;
				if (pending?.saving !== true) return [];
				this.pending = null;
				this.close(pending.ids.filter((id) => !event.failedIds.includes(id)));
				return [];
			}
		}
	}

	private tab(id: string): ReferenceTab | undefined {
		return this.row.find((tab) => tab.id === id);
	}

	private stickyCount(): number {
		return this.row.filter((tab) => tab.sticky).length;
	}

	private stickyAt(index: number): boolean {
		return this.row[index]?.sticky === true;
	}

	private activate(id: string): void {
		this.active = id;
		this.recent = [id, ...this.recent.filter((other) => other !== id)];
	}

	/** A move makes the tab sticky exactly when it lands inside the sticky prefix it moved within. */
	private moveTo(tab: ReferenceTab, index: number): boolean {
		const to = Math.max(0, Math.min(index, this.row.length - 1));
		if (this.row.indexOf(tab) === to) return false;
		const stickyCount = this.stickyCount();
		this.row.splice(this.row.indexOf(tab), 1);
		this.row.splice(to, 0, tab);
		tab.sticky = to < stickyCount;
		return true;
	}

	private stick(tab: ReferenceTab): void {
		const stickyCount = this.stickyCount();
		tab.preview = false;
		this.row.splice(this.row.indexOf(tab), 1);
		this.row.splice(stickyCount, 0, tab);
		tab.sticky = true;
	}

	private open(
		input: FileTabInput,
		options: { readonly index: number | null; readonly pinned: boolean; readonly sticky: boolean },
	): void {
		const id = `file:${input.resource.key}`;
		this.closed = this.closed.filter((entry) => entry.id !== id);
		const sticky = options.sticky || (options.index !== null && this.stickyAt(options.index));
		const pinned = options.pinned || sticky;
		const existing = this.tab(id);
		if (existing !== undefined) {
			if (pinned) existing.preview = false;
			this.activate(id);
			if (options.index !== null) this.moveTo(existing, options.index);
			if (sticky && !existing.sticky) this.stick(existing);
			return;
		}
		let target = options.index ?? this.row.findIndex((tab) => tab.id === this.active) + 1;
		if (options.index === null && !sticky && this.stickyAt(target)) target = this.stickyCount();
		if (sticky) target = Math.min(target, this.stickyCount());
		const replaced = pinned ? undefined : this.row.find((tab) => tab.preview);
		if (replaced !== undefined) {
			if (target > this.row.indexOf(replaced)) target -= 1;
			this.row.splice(this.row.indexOf(replaced), 1);
			this.recent = this.recent.filter((other) => other !== replaced.id);
			if (this.active === replaced.id) this.active = null;
			if (this.pending !== null) {
				this.pending = {
					...this.pending,
					ids: this.pending.ids.filter((other) => other !== replaced.id),
				};
			}
		}
		this.row.splice(target, 0, { id, input, preview: !pinned, sticky, unsaved: false });
		this.activate(id);
	}

	private close(ids: readonly string[]): void {
		const unique = [...new Set(ids)].filter((id) => this.tab(id) !== undefined);
		const ordered = [
			...unique.filter((id) => id !== this.active),
			...unique.filter((id) => id === this.active),
		];
		this.batches += 1;
		for (const id of ordered) {
			const index = this.row.findIndex((tab) => tab.id === id);
			const tab = this.row[index];
			if (tab === undefined) continue;
			this.row.splice(index, 1);
			this.recent = this.recent.filter((other) => other !== id);
			if (this.active === id) this.active = this.recent[0] ?? null;
			this.closed.push({ batch: this.batches, id, index, input: tab.input, sticky: tab.sticky });
			if (this.closed.length > 20) this.closed.shift();
		}
	}

	private requestClose(target: FileTabCloseTarget): void {
		const nonSticky = this.row.filter((tab) => !tab.sticky).map((tab) => tab.id);
		let ids: readonly string[];
		switch (target.kind) {
			case 'tabs':
				ids = target.ids;
				break;
			case 'active': {
				const active = this.active === null ? undefined : this.tab(this.active);
				if (active === undefined) return;
				if (active.sticky) {
					const next = this.recent.find((id) => this.tab(id)?.sticky === false);
					if (next !== undefined) this.activate(next);
					return;
				}
				ids = [active.id];
				break;
			}
			case 'others':
				for (const id of target.keepIds) {
					const tab = this.tab(id);
					if (tab !== undefined) tab.preview = false;
				}
				ids = nonSticky.filter((id) => !target.keepIds.includes(id));
				break;
			case 'left':
			case 'right': {
				if (target.kind === 'right' && this.active !== null) {
					const active = this.tab(this.active);
					if (active !== undefined) active.preview = false;
				}
				const index = this.row.findIndex((tab) => tab.id === target.id);
				if (index < 0) return;
				ids = this.row
					.filter((tab, at) => !tab.sticky && (target.kind === 'left' ? at < index : at > index))
					.map((tab) => tab.id);
				break;
			}
			case 'saved':
				ids = this.recent.filter((id) => this.tab(id)?.sticky === false && !this.tab(id)?.unsaved);
				break;
			case 'all':
				ids = nonSticky;
				break;
			case 'overlay':
				ids = this.row.map((tab) => tab.id);
				break;
		}
		const present = [...new Set(ids)].filter((id) => this.tab(id) !== undefined);
		if (present.length === 0) return;
		const firstUnsaved = present.find((id) => this.tab(id)?.unsaved === true);
		if (firstUnsaved === undefined) {
			this.close(present);
			return;
		}
		this.activate(firstUnsaved);
		this.pending = { ids: present, saving: false };
	}

	/** Undoes the last close: its tabs come back last closed first, then the last closed activates. */
	private reopen(): void {
		const last = this.closed.at(-1);
		if (last === undefined) return;
		const batch = this.closed.filter((entry) => entry.batch === last.batch);
		this.closed = this.closed.filter((entry) => entry.batch !== last.batch);
		for (const entry of batch.reverse()) {
			const fits = entry.sticky === this.stickyAt(entry.index);
			this.open(entry.input, {
				index: fits ? entry.index : null,
				pinned: true,
				sticky: entry.sticky,
			});
		}
		this.activate(last.id);
	}
}

type ModeEventType =
	| 'dismissed'
	| 'full-toggled'
	| 'layout-changed'
	| 'pip-restored'
	| 'terminal-replaced'
	| 'tier-chosen';

/** The production state in the reference's terms. */
function view(state: FileOverlayState): unknown {
	const { tabs } = state;
	return {
		active: tabs.activeId,
		closed: tabs.closed.map((batch) =>
			batch.map(({ id, index, sticky }) => ({ id, index, sticky })),
		),
		mode: state.mode,
		pending:
			tabs.pendingClose === null
				? null
				: { ids: tabs.pendingClose.ids, saving: tabs.pendingClose.phase === 'saving' },
		recent: tabs.recent,
		row: tabs.tabs.map(({ id, unsaved }, index) => ({
			id,
			preview: id === tabs.previewId,
			sticky: index < tabs.stickyCount,
			unsaved,
		})),
	};
}

/** The overlay's invariants that a transition broke; every transition keeps them all. */
function brokenInvariants(
	before: FileOverlayState,
	after: FileOverlayState,
	event: FileOverlayEvent,
): string[] {
	const { mode, tabs } = after;
	const ids = tabs.tabs.map((tab) => tab.id);
	const closed = tabs.closed.flat();
	const pending = tabs.pendingClose;
	const resolvedSave = (id: string): boolean =>
		(event.type === 'close-decided' && event.decision === 'discard') ||
		(event.type === 'close-saved' && !event.failedIds.includes(id));
	const holds: Record<string, boolean> = {
		'a waiting close takes open tabs':
			pending === null || (pending.ids.length > 0 && pending.ids.every((id) => ids.includes(id))),
		'an active tab exactly while tabs are open': (tabs.activeId === null) === (ids.length === 0),
		'an unsaved tab leaves only through a resolved save decision': before.tabs.tabs.every(
			(tab) => !tab.unsaved || ids.includes(tab.id) || resolvedSave(tab.id),
		),
		'closed exactly while no tab is open': (mode.kind === 'closed') === (ids.length === 0),
		'left-expand appears only while offered':
			mode.leftExpandOffered || (mode.tier !== 'left-expand' && mode.restoreTier !== 'left-expand'),
		'sticky tabs fit in the row': tabs.stickyCount >= 0 && tabs.stickyCount <= ids.length,
		'tab ids are unique': new Set(ids).size === ids.length,
		'the active tab leads the recent order': ids.length === 0 || tabs.recent[0] === tabs.activeId,
		'the preview is an open tab past the sticky ones':
			tabs.previewId === null || ids.indexOf(tabs.previewId) >= tabs.stickyCount,
		'the recent order holds every tab once':
			tabs.recent.length === ids.length && ids.every((id) => tabs.recent.includes(id)),
		'the reopen stack holds at most 20 closed tabs in non-empty batches':
			closed.length <= 20 &&
			tabs.closed.every((batch) => batch.length > 0) &&
			closed.every((entry) => !ids.includes(entry.id)),
	};
	return Object.entries(holds)
		.filter(([, held]) => !held)
		.map(([name]) => name);
}

const PATHS = ['/w/a', '/w/b', '/w/c', '/w/d', '/w/e'] as const;

const inputArbitrary = fc
	.constantFrom(...PATHS)
	.map((path): FileTabInput => ({ kind: 'file', resource: { key: path, path } }));
const idArbitrary = fc.constantFrom(...PATHS).map((path) => `file:${path}`);

const targetArbitrary: fc.Arbitrary<FileTabCloseTarget> = fc.oneof(
	fc.constantFrom<FileTabCloseTarget>(
		{ kind: 'active' },
		{ kind: 'all' },
		{ kind: 'overlay' },
		{ kind: 'saved' },
	),
	fc
		.record({ id: idArbitrary, left: fc.boolean() })
		.map(({ id, left }): FileTabCloseTarget => ({ id, kind: left ? 'left' : 'right' })),
	fc
		.uniqueArray(idArbitrary, { maxLength: 2 })
		.map((keepIds): FileTabCloseTarget => ({ keepIds, kind: 'others' })),
	fc
		.uniqueArray(idArbitrary, { maxLength: 3 })
		.map((ids): FileTabCloseTarget => ({ ids, kind: 'tabs' })),
);

const eventArbitrary: fc.Arbitrary<FileOverlayEvent> = fc.oneof(
	{
		arbitrary: fc
			.record({ input: inputArbitrary, preview: fc.boolean() })
			.map(({ input, preview }): FileOverlayEvent => ({ input, preview, type: 'file-opened' })),
		weight: 8,
	},
	{
		arbitrary: fc
			.record({
				id: idArbitrary,
				type: fc.constantFrom('tab-activated', 'tab-kept', 'tab-stuck', 'tab-unstuck'),
			})
			.map((event): FileOverlayEvent => event),
		weight: 5,
	},
	{
		arbitrary: fc
			.record({ id: idArbitrary, index: fc.nat({ max: 5 }) })
			.map(({ id, index }): FileOverlayEvent => ({ id, index, type: 'tab-moved' })),
		weight: 2,
	},
	{
		arbitrary: fc
			.record({ id: idArbitrary, unsaved: fc.boolean() })
			.map(({ id, unsaved }): FileOverlayEvent => ({ id, type: 'unsaved-changed', unsaved })),
		weight: 3,
	},
	{
		arbitrary: targetArbitrary.map(
			(target): FileOverlayEvent => ({
				target,
				type: 'close-requested',
			}),
		),
		weight: 5,
	},
	{
		arbitrary: fc
			.constantFrom('cancel', 'discard', 'save')
			.map((decision): FileOverlayEvent => ({ decision, type: 'close-decided' })),
		weight: 3,
	},
	{
		arbitrary: fc
			.uniqueArray(idArbitrary, { maxLength: 2 })
			.map((failedIds): FileOverlayEvent => ({ failedIds, type: 'close-saved' })),
		weight: 2,
	},
	{ arbitrary: fc.constant<FileOverlayEvent>({ type: 'reopen-requested' }), weight: 3 },
	{
		arbitrary: fc.constantFrom<FileOverlayEvent>(
			{ type: 'dismissed' },
			{ type: 'full-toggled' },
			{ type: 'pip-restored' },
			{ type: 'terminal-replaced' },
			{ tier: 'default', type: 'tier-chosen' },
			{ tier: 'left-expand', type: 'tier-chosen' },
			{ leftExpandOffered: false, type: 'layout-changed' },
			{ leftExpandOffered: true, type: 'layout-changed' },
		),
		weight: 4,
	},
);

describe('the file overlay model', () => {
	it('follows the reference editor group and keeps its invariants under any event sequence', () => {
		fc.assert(
			fc.property(
				fc.boolean(),
				// Without an explicit size fast-check stops at 10 events, too few to stack sticky tabs,
				// previews and a waiting close.
				fc.array(eventArbitrary, { maxLength: 80, size: 'max' }),
				(offered, events) => {
					const reference = new ReferenceOverlay(offered);
					let state = createFileOverlayState(offered);
					for (const event of events) {
						const { effects, state: next } = transitionFileOverlay(state, event);
						expect(effects).toEqual(reference.apply(event));
						expect(view(next)).toEqual(reference.view());
						expect(brokenInvariants(state, next, event)).toEqual([]);
						// A transition that changes nothing keeps the same state, so no subscriber hears it.
						expect(next === state).toBe(JSON.stringify(view(next)) === JSON.stringify(view(state)));
						state = next;
					}
				},
			),
			{ numRuns: 400, seed: 20_261_127 },
		);
	});

	it('remembers the last 20 closed tabs and reopens them together as the row was', () => {
		const paths = Array.from({ length: 22 }, (_, index) => `/w/${String(index).padStart(2, '0')}`);
		let state = createFileOverlayState(false);
		for (const path of paths) {
			const input: FileTabInput = { kind: 'file', resource: { key: path, path } };
			state = transitionFileOverlay(state, { input, preview: false, type: 'file-opened' }).state;
		}
		state = transitionFileOverlay(state, {
			target: { kind: 'all' },
			type: 'close-requested',
		}).state;
		expect(state.mode.kind).toBe('closed');
		expect(state.tabs.closed.flat()).toHaveLength(20);

		state = transitionFileOverlay(state, { type: 'reopen-requested' }).state;
		expect(state.tabs.tabs.map((tab) => tab.input.resource.path)).toEqual(paths.slice(2));
		expect(state.tabs.activeId).toBe('file:/w/21');
		expect(state.mode.kind).toBe('open');
	});
});
