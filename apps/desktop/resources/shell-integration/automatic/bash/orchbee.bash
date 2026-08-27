# Orchbee automatic Bash loader. Selected once as the interactive user rc file.

if [[ -z ${ORCHBEE_SHELL_INTEGRATION_BASH_INJECT-} ]]; then
	builtin return 0
fi

__orchbee_automatic_root=${ORCHBEE_SHELL_INTEGRATION_DIR-}
__orchbee_automatic_rcfile_set=0
__orchbee_automatic_rcfile=''
if [[ -n ${ORCHBEE_SHELL_INTEGRATION_BASH_RCFILE+present} ]]; then
	__orchbee_automatic_rcfile_set=1
	__orchbee_automatic_rcfile=$ORCHBEE_SHELL_INTEGRATION_BASH_RCFILE
fi
builtin unset ORCHBEE_SHELL_INTEGRATION_BASH_INJECT
builtin unset ORCHBEE_SHELL_INTEGRATION_BASH_RCFILE

if (( __orchbee_automatic_rcfile_set )); then
	[[ ! -r "$__orchbee_automatic_rcfile" ]] || builtin source "$__orchbee_automatic_rcfile"
elif [[ -r "$HOME/.bashrc" ]]; then
	builtin source "$HOME/.bashrc"
fi

if [[ -r "$__orchbee_automatic_root/orchbee.bash" ]]; then
	builtin source "$__orchbee_automatic_root/orchbee.bash"
fi

builtin unset __orchbee_automatic_rcfile __orchbee_automatic_rcfile_set
builtin unset __orchbee_automatic_root
