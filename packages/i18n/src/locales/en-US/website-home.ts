export const enUSWebsiteHomeMessages = {
	'website.home.metaTitle':
		'Orchbee — A modern, multiplexed terminal for running and orchestrating AI agents',
	'website.home.metaDescription':
		'Sessions persist in the background and remain accessible from the terminal, the desktop, and the command line simultaneously, giving people and agents a shared environment to work in.',
	'website.home.title':
		'A modern, multiplexed terminal for running and orchestrating AI\u00A0agents.',
	'website.home.lead':
		'Persistent sessions across the terminal, desktop, and command line—run and coordinate all your agents from one place.',
	'website.home.beta': 'Beta',
	'website.home.stage.view': 'Viewing this Session in the',
	'website.home.stage.closeHint': 'Closing the window does not end the Session.',
	'website.home.stage.close': 'Close window',
	'website.home.stage.awayTitle': 'The Session is running in the background.',
	'website.home.stage.awayBody': 'Its shells and agents continue while no client is attached.',
	'website.home.stage.closedFor': 'Window closed for',
	'website.home.stage.reattach': 'Reattach',
	'website.home.stage.caption':
		'<b>Demo</b>One Session with a shell and two agents, drawn from the real interface and played from a script.',
	'website.home.statement':
		'Orchbee is a modern terminal built for the age of AI agents. <quiet>It is a complete terminal first, built on the core of Ghostty, and everything else is built around it.</quiet>',
	'website.home.sheet.clients': 'Clients',
	'website.home.sheet.clientsValue': 'Terminal interface, desktop application, command line',
	'website.home.sheet.agents': 'Agents',
	'website.home.sheet.agentsValue': 'More than 30 agent CLIs recognized',
	'website.home.sheet.terminal': 'Terminal',
	'website.home.sheet.terminalValue': 'Built on the core of Ghostty',
	'website.home.sheet.platform': 'Platform',
	'website.home.sheet.platformValue':
		'Windows (x86_64) today. macOS and Linux builds are in progress.',
	'website.home.sheet.platformValueMacos':
		'Windows (x86_64) and macOS 13 or later today. Linux builds are in progress.',
	'website.home.sheet.platformValueLinux':
		'Windows (x86_64) and Linux (x86_64 and ARM64) today. macOS builds are in progress.',
	'website.home.sheet.platformValueAll':
		'Windows (x86_64), macOS 13 or later, and Linux (x86_64 and ARM64).',
	'website.home.sheet.privacy': 'Privacy',
	'website.home.sheet.privacyValue':
		'The Orchbee terminal does not use telemetry and never will. We do not upload usage data for analytics, track user behavior, or send automatic crash reports. Anonymous and opt-in telemetry are also prohibited.',
	'website.home.sheet.license': 'License',
	'website.home.sheet.source': 'Source',
	'website.home.multiplexing.kicker': 'multiplexing',
	'website.home.multiplexing.title': 'Multiplexed Sessions',
	'website.home.multiplexing.lead':
		'A Session keeps running in the background after its window is closed, along with the shells and agents in it. The terminal interface, the desktop application, and the command line can attach to the same Session at the same time, and input from one is visible in the others. If the process serving the terminals crashes, a new one takes them over, usually within a second or two.',
	'website.home.multiplexing.input': 'Type here',
	'website.home.multiplexing.session': 'One Session',
	'website.home.multiplexing.reader':
		'reads the same terminal from outside, each time you press Enter',
	'website.home.collaboration.kicker': 'collaboration',
	'website.home.collaboration.title': 'Agent-to-Agent Collaboration',
	'website.home.collaboration.lead':
		"Stop managing agents one terminal at a time. Agents can prompt and spawn other agents, read each other's output, send commands across terminals, delegate tasks, and coordinate work—while you stay in control from one place.",
	'website.home.collaboration.window':
		'Claude Code starting and prompting Codex with orchbee commands',
	'website.home.collaboration.caption':
		'<b>Each step is one <code>orchbee</code> command.</b>On the left, Claude Code runs them through its shell tool. Codex starts in the Panel on the right.',
	'website.home.collaboration.captionStacked':
		'<b>Each step is one <code>orchbee</code> command.</b>In the upper Panel, Claude Code runs them through its shell tool. Codex starts in the Panel below it.',
	'website.home.collaboration.step1': 'Starts Codex in a new Panel.',
	'website.home.collaboration.step2':
		'Prints the handle of each agent, such as <code>@panel:0x36</code>.',
	'website.home.collaboration.step3': 'Submits the prompt to that agent.',
	'website.home.collaboration.step4':
		'Blocks until the agent is idle again, for two minutes at most.',
	'website.home.collaboration.step5': 'Reads the last 40 lines of its terminal.',
	'website.home.collaboration.note':
		'Orchbee does not replace your existing CLI agents; it provides the terminal environment in which they can work together. It shifts your role from <b>human-in-the-loop</b> to <b>human-on-the-loop</b>: instead of driving every step, you supervise the system and step in when needed.',
	'website.home.agents.kicker': 'agents',
	'website.home.agents.title': 'Agent Status and Organization',
	'website.home.agents.lead':
		'Orchbee recognizes more than 30 agent CLIs, presents the status of each at a glance, and organizes work by project in the sidebar.',
	'website.home.agents.recognized': 'Recognized today',
	'website.home.agents.sidebar':
		'The sidebar lists every agent across all projects and organizes work by project, Workspace, and Git worktree.',
	'website.home.agents.projectTitle': 'Project',
	'website.home.agents.project': 'Groups related Workspaces.',
	'website.home.agents.workspaceTitle': 'Workspace',
	'website.home.agents.workspace':
		'A folder and its Git branch. A Git worktree is listed as a Workspace of its own.',
	'website.home.agents.agentTitle': 'Agent',
	'website.home.agents.agent':
		'Listed under its Workspace, with its state and what it is working on.',
	'website.home.agents.unreadTitle': 'Unread',
	'website.home.agents.unread': 'A blue dot marks activity you have not looked at yet.',
	'website.home.terminal.kicker': 'terminal',
	'website.home.terminal.title': 'High-Performance Terminal',
	'website.home.terminal.lead':
		'Terminal emulation runs in a lightweight native process built on the core of Ghostty. The terminal interface and the desktop application receive the finished screen from that process and only draw it. The desktop application renders with WebGPU, with a Canvas 2D fallback.',
	'website.home.terminal.themes':
		'{count, plural, one {# built-in theme} other {# built-in themes}}',
	'website.home.terminal.dark': 'Dark',
	'website.home.terminal.light': 'Light',
	'website.home.terminal.fact1': '24-bit color',
	'website.home.terminal.fact2': 'Ligatures',
	'website.home.terminal.fact3': 'Kitty graphics',
	'website.home.terminal.fact4': 'Hyperlinks',
	'website.home.terminal.fact5': 'IME composition',
	'website.home.terminal.fact6': 'Light and dark theme set separately',
	'website.home.remote.tag': 'In development',
	'website.home.remote.title': 'Remote Machines',
	'website.home.remote.lead':
		'When local resources are not enough, other computers can be added over SSH to extend the compute and execution environments available to agents. Remote machines act as an extension of the local machine, and agents use them without changing their workflow.',
	'website.home.clients.kicker': 'clients',
	'website.home.clients.title': 'Each client in detail',
	'website.home.clients.lead':
		'The terminal interface, the desktop application, and the command line present and control the same Session. Each has a page of its own.',
	'website.home.install.kicker': 'beta',
	'website.home.install.title': 'Install',
	'website.home.install.lead':
		'Orchbee is in beta. It runs on Windows (x86_64) today; macOS and Linux builds are in progress.',
	'website.home.install.releaseNotes': '{version} · Release notes',
	'website.home.install.app': 'Desktop app, terminal and CLI',
	'website.home.install.npm': 'Terminal and CLI only',
	'website.home.install.npmNote':
		'Requires Node.js 22 or later. Installs the terminal interface and the CLI, without the desktop app.',
	'website.home.install.start': 'Start',
	'website.home.install.startNote':
		'Opens the terminal interface. Or open Orchbee from the Start menu.',
	'website.home.install.leadMacos':
		'Orchbee is in beta. It runs on Windows (x86_64) and macOS 13 or later today; Linux builds are in progress.',
	'website.home.install.leadLinux':
		'Orchbee is in beta. It runs on Windows (x86_64) and Linux (x86_64 and ARM64) today; macOS builds are in progress.',
	'website.home.install.leadAll':
		'Orchbee is in beta. It runs on Windows (x86_64), macOS 13 or later, and Linux (x86_64 and ARM64).',
	'website.home.install.platform': 'Platform',
	'website.home.install.tabNpm': 'npm (CLI only)',
	'website.home.install.startNoteMacos':
		'Opens the terminal interface. Or open Orchbee from the Start menu on Windows, or from Applications on a Mac.',
	'website.home.install.startNoteLinux':
		'Opens the terminal interface. Or open Orchbee from the Start menu on Windows, or from the applications menu on Linux.',
	'website.home.install.startNoteAll':
		'Opens the terminal interface. Or open Orchbee from the Start menu on Windows, from Applications on a Mac, or from the applications menu on Linux.',
	'website.home.install.source': 'Get the source and install its dependencies',
	'website.home.install.sourceRequirements':
		'Windows (x86_64). Needs Git, Node.js 22+, Corepack, Rust 1.97.0 and Zig 0.16.0.',
	'website.home.install.sourceStart': 'Start the desktop app',
	'website.home.install.sourceNote':
		'Pre-built apps aren’t available yet; this builds from the source.',
} as const;

export type WebsiteHomeMessageId = keyof typeof enUSWebsiteHomeMessages;
