import {
	SEMANTIC_COMMAND_CLI_COMMANDS,
	SEMANTIC_COMMAND_CLI_COMMON_OPTIONS,
	type SemanticCommandCliGrammarCommand,
	type SemanticCommandCliShell,
} from './semantic-command-cli-grammar';

const TOP_WORDS = [
	...new Set(SEMANTIC_COMMAND_CLI_COMMANDS.map((command) => command.path[0])),
	'help',
];

const SINGLE_WORD_COMMANDS = SEMANTIC_COMMAND_CLI_COMMANDS.filter(
	(command) => command.path.length === 1,
).map((command) => command.path[0]);

const FAMILIES: readonly { readonly name: string; readonly verbs: readonly string[] }[] =
	Object.entries(
		SEMANTIC_COMMAND_CLI_COMMANDS.filter((command) => command.path.length === 2).reduce<
			Record<string, string[]>
		>((accumulator, command) => {
			const name = command.path[0];
			if (name !== undefined) {
				accumulator[name] = [...(accumulator[name] ?? []), command.path[1] ?? ''];
			}
			return accumulator;
		}, {}),
	).map(([name, verbs]) => ({ name, verbs }));

const VALUE_OPTIONS = [
	...new Set(
		[
			...SEMANTIC_COMMAND_CLI_COMMON_OPTIONS,
			...SEMANTIC_COMMAND_CLI_COMMANDS.flatMap((command) => command.options),
		]
			.filter((option) => option.takesValue)
			.map((option) => `--${option.name}`),
	),
];

const CLOSED_VALUES: readonly { readonly option: string; readonly values: readonly string[] }[] = [
	...SEMANTIC_COMMAND_CLI_COMMON_OPTIONS,
	...SEMANTIC_COMMAND_CLI_COMMANDS.flatMap((command) => command.options),
]
	.filter(
		(option): option is typeof option & { values: readonly string[] } =>
			option.takesValue && option.values !== undefined,
	)
	.map((option) => ({ option: `--${option.name}`, values: option.values }));

const OPEN_VALUE_OPTIONS = VALUE_OPTIONS.filter(
	(option) => !CLOSED_VALUES.some((closed) => closed.option === option),
);

function commandOptionWords(command: SemanticCommandCliGrammarCommand): string {
	const options = [
		...command.options.map((option) => `--${option.name}`),
		...(command.coordinator
			? SEMANTIC_COMMAND_CLI_COMMON_OPTIONS.map((option) => `--${option.name}`)
			: []),
	];
	return options.join(' ');
}

const PATH_OPTIONS: readonly { readonly path: string; readonly options: string }[] =
	SEMANTIC_COMMAND_CLI_COMMANDS.map((command) => ({
		options: commandOptionWords(command),
		path: command.path.join(' '),
	}));

const TOPIC_WORDS = [...new Set(TOP_WORDS.filter((word) => word !== 'help'))];

export function renderSemanticCommandCliCompletion(shell: SemanticCommandCliShell): string {
	switch (shell) {
		case 'bash':
			return `${renderBash()}\n`;
		case 'zsh':
			return `${renderZsh()}\n`;
		case 'fish':
			return `${renderFish()}\n`;
	}
}

