import {
	SEMANTIC_COMMAND_CLI_COMMANDS,
	SEMANTIC_COMMAND_CLI_COMMON_OPTIONS,
	type SemanticCommandCliGrammarCommand,
	type SemanticCommandCliGrammarOption,
	type SemanticCommandCliShell,
} from './semantic-command-cli-grammar';

const TOP_WORDS = [
	...new Set(SEMANTIC_COMMAND_CLI_COMMANDS.map((command) => command.path[0])),
	'help',
];

const TOPIC_WORDS = TOP_WORDS.filter((word) => word !== 'help');

const FAMILY_COMMANDS = SEMANTIC_COMMAND_CLI_COMMANDS.filter(
	(command) => command.path.length === 2,
);

const FAMILY_NAMES = [...new Set(FAMILY_COMMANDS.map((command) => command.path[0]))];

const SINGLE_WORDS = SEMANTIC_COMMAND_CLI_COMMANDS.filter(
	(command) => command.path.length === 1,
).map((command) => command.path[0]);

function commandOptions(command: SemanticCommandCliGrammarCommand) {
	return [...command.options, ...(command.coordinator ? SEMANTIC_COMMAND_CLI_COMMON_OPTIONS : [])];
}

interface PathProjection {
	readonly cap: number;
	readonly key: string;
	readonly locationOptions: readonly string[];
	readonly offerableOptions: readonly string[];
	readonly operandValues: readonly string[];
	readonly valueOptions: readonly string[];
}

const PATHS: readonly PathProjection[] = SEMANTIC_COMMAND_CLI_COMMANDS.map((command) => ({
	cap: command.operand === undefined ? 0 : 1,
	key: command.path.join(' '),
	locationOptions: commandOptions(command)
		.filter((option) => option.location === true)
		.map((option) => `--${option.name}`),
	offerableOptions: commandOptions(command).map((option) => `--${option.name}`),
	operandValues: command.operandValues ?? [],
	valueOptions: commandOptions(command)
		.filter((option) => option.takesValue)
		.map((option) => `--${option.name}`),
}));

const FAMILIES: readonly {
	readonly name: string;
	readonly unionOfferable: readonly string[];
	readonly unionValue: readonly string[];
	readonly verbs: readonly string[];
}[] = FAMILY_NAMES.map((name) => {
	const members = FAMILY_COMMANDS.filter((command) => command.path[0] === name);
	return {
		name: name ?? '',
		unionOfferable: [
			...new Set(members.flatMap((command) => commandOptions(command).map((o) => `--${o.name}`))),
		],
		unionValue: [
			...new Set(
				members.flatMap((command) =>
					commandOptions(command)
						.filter((option) => option.takesValue)
						.map((o) => `--${o.name}`),
				),
			),
		],
		verbs: members.map((command) => command.path[1] ?? ''),
	};
});

const ALL_OPTIONS: readonly SemanticCommandCliGrammarOption[] = [
	...SEMANTIC_COMMAND_CLI_COMMON_OPTIONS,
	...SEMANTIC_COMMAND_CLI_COMMANDS.flatMap((command) => command.options),
];

const VALUE_OPTIONS = [
	...new Set(ALL_OPTIONS.filter((option) => option.takesValue).map((option) => `--${option.name}`)),
];

const CLOSED_VALUES: readonly { readonly option: string; readonly values: readonly string[] }[] =
	Object.values(
		ALL_OPTIONS.filter(
			(option): option is typeof option & { values: readonly string[] } =>
				option.takesValue && option.values !== undefined,
		).reduce<Record<string, { option: string; values: readonly string[] }>>(
			(accumulator, option) => ({
				...accumulator,
				[option.name]: { option: `--${option.name}`, values: option.values },
			}),
			{},
		),
	);

const REPEATABLES = [
	...new Set(ALL_OPTIONS.filter((option) => option.repeatable).map((option) => option.name)),
].map((name) => ({ option: `--${name}`, variable: `rep_${name.replaceAll('-', '_')}` }));

const CLOSED_PENDING_ARMS = [
	...CLOSED_VALUES.map((closed) => {
		const repeatable = REPEATABLES.find((candidate) => candidate.option === closed.option);
		const track =
			repeatable === undefined ? '' : `${repeatable.variable}="$${repeatable.variable} $tok"; `;
		return `\t\t\t\t${closed.option})\t${track}case " ${closed.values.join(' ')} " in *" $tok "*) ;; *)\tbad=1 ;; esac ;;`;
	}),
	...REPEATABLES.filter(
		(repeatable) => !CLOSED_VALUES.some((closed) => closed.option === repeatable.option),
	).map(
		(repeatable) =>
			`\t\t\t\t${repeatable.option})\t${repeatable.variable}="$${repeatable.variable} $tok" ;;`,
	),
].join('\n');

