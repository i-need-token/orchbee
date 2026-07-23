import type { Size } from './proposed-layout';

export interface CollectionChildLayoutData {
	id: string;
	preferredSize: Readonly<Size>;
	minimumSize?: Readonly<Size>;
	visible?: boolean;
	group?: boolean;
	drag?: {
		x: number;
		y: number;
		shouldHide?: boolean;
		shouldFloat?: boolean;
	};
}