function renderBash(): string {
	const valueSkip = VALUE_OPTIONS.join('|');
	const closedCases = CLOSED_VALUES.map(
		(closed) =>
			`\t\t${closed.option})\tCOMPREPLY=( $(compgen -W "${closed.values.join(' ')}" -- "$cur") ); return ;;`,
	).join('\n');
	const familyCases = FAMILIES.map(
		(family) =>
			`\t\t\t${family.name})\tCOMPREPLY=( $(compgen -W "${family.verbs.join(' ')}" -- "$cur") ); return ;;`,
	).join('\n');
	const pathCases = PATH_OPTIONS.map(
		(entry) => `\t\t"${entry.path}")\topts="${entry.options}" ;;`,
	).join('\n');
	return `# bash completion for orchbee-cli — generated from the declarative command table.
_orchbee_cli() {
	local cur prev
	COMPREPLY=()
	cur="\${COMP_WORDS[COMP_CWORD]}"
	prev="\${COMP_WORDS[COMP_CWORD-1]:-}"

	case "$prev" in
${closedCases}
		${OPEN_VALUE_OPTIONS.join('|')})\treturn ;;
	esac

	local w1="" w2="" w3="" skip="" tok
	for tok in "\${COMP_WORDS[@]:1:COMP_CWORD-1}"; do
		if [[ -n "$skip" ]]; then skip=""; continue; fi
		case "$tok" in
			${valueSkip})\tskip=1 ;;
			--*)\t;;
			*)\tif [[ -z "$w1" ]]; then w1="$tok"; elif [[ -z "$w2" ]]; then w2="$tok"; else w3=1; fi ;;
		esac
	done

	if [[ "$w1" == "help" ]]; then
		[[ -n "$w3" ]] && return
		if [[ -z "$w2" ]]; then
			COMPREPLY=( $(compgen -W "${TOPIC_WORDS.join(' ')}" -- "$cur") )
			return
		fi
		case "$w2" in
${familyCases}
		esac
		return
	fi

	if [[ -z "$w1" ]]; then
		COMPREPLY=( $(compgen -W "${TOP_WORDS.join(' ')} --skill --version --help -h" -- "$cur") )
		return
	fi

	if [[ -z "$w2" ]]; then
		case "$w1" in
${familyCases}
			completion)\tCOMPREPLY=( $(compgen -W "bash zsh fish" -- "$cur") ); return ;;
			${SINGLE_WORD_COMMANDS.filter((word) => word !== 'completion').join('|')})\t;;
			*)\treturn ;;
		esac
	fi

	local opts=""
	case "$w1 $w2" in
${pathCases}
	esac
	[[ -z "$opts" ]] && return
	[[ -z "$w3" ]] && opts="$opts --help -h"

	local out="" o used
	for o in $opts; do
		used=""
		if [[ "$o" != "--until" ]]; then
			for tok in "\${COMP_WORDS[@]:1:COMP_CWORD-1}"; do
				[[ "$tok" == "$o" ]] && used=1 && break
			done
		fi
		[[ -z "$used" ]] && out="$out $o"
	done
	COMPREPLY=( $(compgen -W "$out" -- "$cur") )
}
complete -F _orchbee_cli orchbee-cli`;
}