const CLOSED_PENDING_ARMS_FISH = [
	...CLOSED_VALUES.map((closed) => {
		const repeatable = REPEATABLES.find((candidate) => candidate.option === closed.option);
		const track =
			repeatable === undefined
				? ''
				: `\n                    set ${repeatable.variable} "$${repeatable.variable} $tok"`;
		return `                case ${closed.option}${track}\n                    if not string match -q "* $tok *" " ${closed.values.join(' ')} "\n                        set bad 1\n                    end`;
	}),
	...REPEATABLES.filter(
		(repeatable) => !CLOSED_VALUES.some((closed) => closed.option === repeatable.option),
	).map(
		(repeatable) =>
			`                case ${repeatable.option}\n                    set ${repeatable.variable} "$${repeatable.variable} $tok"`,
	),
].join('\n');

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

const VALUE_OPTION_WORDS = VALUE_OPTIONS.join(' ');
const REPEATABLE_WORDS = REPEATABLES.map((repeatable) => repeatable.option).join(' ');

function pathCases(
	indent: string,
	select: (projection: PathProjection) => string | undefined,
): string {
	return PATHS.map((projection) => {
		const value = select(projection);
		return value === undefined ? '' : `${indent}"${projection.key}")\t${value} ;;`;
	})
		.filter((line) => line !== '')
		.join('\n');
}

function familyVerbsCases(indent: string, body: (verbs: string) => string): string {
	return FAMILIES.map(
		(family) => `${indent}${family.name})\t${body(family.verbs.join(' '))} ;;`,
	).join('\n');
}

