import { codeEditorMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import { splitFileTabPath } from '../../../../../../../model';
import {
	useFileOverlayHostContents,
	useFileOverlayHostPlatform,
} from '../../../../contexts/file-overlay-host-store';
import { useFileTabDocuments } from './hooks/useFileTabDocuments';
import styles from './FileTabContent.module.css';

/**
 * The active tab's content: its file in the text editor or the image preview, each keeping one
 * editor while it shows and moving it from file to file. Nothing shows while the code editor loads,
 * and a chunk that failed to load says so, as VS Code says an editor failed to open, with Try Again.
 */
export function FileTabContent(): ReactNode {
	const intl = useOrchbeeIntl();
	const contents = useFileOverlayHostContents();
	const platform = useFileOverlayHostPlatform();
	const { load, retry, shown } = useFileTabDocuments(contents);
	if (load.kind === 'failed') {
		return (
			<div className={styles['failure']} data-slot='file-tab-content-failure'>
				<p className={styles['message']}>{intl.formatMessage(codeEditorMessages.failed)}</p>
				<Button onClick={retry}>{intl.formatMessage(codeEditorMessages.retry)}</Button>
			</div>
		);
	}
	if (load.kind !== 'ready' || shown === null || contents === null) return null;
	const { runtime } = load;
	const { hold, tabId } = shown;
	const { name } = splitFileTabPath(shown.path);
	if (hold.kind === 'image') {
		return (
			<runtime.ImagePreview
				document={hold.document}
				name={name}
				onOpenAsText={() => contents.openAsText(tabId)}
				platform={platform}
				viewStateKey={hold.viewStateKey}
				viewStates={hold.viewStates}
			/>
		);
	}
	return (
		<runtime.CodeEditorTextView
			document={hold.document}
			name={name}
			runtime={runtime}
			viewStateKey={hold.viewStateKey}
			viewStates={hold.viewStates}
		/>
	);
}
