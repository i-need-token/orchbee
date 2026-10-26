__orchbee_has() {
  local needle=$1 item
  shift
  for item in "$@"; do [[ $item == "$needle" ]] && return 0; done
  return 1
}
__orchbee_any() {
  local item
  for item in "$@"; do __orchbee_has "$item" "${seen[@]}" && return 0; done
  return 1
}
__orchbee_limit() {
  local key=$1 proposed=${2-} group flag
  local -a reductions
  __orchbee_meta "limit|$key"; limit=${reply[0]}
  __orchbee_meta "reductions|$key"; reductions=("${reply[@]}")
  for group in "${reductions[@]}"; do
    local -a members
    local IFS=' '; members=($group)
    if __orchbee_any "${members[@]}" || __orchbee_has "$proposed" "${members[@]}"; then ((limit--)); fi
  done
}
__orchbee_string() {
  local key=$1 value=$2 pattern
  __orchbee_meta "string|$key"; ((${#reply[@]})) || return 0
  pattern=${reply[0]}
  __orchbee_meta "trim|$key"
  if ((${#reply[@]})); then
    value="${value#"${value%%[![:space:]]*}"}"
    value="${value%"${value##*[![:space:]]}"}"
  fi
  __orchbee_meta "excluded|$key"
  __orchbee_has "$value" "${reply[@]}" && return 1
  [[ $value =~ $pattern ]]
}
__orchbee_candidates() {
  if [[ -n ${ZSH_VERSION-} ]]; then emulate -L ksh; fi
  local prefix=$1 word flag pending='' machine=0 index count valid key width operands limit
  local joined='' selected offset candidate group help_key='' business=0
  shift
  local -a reply positionals seen value_flags values commands viable candidates offered allowed repeats selectors command_words
  positionals=(); seen=(); value_flags=(); values=(); viable=(); candidates=(); offered=()
  if [[ ${1-} == --machine ]]; then
    (($# >= 2)) || return
    [[ -n $2 && $2 != -* ]] || return
    machine=1; shift 2
  elif [[ ${1-} == --machine=* ]]; then
    word=${1#--machine=}; [[ -n $word && $word != -* ]] || return
    machine=1; shift
  fi
  for word in "$@"; do
    [[ $word == -- ]] && return
    if [[ -n $pending ]]; then value_flags+=("$pending"); values+=("$word"); pending=''; continue; fi
    __orchbee_meta modes
    if __orchbee_has "$word" "${reply[@]}" || [[ $word == -h ]]; then return; fi
    if [[ $word == -* && $word != - ]]; then
      flag=${word%%=*}
      __orchbee_meta flags; __orchbee_has "$flag" "${reply[@]}" || return
      if [[ $word == *=* ]]; then __orchbee_meta joined-flags; __orchbee_has "$flag" "${reply[@]}" || return; fi
      seen+=("$flag")
      __orchbee_meta value-flags
      if __orchbee_has "$flag" "${reply[@]}"; then
        if [[ $word == *=* ]]; then value_flags+=("$flag"); values+=("${word#*=}"); else pending=$flag; fi
      elif [[ $word == *=* ]]; then return
      fi
    else positionals+=("$word")
    fi
  done
  __orchbee_meta global
  for flag in "${seen[@]}"; do __orchbee_has "$flag" "${reply[@]}" || business=1; done
  if ((machine)) && __orchbee_has --session "${seen[@]}"; then return; fi
  if [[ ${positionals[0]-} == help || ${positionals[0]-} == completion ]]; then
    ((business == 0)) && [[ -z $pending ]] || return
    if [[ ${positionals[0]} == completion ]]; then
      ((${#positionals[@]} == 1 && machine == 0)) || return
      __orchbee_meta shells; candidates=("${reply[@]}" --help)
    elif ((${#positionals[@]} == 1)); then __orchbee_meta root; candidates=("${reply[@]}")
    else
      for ((index=1; index<${#positionals[@]}; index++)); do help_key=${help_key:+$help_key:}${positionals[index]}; done
      __orchbee_meta "help|$help_key"; candidates=("${reply[@]}")
    fi
  else
    __orchbee_meta commands; commands=("${reply[@]}")
    for key in "${commands[@]}"; do
      __orchbee_meta "words|$key"; command_words=("${reply[@]}"); width=${#command_words[@]}; valid=1
      for ((index=0; index<width && index<${#positionals[@]}; index++)); do
        [[ ${command_words[index]} == "${positionals[index]}" ]] || { valid=0; break; }
      done
      ((valid)) || continue
      if ((machine)); then __orchbee_meta "machine|$key"; ((${#reply[@]})) || continue; fi
      __orchbee_meta "flags|$key"; allowed=("${reply[@]}")
      __orchbee_meta "repeat|$key"; repeats=("${reply[@]}")
      for flag in "${seen[@]}"; do
        __orchbee_has "$flag" "${allowed[@]}" || { valid=0; break; }
        count=0; for word in "${seen[@]}"; do [[ $word == "$flag" ]] && ((count++)); done
        if ((count > 1)) && ! __orchbee_has "$flag" "${repeats[@]}"; then valid=0; break; fi
        __orchbee_meta "conflicts|$key|$flag"; if __orchbee_any "${reply[@]}"; then valid=0; break; fi
      done
      ((valid)) || continue
      for ((index=0; index<${#value_flags[@]}; index++)); do
        flag=${value_flags[index]}
        __orchbee_string "$key|$flag" "${values[index]}" || { valid=0; break; }
        __orchbee_meta "closed|$key|$flag"
        if ((${#reply[@]})); then
          __orchbee_meta "values|$key|$flag"
          __orchbee_has "${values[index]}" "${reply[@]}" || { valid=0; break; }
        fi
      done
      ((valid)) || continue
      __orchbee_limit "$key"
      ((${#positionals[@]} <= width + limit)) || continue
      if ((${#positionals[@]} > width)); then
        __orchbee_string "$key|operand" "${positionals[width]}" || continue
        __orchbee_meta "operand-equals|$key"
        if ((${#reply[@]})); then
          flag=${reply[0]}
          for ((index=0; index<${#value_flags[@]}; index++)); do
            if [[ ${value_flags[index]} == "$flag" && ${values[index]} != "${positionals[width]}" ]]; then valid=0; break; fi
          done
          ((valid)) || continue
        fi
      fi
      viable+=("$key")
      ((${#positionals[@]} < width)) && candidates+=("${command_words[${#positionals[@]}]}")
      __orchbee_meta "selectors|$key"; selectors=("${reply[@]}"); selected=0
      __orchbee_any "${selectors[@]}" && selected=1
      __orchbee_meta "agent|$key"
      if ((${#reply[@]} && ${#positionals[@]} == width && selected == 0)); then
        __orchbee_meta agent-targets; candidates+=("${reply[@]}")
      fi
      operands=$((${#positionals[@]} - width))
      __orchbee_meta "offset|$key"; offset=$((${reply[0]} - selected))
      if ((operands >= offset && operands < limit)); then
        __orchbee_meta "operand-values|$key"; candidates+=("${reply[@]}")
      fi
      for flag in "${allowed[@]}"; do
        if ((machine)) && [[ $flag == --session ]]; then continue; fi
        if ((operands > 0)) && __orchbee_has "$flag" "${selectors[@]}"; then continue; fi
        if __orchbee_has "$flag" "${seen[@]}" && ! __orchbee_has "$flag" "${repeats[@]}"; then continue; fi
        __orchbee_meta "conflicts|$key|$flag"; __orchbee_any "${reply[@]}" && continue
        __orchbee_meta "requires|$key|$flag"; if ((${#reply[@]})) && ! __orchbee_any "${reply[@]}"; then continue; fi
        __orchbee_limit "$key" "$flag"; ((operands <= limit)) || continue
        offered+=("$flag")
      done
      candidates+=(--help)
    done
    ((${#viable[@]})) || return
    if [[ -z $pending && $prefix == --*=* ]]; then
      flag=${prefix%%=*}; __orchbee_has "$flag" "${offered[@]}" || return
      __orchbee_meta joined-flags; __orchbee_has "$flag" "${reply[@]}" || return
      pending=$flag; joined=$flag=
    fi
    if [[ -n $pending ]]; then
      candidates=()
      for key in "${viable[@]}"; do
        __orchbee_meta "values|$key|$pending"
        for word in "${reply[@]}"; do candidates+=("$joined$word"); done
      done
    else
      candidates+=("${offered[@]}")
      if ((${#positionals[@]} == 0 && business == 0)); then
        candidates+=(help)
        if ((machine == 0)); then
          candidates+=(completion)
          __orchbee_meta empty-modes; local -a empty_modes; empty_modes=("${reply[@]}")
          __orchbee_meta modes
          for flag in "${reply[@]}"; do
            if ((${#seen[@]})) && __orchbee_has "$flag" "${empty_modes[@]}"; then continue; fi
            candidates+=("$flag")
          done
        fi
      fi
    fi
  fi
  if (($# == 0 && machine == 0)); then __orchbee_meta prefix; candidates+=("${reply[@]}"); fi
  local -a emitted; emitted=()
  for candidate in "${candidates[@]}"; do
    if [[ $candidate == "$prefix"* ]] && ! __orchbee_has "$candidate" "${emitted[@]}"; then
      printf '%s\n' "$candidate"; emitted+=("$candidate")
    fi
  done
}
