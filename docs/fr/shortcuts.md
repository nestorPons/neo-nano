# Raccourcis clavier

nnano utilise par défaut un **agencement de touches de type bureau**. Les raccourcis de nano qui n'entrent pas en conflit sont conservés.

**Notation :** `Ctrl` = `^` · `Alt` = `M-`. Sous macOS, `Alt` est `Option` ; `Cmd+*` n'atteint jamais l'application.
La colonne *Alternative* donne le raccourci hérité de nano, lorsqu'il existe.

## Fichier

| Action | Touche | Alternative |
|---|---|---|
| Enregistrer | `Ctrl+S` | — |
| Enregistrer sous | `Alt+S` | — |
| Ouvrir un fichier | `Ctrl+O` | — |
| Recharger depuis le disque | `Ctrl+R` | — |
| Insérer un fichier au curseur | `Alt+I` | `Ctrl+R` (nano) |
| Nouveau tampon | `Ctrl+N` | — |
| Nouvel onglet | `Ctrl+T` | — |
| Fermer le tampon | `Ctrl+W` | — |
| Quitter | `Ctrl+Q` | — |

## Édition

| Action | Touche | Alternative |
|---|---|---|
| Annuler | `Ctrl+Z` | `Alt+U` |
| Rétablir | `Ctrl+Y` | `Alt+E` |
| Couper | `Ctrl+X` | — |
| Copier | `Ctrl+C` | — |
| Coller | `Ctrl+V` | `Ctrl+U` |
| Couper la ligne | `Ctrl+K` | — |
| Tout sélectionner | `Ctrl+A` | — |
| Commencer la marque | `Alt+A` | `Alt+A` |
| Copier le texte marqué | `Alt+6` | `Alt+6` |
| Marque rectangulaire | `Alt+Z` | — |
| Supprimer un caractère | `Ctrl+D` | — |
| Remplacer | `Ctrl+\` | `Alt+R` |
| Justifier le paragraphe | `Ctrl+J` | — |
| Exécuter une commande externe | `Alt+X` | — |

## Déplacement

| Action | Touche | Alternative |
|---|---|---|
| Aller à la ligne | `Ctrl+G` | `Alt+G` |
| Début de ligne | `Home` | — |
| Fin de ligne | `End` | `Ctrl+E` |
| Première ligne | `Alt+\` | `Alt+\` |
| Dernière ligne | `Alt+/` | `Alt+/` |
| Un mot à gauche / à droite | `Ctrl+←` / `Ctrl+→` | `Alt+←` / `Alt+→` |
| Étendre la sélection vers le haut / le bas | `Shift+↑` / `Shift+↓` | — |
| Une page en haut | `PgUp` | `Alt+↑` |
| Une page en bas | `PgDn` | `Alt+↓` |
| Un caractère en arrière | `Ctrl+B` | `Ctrl+B` |

`Shift+↑`/`Shift+↓` commencent une marque au curseur et l'étendent par lignes entières en conservant la colonne visuelle ; `Esc` la retire. Une fois la marque posée, tout déplacement l'étend, comme dans nano.

## Recherche

| Action | Touche | Alternative |
|---|---|---|
| Rechercher | `Ctrl+F` | — |
| Rechercher le suivant | `F3` | `Alt+W` |
| Rechercher le précédent | `Shift+F3` | `Alt+B` |
| Sensibilité à la casse (dans l'invite) | `Alt+C` | `Alt+C` |
| Expression régulière (dans l'invite) | `Alt+R` | `Alt+R` |

La recherche imite nano : elle **n'est pas incrémentale** (elle s'exécute quand vous validez l'invite), elle est littérale par défaut, insensible à la casse par défaut, et elle revient au début du tampon en affichant *recherche relancée depuis le début*.
`Alt+R` est modal : dans une invite il bascule les expressions régulières, en dehors il ouvre Remplacer.

## Interface

| Action | Touche | Alternative |
|---|---|---|
| Aide | `Ctrl+H` | `F1` |
| Menu des paramètres | `Alt+,` | — |
| Palette de commandes | `Ctrl+P` | — |
| Activer/désactiver la coloration | `Alt+Y` | `Alt+Y` |
| Redessiner l'écran | `Ctrl+L` | `Ctrl+L` |
| Langue de l'interface | `Alt+,` → Langue | `--language <code>` |

La palette (`Ctrl+P`) affiche la touche de chaque commande à droite, et taper une touche (`ctrl+q`) filtre par raccourci. L'aide (`Ctrl+H`) et la palette sont générées depuis la même table que le décodeur de touches : elles ne peuvent pas annoncer une touche qui ne fonctionne pas.

L'interface parle dix langues : English, 简体中文, हिन्दी, Español, العربية, Français, বাংলা, Português, Bahasa Indonesia et 日本語. Choisissez-en une dans les paramètres (`Alt+,` → Langue) ou avec `--language fr` ; `language = "auto"` dans `config.toml` suit `$LC_ALL`/`$LANG`.

## Ce qui change par rapport à nano

| Avant (nano) | Maintenant |
|---|---|
| `Ctrl+O` enregistrer | `Ctrl+S` enregistre |
| `Ctrl+X` quitter | `Ctrl+Q` quitte · `Ctrl+X` coupe |
| `Ctrl+W` rechercher | `Ctrl+F` recherche · `Ctrl+W` ferme le tampon |
| `Ctrl+C` position du curseur | Barre d'état · `Ctrl+C` copie |
| `Ctrl+V` page suivante | `PgDn` / `Alt+↓` · `Ctrl+V` colle |
| `Ctrl+Y` page précédente | `PgUp` / `Alt+↑` · `Ctrl+Y` rétablit |
| `Ctrl+A` début de ligne | `Home` · `Ctrl+A` sélectionne tout |
| `Ctrl+Z` suspendre | Supprimé · `Ctrl+Z` annule |
| `Ctrl+R` insérer un fichier | `Alt+I` · `Ctrl+R` recharge |
| `Ctrl+N` / `Ctrl+P` ligne suivante / précédente | Flèches `↓` / `↑` · nouveau tampon / palette |
| `Ctrl+T` exécuter | `Alt+X` · `Ctrl+T` ouvre un onglet |
| `Ctrl+G` aide | `F1` · `Ctrl+G` va à la ligne |

Inchangés : `Ctrl+K`, `Ctrl+U`, `Ctrl+E`, `Ctrl+D`, `Ctrl+J`, `Ctrl+L`, `Ctrl+B`, `Ctrl+\`, `Alt+U`, `Alt+E`, `Alt+A`, `Alt+6`, `Alt+G`, `Alt+\`, `Alt+/`, `Alt+W`, `Alt+B`, `Alt+C`, `Alt+R`, `Alt+Y`.

## Limites du terminal

- `Ctrl+S` et `Ctrl+Q` exigent que le contrôle de flux XON/XOFF soit désactivé ; sinon le terminal les avale.
- `Enter`, `Tab`, `Backspace` et `Esc` ne sont pas réassignables.
- `Ctrl+B` est le préfixe par défaut de tmux ; `Ctrl+A`, celui de screen.
- `Ctrl+Shift+<lettre>` n'est pas fiable sur les terminaux anciens : il n'y a pas de `Ctrl+Shift+Z` pour rétablir.
- Pour sélectionner du texte avec le terminal, maintenez `Shift` en glissant.
- nnano utilise la souris quand l'option `mouse` est active (par défaut) : un clic place le curseur, un glissement marque, `Alt`+glissement marque un rectangle, la molette défile sans déplacer le curseur, et un clic sur la barre de défilement à droite saute à cette partie du fichier.
- La barre d'état affiche `ligne:colonne/total` : la ligne du curseur, sa colonne visuelle et le nombre total de lignes du document.
- Groupes d'annulation : la frappe est groupée par mot et par une pause de 400 ms ; couper, coller, enregistrer et changer de tampon ferment le groupe. « Tout remplacer » est une seule étape.
- `Ctrl+H` ouvre la référence des raccourcis. Retour arrière est la touche `Backspace` du terminal (`0x7F`) : si votre terminal envoie `0x08`, cette touche ouvre l'aide — configurez-le pour envoyer `0x7F`.

## Invites et fichier de récupération

- `Alt+S` demande un chemin, et `Ctrl+S` aussi sur un tampon sans nom (`nnano -`, un tampon neuf, ou un « enregistrer sous » annulé). `Tab` complète le chemin dans *Enregistrer sous*, *Ouvrir* et *Insérer un fichier* ; une seule correspondance remplit la ligne, plusieurs l'étendent au préfixe commun.
- `Alt+S` écrit un nouveau fichier et adopte son nom : la barre des tampons, la barre d'état et la grammaire de coloration suivent. Un échec d'écriture laisse l'ancien nom en place.
- `Alt+X` exécute la ligne avec `sh -c` et insère sa sortie standard au curseur en une seule étape d'annulation ; la barre d'état indique le code de sortie et la première ligne de l'erreur standard.
- `Ctrl+R` relit le fichier du tampon depuis le disque et remplace le texte ; l'historique d'annulation de l'ancien texte est abandonné. En cas de modifications non enregistrées, il demande une fois (`Ctrl+R` à nouveau recharge, `Ctrl+S` enregistre).
- `Alt+I` demande un fichier et insère son texte au curseur en une seule étape d'annulation ; `Tab` complète le chemin. Le tampon garde son nom et son encodage.
- Tant qu'un tampon a des modifications non enregistrées, nnano écrit un instantané de récupération 2 s après la dernière modification dans `$XDG_STATE_HOME/nnano/recovery/`. Un plantage ou une session SSH tuée est restauré automatiquement au démarrage suivant (`recovered unsaved changes for …`) ; enregistrer ou quitter proprement le supprime.
- `Ctrl+C` et `Alt+6` copient toujours dans le registre interne. Activez l'option `osc52` (`--osc52`) pour envoyer aussi la copie au presse-papiers du système, plafonnée à 64 Kio ; une troncature est signalée dans la barre d'état.
- `Ctrl+J` réenroule le paragraphe autour du curseur (ou les lignes marquées) à la largeur de la zone de texte, en conservant l'indentation de la première ligne. C'est une seule étape d'annulation.