function renderBash(): string {
	const repDeclares = REPEATABLES.map((repeatable) => `	local ${repeatable.variable}=""`).join(
		'\n',
	);
	const repUsedSelect =
		REPEATABLES.map(
			(repeatable) => `\t\t\t${repeatable.option})\tusedv="$${repeatable.variable}" ;;`,
		).join('\n') || '\t\t\t*)\tusedv="" ;;';
	const closedValueCases = CLOSED_VALUES.map(
		(closed) => `\t\t\t${closed.option})\tvals="${closed.values.join(' ')}" ;;`,
	).join('\n');
	const valueScopeCases = pathCases('\t\t\t\t\t', (p) =>
		p.valueOptions.length === 0 ? 'scope=""' : `scope=" ${p.valueOptions.join(' ')} "`,
	);
	const familyUnionValueCases = FAMILIES.map(
		(family) => `\t\t\t\t\t\t${family.name})\tscope=" ${family.unionValue.join(' ')} " ;;`,
	).join('\n');
	const offerCases = pathCases('\t\t', (p) => {
		const parts = [
			`opts="${p.offerableOptions.join(' ')}"`,
			`locopts="${p.locationOptions.join(' ')}"`,
			`cap=${p.cap}`,
			p.operandValues.length === 0 ? '' : `opvals="${p.operandValues.join(' ')}"`,
		].filter((part) => part !== '');
		return parts.join('; ');
	});
	const familyUnionOfferCases = FAMILIES.map(
		(family) => `\t\t\t\t\t${family.name})\tunion=" ${family.unionOfferable.join(' ')} " ;;`,
	).join('\n');
	const familyVerbOfferCases = familyVerbsCases('\t\t\t\t\t', (verbs) => `out="${verbs}"`);
	return `# bash completion for orchbee-cli — generated from the declarative command table.
_orchbee_cli() {
	local cur fam p2 extra used pending tok bad key operand cap sub
	local opts valopts locopts opvals out o u v scope vals usedv union
${repDeclares}
	COMPREPLY=()
	cur="\${COMP_WORDS[COMP_CWORD]}"
	fam=""; p2=""; extra=0; used=""; pending=""; bad=""; key=""; operand=0

	for tok in "\${COMP_WORDS[@]:1:COMP_CWORD-1}"; do
		if [[ -n "$pending" ]]; then
			case "$pending" in
${CLOSED_PENDING_ARMS}
			esac
			pending=""
			continue
		fi
		case "$tok" in
			-h|--help|--skill|--version)\tbad=1 ;;
			--*)
				case " $used " in *" $tok "*)
					case " ${REPEATABLE_WORDS} " in *" $tok "*) ;; *)\tbad=1 ;; esac ;;
				esac
				used="$used $tok"
				case "$tok" in
					${VALUE_OPTION_WORDS.replaceAll(' ', '|')})\tpending="$tok" ;;
				esac ;;
			*)
				if [[ -z "$fam" ]]; then fam="$tok"
				elif [[ -z "$p2" ]]; then p2="$tok"
				else extra=$((extra + 1)); fi ;;
		esac
	done
	[[ -n "$bad" ]] && return

	__oc_vals() {
		local vals="" usedv="" v outv=""
		case "$1" in
${closedValueCases}
			*)\treturn 1 ;;
		esac
		case "$1" in
${repUsedSelect}
			*)\tusedv="" ;;
		esac
		for v in $vals; do
			case " $usedv " in *" $v "*) ;; *)\toutv="$outv $v" ;; esac
		done
		printf '%s' "$outv"
	}

	if [[ -n "$pending" ]]; then
		scope=""
		case "$fam" in
			""|help)\treturn ;;
			${FAMILY_NAMES.join('|')})
				if [[ -z "$p2" ]]; then
					case "$fam" in
${familyUnionValueCases}
					esac
				else
					key="$fam $p2"
					case "$key" in
${valueScopeCases}
						*)\treturn ;;
					esac
				fi ;;
			${SINGLE_WORDS.join('|')})
				key="$fam"
				case "$key" in
${valueScopeCases}
					*)\treturn ;;
				esac ;;
			*)\treturn ;;
		esac
		case "$scope" in *" $pending "*) ;; *)\treturn ;; esac
		out=$(__oc_vals "$pending") || return
		COMPREPLY=( $(compgen -W "$out" -- "$cur") )
		return
	fi

	case "$fam" in
		"")
			[[ -n "$used" ]] && return
			COMPREPLY=( $(compgen -W "${TOP_WORDS.join(' ')} --skill --version --help -h" -- "$cur") )
			return ;;
		help)
			[[ -n "$used" || $extra -gt 1 ]] && return
			if [[ -z "$p2" ]]; then
				COMPREPLY=( $(compgen -W "${TOPIC_WORDS.join(' ')}" -- "$cur") )
				return
			fi
			[[ $extra -gt 0 ]] && return
			case "$p2" in
${familyVerbsCases('\t\t\t\t', (verbs) => `COMPREPLY=( $(compgen -W "${verbs}" -- "$cur") )`)}
			esac
			return ;;
		${FAMILY_NAMES.join('|')})
			if [[ -z "$p2" ]]; then
				union=""
				case "$fam" in
${familyUnionOfferCases}
				esac
				for u in $used; do
					case "$union" in *" $u "*) ;; *)\treturn ;; esac
				done
				out=""
				case "$fam" in
${familyVerbOfferCases}
				esac
				[[ -z "$used" && $extra -eq 0 ]] && out="$out --help -h"
				COMPREPLY=( $(compgen -W "$out" -- "$cur") )
				return
			fi
			sub="$p2"; key="$fam $sub"; operand=$extra ;;
		${SINGLE_WORDS.join('|')})
			key="$fam"; operand=$extra
			[[ -n "$p2" ]] && operand=$((operand + 1)) ;;
		*)\treturn ;;
	esac

	opts=""; locopts=""; cap=0; opvals=""
	case "$key" in
${offerCases}
		*)\treturn ;;
	esac
	[[ $operand -gt $cap ]] && return
	for u in $used; do
		case " $opts " in *" $u "*) ;; *)\treturn ;; esac
	done
	out=""
	for o in $opts; do
		case " $used " in *" $o "*) continue ;; esac
		case " $used " in *" --standalone "*)
			case " $locopts " in *" $o "*) continue ;; esac ;;
		esac
		case " $used " in
			*" --project "*|*" --workspace "*)
				[[ "$o" == "--standalone" ]] && continue ;;
		esac
		out="$out $o"
	done
	[[ -z "$used" && $operand -eq 0 ]] && out="$out --help -h"
	[[ $operand -lt $cap && -n "$opvals" ]] && out="$out $opvals"
	COMPREPLY=( $(compgen -W "$out" -- "$cur") )
}
complete -F _orchbee_cli orchbee-cli`;
}

