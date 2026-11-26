import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import type * as MonacoApi from 'monaco-editor/editor/editor.api';
import * as React from 'react';

import type { WorkbenchWorkspaceFilesSource } from '../../../../../workbench';
import { splitPath } from '../../../../change-groups';
import { languageForPath } from './languageForPath';
import { EDITOR_THEME } from './monaco-theme';
import { startMonaco } from './startMonaco';
import type { FloatTab } from '../../../../float-tabs';
import { type EditorContent, readEditorContent } from './readEditorContent';

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
	readonly handleRef: React.Ref<EditorPaneHandle>;
}

type Loaded = EditorContent | { readonly kind: 'loading' };

const LOADING: Loaded = { kind: 'loading' };

/** One tab's text models, and where its view stood when another tab took its place. */
/** A tab's text and where its view was left: one model for a file, two for a diff. */
type TabModels =
	| {
			readonly kind: 'file';
			readonly model: MonacoApi.editor.ITextModel;
			viewState: MonacoApi.editor.ICodeEditorViewState | null;
	  }
	| {
			readonly kind: 'diff';
			readonly original: MonacoApi.editor.ITextModel;
			readonly modified: MonacoApi.editor.ITextModel;
			viewState: MonacoApi.editor.IDiffEditorViewState | null;
	  };

function disposeModels(entry: TabModels): void {
	if (entry.kind === 'file') {
		entry.model.dispose();
	} else {
		entry.original.dispose();
		entry.modified.dispose();
	}
}

interface DiffStat {
	readonly added: number;
	readonly removed: number;
	readonly changes: number;
}

const NOTE_MESSAGES = {
	absent: workbenchMessages.rightBarPreviewAbsent,
	binary: workbenchMessages.rightBarPreviewBinary,
	failed: workbenchMessages.rightBarFailed,
	'not-repository': workbenchMessages.rightBarNotRepository,
	'too-large': workbenchMessages.rightBarPreviewTooLarge,
} as const;

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

/** Replaces a model's text only when it changed, keeping the cursor and scroll where they were. */
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
 * each kind and swaps text models between tabs, keeping each tab's scroll and cursor.
 */
