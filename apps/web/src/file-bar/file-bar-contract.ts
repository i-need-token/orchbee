import type { ComponentType } from 'react';

/** What the File Bar's chunk gives the Workbench, which places it and decides when it shows. */
export interface FileBarModule {
	/** The column's content inside the Workbench's File Bar landmark, from its 36px row down. */
	readonly FileBarColumn: ComponentType;
}
