// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { PointerDragKind, type PointerDragCargo } from '../pointer-drag-cargo';
import type { PointerDrag, PointerDragModifiers, PointerDropTarget } from '../pointer-drag-types';
import { registerPointerDropTarget } from '../pointer-drop-targets';
import { startPointerDrag } from '../startPointerDrag';

// The gesture rules every pointer drag follows: a press becomes a drag only past the Tab Strip's
// 10 px, the target under the pointer hears it on every frame, a release drops only where that
// target accepts, every key belongs to the drag, and a press that dragged never ends in a click.
// jsdom has no layout, so `elementFromPoint` answers with the element each test puts under the
// pointer.

const files = new PointerDragKind<readonly string[]>();
const offsets = new PointerDragKind<number>();

interface Heard {
	readonly kind: 'drop' | 'leave' | 'over';
	readonly files: readonly string[] | undefined;
	readonly modifiers: PointerDragModifiers;
	readonly x: number | null;
}

let holder: HTMLElement;
let zone: HTMLElement;
let inside: HTMLElement;
let outside: HTMLElement;
/** The element `elementFromPoint` finds under the pointer. */
let under: Element | null;
let source: () => PointerDragCargo | null;
let clicks: number;
const releases: (() => void)[] = [];

function heardBy(accepts: () => boolean): {
	readonly heard: Heard[];
	readonly target: PointerDropTarget;
} {
	const heard: Heard[] = [];
	const record = (kind: Heard['kind'], drag: PointerDrag, x: number | null) => {
		heard.push({ files: files.read(drag), kind, modifiers: { ...drag.modifiers }, x });
	};
	return {
		heard,
		target: {
			drop: (drag, point) => {
				record('drop', drag, point.clientX);
			},
			leave: (drag) => {
				record('leave', drag, null);
			},
			over: (drag, point) => {
				record('over', drag, point.clientX);
				return accepts();
			},
		},
	};
}

function register(element: Element, target: PointerDropTarget): void {
	releases.push(registerPointerDropTarget(element, target));
}

function pointer(
	type: 'pointercancel' | 'pointerdown' | 'pointermove' | 'pointerup',
	clientX: number,
	init: PointerEventInit = {},
): PointerEvent {
	return new PointerEvent(type, {
		bubbles: true,
		buttons: type === 'pointerup' ? 0 : 1,
		cancelable: true,
		clientX,
		clientY: 0,
		isPrimary: true,
		pointerId: 1,
		...init,
	});
}

function press(clientX = 0): void {
	holder.dispatchEvent(pointer('pointerdown', clientX));
}

function move(clientX: number, init: PointerEventInit = {}): void {
	document.body.dispatchEvent(pointer('pointermove', clientX, init));
}

function release(clientX: number): void {
	document.body.dispatchEvent(pointer('pointerup', clientX));
}

/** Presses a key on the holder, as the keyboard does while it has focus. */
function key(init: KeyboardEventInit): void {
	holder.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init }));
}

/** Whether a click on the holder reaches it. */
function clickArrives(): boolean {
	const before = clicks;
	holder.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
	return clicks > before;
}

function label(): Element | undefined {
	return [...document.body.children].find((element) => element.textContent === 'two files');
}

beforeEach(() => {
	vi.useFakeTimers({
		toFake: ['requestAnimationFrame', 'cancelAnimationFrame', 'setTimeout', 'clearTimeout'],
	});
	holder = document.createElement('div');
	zone = document.createElement('div');
	inside = document.createElement('span');
	outside = document.createElement('div');
	zone.append(inside);
	document.body.replaceChildren(holder, zone, outside);
	under = null;
	document.elementFromPoint = () => under;
	source = () => files.carry(['a.ts', 'b.ts'], 'two files');
	clicks = 0;
	holder.addEventListener('click', () => {
		clicks += 1;
	});
	holder.addEventListener('pointerdown', (event) => {
		startPointerDrag(event, holder, () => source());
	});
});

afterEach(() => {
	document.body.dispatchEvent(pointer('pointercancel', 0));
	for (const unregister of releases.splice(0)) unregister();
	vi.runOnlyPendingTimers();
	vi.useRealTimers();
});

