import type * as React from 'react';

import styles from './SettingFailure.module.css';

/** The message under a row whose last change did not land, or under a draft that was not admitted. */
export function SettingFailure({ message }: { readonly message: string | null }): React.ReactNode {
	if (message === null) return null;
	return (
		<div className={styles['root']} role='alert'>
			{message}
		</div>
	);
}