function renderZsh(): string {
	const valueSkip = VALUE_OPTIONS.join('|');
	const closedCases = CLOSED_VALUES.map(
		(closed) => `\t\t${closed.option})\tcandidates=(${closed.values.join(' ')}) ;;`,
	).join('\n');
	const familyCases = FAMILIES.map(
		(family) => `\t\t\t${family.name})\tcandidates=(${family.verbs.join(' ')}) ;;`,
	).join('\n');
	const pathCases = PATH_OPTIONS.map(
		(entry) => `\t\t"${entry.path}")\topts="${entry.options}" ;;`,
	).join('\n');
	return `#compdef orchbee-cli
# zsh completion for orchbee-cli — generated from the declarative command table.
_orchbee_cli() {
	local -a candidates
	candidates=()
	local cur prev w1 w2 w3 skip tok opts out o used
	cur="\${words[CURRENT]}"
	prev="\${words[CURRENT-1]:-}"
	w1=""; w2=""; w3=""; skip=""; opts=""; out=""

	case "$prev" in
${closedCases}
		${OPEN_VALUE_OPTIONS.join('|')})\t;;
		*)
			for tok in "\${(@)words[2,CURRENT-1]}"; do
				if [[ -n "$skip" ]]; then skip=""; continue; fi
				case "$tok" in
					${valueSkip})\tskip=1 ;;
					--*)\t;;
					*)\tif [[ -z "$w1" ]]; then w1="$tok"; elif [[ -z "$w2" ]]; then w2="$tok"; else w3=1; fi ;;
				esac
			done

			if [[ "$w1" == "help" ]]; then
				if [[ -n "$w3" ]]; then
					:
				elif [[ -z "$w2" ]]; then
					candidates=(${TOPIC_WORDS.join(' ')})
				else
					case "$w2" in
${familyCases}
					esac
				fi
			elif [[ -z "$w1" ]]; then
				candidates=(${TOP_WORDS.join(' ')} --skill --version --help -h)
			elif [[ -z "$w2" ]]; then
				case "$w1" in
${familyCases}
					completion)\tcandidates=(bash zsh fish) ;;
					${SINGLE_WORD_COMMANDS.filter((word) => word !== 'completion').join('|')})\t
						case "$w1 $w2" in
${pathCases}
						esac
						[[ -n "$opts" ]] && {
							[[ -z "$w3" ]] && opts="$opts --help -h"
							for o in \${(z)opts}; do
								used=""
								if [[ "$o" != "--until" ]]; then
									for tok in "\${(@)words[2,CURRENT-1]}"; do
										[[ "$tok" == "$o" ]] && used=1 && break
									done
								fi
								[[ -z "$used" ]] && candidates+=("$o")
							done
						}
						;;
					*)\t;;
				esac
			else
				case "$w1 $w2" in
${pathCases}
				esac
				if [[ -n "$opts" ]]; then
					[[ -z "$w3" ]] && opts="$opts --help -h"
					for o in \${(z)opts}; do
						used=""
						if [[ "$o" != "--until" ]]; then
							for tok in "\${(@)words[2,CURRENT-1]}"; do
								[[ "$tok" == "$o" ]] && used=1 && break
							done
						fi
						[[ -z "$used" ]] && candidates+=("$o")
					done
				fi
			fi
			;;
	esac

	if [[ -n "\${compstate:-}" ]]; then
		compadd -- "\${candidates[@]}"
	else
		print -l -- "\${candidates[@]}"
	fi
}
compdef _orchbee_cli orchbee-cli`;
}

function renderFish(): string {
	const lines = [
		'# fish completion for orchbee-cli — generated from the declarative command table.',
		`complete -f -c orchbee-cli -n '__fish_use_subcommand' -a '${TOP_WORDS.join(' ')}'`,
		`complete -f -c orchbee-cli -n '__fish_use_subcommand' -l skill`,
		`complete -f -c orchbee-cli -n '__fish_use_subcommand' -l version`,
		`complete -f -c orchbee-cli -n '__fish_use_subcommand' -s h -l help`,
	];
	for (const family of FAMILIES) {
		lines.push(
			`complete -f -c orchbee-cli -n '__fish_seen_subcommand_from ${family.name}; and not __fish_seen_subcommand_from ${family.verbs.join(' ')}' -a '${family.verbs.join(' ')}'`,
		);
	}
	lines.push(
		`complete -f -c orchbee-cli -n '__fish_seen_subcommand_from help' -a '${TOPIC_WORDS.join(' ')}'`,
		`complete -f -c orchbee-cli -n '__fish_seen_subcommand_from completion' -a 'bash zsh fish'`,
	);
	for (const command of SEMANTIC_COMMAND_CLI_COMMANDS) {
		const condition =
			command.path.length === 1
				? `__fish_seen_subcommand_from ${command.path[0]}`
				: `__fish_seen_subcommand_from ${command.path[0]}; and __fish_seen_subcommand_from ${command.path[1]}`;
		for (const option of command.options) {
			const requirement = option.takesValue ? ' -r' : '';
			const values = option.values === undefined ? '' : ` -a '${option.values.join(' ')}'`;
			lines.push(
				`complete -f -c orchbee-cli -n '${condition}' -l ${option.name}${requirement}${values}`,
			);
		}
		if (command.coordinator) {
			for (const option of SEMANTIC_COMMAND_CLI_COMMON_OPTIONS) {
				const values = option.values === undefined ? '' : ` -a '${option.values.join(' ')}'`;
				lines.push(`complete -f -c orchbee-cli -n '${condition}' -l ${option.name} -r${values}`);
			}
		}
		lines.push(`complete -f -c orchbee-cli -n '${condition}' -s h -l help`);
	}
	return lines.join('\n');
}
