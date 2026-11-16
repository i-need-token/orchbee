# Orchbee's prompt and command marks for an interactive zsh, loaded by the managed .zshenv before
# the person's other startup files. They are written with the managed PowerShell's private
# grammar, so the Terminal Host reads every managed shell alike:
#   at the prompt, once the line editor has drawn it: OSC 133 P, OSC 133 B, APC ready;
#   when a line is accepted: OSC 133 C, then APC start;<token> once ORCHBEE_RUN_TOKEN holds it;
#   at the next prompt: OSC 133 D;<status>, APC finish;<token>;<status>, and the token unset.
# A line Orchbee submits starts with ": orchbee-run:<32 hex digits>; ". The line editor strips that
# prefix, the command runs with that token, and neither form stays in the history. The Agent start
# the launch carried runs once, before the first prompt, after every startup file and every
# earlier prompt hook. The hooks install at the first prompt, so the person's own hooks and theme
# are in place first.

__orchbee_load() {
	builtin emulate -L zsh -o no_aliases
	(( ! ${+__orchbee_seed} )) || return 0
	builtin typeset -g __orchbee_seed __orchbee_run= __orchbee_submitted=
	builtin typeset -gi __orchbee_count=0
	# Run tokens are this seed, read once, and a counter, so every token is unique and nonzero.
	local seed=${(L)$(command od -An -N12 -tx1 /dev/urandom 2>/dev/null)//[^0-9a-fA-F]/}
	if (( ${#seed} != 24 )); then
		builtin printf -v seed '%04x%04x%04x%04x%04x%04x' $RANDOM $RANDOM $RANDOM $RANDOM $RANDOM $RANDOM
	fi
	__orchbee_seed=$seed
	builtin typeset -ga precmd_functions
	precmd_functions+=(__orchbee_first_prompt)
}

__orchbee_mint() {
	(( ++__orchbee_count ))
	builtin printf -v REPLY '%s%08x' $__orchbee_seed $__orchbee_count
}

# Runs once, in the first prompt's hooks. It installs the marks, then runs the Agent start, if any,
# in the person's own options, as a line they had typed would run. Ctrl+C ends only the Agent, as
# it would end a typed line: the start still finishes and the prompt's other hooks still run.
__orchbee_first_prompt() {
	__orchbee_install
	[[ -n ${__orchbee_start-} ]] || return 0
	__orchbee_begin_start
	{
		builtin eval "$__orchbee_line"
	} always {
		__orchbee_end_start $?
		TRY_BLOCK_INTERRUPT=0
	}
}

__orchbee_install() {
	builtin emulate -L zsh -o no_aliases
	precmd_functions=(__orchbee_precmd ${precmd_functions:#__orchbee_first_prompt})
	builtin typeset -ga preexec_functions zshaddhistory_functions
	preexec_functions+=(__orchbee_preexec)
	zshaddhistory_functions+=(__orchbee_history)
	# zsh 5.3 and later chain line editor hooks; without it, the prompt carries no marks and
	# submitted lines keep their prefix, which preexec still reads.
	if builtin autoload -Uz add-zle-hook-widget 2>/dev/null; then
		builtin zle -N __orchbee_line_init
		builtin zle -N __orchbee_line_finish
		add-zle-hook-widget line-init __orchbee_line_init
		add-zle-hook-widget line-finish __orchbee_line_finish
	fi
}

__orchbee_begin_start() {
	builtin emulate -L zsh -o no_aliases
	builtin typeset -g __orchbee_line=$__orchbee_start
	builtin unset __orchbee_start
	local REPLY
	__orchbee_mint
	__orchbee_run=$REPLY
	builtin export ORCHBEE_RUN_TOKEN=$REPLY
	builtin print -rn -- $'\e_orchbee.command.v1;start;'$REPLY$'\e\\'
}

__orchbee_end_start() {
	builtin emulate -L zsh -o no_aliases
	builtin unset __orchbee_line ORCHBEE_RUN_TOKEN
	builtin print -rn -- $'\e_orchbee.command.v1;finish;'$__orchbee_run';'$1$'\e\\'
	__orchbee_run=
}

__orchbee_precmd() {
	local -i code=$?
	builtin emulate -L zsh -o no_aliases
	__orchbee_submitted=
	[[ -n $__orchbee_run ]] || return 0
	builtin print -rn -- $'\e]133;D;'$code$'\a\e_orchbee.command.v1;finish;'$__orchbee_run';'$code$'\e\\'
	__orchbee_run=
	builtin unset ORCHBEE_RUN_TOKEN
}

__orchbee_preexec() {
	builtin emulate -L zsh -o no_aliases
	local REPLY=$__orchbee_submitted
	__orchbee_submitted=
	if [[ -z $REPLY ]] && __orchbee_prefix $1; then
		REPLY=${1[15,46]}
	fi
	[[ -n $REPLY ]] || __orchbee_mint
	__orchbee_run=$REPLY
	builtin export ORCHBEE_RUN_TOKEN=$REPLY
	builtin print -rn -- $'\e]133;C\a\e_orchbee.command.v1;start;'$REPLY$'\e\\'
}

# Whether $1 starts with a submitted line's prefix.
__orchbee_prefix() {
	builtin emulate -L zsh
	local id=${1[15,46]}
	[[ ${1[1,14]} == ': orchbee-run:' && ${#id} -eq 32 && $id != *[^0-9a-f]* && ${1[47,48]} == '; ' ]]
}

__orchbee_history() {
	builtin emulate -L zsh
	[[ -z $__orchbee_submitted ]] && ! __orchbee_prefix $1
}

__orchbee_line_init() {
	builtin emulate -L zsh
	[[ $CONTEXT == start ]] || return 0
	builtin print -rn -- $'\e]133;P\a\e]133;B\a\e_orchbee.command.v1;ready\e\\'
}

__orchbee_line_finish() {
	builtin emulate -L zsh
	[[ $CONTEXT == start ]] && __orchbee_prefix $BUFFER || return 0
	__orchbee_submitted=${BUFFER[15,46]}
	BUFFER=${BUFFER[49,-1]}
}

__orchbee_load
builtin unfunction __orchbee_load
