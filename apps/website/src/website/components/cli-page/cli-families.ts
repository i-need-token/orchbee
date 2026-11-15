import { websiteCliMessages as messages } from '@orchbee/i18n';

// The command families in the order and groups `orchbee --help` lists them, each with the command
// line's own one-line help. The English catalog holds the help as the program prints it, so the
// drawn `--help` output reads its English text from the same messages the family table shows.
export const CLI_FAMILIES = [
	{
		id: 'terminals',
		title: messages.familiesTerminals,
		rows: [
			{ command: 'tui', help: messages.familyTui },
			{ command: 'workspace', help: messages.familyWorkspace },
			{ command: 'tab', help: messages.familyTab },
			{ command: 'panel', help: messages.familyPanel },
			{ command: 'terminal', help: messages.familyTerminal },
			{ command: 'layout', help: messages.familyLayout },
			{ command: 'client', help: messages.familyClient },
		],
	},
	{
		id: 'agents',
		title: messages.familiesAgents,
		rows: [
			{ command: 'agent', help: messages.familyAgent },
			{ command: 'project', help: messages.familyProject },
			{ command: 'worktree', help: messages.familyWorktree },
			{ command: 'launch-profile', help: messages.familyLaunchProfile },
			{ command: 'command', help: messages.familyCommand },
			{ command: 'events', help: messages.familyEvents },
			{ command: 'notification', help: messages.familyNotification },
		],
	},
	{
		id: 'sessions',
		title: messages.familiesSessions,
		rows: [
			{ command: 'status', help: messages.familyStatus },
			{ command: 'doctor', help: messages.familyDoctor },
			{ command: 'operation', help: messages.familyOperation },
			{ command: 'server', help: messages.familyServer },
			{ command: 'session', help: messages.familySession },
			{ command: 'machine', help: messages.familyMachine },
		],
	},
	{
		id: 'setup',
		title: messages.familiesSetup,
		rows: [
			{ command: 'config', help: messages.familyConfig },
			{ command: 'diagnostics', help: messages.familyDiagnostics },
			{ command: 'update', help: messages.familyUpdate },
			{ command: 'uninstall', help: messages.familyUninstall },
			{ command: 'api', help: messages.familyApi },
			{ command: 'completion', help: messages.familyCompletion },
		],
	},
] as const;
