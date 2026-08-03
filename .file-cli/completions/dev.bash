# bash completion
_dev_complete()
{
    local cur prev words cword
    _init_completion -n : || return
    case "${prev}" in
        completion)
            COMPREPLY=( $(compgen -W "bash fish zsh powershell" -- "$cur") )
            return
            ;;
        new|add|find|archive)
            return
            ;;
    esac

    COMPREPLY=( $(compgen -W "--help -h --all --rust --python --next --node --tool --asset --tools" -- "$cur") )
}
complete -F _dev_complete dev
