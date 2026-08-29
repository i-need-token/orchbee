import styles from './TabIcon.module.css';
import type { ReactNode } from 'react';
import type { TabData } from '../../../../../../model/tab-model';
import { TabStripIcon } from '../../../tab-strip-icon/TabStripIcon';
import type { TabStatusAdornment } from '../../../../../../types/tab-strip-options';

import { useTabIcon } from './hooks/useTabIcon';

export interface TabIconProps {
	tab: Readonly<TabData>;
	availableWidth?: number;
	discardRingTreatmentEnabled?: boolean;
	identityIcon?: ReactNode | undefined;
	statusAdornment?: Readonly<TabStatusAdornment> | undefined;
}

export function TabIcon(props: TabIconProps): ReactNode {
	const view = useTabIcon(props);
	let favicon: ReactNode;
	if (props.identityIcon !== undefined) {
		favicon = <span className={styles['ct-tab-icon__status-icon']}>{props.identityIcon}</span>;
	} else if (props.statusAdornment) {
		favicon = (
			<span className={styles['ct-tab-icon__status-icon']}>{props.statusAdornment.icon}</span>
		);
	} else if (props.tab.faviconUrl) {
		favicon = (
			<img
				className={styles['ct-tab-icon__favicon']}
				data-tab-icon-favicon=''
				src={props.tab.faviconUrl}
				alt=''
				draggable={false}
			/>
		);
	} else {
		favicon = <TabStripIcon name='default-favicon' variantClass='ct-tab-icon__favicon' />;
	}

	return (
		<span
			className={styles['ct-tab-icon']}
			data-tab-icon=''
			aria-hidden='true'
			data-network-state={view.visual.networkState}
			data-loading={view.visual.showingLoadingAnimation || undefined}
			data-show-favicon={view.visual.showingFavicon || undefined}
			data-custom-favicon={view.visual.hasCustomFavicon || undefined}
			data-crashed={view.visual.showingCrashedFavicon || undefined}
			data-crash-transition={view.crashTransition || undefined}
			data-attention={view.visual.showingAttentionIndicator || undefined}
			data-discarded={view.visual.showingDiscardIndicator || undefined}
			data-expanded-discard-ring={view.visual.increasedDiscardIndicatorRadius > 0 || undefined}
			data-identity-icon={props.identityIcon !== undefined || undefined}
			data-status-adornment={props.statusAdornment?.kind}
		>
			<span className={styles['ct-tab-icon__normal']}>{favicon}</span>
			<TabStripIcon name='sad-tab' variantClass='ct-tab-icon__crashed' />
			{view.visual.showingLoadingAnimation ? (
				<TabStripIcon name='spinner' variantClass='ct-tab-icon__spinner' />
			) : null}
			{view.visual.showingDiscardIndicator ? (
				<span className={styles['ct-tab-icon__discard-ring']} />
			) : null}
			{view.visual.showingAttentionIndicator ? (
				<span className={styles['ct-tab-icon__attention']} />
			) : null}
			{props.statusAdornment ? (
				<span className={styles['ct-tab-icon__status']} data-tab-icon-status='' />
			) : null}
		</span>
	);
}
