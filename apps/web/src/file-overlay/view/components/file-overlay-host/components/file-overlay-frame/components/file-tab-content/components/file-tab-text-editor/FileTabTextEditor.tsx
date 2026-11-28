import { ContextMenu, ContextMenuTrigger } from '@orchbee/ui/components';
import { useRef, useState, type ComponentProps, type ReactNode } from 'react';

import {
	codeEditorKeyChords,
	type CodeEditorTextHandle,
	type loadCodeEditor,
} from '../../../../../../../../../../code-editor';
import { useWorkbenchKeyboardScope } from '../../../../../../../../../../workbench-keyboard-scopes';
import {
	useFileOverlayHostEditor,
	useFileOverlayHostPlatform,
} from '../../../../../../contexts/file-overlay-host-store';
import { FileTabEditorMenu } from './components/file-tab-editor-menu/FileTabEditorMenu';
import { useFileTabEditorActions } from './hooks/useFileTabEditorActions';
import { useFileTabEditorMenu } from './hooks/useFileTabEditorMenu';
import styles from './FileTabTextEditor.module.css';

type CodeEditorRuntime = Awaited<ReturnType<typeof loadCodeEditor>>;
type TextViewProps = ComponentProps<CodeEditorRuntime['CodeEditorTextView']>;

interface FileTabTextEditorProps {
	readonly document: TextViewProps['document'];
	readonly name: string;
	readonly runtime: CodeEditorRuntime;
	readonly viewStateKey: string;
	readonly viewStates: TextViewProps['viewStates'];
}

/**
 * The active tab's text editor with what VS Code gives an editor around it: the keys Monaco binds,
 * which no wider scope takes while focus is in the editor, the editor's commands as Actions that its
 * `Mod+K` keys and Action search run, and VS Code's editor menu in place of Monaco's.
 */
export function FileTabTextEditor({
	document,
	name,
	runtime,
	viewStateKey,
	viewStates,
}: FileTabTextEditorProps): ReactNode {
	const platform = useFileOverlayHostPlatform();
	const host = useFileOverlayHostEditor();
	const rootReference = useRef<HTMLDivElement>(null);
	const [handle, setHandle] = useState<CodeEditorTextHandle | null>(null);
	const actionScope = useFileTabEditorActions(handle, host);
	useWorkbenchKeyboardScope(rootReference, {
		actionScope,
		kind: 'code-editor',
		yields: codeEditorKeyChords(platform, navigator.userAgent),
	});
	const menu = useFileTabEditorMenu(handle, host, platform);
	return (
		<ContextMenu open={menu.open} onOpenChange={menu.onOpenChange}>
			<ContextMenuTrigger className={styles['root']} ref={rootReference}>
				<runtime.CodeEditorTextView
					document={document}
					name={name}
					onHandle={setHandle}
					runtime={runtime}
					viewStateKey={viewStateKey}
					viewStates={viewStates}
				/>
			</ContextMenuTrigger>
			<FileTabEditorMenu
				entries={menu.entries}
				finalFocus={menu.finalFocus}
				onChoose={menu.choose}
			/>
		</ContextMenu>
	);
}