export function EditorPane({
	source,
	workspaceId,
	tabs,
	active,
	refreshCount,
	diffOptions,
	handleRef,
}: EditorPaneProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const editorHost = React.useRef<HTMLDivElement | null>(null);
	const diffHost = React.useRef<HTMLDivElement | null>(null);
	const fontProbe = React.useRef<HTMLSpanElement | null>(null);
	const editor = React.useRef<MonacoApi.editor.IStandaloneCodeEditor | null>(null);
	const diff = React.useRef<MonacoApi.editor.IStandaloneDiffEditor | null>(null);
	const models = React.useRef(new Map<string, TabModels>());
	const shown = React.useRef<string | null>(null);
	// The refresh each tab was last read for; a tab read for an older one reads again.
	const readFor = React.useRef(new Map<string, number>());
	const [loaded, setLoaded] = React.useState<ReadonlyMap<string, Loaded>>(() => new Map());
	const [diffStat, setDiffStat] = React.useState<DiffStat | null>(null);

	React.useEffect(() => {
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
		const owned = models.current;
		return () => {
			update.dispose();
			codeEditor.dispose();
			diffEditor.dispose();
			for (const entry of owned.values()) disposeModels(entry);
			owned.clear();
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

	// Reads the active tab when it has never been read, or was read before the latest refresh.
	React.useEffect(() => {
		if (readFor.current.get(active.id) === refreshCount) return;
		readFor.current.set(active.id, refreshCount);
		const { id } = active;
		setLoaded((previous) => (previous.has(id) ? previous : new Map(previous).set(id, LOADING)));
		let current = true;
		void readEditorContent(source, workspaceId, active.target).then((content) => {
			if (current && readFor.current.get(id) === refreshCount) {
				setLoaded((previous) => new Map(previous).set(id, content));
			}
		});
		return () => {
			current = false;
		};
	}, [active, refreshCount, source, workspaceId]);

	// Releases the text of tabs the window no longer holds.
	React.useEffect(() => {
		const open = new Set(tabs.map((tab) => tab.id));
		for (const [id, entry] of models.current) {
			if (open.has(id)) continue;
			disposeModels(entry);
			models.current.delete(id);
			readFor.current.delete(id);
		}
		setLoaded((previous) => {
			if ([...previous.keys()].every((id) => open.has(id))) return previous;
			return new Map([...previous].filter(([id]) => open.has(id)));
		});
	}, [tabs]);

	const content = loaded.get(active.id) ?? LOADING;

	// Puts the active tab's text into its models and shows them, saving the previous tab's view.
	React.useLayoutEffect(() => {
		const codeEditor = editor.current;
		const diffEditor = diff.current;
		if (codeEditor === null || diffEditor === null) return;
		const previous = shown.current === null ? undefined : models.current.get(shown.current);
		if (previous !== undefined && shown.current !== active.id) {
			if (previous.kind === 'file') previous.viewState = codeEditor.saveViewState();
			else previous.viewState = diffEditor.saveViewState();
		}
		const entry = modelsFor(active, content);
		if (entry === null) {
			if (shown.current !== active.id) {
				codeEditor.setModel(null);
				diffEditor.setModel(null);
				shown.current = null;
			}
			return;
		}
		if (shown.current === active.id) return;
		shown.current = active.id;
		if (entry.kind === 'file') {
			diffEditor.setModel(null);
			codeEditor.setModel(entry.model);
			if (entry.viewState !== null) codeEditor.restoreViewState(entry.viewState);
		} else {
			codeEditor.setModel(null);
			setDiffStat(null);
			diffEditor.setModel({ modified: entry.modified, original: entry.original });
			if (entry.viewState !== null) diffEditor.restoreViewState(entry.viewState);
		}

		/** The tab's models holding its latest text, made the first time it shows; null while it has none. */
		function modelsFor(tab: FloatTab, read: Loaded): TabModels | null {
			const path = pathOf(tab);
			const existing = models.current.get(tab.id);
			if (read.kind === 'file') {
				if (existing?.kind === 'file') {
					updateText(existing.model, read.text);
					return existing;
				}
				const created: TabModels = {
					kind: 'file',
					model: monaco.editor.createModel(
						read.text,
						languageFor(path, read.byteLength),
						modelUri(workspaceId, 'worktree', path),
					),
					viewState: null,
				};
				models.current.set(tab.id, created);
				return created;
			}
			if (read.kind !== 'diff' || tab.target.kind !== 'diff') return null;
			if (existing?.kind === 'diff') {
				updateText(existing.original, read.before);
				updateText(existing.modified, read.after);
				return existing;
			}
			const { item } = tab.target;
			const side = `${item.before ?? 'none'}-${item.after}`;
			const language = languageFor(path, read.byteLength);
			const created: TabModels = {
				kind: 'diff',
				modified: monaco.editor.createModel(
					read.after,
					language,
					modelUri(workspaceId, `${side}/after`, path),
				),
				original: monaco.editor.createModel(
					read.before,
					language,
					modelUri(workspaceId, `${side}/before`, path),
				),
				viewState: null,
			};
			models.current.set(tab.id, created);
			return created;
		}
	}, [active, content, workspaceId]);

	const path = pathOf(active);
	const { name, folder } = splitPath(path);
	const meta: string[] = [];
	if (content.kind === 'file') {
		meta.push(languageName(languageFor(path, content.byteLength)));
		meta.push(
			intl.formatMessage(workbenchMessages.rightBarLineCount, {
				count: content.text === '' ? 1 : content.text.split('\n').length,
			}),
		);
		meta.push(
			intl.formatNumber(content.byteLength, {
				notation: 'compact',
				style: 'unit',
				unit: 'byte',
				unitDisplay: 'narrow',
			}),
		);
	} else if (content.kind === 'diff' && active.target.kind === 'diff') {
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
	const showsDiff = content.kind === 'diff';
	const showsFile = content.kind === 'file';
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
			<div className={styles['body']}>
				<div className={styles['host']} hidden={!showsFile} ref={editorHost} />
				<div className={styles['host']} hidden={!showsDiff} ref={diffHost} />
				{content.kind === 'loading' ? (
					<p className={styles['note']}>{intl.formatMessage(workbenchMessages.rightBarLoading)}</p>
				) : null}
				{content.kind === 'note' ? (
					<p className={styles['note']}>{intl.formatMessage(NOTE_MESSAGES[content.note])}</p>
				) : null}
			</div>
		</div>
	);
}
