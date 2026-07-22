import type { CSSProperties, ReactNode } from 'react';
import type { TabAlertState } from '../../../../model';

import { TabAlertIndicatorGlyph } from '../tab-alert-indicator-glyph/TabAlertIndicatorGlyph';
import { useAlertIndicatorButton } from './hooks/useAlertIndicatorButton';

export interface AlertIndicatorButtonProps {
	alertState: TabAlertState;
}

export function AlertIndicatorButton(props: AlertIndicatorButtonProps): ReactNode {
	const view = useAlertIndicatorButton(props);
	if (!view.visible) return null;

	return (
		<span
			className='ct-tab__alert'
			aria-hidden='true'
			title={view.label}
			data-fade-kind={view.snapshot.fadeKind}
			data-alert-state={view.snapshot.showingAlertState}
			data-alert-color={view.colorRole}
			style={
				{
					'--ct-alert-fade-duration': `${view.snapshot.fadeDurationMs}ms`,
				} satisfies CSSProperties & Record<'--ct-alert-fade-duration', string>
			}
		>
			<TabAlertIndicatorGlyph state={view.snapshot.showingAlertState} />
		</span>
	);
}
