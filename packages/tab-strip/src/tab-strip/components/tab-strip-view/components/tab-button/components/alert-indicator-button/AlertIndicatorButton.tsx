import styles from './AlertIndicatorButton.module.css';
import type { CSSProperties, ReactNode } from 'react';
import type { TabAlertState } from '../../../../../../model/tab-model';

import { TabAlertIndicatorGlyph } from '../../../tab-alert-indicator-glyph/TabAlertIndicatorGlyph';
import { useAlertIndicatorButton } from './hooks/useAlertIndicatorButton';

interface AlertIndicatorButtonProps {
	alertState: TabAlertState;
}

export function AlertIndicatorButton(props: AlertIndicatorButtonProps): ReactNode {
	const view = useAlertIndicatorButton(props);
	if (!view.visible) return null;

	return (
		<span
			className={styles['ct-tab__alert']}
			data-tab-alert-indicator=''
			aria-hidden='true'
			title={view.label}
			data-fade-kind={view.snapshot.fadeKind}
			data-alert-state={view.snapshot.showingAlertState}
			data-alert-color={view.colorRole}
			style={
				{
					'--ob-comp-tab-strip-alert-fade-duration': `${view.snapshot.fadeDurationMs}ms`,
				} satisfies CSSProperties & Record<'--ob-comp-tab-strip-alert-fade-duration', string>
			}
		>
			<TabAlertIndicatorGlyph state={view.snapshot.showingAlertState} />
		</span>
	);
}
