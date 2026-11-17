import styles from './AlertIndicatorButton.module.css';
import { Tooltip, TooltipContent, TooltipTrigger } from '@orchbee/ui/components';
import type { CSSProperties, ReactNode } from 'react';
import type { TabAlertState } from '../../../../../../model/tab-model';

import { TabAlertIndicatorGlyph } from '../../../tab-alert-indicator-glyph/TabAlertIndicatorGlyph';
import { useAlertIndicatorButton } from './hooks/useAlertIndicatorButton';

interface AlertIndicatorButtonProps {
	alertState: TabAlertState;
	dragging?: boolean | undefined;
	tooltipSide?: 'bottom' | 'right' | undefined;
}

export function AlertIndicatorButton(props: AlertIndicatorButtonProps): ReactNode {
	const view = useAlertIndicatorButton(props);
	if (!view.visible) return null;

	return (
		<Tooltip disabled={props.dragging ?? false}>
			<TooltipTrigger
				render={
					<span
						className={styles['ct-tab__alert']}
						data-tab-alert-indicator=''
						aria-hidden='true'
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
				}
			/>
			<TooltipContent side={props.tooltipSide ?? 'bottom'}>{view.label}</TooltipContent>
		</Tooltip>
	);
}
