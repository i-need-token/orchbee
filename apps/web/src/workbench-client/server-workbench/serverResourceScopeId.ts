/** A Server Workspace reads as owning one resource scope of its own, which no Client can edit. */
export function serverResourceScopeId(workspaceId: string): string {
	return `resource-scope:${workspaceId}`;
}