function renderZsh(): string {
	const repDeclares = REPEATABLES.map((repeatable) => `	local ${repeatable.variable}=""`).join(
		'\n',
	);
	const repUsedSelect =
		REPEATABLES.map(
			(repeatable) => `\t\t\t${repeatable.option})\tusedv="$${repeatable.variable}" ;;`,
		).join('\n') || '\t\t\t*)\tusedv="" ;;';
	const closedValueCases = CLOSED_VALUES.map(
		(closed) => `\t\t\t${closed.option})\tvals="${closed.values.join(' ')}" ;;`,
	).join('\n');
	const valueScopeCases = pathCases('\t\t\t\t\t', (p) =>
		p.valueOptions.length === 0 ? 'scope=""' : `scope=" ${p.valueOptions.join(' ')} "`,
	);
	const familyUnionValueCases = FAMILIES.map(
		(family) => `\t\t\t\t\t\t${family.name})\tscope=" ${family.unionValue.join(' ')} " ;;`,
	).join('\n');
	const offerCases = pathCases('\t\t', (p) => {
		const parts = [
			`opts="${p.offerableOptions.join(' ')}"`,
			`locopts="${p.locationOptions.join(' ')}"`,
			`cap=${p.cap}`,
			p.operandValues.length === 0 ? '' : `opvals="${p.operandValues.join(' ')}"`,
		].filter((part) => part !== '');
		return parts.join('; ');
	});
	const familyUnionOfferCases = FAMILIES.map(
		(family) => `\t\t\t\t\t${family.name})\tunion=" ${family.unionOfferable.join(' ')} " ;;`,
	).join('\n');
	const familyVerbOfferCases = familyVerbsCases('\t\t\t\t\t', (verbs) => `out="${verbs}"`);
	return `#compdef orchbee-cli
# zsh completion for orchbee-cli — generated from the declarative command table.
_orchbee_cli() {
	local -a candidates
	candidates=()
	local cur fam p2 extra used pending tok bad key operand cap sub
	local opts valopts locopts opvals out o u v scope vals usedv union
${repDeclares}
	cur="\${words[CURRENT]}"
	fam=""; p2=""; extra=0; used=""; pending=""; bad=""; key=""; operand=0

	for tok in "\${(@)words[2,CURRENT-1]}"; do
		if [[ -n "$pending" ]]; then
			case "$pending" in
${CLOSED_PENDING_ARMS}
			esac
			pending=""
			continue
		fi
		case "$tok" in
			-h|--help|--skill|--version)\tbad=1 ;;
			--*)
				case " $used " in *" $tok "*)
					case " ${REPEATABLE_WORDS} " in *" $tok "*) ;; *)\tbad=1 ;; esac ;;
				esac
				used="$used $tok"
				case "$tok" in
					${VALUE_OPTION_WORDS.replaceAll(' ', '|')})\tpending="$tok" ;;
				esac ;;
			*)
				if [[ -z "$fam" ]]; then fam="$tok"
				elif [[ -z "$p2" ]]; then p2="$tok"
				else extra=$((extra + 1)); fi ;;
		esac
	done
	[[ -n "$bad" ]] && return

	__oc_vals() {
		local vals="" usedv="" v outv=""
		case "$1" in
${closedValueCases}
			*)\treturn 1 ;;
		esac
		case "$1" in
${repUsedSelect}
			*)\tusedv="" ;;
		esac
		for v in \${(z)vals}; do
			case " $usedv " in *" $v "*) ;; *)\toutv="$outv $v" ;; esac
		done
		print -r -- "$outv"
	}

	if [[ -n "$pending" ]]; then
		scope=""
		case "$fam" in
			""|help)\treturn ;;
			${FAMILY_NAMES.join('|')})
				if [[ -z "$p2" ]]; then
					case "$fam" in
${familyUnionValueCases}
					esac
				else
					key="$fam $p2"
					case "$key" in
${valueScopeCases}
						*)\treturn ;;
					esac
				fi ;;
			${SINGLE_WORDS.join('|')})
				key="$fam"
				case "$key" in
${valueScopeCases}
					*)\treturn ;;
				esac ;;
			*)\treturn ;;
		esac
		case "$scope" in *" $pending "*) ;; *)\treturn ;; esac
		out=$(__oc_vals "$pending") || return
		for o in \${(z)out}; do candidates+=("$o"); done
		[[ -n "\${compstate:-}" ]] && compadd -- "\${candidates[@]}" || print -l -- "\${candidates[@]}"
		return
	fi

	case "$fam" in
		"")
			[[ -n "$used" ]] && return
			candidates=(${TOP_WORDS.join(' ')} --skill --version --help -h) ;;
		help)
			[[ -n "$used" || $extra -gt 1 ]] && return
			if [[ -z "$p2" ]]; then
				candidates=(${TOPIC_WORDS.join(' ')})
			elif [[ $extra -eq 0 ]]; then
				case "$p2" in
${familyVerbsCases('\t\t\t\t\t', (verbs) => `candidates=(${verbs})`)}
				esac
			fi ;;
		${FAMILY_NAMES.join('|')})
			if [[ -z "$p2" ]]; then
				union=""
				case "$fam" in
${familyUnionOfferCases}
				esac
				for u in \${(z)used}; do
					case "$union" in *" $u "*) ;; *)\treturn ;; esac
				done
				out=""
				case "$fam" in
${familyVerbOfferCases}
				esac
				[[ -z "$used" && $extra -eq 0 ]] && out="$out --help -h"
				for o in \${(z)out}; do candidates+=("$o"); done
			else
				sub="$p2"; key="$fam $sub"; operand=$extra
			fi ;;
		${SINGLE_WORDS.join('|')})
			key="$fam"; operand=$extra
			[[ -n "$p2" ]] && operand=$((operand + 1)) ;;
		*)\treturn ;;
	esac

	if [[ -n "$key" ]]; then
		opts=""; locopts=""; cap=0; opvals=""
		case "$key" in
${offerCases}
			*)\treturn ;;
		esac
		[[ $operand -gt $cap ]] && return
		for u in \${(z)used}; do
			case " $opts " in *" $u "*) ;; *)\treturn ;; esac
		done
		out=""
		for o in \${(z)opts}; do
			case " $used " in *" $o "*) continue ;; esac
			case " $used " in *" --standalone "*)
				case " $locopts " in *" $o "*) continue ;; esac ;;
			esac
			case " $used " in
				*" --project "*|*" --workspace "*)
					[[ "$o" == "--standalone" ]] && continue ;;
			esac
			out="$out $o"
		done
		[[ -z "$used" && $operand -eq 0 ]] && out="$out --help -h"
		[[ $operand -lt $cap && -n "$opvals" ]] && out="$out $opvals"
		for o in \${(z)out}; do candidates+=("$o"); done
	fi

	if [[ -n "\${compstate:-}" ]]; then
		compadd -- "\${candidates[@]}"
	else
		print -l -- "\${candidates[@]}"
	fi
}
compdef _orchbee_cli orchbee-cli`;
}

