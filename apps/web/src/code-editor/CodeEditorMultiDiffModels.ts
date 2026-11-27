import type { editor } from './monaco-entry';

/** The two sides a diff compares. */
interface DiffSides {
	readonly modified: editor.ITextModel;
	readonly original: editor.ITextModel;
}

interface HeldDiff {
	readonly sides: DiffSides;
	readonly viewModel: editor.IDiffEditorViewModel;
	/** Settles once the first diff is computed. */
	readonly computed: Promise<void>;
	readonly watches: readonly { dispose(): void }[];
	shownIn: editor.IStandaloneDiffEditor | null;
}

/**
 * The diff view models of a multi-diff's files, kept while the files are listed, as VS Code's
 * multi-diff keeps them: a file that scrolls back into sight, in whichever editor the pool lends it,
 * shows its computed diff and the regions revealed in it without computing again. Monaco reports a
 * side disposed of while a diff still compares it as a bug, so each view model watches its sides from
 * before any editor shows it, and leaves its editor and goes before either side does.
 */
export class CodeEditorMultiDiffModels {
	private readonly held = new Map<string, HeldDiff>();
	/** Which file each editor shows. */
	private readonly shown = new Map<editor.IStandaloneDiffEditor, string>();

	/**
	 * Shows a file's sides in `target` and resolves once their diff is computed. The pool's editors
	 * share their options, so a view model one of them created serves them all.
	 */
	show(key: string, sides: DiffSides, target: editor.IStandaloneDiffEditor): Promise<void> {
		let diff = this.held.get(key);
		if (
			diff !== undefined &&
			(diff.sides.original !== sides.original || diff.sides.modified !== sides.modified)
		) {
			this.drop(key);
			diff = undefined;
		}
		if (diff === undefined) {
			const viewModel = target.createViewModel(sides);
			const drop = (): void => this.drop(key);
			diff = {
				computed: viewModel.waitForDiff().catch(() => undefined),
				shownIn: null,
				sides,
				viewModel,
				watches: [sides.original.onWillDispose(drop), sides.modified.onWillDispose(drop)],
			};
			this.held.set(key, diff);
		}
		if (diff.shownIn !== target) {
			if (diff.shownIn !== null) this.release(diff.shownIn);
			this.release(target);
			target.setModel(diff.viewModel);
			diff.shownIn = target;
			this.shown.set(target, key);
		}
		return diff.computed;
	}

	/** Takes whatever diff `target` shows out of it, keeping the diff for its file's next showing. */
	release(target: editor.IStandaloneDiffEditor): void {
		const key = this.shown.get(target);
		if (key === undefined) return;
		this.shown.delete(target);
		const diff = this.held.get(key);
		if (diff !== undefined) diff.shownIn = null;
		target.setModel(null);
	}

	/** Lets go of the diffs of files no longer listed. */
	retain(keys: ReadonlySet<string>): void {
		for (const key of this.held.keys()) {
			if (!keys.has(key)) this.drop(key);
		}
	}

	dispose(): void {
		for (const key of this.held.keys()) this.drop(key);
	}

	private drop(key: string): void {
		const diff = this.held.get(key);
		if (diff === undefined) return;
		this.held.delete(key);
		for (const watch of diff.watches) watch.dispose();
		if (diff.shownIn !== null) this.release(diff.shownIn);
		diff.viewModel.dispose();
	}
}
