# Orchbee automatic Zsh loader. Loaded once through a Desktop-owned ZDOTDIR.

'builtin' 'typeset' __orchbee_automatic_root="${${(%):-%x}:A:h:h:h}"
if [[ -n "${ORCHBEE_SHELL_INTEGRATION_ORIGINAL_ZDOTDIR+present}" ]]; then
	'builtin' 'export' ZDOTDIR="$ORCHBEE_SHELL_INTEGRATION_ORIGINAL_ZDOTDIR"
	'builtin' 'unset' 'ORCHBEE_SHELL_INTEGRATION_ORIGINAL_ZDOTDIR'
else
	'builtin' 'unset' 'ZDOTDIR'
fi

{
	'builtin' 'typeset' __orchbee_user_zshenv=${ZDOTDIR-${HOME}}'/.zshenv'
	[[ ! -r "$__orchbee_user_zshenv" ]] || 'builtin' 'source' '--' "$__orchbee_user_zshenv"
} always {
	if [[ -o 'interactive' && -r "$__orchbee_automatic_root/orchbee.zsh" ]]; then
		'builtin' 'source' '--' "$__orchbee_automatic_root/orchbee.zsh"
	fi
	'builtin' 'unset' '__orchbee_user_zshenv' '__orchbee_automatic_root'
}