function renderFish(): string {
	const repDeclares = REPEATABLES.map((repeatable) => `    set -l ${repeatable.variable} ""`).join(
		'\n',
	);
	const repUsedSelect =
		REPEATABLES.map(
			(repeatable) =>
				`            case ${repeatable.option}\n                set usedv "$${repeatable.variable}"`,
		).join('\n') || '            case "*"\n                set usedv ""';
	const closedValueCases = CLOSED_VALUES.map(
		(closed) =>
			`            case ${closed.option}\n                set vals ${closed.values.join(' ')}`,
	).join('\n');
	const valueScopeCases = pathCases('\t\t\t\t', (p) =>
		p.valueOptions.length === 0 ? 'set scope' : `set scope ${p.valueOptions.join(' ')}`,
	).replaceAll('\t', '    ');
	const familyUnionValueCases = FAMILIES.map(
		(family) =>
			`                        case ${family.name}\n                            set scope ${family.unionValue.join(' ')}`,
	).join('\n');
	const offerCases = pathCases('\t\t', (p) => {
		const parts = [
			`set opts ${p.offerableOptions.join(' ')}`,
			p.locationOptions.length === 0 ? 'set locopts' : `set locopts ${p.locationOptions.join(' ')}`,
			`set cap ${p.cap}`,
			p.operandValues.length === 0 ? 'set opvals' : `set opvals ${p.operandValues.join(' ')}`,
		];
		return parts.join('; ');
	}).replaceAll('\t', '    ');
	const familyUnionOfferCases = FAMILIES.map(
		(family) =>
			`                    case ${family.name}\n                        set union ${family.unionOfferable.join(' ')}`,
	).join('\n');
	const familyVerbOfferCases = FAMILIES.map(
		(family) =>
			`                    case ${family.name}\n                        set out ${family.verbs.join(' ')}`,
	).join('\n');
	const familyHelpVerbCases = FAMILIES.map(
		(family) =>
			`                case ${family.name}\n                    printf '%s\\n' ${family.verbs.join(' ')}`,
	).join('\n');
	return `# fish completion for orchbee-cli — generated from the declarative command table.
function __orchbee_cli_suggest
    set -l tokens (commandline -opc)
    set -l cur $tokens[-1]
    set -l completed
    if test (count $tokens) -gt 2
        set completed $tokens[2..-2]
    end
${repDeclares}
    set -l fam ""; set -l p2 ""; set -l extra 0
    set -l used ""; set -l pending ""; set -l bad ""; set -l key ""; set -l operand 0

    for tok in $completed
        if test -n "$pending"
            switch $pending
${CLOSED_PENDING_ARMS_FISH}
            end
            set pending ""
            continue
        end
        switch $tok
            case -h --help --skill --version
                set bad 1
            case '--*'
                if string match -q "* $tok *" " $used "
                    if not string match -q "* $tok *" " ${REPEATABLE_WORDS} "
                        set bad 1
                    end
                end
                set used "$used $tok"
                switch $tok
                    case ${VALUE_OPTION_WORDS}
                        set pending $tok
                end
            case '*'
                if test -z "$fam"
                    set fam $tok
                else if test -z "$p2"
                    set p2 $tok
                else
                    set extra (math $extra + 1)
                end
        end
    end
    test -n "$bad"; and return

    if test -n "$pending"
        set -l scope
        switch $fam
            case '' help
                return
            case ${FAMILY_NAMES.join(' ')}
                if test -z "$p2"
                    switch $fam
${familyUnionValueCases}
                    end
                else
                    set key "$fam $p2"
                    switch $key
${valueScopeCases}
                        case '*'
                            return
                    end
                end
            case ${SINGLE_WORDS.join(' ')}
                set key "$fam"
                switch $key
${valueScopeCases}
                    case '*'
                        return
                end
            case '*'
                return
        end
        if not string match -q "* $pending *" " $scope "
            return
        end
        set -l vals; set -l usedv ""
        switch $pending
${closedValueCases}
            case '*'
                return
        end
        switch $pending
${repUsedSelect}
        end
        for v in $vals
            if not string match -q "* $v *" " $usedv "
                echo $v
            end
        end
        return
    end

    switch $fam
        case ''
            if test -n "$used"
                return
            end
            printf '%s\\n' ${TOP_WORDS.join(' ')} --skill --version --help -h
            return
        case help
            if test -n "$used"; or test $extra -gt 1
                return
            end
            if test -z "$p2"
                printf '%s\\n' ${TOPIC_WORDS.join(' ')}
                return
            end
            test $extra -gt 0; and return
            switch $p2
${familyHelpVerbCases}
            end
            return
        case ${FAMILY_NAMES.join(' ')}
            if test -z "$p2"
                set -l union
                switch $fam
${familyUnionOfferCases}
                end
                for u in (string split ' ' -- (string trim -- $used))
                    test -z "$u"; and continue
                    if not string match -q "* $u *" " $union "
                        return
                    end
                end
                set -l out
                switch $fam
${familyVerbOfferCases}
                end
                if test -z "$used"; and test $extra -eq 0
                    set -a out -- --help -h
                end
                printf '%s\\n' $out
                return
            end
            set key "$fam $p2"; set operand $extra
        case ${SINGLE_WORDS.join(' ')}
            set key "$fam"; set operand $extra
            if test -n "$p2"
                set operand (math $operand + 1)
            end
        case '*'
            return
    end

    set -l opts; set -l locopts; set -l cap 0; set -l opvals
    switch $key
${offerCases}
        case '*'
            return
    end
    test $operand -gt $cap; and return
    for u in (string split ' ' -- (string trim -- $used))
        test -z "$u"; and continue
        if not string match -q "* $u *" " $opts "
            return
        end
    end
    set -l out
    for o in $opts
        if string match -q "* $o *" " $used "
            continue
        end
        if string match -q "* --standalone *" " $used "
            if string match -q "* $o *" " $locopts "
                continue
            end
        end
        if string match -qr ' --(project|workspace) ' " $used "
            if test "$o" = --standalone
                continue
            end
        end
        set -a out $o
    end
    if test -z "$used"; and test $operand -eq 0
        set -a out -- --help -h
    end
    if test $operand -lt $cap; and test -n "$opvals"
        set -a out $opvals
    end
    printf '%s\\n' $out
end
complete -f -c orchbee-cli -a '(__orchbee_cli_suggest)'`;
}
