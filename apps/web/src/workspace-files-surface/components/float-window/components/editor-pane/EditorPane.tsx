import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import { MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES } from '@orchbee/workbench-client-protocol';
import type * as MonacoApi from 'monaco-editor/editor/editor.api';
import * as React from 'react';

import type { WorkbenchWorkspaceFilesSource } from '../../../../../workbench';
import { splitPath } from '../../../../change-groups';
import type { EditorDocuments } from '../../../../EditorDocuments';
import { documentPathOf, type FloatTab } from '../../../../float-tabs';
import { readFileChunks } from '../../../../readFileChunks';
import type { MatchReveal } from '../../../../search-results';
import { DocumentNoticeBar } from './components/document-notice-bar/DocumentNoticeBar';
import { languageForPath } from './languageForPath';
import { EDITOR_THEME } from './monaco-theme';
import { type EditorContent, readEditorContent } from './readEditorContent';
import { startMonaco } from './startMonaco';

import styles from './EditorPane.module.css';

const monaco = startMonaco(window);

/** Above this size a file opens without colorizing, as VS Code does, so a large file stays fast. */
const PLAIN_TEXT_BYTES = 1024 * 1024;

/** How a diff tab compares its sides; the float window's header sets them. */
export interface DiffOptions {
	readonly sideBySide: boolean;
	readonly ignoreTrimWhitespace: boolean;
	readonly hideUnchangedRegions: boolean;
}

/** What the float window's header asks of the pane it holds. */
export interface EditorPaneHandle {
	readonly goToChange: (direction: 'next' | 'previous') => void;
}

export interface EditorPaneProps {
	readonly source: WorkbenchWorkspaceFilesSource;
	readonly workspaceId: string;
	/** Every tab the window holds, so the pane releases the text of a closed one. */
	readonly tabs: readonly FloatTab[];
	readonly active: FloatTab;
	/** Changes when the files may have changed, which reads the active tab again. */
	readonly refreshCount: number;
	readonly diffOptions: DiffOptions;
	/** The working-tree files the window edits, which outlive the pane. */
	readonly documents: EditorDocuments;
	/** Opens a file's own tab, where comparing its edits with the disk shows. */
	readonly onOpenFile: (path: string) => void;
	readonly handleRef: React.Ref<EditorPaneHandle>;
	/** A search match to select in the active file once its text shows. */
	readonly reveal: MatchReveal | null;
}

/** A tab's read, stamped with when it started so an older read never undoes a save. */
type Loaded =
	| { readonly kind: 'loading' }
	| { readonly kind: 'read'; readonly content: EditorContent; readonly readAt: number };

const LOADING: Loaded = { kind: 'loading' };

/**
 * The text a diff tab owns: its earlier side, and its later side unless that is the working tree,
 * whose text the documents hold. Released when the tab closes or the pane goes.
 */
interface DiffModels {
	readonly original: MonacoApi.editor.ITextModel;
	readonly modified: MonacoApi.editor.ITextModel | null;
}

/** What the editors show for the active tab, named so its view can be kept when another shows. */
type View =
	| { readonly kind: 'file'; readonly key: string; readonly model: MonacoApi.editor.ITextModel }
	| {
			readonly kind: 'diff';
			readonly key: string;
			readonly original: MonacoApi.editor.ITextModel;
			readonly modified: MonacoApi.editor.ITextModel;
	  };

function disposeDiffModels(entry: DiffModels): void {
	entry.original.dispose();
	entry.modified?.dispose();
}

interface DiffStat {
	readonly added: number;
	readonly removed: number;
	readonly changes: number;
}

type Intl = ReturnType<typeof useOrchbeeIntl>;

/** A byte count as a short size, such as 1.2 MB. */
function formatSize(intl: Intl, byteLength: number): string {
	return intl.formatNumber(byteLength, {
		maximumFractionDigits: 1,
		notation: 'compact',
		style: 'unit',
		unit: 'byte',
		unitDisplay: 'narrow',
	});
}

