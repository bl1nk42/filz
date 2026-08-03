# powershell completion
Register-ArgumentCompleter -CommandName {exe} -ScriptBlock {
    param($commandName, $wordToComplete, $cursorPosition)
    $commands = 'new','add','list','find','archive','clean','doctor','system','tools','completion'
    $flags = '--help','-h','--all','--rust','--python','--next','--node','--tool','--asset'
    $commands + $flags | Where-Object { $_ -like "$wordToComplete*" } | ForEach-Object {
        [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_)
    }
}
