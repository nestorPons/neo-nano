# bash completion for nnano(1)
#
# Installed to /usr/share/bash-completion/completions/nnano by the package.
# Every option comes from the registry in nnano-core; the test in
# crates/nnano/tests/docs.rs fails when one is missing here.

_nnano() {
    local cur prev opts langs
    COMPREPLY=()
    cur=${COMP_WORDS[COMP_CWORD]}
    prev=${COMP_WORDS[COMP_CWORD-1]}
    langs="auto en zh-Hans hi es ar fr bn pt id ja"

    case $prev in
        -T|--tabsize)
            # A number, not a path.
            return 0
            ;;
        --language)
            COMPREPLY=($(compgen -W "$langs" -- "$cur"))
            return 0
            ;;
    esac

    if [[ $cur == --language=* ]]; then
        COMPREPLY=($(compgen -W "$langs" -- "${cur#*=}"))
        COMPREPLY=("${COMPREPLY[@]/#/--language=}")
        return 0
    fi

    if [[ $cur == -* ]]; then
        opts="-T --tabsize -C --casesensitive --no-casesensitive
              --syntax --no-syntax -m --mouse --no-mouse
              --osc52 --no-osc52 --language -h --help -V --version"
        COMPREPLY=($(compgen -W "$opts" -- "$cur"))
        return 0
    fi

    COMPREPLY=($(compgen -f -- "$cur"))
}

complete -o filenames -F _nnano nnano
