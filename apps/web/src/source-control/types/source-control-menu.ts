/** One menu row: a command, or one choice of a group that checks the choice in force. */
export interface SourceControlMenuCommand {
	readonly checked: boolean;
	readonly disabled: boolean;
	readonly id: string;
	readonly label: string;
	readonly onSelect: () => void;
}

/** The rows between two separators; a choice group's rows are its choices. */
export interface SourceControlMenuGroup {
	readonly choice: boolean;
	readonly commands: readonly SourceControlMenuCommand[];
}
