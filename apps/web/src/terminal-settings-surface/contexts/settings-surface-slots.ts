import { createContext } from 'react';

/** A child page open in place of its category: how Escape leaves it. */
export interface SettingsChildPageEntry {
	readonly back: () => void;
}

/**
 * Where Settings shows what an action opens or reports, so no action changes a row's height: a
 * child page replaces its category's content behind a route back, and a notice shows on the
 * dialog's notice line over the content's foot.
 */
export interface SettingsSurfaceSlots {
	readonly childPageHost: HTMLElement | null;
	/** Hides the category under the child page until the returned leave runs. */
	readonly enterChildPage: (entry: SettingsChildPageEntry) => () => void;
	readonly noticeHost: HTMLElement | null;
}

export const SettingsSurfaceSlotsContext = createContext<SettingsSurfaceSlots | null>(null);
