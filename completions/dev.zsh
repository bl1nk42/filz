# zsh completion
_{exe}()
{
  local context state line
  _arguments -C \
    '(-h --help)'{{-h,--help}}'[แสดง help]' \
    '1:command:(new add list find archive clean doctor system tools completion)' \
    '*::arg:->args'

  case $state in
    args)
      case $words[2] in
        completion)
          _values 'shells' bash fish zsh powershell
          ;;
        list)
          _arguments \
            '--all[แสดงทั้งหมด]' \
            '--rust[กรอง rust]' \
            '--python[กรอง python]' \
            '--next[กรอง next]' \
            '--node[กรอง node]' \
            '--tool[กรอง tool]' \
            '--asset[กรอง asset]'
          ;;
      esac
    ;;
  esac
}
compdef _{exe} {exe}
