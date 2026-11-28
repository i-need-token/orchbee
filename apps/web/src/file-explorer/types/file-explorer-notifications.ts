/**
 * The Workbench's notification cards, where the Explorer reports what failed out of sight of the
 * person, such as a folder it could not list.
 */
export interface FileExplorerNotifications {
	notify(request: { readonly message: string; readonly severity: 'error' }): Promise<unknown>;
}
