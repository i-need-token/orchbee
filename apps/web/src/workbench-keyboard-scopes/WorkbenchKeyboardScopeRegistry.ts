import type { ActionScope } from '@orchbee/actions';
import type { KeyboardShortcutChord } from '@orchbee/keyboard-shortcuts';

/**
 * A region that owns keys while focus is inside it. From narrowest to widest: a code editor, a tree
 * or list widget, the file overlay and the File Bar; the terminal and the window keep their own
 * owners. Each kind admits `Mod+K` sequences.
 */
export type WorkbenchKeyboardScope =
	| {
			readonly kind: 'code-editor';
			/** The Action scope the scope's bindings resolve in; the Workbench's when absent. */
			readonly actionScope?: ActionScope | undefined;
			/**
			 * The chords the editor binds itself. While focus is inside it they go to the editor, which
			 * no wider binding takes from it; `Mod+K` and a sequence's second step stay with the window.
			 */
			readonly yields: readonly KeyboardShortcutChord[];
	  }
	| {
			readonly kind: 'widget';
			readonly actionScope?: ActionScope | undefined;
			/**
			 * The widget's name in the shortcut catalog, such as `explorer`: the bindings declared with
			 * the scope `widget:explorer` apply while focus is on it, but not in a text field inside it.
			 */
			readonly widget: string;
	  }
	| {
			/** The overlay counts as inside the File Bar, so File Bar bindings apply there too. */
			readonly kind: 'file-bar' | 'file-overlay';
			readonly actionScope?: ActionScope | undefined;
	  };

/** What holds a scope's element, as a React ref does; it is read at each key press. */
interface WorkbenchKeyboardScopeHolder {
	readonly current: Element | null;
}

interface WorkbenchKeyboardScopeRegistration {
	readonly holder: WorkbenchKeyboardScopeHolder;
	readonly scope: WorkbenchKeyboardScope;
}

interface WorkbenchKeyboardScopeEntry {
	readonly element: Element;
	readonly scope: WorkbenchKeyboardScope;
}

/** Which rendered elements own keys, so the Workbench listener can find the scopes around focus. */
export class WorkbenchKeyboardScopeRegistry {
	readonly #registrations = new Set<WorkbenchKeyboardScopeRegistration>();

	/** Registers the element `holder` holds, whenever it holds one, as `scope` until disposed. */
	register(holder: WorkbenchKeyboardScopeHolder, scope: WorkbenchKeyboardScope): () => void {
		const registration = { holder, scope };
		this.#registrations.add(registration);
		return () => {
			this.#registrations.delete(registration);
		};
	}

	/** The registered scopes whose element contains `target`, narrowest first. */
	scopesAt(target: EventTarget | null): readonly WorkbenchKeyboardScopeEntry[] {
		if (this.#registrations.size === 0 || !(target instanceof Node)) return [];
		const entries: WorkbenchKeyboardScopeEntry[] = [];
		for (const { holder, scope } of this.#registrations) {
			const element = holder.current;
			if (element?.contains(target) === true) entries.push({ element, scope });
		}
		return entries.sort(narrowestFirst);
	}
}

/** Every entry contains the same target, so of two different elements one contains the other. */
function narrowestFirst(
	left: WorkbenchKeyboardScopeEntry,
	right: WorkbenchKeyboardScopeEntry,
): number {
	if (left.element === right.element) return 0;
	return left.element.contains(right.element) ? 1 : -1;
}
