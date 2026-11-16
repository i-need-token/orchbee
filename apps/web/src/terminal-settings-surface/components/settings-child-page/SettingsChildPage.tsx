import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button, ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';
import { createPortal } from 'react-dom';

import { useSettingsChildPageEntry } from './hooks/useSettingsChildPageEntry';
import styles from './SettingsChildPage.module.css';

interface SettingsChildPageProps {
	readonly children: React.ReactNode;
	/** Leaves the page; a page that edits several values commits them first. */
	readonly onBack: () => void;
	readonly title: string;
}

/**
 * A purpose-named page that an action opens in place of its category, with Back above its title,
 * so the list it came from never grows. Rendering it opens it; Back, Escape, another category or
 * a search leave it.
 */
export function SettingsChildPage({
	children,
	onBack,
	title,
}: SettingsChildPageProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	// The title takes focus as the page opens, so a screen reader announces where Back leads from.
	const titleReference = React.useRef<HTMLHeadingElement>(null);
	const host = useSettingsChildPageEntry(onBack, titleReference);
	const page = (
		<section aria-label={title} className={styles['root']} data-slot='settings-child-page'>
			<Button className={styles['back']} size='sm' type='button' variant='ghost' onClick={onBack}>
				<ProductIcon name='arrow-left' />
				{intl.formatMessage(terminalSettingsMessages.pageBack)}
			</Button>
			<h1 ref={titleReference} className={styles['title']} tabIndex={-1}>
				{title}
			</h1>
			{children}
		</section>
	);
	return host === null ? page : createPortal(page, host);
}