/** Why the tab shows no content, with the size and limit the note names. */
function noteText(intl: Intl, content: Extract<EditorContent, { kind: 'note' }>): string {
	switch (content.note) {
		case 'binary':
			return intl.formatMessage(workbenchMessages.rightBarPreviewBinary, {
				size: formatSize(intl, content.byteLength),
				type: content.type ?? 'none',
			});
		case 'too-large':
			return intl.formatMessage(workbenchMessages.rightBarPreviewTooLarge, {
				limit: formatSize(intl, content.limit),
				size: formatSize(intl, content.byteLength),
			});
		case 'absent':
			return intl.formatMessage(workbenchMessages.rightBarPreviewAbsent);
		case 'not-repository':
			return intl.formatMessage(workbenchMessages.rightBarNotRepository);
		case 'failed':
			return intl.formatMessage(workbenchMessages.rightBarFailed);
	}
}

/** An image type's short name, such as PNG or SVG. */
function imageTypeName(type: string): string {
	return type
		.slice(type.indexOf('/') + 1)
		.replace(/\+.*$/u, '')
		.toUpperCase();
}

const SIDE_MESSAGES = {
	head: workbenchMessages.rightBarSideHead,
	index: workbenchMessages.rightBarSideIndex,
	none: workbenchMessages.rightBarSideNone,
	worktree: workbenchMessages.rightBarSideWorktree,
} as const;

function pathOf(tab: FloatTab): string {
	return tab.target.kind === 'file' ? tab.target.path : tab.target.item.path;
}

/** A model's address: unique per Workspace, side and path, so two tabs never share one by accident. */
function modelUri(workspaceId: string, side: string, path: string): MonacoApi.Uri {
	return monaco.Uri.from({ path: `/${workspaceId}/${side}/${path}`, scheme: 'orchbee-workspace' });
}

/** Replaces the text of a model nobody edits, keeping the cursor and scroll where they were. */
function updateText(model: MonacoApi.editor.ITextModel, text: string): void {
	if (model.getValue() === text) return;
	model.applyEdits([{ range: model.getFullModelRange(), text }]);
}

function languageFor(path: string, byteLength: number): string {
	if (byteLength > PLAIN_TEXT_BYTES) return 'plaintext';
	return languageForPath(path, monaco.languages.getLanguages());
}

function languageName(id: string): string {
	const language = monaco.languages.getLanguages().find((candidate) => candidate.id === id);
	return language?.aliases?.[0] ?? id;
}

/** The code typography the pane's probe element computes, as Monaco's font options. */
function fontOptions(probe: HTMLElement | null): MonacoApi.editor.IEditorOptions {
	if (probe === null) return {};
	const style = getComputedStyle(probe);
	const fontSize = Number.parseFloat(style.fontSize);
	const lineHeight = Number.parseFloat(style.lineHeight);
	return {
		fontFamily: style.fontFamily,
		...(Number.isFinite(fontSize) ? { fontSize } : {}),
		...(Number.isFinite(lineHeight) ? { lineHeight } : {}),
	};
}

const SHARED_OPTIONS: MonacoApi.editor.IEditorOptions = {
	automaticLayout: true,
	contextmenu: true,
	domReadOnly: false,
	fixedOverflowWidgets: true,
	minimap: { enabled: false },
	readOnly: true,
	renderLineHighlightOnlyWhenFocus: true,
	scrollBeyondLastLine: false,
	smoothScrolling: false,
	stickyScroll: { enabled: false },
};

/**
 * The float window's content: the active tab's file in Monaco's editor, or its change in Monaco's
 * diff editor, under a line naming the file and what the view shows. The pane keeps one editor of
 * each kind and swaps text models between tabs, keeping each tab's scroll and cursor. A working-
 * tree file is editable when it was read whole with its hash; its text lives in the documents, so
 * its file tab and a diff against it edit one text, and edits outlive the pane.
 */