describe('pointer drag', () => {
	it('starts only past 10 px, and a press whose source declines stays a click', () => {
		const start = vi.fn<() => PointerDragCargo>(() => files.carry(['a.ts', 'b.ts'], 'two files'));
		source = start;
		press();
		move(6, { clientY: 8 });
		expect(start).not.toHaveBeenCalled();
		move(11);
		expect(start).toHaveBeenCalledTimes(1);
		expect(label()?.getAttribute('aria-hidden')).toBe('true');
		release(11);
		expect(clickArrives()).toBe(false);
		expect(label()).toBeUndefined();

		vi.runOnlyPendingTimers();
		const decline = vi.fn<() => null>(() => null);
		source = decline;
		press();
		move(20);
		expect(decline).toHaveBeenCalledTimes(1);
		expect(label()).toBeUndefined();
		release(20);
		expect(clickArrives()).toBe(true);
	});

	it('lets the target under the pointer hear the drag on every frame and drops where it accepts', () => {
		const { heard, target } = heardBy(() => true);
		register(zone, target);
		under = inside;
		press();
		move(20);
		vi.advanceTimersToNextFrame();
		vi.advanceTimersToNextFrame();
		expect(heard.map((entry) => entry.kind)).toEqual(['over', 'over', 'over']);
		expect(heard[0]?.files).toEqual(['a.ts', 'b.ts']);
		release(24);
		expect(heard.at(-1)).toMatchObject({ kind: 'drop', x: 24 });
		expect(heard.some((entry) => entry.kind === 'leave')).toBe(false);
		expect(label()).toBeUndefined();
		expect(clickArrives()).toBe(false);
		vi.runOnlyPendingTimers();
		expect(clickArrives()).toBe(true);
	});

	it('reads a payload back only through the kind that carried it', () => {
		const read: unknown[] = [];
		register(zone, {
			drop: () => undefined,
			leave: () => undefined,
			over: (drag) => {
				read.push(files.read(drag), offsets.read(drag));
				return true;
			},
		});
		under = inside;
		source = () => offsets.carry(7, 'two files');
		press();
		move(20);
		expect(read).toEqual([undefined, 7]);
	});

	it('drops nothing where the target refuses or off every target', () => {
		const refusing = heardBy(() => false);
		register(zone, refusing.target);
		under = inside;
		press();
		move(20);
		release(20);
		expect(refusing.heard.map((entry) => entry.kind)).toEqual(['over', 'over', 'leave']);

		vi.runOnlyPendingTimers();
		for (const unregister of releases.splice(0)) unregister();
		const accepting = heardBy(() => true);
		register(zone, accepting.target);
		press();
		move(20);
		under = outside;
		move(30);
		release(30);
		expect(accepting.heard.map((entry) => entry.kind)).toEqual(['over', 'leave']);
	});

	it('gives the drag to the innermost target under the pointer', () => {
		const outer = heardBy(() => true);
		const inner = heardBy(() => true);
		register(zone, outer.target);
		register(inside, inner.target);
		under = inside;
		press();
		move(20);
		release(20);
		expect(inner.heard.map((entry) => entry.kind)).toEqual(['over', 'over', 'drop']);
		expect(outer.heard).toEqual([]);
	});

	it('keeps every key for the drag: modifiers reach the target and Esc cancels without a click', () => {
		const { heard, target } = heardBy(() => true);
		register(zone, target);
		under = inside;
		const pageKeys: string[] = [];
		holder.addEventListener('keydown', (event) => {
			pageKeys.push(event.key);
		});
		press();
		move(20);
		key({ ctrlKey: true, key: 'Control' });
		expect(heard.at(-1)).toMatchObject({ kind: 'over', modifiers: { ctrlKey: true } });
		key({ key: 'Escape' });
		expect(heard.at(-1)?.kind).toBe('leave');
		expect(label()).toBeUndefined();
		expect(pageKeys).toEqual([]);

		move(40);
		vi.advanceTimersToNextFrame();
		expect(heard.at(-1)?.kind).toBe('leave');
		release(40);
		expect(clickArrives()).toBe(false);
		vi.runOnlyPendingTimers();
		key({ key: 'Escape' });
		expect(pageKeys).toEqual(['Escape']);
	});

	it.each([
		{
			end: () => window.dispatchEvent(new FocusEvent('blur')),
			name: 'the window loses focus',
			releaseClicks: false,
		},
		{
			end: () => document.body.dispatchEvent(pointer('pointercancel', 20)),
			name: 'the browser takes the pointer',
			releaseClicks: true,
		},
		{ end: () => move(30, { buttons: 0 }), name: 'the button is up', releaseClicks: true },
		{ end: () => press(30), name: 'the pointer presses again', releaseClicks: true },
	])('cancels when $name', ({ end, releaseClicks }) => {
		const { heard, target } = heardBy(() => true);
		register(zone, target);
		under = inside;
		press();
		move(20);
		end();
		expect(heard.at(-1)?.kind).toBe('leave');
		expect(label()).toBeUndefined();
		release(30);
		expect(clickArrives()).toBe(releaseClicks);
		expect(heard.filter((entry) => entry.kind === 'drop')).toEqual([]);
	});
});
