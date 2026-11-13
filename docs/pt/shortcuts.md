# Atalhos de teclado

O nnano usa por padrão um **mapa de teclas no estilo desktop**. Os atalhos do nano que não colidem são mantidos.

**Notação:** `Ctrl` = `^` · `Alt` = `M-`. No macOS, `Alt` é `Option`; `Cmd+*` nunca chega à aplicação.
A coluna *Alternativa* é o atalho herdado do nano, quando existe.

## Arquivo

| Ação | Tecla | Alternativa |
|---|---|---|
| Salvar | `Ctrl+S` | — |
| Salvar como | `Alt+S` | — |
| Abrir arquivo | `Ctrl+O` | — |
| Recarregar do disco | `Ctrl+R` | — |
| Inserir arquivo no cursor | `Alt+I` | `Ctrl+R` (nano) |
| Novo buffer | `Ctrl+N` | — |
| Nova aba | `Ctrl+T` | — |
| Fechar buffer | `Ctrl+W` | — |
| Sair | `Ctrl+Q` | — |

## Edição

| Ação | Tecla | Alternativa |
|---|---|---|
| Desfazer | `Ctrl+Z` | `Alt+U` |
| Refazer | `Ctrl+Y` | `Alt+E` |
| Cortar | `Ctrl+X` | — |
| Copiar | `Ctrl+C` | — |
| Colar | `Ctrl+V` | `Ctrl+U` |
| Cortar a linha | `Ctrl+K` | — |
| Selecionar tudo | `Ctrl+A` | — |
| Começar a marca | `Alt+A` | `Alt+A` |
| Copiar o texto marcado | `Alt+6` | `Alt+6` |
| Marca retangular | `Alt+Z` | — |
| Apagar caractere | `Ctrl+D` | — |
| Substituir | `Ctrl+\` | `Alt+R` |
| Justificar parágrafo | `Ctrl+J` | — |
| Executar comando externo | `Alt+X` | — |

## Movimento

| Ação | Tecla | Alternativa |
|---|---|---|
| Ir para linha | `Ctrl+G` | `Alt+G` |
| Início da linha | `Home` | — |
| Fim da linha | `End` | `Ctrl+E` |
| Primeira linha | `Alt+\` | `Alt+\` |
| Última linha | `Alt+/` | `Alt+/` |
| Uma palavra à esquerda / direita | `Ctrl+←` / `Ctrl+→` | `Alt+←` / `Alt+→` |
| Estender a seleção por caractere | `Shift+←` / `Shift+→` | — |
| Estender a seleção por linha | `Shift+↑` / `Shift+↓` | — |
| Uma página acima | `PgUp` | `Alt+↑` |
| Uma página abaixo | `PgDn` | `Alt+↓` |
| Um caractere atrás | `Ctrl+B` | `Ctrl+B` |

`Shift+←`/`Shift+→` estendem a marca por caracteres e `Shift+↑`/`Shift+↓` por linhas mantendo a coluna visual, como em um editor de desktop; `Esc` limpa a marca. Com uma marca ativa, qualquer movimento a estende, como no nano. `Ctrl+C` copia e depois limpa a marca, então o próximo `Ctrl+V` insere no cursor.

## Busca

| Ação | Tecla | Alternativa |
|---|---|---|
| Buscar | `Ctrl+F` | — |
| Buscar próximo | `F3` | `Alt+W` |
| Buscar anterior | `Shift+F3` | `Alt+B` |
| Diferenciar maiúsculas (no prompt) | `Alt+C` | `Alt+C` |
| Expressão regular (no prompt) | `Alt+R` | `Alt+R` |

A busca imita o nano: **não é incremental** (roda ao aceitar o prompt), é literal por padrão, não diferencia maiúsculas por padrão e volta ao início do buffer avisando *busca recomeçou do início*.
`Alt+R` é modal: dentro de um prompt alterna expressões regulares; fora dele abre Substituir.

## Interface

| Ação | Tecla | Alternativa |
|---|---|---|
| Ajuda | `Ctrl+H` | `F1` |
| Menu de configurações | `Alt+,` | — |
| Paleta de comandos | `Ctrl+P` | — |
| Ligar/desligar realce de sintaxe | `Alt+Y` | `Alt+Y` |
| Redesenhar a tela | `Ctrl+L` | `Ctrl+L` |
| Idioma da interface | `Alt+,` → Idioma | `--language <código>` |

A paleta (`Ctrl+P`) mostra à direita a tecla de cada comando, e digitar uma tecla (`ctrl+q`) filtra por atalho. A ajuda (`Ctrl+H`) e a paleta são geradas da mesma tabela que o decodificador de teclas lê, então nunca anunciam uma tecla que não funciona.

A interface fala dez idiomas: English, 简体中文, हिन्दी, Español, العربية, Français, বাংলা, Português, Bahasa Indonesia e 日本語. Escolha um nas configurações (`Alt+,` → Idioma) ou com `--language pt`; `language = "auto"` no `config.toml` segue `$LC_ALL`/`$LANG`.

## O que muda em relação ao nano

| Antes (nano) | Agora |
|---|---|
| `Ctrl+O` salvar | `Ctrl+S` salva |
| `Ctrl+X` sair | `Ctrl+Q` sai · `Ctrl+X` corta |
| `Ctrl+W` buscar | `Ctrl+F` busca · `Ctrl+W` fecha o buffer |
| `Ctrl+C` posição do cursor | Barra de status · `Ctrl+C` copia |
| `Ctrl+V` página abaixo | `PgDn` / `Alt+↓` · `Ctrl+V` cola |
| `Ctrl+Y` página acima | `PgUp` / `Alt+↑` · `Ctrl+Y` refaz |
| `Ctrl+A` início da linha | `Home` · `Ctrl+A` seleciona tudo |
| `Ctrl+Z` suspender | Removido · `Ctrl+Z` desfaz |
| `Ctrl+R` inserir arquivo | `Alt+I` · `Ctrl+R` recarrega |
| `Ctrl+N` / `Ctrl+P` linha seguinte / anterior | Setas `↓` / `↑` · novo buffer / paleta |
| `Ctrl+T` executar | `Alt+X` · `Ctrl+T` abre uma aba |
| `Ctrl+G` ajuda | `F1` · `Ctrl+G` vai para a linha |

Sem mudanças: `Ctrl+K`, `Ctrl+U`, `Ctrl+E`, `Ctrl+D`, `Ctrl+J`, `Ctrl+L`, `Ctrl+B`, `Ctrl+\`, `Alt+U`, `Alt+E`, `Alt+A`, `Alt+6`, `Alt+G`, `Alt+\`, `Alt+/`, `Alt+W`, `Alt+B`, `Alt+C`, `Alt+R`, `Alt+Y`.

## Limites do terminal

- `Ctrl+S` e `Ctrl+Q` exigem o controle de fluxo XON/XOFF desligado; senão o terminal os engole.
- `Enter`, `Tab`, `Backspace` e `Esc` não podem ser remapeados.
- `Ctrl+B` é o prefixo padrão do tmux; `Ctrl+A`, do screen.
- `Ctrl+Shift+<letra>` não é confiável em terminais antigos: não existe `Ctrl+Shift+Z` para refazer.
- Para selecionar texto com o terminal, segure `Shift` ao arrastar.
- O nnano usa o mouse quando a opção `mouse` está ligada (o padrão): clique posiciona o cursor, arrastar marca, `Alt`+arrastar marca um retângulo, a roda rola sem mover o cursor e clicar na barra à direita salta para essa parte do arquivo. Soltar o botão copia o texto marcado para o registro interno e para a área de transferência do sistema (OSC 52); a marca continua visível até clicar noutro lugar ou premir `Esc`.
- A barra de status mostra `linha:coluna/total`: a linha do cursor, sua coluna visual e o total de linhas do documento.
- Grupos de desfazer: a digitação é agrupada por palavra e por uma pausa de 400 ms; cortar, colar, salvar e trocar de buffer fecham o grupo. "Substituir tudo" é um único passo.
- `Ctrl+H` abre a referência de atalhos. Backspace é a tecla Backspace do terminal (`0x7F`): se o seu terminal envia `0x08`, essa tecla abre a ajuda — configure-o para enviar `0x7F`.

## Prompts e arquivo de recuperação

- `Alt+S` pede um caminho, e `Ctrl+S` também em um buffer sem nome (`nnano -`, um buffer novo ou um "salvar como" cancelado). `Tab` completa o caminho em *Salvar como*, *Abrir* e *Inserir arquivo*; um único resultado preenche a linha, vários a estendem até o prefixo comum.
- `Alt+S` grava um novo arquivo e adota o nome: a barra de buffers, a barra de status e a gramática de realce acompanham. Uma falha de escrita mantém o nome antigo.
- `Alt+X` executa a linha com `sh -c` e insere a saída padrão no cursor como um único passo de desfazer; a barra de status informa o código de saída e a primeira linha do erro padrão.
- `Ctrl+R` relê do disco o arquivo deste buffer e substitui o texto; o histórico de desfazer do texto anterior é descartado. Com alterações não salvas, pergunta uma vez (`Ctrl+R` de novo recarrega, `Ctrl+S` salva).
- `Alt+I` pede um arquivo e insere o texto dele no cursor como um único passo de desfazer; `Tab` completa o caminho. O buffer mantém o próprio nome e codificação.
- Enquanto um buffer tem alterações não salvas, o nnano grava um instantâneo de recuperação 2 s após a última edição em `$XDG_STATE_HOME/nnano/recovery/`. Uma queda ou uma sessão SSH morta é restaurada automaticamente na próxima abertura (`recovered unsaved changes for …`); salvar ou sair limpo o apaga.
- `Ctrl+C` e `Alt+6` sempre copiam para o registro interno. Ligue a opção `osc52` (`--osc52`) para enviar também a cópia para a área de transferência do sistema, limitada a 64 KiB; um truncamento é avisado na barra de status. A barra mostra quanto foi copiado, e `Ctrl+V` com o registro vazio avisa.
- `Ctrl+J` reflui o parágrafo em volta do cursor (ou as linhas marcadas) para a largura da área de texto, mantendo a indentação da primeira linha. É um único passo de desfazer.