export function EditorPane({
	source,
	workspaceId,
	tabs,
	active,
	refreshCount,
	diffOptions,
	documents,
	onOpenFile,
	handleRef,
	reveal,
}: EditorPaneProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const editorHost = React.useRef<HTMLDivElement | null>(null);
	const diffHost = React.useRef<HTMLDivElement | null>(null);
	const fontProbe = React.useRef<HTMLSpanElement | null>(null);
	const editor = React.useRef<MonacoApi.editor.IStandaloneCodeEditor | null>(null);
	const diff = React.useRef<MonacoApi.editor.IStandaloneDiffEditor | null>(null);
	const diffModels = React.useRef(new Map<string, DiffModels>());
	// Where each view was left, by its key, so a tab shows again as it was.
	const fileViewStates = React.useRef(new Map<string, MonacoApi.editor.ICodeEditorViewState>());
	const diffViewStates = React.useRef(new Map<string, MonacoApi.editor.IDiffEditorViewState>());
	// The files on disk the person compares with their edits, by path; the pane owns them.
	const compareModels = React.useRef(new Map<string, MonacoApi.editor.ITextModel>());
	const shown = React.useRef<View | null>(null);
	// The refresh each tab was last read for; a tab read for an older one reads again.
	const readFor = React.useRef(new Map<string, string>());
	const comparing = React.useRef(new Set<string>());
	const mounted = React.useRef(false);
	React.useEffect(() => {
		mounted.current = true;
		return () => {
			mounted.current = false;
		};
	}, []);
	const [compared, setCompared] = React.useState<ReadonlySet<string>>(() => new Set());
	const [loaded, setLoaded] = React.useState<ReadonlyMap<string, Loaded>>(() => new Map());
	// Reads the active tab again after its edits were thrown away.
	const [reloads, setReloads] = React.useState(0);
	const [diffStat, setDiffStat] = React.useState<DiffStat | null>(null);
	// The view showing, as state, so the render knows which editor to reveal and what to count.
	const [shownView, setShownView] = React.useState<{
		readonly kind: View['kind'];
		readonly model: MonacoApi.editor.ITextModel;
	} | null>(null);
	const shownModel = shownView?.model ?? null;
	const [lineCount, setLineCount] = React.useState(0);
	// The pixel size of the image shown, once it loads, named by its address.
	const [imageSize, setImageSize] = React.useState<{
		readonly url: string;
		readonly width: number;
		readonly height: number;
	} | null>(null);
	React.useSyncExternalStore(documents.subscribe, documents.getVersion);

	// A layout effect, so its cleanup detaches the documents' models before anything releases them.
	React.useLayoutEffect(() => {
		const editorElement = editorHost.current;
		const diffElement = diffHost.current;
		if (editorElement === null || diffElement === null) return;
		const font = fontOptions(fontProbe.current);
		const codeEditor = monaco.editor.create(editorElement, {
			...SHARED_OPTIONS,
			...font,
			model: null,
			theme: EDITOR_THEME,
		});
		const diffEditor = monaco.editor.createDiffEditor(diffElement, {
			...SHARED_OPTIONS,
			...font,
			originalEditable: false,
			renderOverviewRuler: true,
			renderSideBySideInlineBreakpoint: 900,
			theme: EDITOR_THEME,
			useInlineViewWhenSpaceIsLimited: true,
		});
		const update = diffEditor.onDidUpdateDiff(() => {
			const changes = diffEditor.getLineChanges() ?? [];
			let added = 0;
			let removed = 0;
			for (const change of changes) {
				if (change.modifiedEndLineNumber >= change.modifiedStartLineNumber) {
					added += change.modifiedEndLineNumber - change.modifiedStartLineNumber + 1;
				}
				if (change.originalEndLineNumber >= change.originalStartLineNumber) {
					removed += change.originalEndLineNumber - change.originalStartLineNumber + 1;
				}
			}
			setDiffStat({ added, changes: changes.length, removed });
		});
		editor.current = codeEditor;
		diff.current = diffEditor;
		const owned = diffModels.current;
		const fileStates = fileViewStates.current;
		const diffStates = diffViewStates.current;
		const compares = compareModels.current;
		return () => {
			update.dispose();
			// Detach first: the documents' models outlive both editors.
			codeEditor.setModel(null);
			diffEditor.setModel(null);
			codeEditor.dispose();
			diffEditor.dispose();
			for (const entry of owned.values()) disposeDiffModels(entry);
			owned.clear();
			for (const model of compares.values()) model.dispose();
			compares.clear();
			fileStates.clear();
			diffStates.clear();
			editor.current = null;
			diff.current = null;
			shown.current = null;
		};
	}, []);

	React.useEffect(() => {
		diff.current?.updateOptions({
			hideUnchangedRegions: { enabled: diffOptions.hideUnchangedRegions },
			ignoreTrimWhitespace: diffOptions.ignoreTrimWhitespace,
			renderSideBySide: diffOptions.sideBySide,
		});
	}, [diffOptions]);

	React.useImperativeHandle(
		handleRef,
		() => ({
			goToChange: (direction) => {
				diff.current?.goToDiff(direction);
				diff.current?.focus();
			},
		}),
		[],
	);

	// Reads the active tab when it has never been read, or was read before the latest refresh. A
	// read cut short by a re-run, such as the tab being kept, is started again.
	const { id: activeId, target: activeTarget } = active;
	React.useEffect(() => {
		const reads = readFor.current;
		const stamp = `${refreshCount}:${reloads}`;
		if (reads.get(activeId) === stamp) return;
		reads.set(activeId, stamp);
		setLoaded((previous) =>
			previous.has(activeId) ? previous : new Map(previous).set(activeId, LOADING),
		);
		let current = true;
		let landed = false;
		const readAt = documents.now();
		void readEditorContent(source, workspaceId, activeTarget).then((content) => {
			if (current && reads.get(activeId) === stamp) {
				landed = true;
				setLoaded((previous) => new Map(previous).set(activeId, { content, kind: 'read', readAt }));
			}
		});
		return () => {
			current = false;
			if (!landed && reads.get(activeId) === stamp) reads.delete(activeId);
		};
	}, [activeId, activeTarget, documents, refreshCount, reloads, source, workspaceId]);

	// Releases the text of diff tabs the window no longer holds; the documents release their own.
	React.useEffect(() => {
		const open = new Set(tabs.map((tab) => tab.id));
		for (const [id, entry] of diffModels.current) {
			if (open.has(id)) continue;
			disposeDiffModels(entry);
			diffModels.current.delete(id);
		}
		for (const states of [fileViewStates.current, diffViewStates.current]) {
			for (const key of states.keys()) {
				if (!open.has(key.slice(key.indexOf(':') + 1))) states.delete(key);
			}
		}
		for (const id of readFor.current.keys()) {
			if (!open.has(id)) readFor.current.delete(id);
		}
		setLoaded((previous) => {
			if ([...previous.keys()].every((id) => open.has(id))) return previous;
			return new Map([...previous].filter(([id]) => open.has(id)));
		});
	}, [tabs]);

	const read = loaded.get(active.id) ?? LOADING;
	const content: EditorContent | null = read.kind === 'read' ? read.content : null;
	const documentPath = documentPathOf(active.target);
	const document = documentPath === null ? null : documents.state(workspaceId, documentPath);
	// A comparison shows while the conflict it helps with stands.
	const comparesNow =
		documentPath !== null && compared.has(documentPath) && document?.notice?.kind === 'conflict';

	// Puts the active tab's text into its models and shows them, keeping the previous view's place.
	React.useLayoutEffect(() => {
		const codeEditor = editor.current;
		const diffEditor = diff.current;
		if (codeEditor === null || diffEditor === null) return;
		// Models a rebuilt diff no longer uses, released once the editor holds the new ones.
		const retired: MonacoApi.editor.ITextModel[] = [];
		const view = viewFor(active, read);
		const previous = shown.current;
		if (previous?.key !== view?.key || !sameModels(previous, view)) {
			if (previous?.kind === 'file') {
				const state = codeEditor.saveViewState();
				if (state !== null) fileViewStates.current.set(previous.key, state);
			} else if (previous?.kind === 'diff') {
				const state = diffEditor.saveViewState();
				if (state !== null) diffViewStates.current.set(previous.key, state);
			}
			shown.current = view;
			if (view === null) {
				codeEditor.setModel(null);
				diffEditor.setModel(null);
				setShownView(null);
			} else if (view.kind === 'file') {
				diffEditor.setModel(null);
				codeEditor.setModel(view.model);
				const state = fileViewStates.current.get(view.key);
				if (state !== undefined) codeEditor.restoreViewState(state);
				setShownView({ kind: 'file', model: view.model });
			} else {
				codeEditor.setModel(null);
				setDiffStat(null);
				diffEditor.setModel({ modified: view.modified, original: view.original });
				const state = diffViewStates.current.get(view.key);
				if (state !== undefined) diffEditor.restoreViewState(state);
				setShownView({ kind: 'diff', model: view.modified });
			}
		}
		for (const model of retired) model.dispose();
		// A comparison that ended is released once no editor holds it.
		for (const [path, model] of compareModels.current) {
			const stands =
				compared.has(path) && documents.state(workspaceId, path)?.notice?.kind === 'conflict';
			if (stands || (shown.current?.kind === 'diff' && shown.current.original === model)) continue;
			model.dispose();
			compareModels.current.delete(path);
		}

		function sameModels(left: View | null, right: View | null): boolean {
			if (left === null || right === null) return left === right;
			if (left.kind === 'file' && right.kind === 'file') return left.model === right.model;
			if (left.kind === 'diff' && right.kind === 'diff') {
				return left.original === right.original && left.modified === right.modified;
			}
			return false;
		}

		/** The working-tree file's model holding the latest read, or its edits while none landed. */
		function documentModel(
			path: string,
			text: string | null,
			version: Parameters<EditorDocuments['adopt']>[3],
			byteLength: number,
			readAt: number,
		): MonacoApi.editor.ITextModel | null {
			const language = languageFor(path, byteLength);
			if (text === null) return documents.model(workspaceId, path);
			const model = documents.adopt(workspaceId, path, text, version, readAt, (initial) =>
				monaco.editor.createModel(initial, language, modelUri(workspaceId, 'worktree', path)),
			);
			monaco.editor.setModelLanguage(model, language);
			return model;
		}

		/** What the tab shows: made the first time it has text, null while it has none to show. */
		function viewFor(tab: FloatTab, loadedRead: Loaded): View | null {
			const path = pathOf(tab);
			const target = tab.target;
			if (target.kind === 'file') {
				let model: MonacoApi.editor.ITextModel | null = null;
				if (loadedRead.kind === 'loading') {
					// Edits kept while the window was collapsed show at once.
					model = documents.model(workspaceId, path);
				} else if (loadedRead.content.kind === 'file') {
					const { byteLength, text, version } = loadedRead.content;
					model = documentModel(path, text, version, byteLength, loadedRead.readAt);
				} else if (loadedRead.content.kind === 'note') {
					// A file gone from disk, or a read that failed, keeps showing edits not yet saved.
					if (loadedRead.content.note === 'absent') {
						model = documents.gone(workspaceId, path, loadedRead.readAt);
					} else if (documents.state(workspaceId, path)?.dirty === true) {
						model = documents.model(workspaceId, path);
					}
				}
				if (model === null) return null;
				const compare =
					compared.has(path) && documents.state(workspaceId, path)?.notice?.kind === 'conflict'
						? (compareModels.current.get(path) ?? null)
						: null;
				return compare === null
					? { key: `file:${tab.id}`, kind: 'file', model }
					: { key: `compare:${tab.id}`, kind: 'diff', modified: model, original: compare };
			}
			if (loadedRead.kind !== 'read' || loadedRead.content.kind !== 'diff') return null;
			const { after, afterVersion, before, byteLength } = loadedRead.content;
			const { item } = target;
			const language = languageFor(path, byteLength);
			const edits =
				afterVersion === null
					? null
					: documentModel(path, after, afterVersion, byteLength, loadedRead.readAt);
			let owned = diffModels.current.get(tab.id);
			if (owned === undefined || (owned.modified === null) !== (edits !== null)) {
				if (owned !== undefined) {
					retired.push(owned.original);
					if (owned.modified !== null) retired.push(owned.modified);
				}
				const side = `${item.before ?? 'none'}-${item.after}`;
				owned = {
					modified:
						edits === null
							? monaco.editor.createModel(
									after,
									language,
									modelUri(workspaceId, `${side}/after`, path),
								)
							: null,
					original: monaco.editor.createModel(
						before,
						language,
						modelUri(workspaceId, `${side}/before`, path),
					),
				};
				diffModels.current.set(tab.id, owned);
			} else {
				updateText(owned.original, before);
				monaco.editor.setModelLanguage(owned.original, language);
				if (owned.modified !== null) {
					updateText(owned.modified, after);
					monaco.editor.setModelLanguage(owned.modified, language);
				}
			}
			const modified = edits ?? owned.modified;
			if (modified === null) return null;
			return { key: `diff:${tab.id}`, kind: 'diff', modified, original: owned.original };
		}
	}, [active, read, compared, comparesNow, documents, workspaceId]);

	// Selects a search match once its file's text shows; a tab changed meanwhile drops it.
	const revealed = React.useRef(0);
	React.useEffect(() => {
		if (reveal === null || reveal.serial === revealed.current) return;
		if (activeTarget.kind !== 'file' || activeTarget.path !== reveal.path) {
			revealed.current = reveal.serial;
			return;
		}
		let codeEditor: MonacoApi.editor.ICodeEditor | null = null;
		if (shownView?.kind === 'file') codeEditor = editor.current;
		else if (shownView?.kind === 'diff') codeEditor = diff.current?.getModifiedEditor() ?? null;
		if (codeEditor === null || codeEditor.getModel() !== shownView?.model) return;
		revealed.current = reveal.serial;
		if (reveal.range !== null) {
			codeEditor.setSelection(reveal.range);
			codeEditor.revealRangeInCenterIfOutsideViewport(
				reveal.range,
				monaco.editor.ScrollType.Immediate,
			);
		}
		if (reveal.focus) codeEditor.focus();
	}, [activeTarget, reveal, shownView]);

	// Counts the shown text's lines as it is edited.
	React.useEffect(() => {
		if (shownModel === null) return;
		setLineCount(shownModel.getLineCount());
		const listener = shownModel.onDidChangeContent(() => setLineCount(shownModel.getLineCount()));
		return () => listener.dispose();
	}, [shownModel]);

	const showsDocument =
		shownModel !== null &&
		documentPath !== null &&
		shownModel === documents.model(workspaceId, documentPath);
	// A diff whose working-tree side is gone shows a stand-in, which takes no edits.
	const editable = document?.editable === true && showsDocument;
	const limit = formatSize(intl, MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES);

	// Only the working-tree file takes edits; elsewhere typing says why it cannot.
	React.useEffect(() => {
		const readOnlyMessage = {
			value: intl.formatMessage(
				document !== null && !document.editable
					? workbenchMessages.rightBarEditReadOnlyLarge
					: workbenchMessages.rightBarEditReadOnlySide,
				{ limit },
			),
		};
		editor.current?.updateOptions({ readOnly: !editable, readOnlyMessage });
		diff.current?.updateOptions({ readOnly: !editable, readOnlyMessage });
		diff.current?.getOriginalEditor().updateOptions({
			readOnlyMessage: { value: intl.formatMessage(workbenchMessages.rightBarEditReadOnlySide) },
		});
	}, [document, editable, intl, limit]);

	const compare = (): void => {
		if (documentPath === null || comparing.current.has(documentPath)) return;
		const path = documentPath;
		comparing.current.add(path);
		if (active.target.kind === 'diff') onOpenFile(path);
		void readFileChunks(source, workspaceId, path, 'worktree', 'text')
			.then((whole) => {
				// A pane gone meanwhile has released its comparisons and takes no new one.
				if (!mounted.current || whole.kind !== 'text') return;
				if (documents.model(workspaceId, path) === null) return;
				// A comparison that ended but is not released yet gives its address to the new one.
				compareModels.current.get(path)?.dispose();
				compareModels.current.set(
					path,
					monaco.editor.createModel(
						whole.text,
						languageFor(path, whole.byteLength),
						modelUri(workspaceId, 'disk', path),
					),
				);
				setCompared((previous) => new Set(previous).add(path));
			})
			.finally(() => comparing.current.delete(path));
	};
	const stopComparing = (path: string): void => {
		setCompared((previous) => {
			const next = new Set(previous);
			next.delete(path);
			return next;
		});
	};

	const path = pathOf(active);
	const { name, folder } = splitPath(path);
	const meta: string[] = [];
	if (comparesNow && showsDocument && shownView?.kind === 'diff') {
		meta.push(
			intl.formatMessage(workbenchMessages.rightBarCompare, {
				after: intl.formatMessage(workbenchMessages.rightBarSideEdits),
				before: intl.formatMessage(workbenchMessages.rightBarSideDisk),
			}),
		);
	} else if (active.target.kind === 'file' && shownModel !== null) {
		meta.push(languageName(shownModel.getLanguageId()));
		meta.push(intl.formatMessage(workbenchMessages.rightBarLineCount, { count: lineCount }));
		if (content?.kind === 'file') meta.push(formatSize(intl, content.byteLength));
	} else if (content?.kind === 'image') {
		meta.push(imageTypeName(content.type));
		if (imageSize !== null && imageSize.url === content.url) {
			meta.push(
				intl.formatMessage(workbenchMessages.rightBarImageSize, {
					height: imageSize.height,
					width: imageSize.width,
				}),
			);
		}
		meta.push(formatSize(intl, content.byteLength));
	} else if (content?.kind === 'diff' && active.target.kind === 'diff') {
		const { item } = active.target;
		if (diffStat !== null) {
			meta.push(`+${diffStat.added} −${diffStat.removed}`);
			meta.push(
				intl.formatMessage(workbenchMessages.rightBarChangeCount, { count: diffStat.changes }),
			);
		}
		meta.push(
			intl.formatMessage(workbenchMessages.rightBarCompare, {
				after: intl.formatMessage(SIDE_MESSAGES[item.after]),
				before: intl.formatMessage(SIDE_MESSAGES[item.before ?? 'none']),
			}),
		);
	}
	if (document?.saving === true)
		meta.push(intl.formatMessage(workbenchMessages.rightBarFloatSaving));
	const showsDiff = shownView?.kind === 'diff';
	const showsFile = shownView?.kind === 'file';
	const notice = showsDocument ? (document?.notice ?? null) : null;
	return (
		<div className={styles['root']} data-slot='workspace-editor-pane'>
			<span aria-hidden='true' className={styles['fontProbe']} ref={fontProbe} />
			<div className={styles['meta']}>
				<span className={styles['crumb']}>
					{folder === '' ? null : (
						<span className={styles['folder']}>{folder.split('/').join(' › ')} › </span>
					)}
					<span className={styles['name']}>{name}</span>
				</span>
				<span className={styles['facts']}>
					{meta.map((fact) => (
						<span key={fact}>{fact}</span>
					))}
				</span>
			</div>
			{notice === null || documentPath === null ? null : (
				<DocumentNoticeBar
					busy={document?.saving === true}
					limit={limit}
					notice={notice}
					onCompare={
						!comparesNow && notice.kind === 'conflict' && notice.version !== null
							? compare
							: undefined
					}
					onDiscard={() => {
						void documents.reload(source, workspaceId, documentPath).then(() => {
							setReloads((count) => count + 1);
						});
					}}
					onDismiss={() => documents.dismiss(workspaceId, documentPath)}
					onOverwrite={
						notice.kind !== 'conflict' ||
						(notice.version !== null && notice.version.sha256 === null)
							? undefined
							: () => {
									void documents.save(source, workspaceId, documentPath, {
										expected: notice.version?.sha256 ?? null,
									});
								}
					}
					onStopComparing={
						comparesNow && active.target.kind === 'file'
							? () => stopComparing(documentPath)
							: undefined
					}
				/>
			)}
			<div className={styles['body']}>
				<div className={styles['host']} hidden={!showsFile} ref={editorHost} />
				<div className={styles['host']} hidden={!showsDiff} ref={diffHost} />
				{shownModel === null && read.kind === 'loading' ? (
					<p className={styles['note']}>{intl.formatMessage(workbenchMessages.rightBarLoading)}</p>
				) : null}
				{shownModel === null && content?.kind === 'note' ? (
					<p className={styles['note']}>{noteText(intl, content)}</p>
				) : null}
				{content?.kind === 'image' ? (
					<div className={styles['image']}>
						<img
							alt={name}
							key={content.url}
							onLoad={(event) => {
								const { naturalHeight, naturalWidth } = event.currentTarget;
								setImageSize({ height: naturalHeight, url: content.url, width: naturalWidth });
							}}
							src={content.url}
						/>
					</div>
				) : null}
			</div>
		</div>
	);
}
