# fish completion for nnano(1)
#
# Installed to /usr/share/fish/vendor_completions.d/nnano.fish by the package.
# Files complete as usual for the positional argument.

complete -c nnano -s T -l tabsize -x -d 'Columns a tab advances; 1-64'
complete -c nnano -s C -l casesensitive -d 'Make searches match case by default'
complete -c nnano -l no-casesensitive -d 'Turn case_sensitive off'
complete -c nnano -l syntax -d 'Colour lines with the grammar of the file extension'
complete -c nnano -l no-syntax -d 'Turn syntax off'
complete -c nnano -s m -l mouse -d 'Click to place the cursor, drag to select, wheel to scroll'
complete -c nnano -l no-mouse -d 'Turn mouse off'
complete -c nnano -l osc52 -d 'Also send a copy to the system clipboard, capped at 64 KiB'
complete -c nnano -l no-osc52 -d 'Turn osc52 off'
complete -c nnano -l language -x -a 'auto en zh-Hans hi es ar fr bn pt id ja' -d 'Interface language'
complete -c nnano -s h -l help -d 'Print the options and exit'
complete -c nnano -s V -l version -d 'Print the version and exit'
