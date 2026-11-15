export const enUSWebsiteCliMessages = {
	'website.cli.metaTitle': 'Command line — Orchbee',
	'website.cli.metaDescription':
		'Every operation available to a person is equally available to an agent through the orchbee command line.',
	'website.cli.lead':
		'Every operation available to a person is equally available to an agent through the <code>orchbee</code> command line.',
	'website.cli.stage.caption':
		'<b>Demo</b>A terminal outside Orchbee, asking the running Session about its agents.',
	'website.cli.grammar.kicker': 'grammar',
	'website.cli.grammar.title': 'One shape for every command',
	'website.cli.grammar.lead':
		'A family, an action, a target, then options. The same handle names the same Panel in every command.',
	'website.cli.grammar.family.title': 'Family',
	'website.cli.grammar.family.body':
		'<code>agent</code>, <code>terminal</code>, <code>panel</code>, <code>tab</code>, <code>workspace</code>, and the rest of the table below.',
	'website.cli.grammar.action.title': 'Action',
	'website.cli.grammar.action.body':
		'What to do: <code>list</code>, <code>start</code>, <code>prompt</code>, <code>wait</code>, <code>read</code>.',
	'website.cli.grammar.target.title': 'Target',
	'website.cli.grammar.target.body':
		'A handle printed by <code>orchbee agent list</code>, or the name you gave an agent.',
	'website.cli.grammar.argument.title': 'Argument',
	'website.cli.grammar.argument.body': 'Here, the prompt to submit.',
	'website.cli.grammar.option.title': 'Option',
	'website.cli.grammar.option.body':
		'<code>--wait</code> keeps the command open until the agent is idle, done, or blocked.',
	'website.cli.output.kicker': 'output',
	'website.cli.output.title': 'What the commands print',
	'website.cli.output.lead': 'Text for people by default. Choose a command.',
	'website.cli.output.list': 'Commands',
	'website.cli.output.note': 'The JSON document is abridged here.',
	'website.cli.highlights.title': 'Made for scripts and for agents',
	'website.cli.highlight.handles.title': 'Handles',
	'website.cli.highlight.handles.body':
		'Every Panel, Tab, Workspace, and Project has a handle such as <code>@panel:0x36</code>. Commands print handles and take them, so the output of one command is the input of the next.',
	'website.cli.highlight.formats.title': 'Text or JSON',
	'website.cli.highlight.formats.body':
		'Text for people by default. Add <code>--format json</code> where a command supports it and get a document with a schema name and a version, for scripts.',
	'website.cli.highlight.waiting.title': 'Waiting is a command',
	'website.cli.highlight.waiting.body':
		'<code>orchbee agent wait</code> blocks until an agent is idle, done, or blocked. <code>orchbee terminal wait-output</code> blocks until text appears. No polling loops.',
	'website.cli.highlight.guide.title': 'A guide for agents, built in',
	'website.cli.highlight.guide.body':
		'<code>orchbee --skill</code> prints an offline guide written for agents: what the commands are and how to use them. An agent that has read it can drive Orchbee.',
	'website.cli.highlight.completion.title': 'Completion for five shells',
	'website.cli.highlight.completion.body':
		'<code>orchbee completion</code> generates scripts for bash, elvish, fish, PowerShell, and zsh.',
	'website.cli.highlight.exitCodes.title': 'Exit codes that mean something',
	'website.cli.highlight.exitCodes.body':
		'<code>0</code> for success, <code>2</code> for invalid usage, <code>3</code> when the target is not found, <code>7</code> on a timeout, <code>130</code> when cancelled. Scripts can branch on them.',
	'website.cli.families.title': 'Command families',
	'website.cli.families.lead':
		"<code>orchbee '<'command'>' -h</code> prints the help of any of them.",
	'website.cli.families.terminals': 'Terminals and layout',
	'website.cli.families.agents': 'Agents and workflows',
	'website.cli.families.sessions': 'Sessions and connections',
	'website.cli.families.setup': 'Setup and integration',
	'website.cli.family.tui': 'Open the terminal interface.',
	'website.cli.family.workspace': 'Create and organize workspaces.',
	'website.cli.family.tab': 'Create, arrange and close tabs.',
	'website.cli.family.panel': 'Split, arrange and control terminal panels.',
	'website.cli.family.terminal': 'Read terminal output and send input.',
	'website.cli.family.layout': 'Save, restore and adjust terminal layouts.',
	'website.cli.family.client': 'Inspect connected terminal clients.',
	'website.cli.family.agent': 'Find, run and interact with AI Agents.',
	'website.cli.family.project': 'Group related workspaces into projects.',
	'website.cli.family.worktree': 'Create and manage Git worktrees.',
	'website.cli.family.launchProfile': 'Manage reusable terminal launch settings.',
	'website.cli.family.command': 'Find and run your configured commands.',
	'website.cli.family.events': 'Observe events from a running session.',
	'website.cli.family.notification': 'Show notifications in the terminal interface.',
	'website.cli.family.status': 'Show the current terminal session status.',
	'website.cli.family.doctor': 'Check whether Orchbee works here.',
	'website.cli.family.operation': 'Inspect or continue an earlier operation by its request ID.',
	'website.cli.family.server': "Manage the session's background service.",
	'website.cli.family.session': 'Find and reconnect to named local sessions.',
	'website.cli.family.machine': 'Manage saved remote machines.',
	'website.cli.family.config': 'Check, print and reload settings.',
	'website.cli.family.diagnostics': 'Collect diagnostics to send when you ask for help.',
	'website.cli.family.update': 'Install the newest version of the channel.',
	'website.cli.family.uninstall': 'Remove the installed Orchbee; data stays unless --delete-data.',
	'website.cli.family.api': 'Read snapshots and offline API schemas.',
	'website.cli.family.completion': 'Generate shell completion scripts.',
} as const;

export type WebsiteCliMessageId = keyof typeof enUSWebsiteCliMessages;
