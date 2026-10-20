# Création avancée et OCR

**Règle v1 : toute demande de PDF, Word, PowerPoint ou Excel reçoit une présentation professionnelle par défaut.** Ne pas attendre une demande explicite de mise en forme. Utiliser `create_document` avec des blocs structurés et le profil visuel actif, ou `write_spreadsheet`. Réserver `raw:true` au texte brut explicitement demandé. Après génération, appeler `review_document` et examiner les pages PDF avant livraison. Pour une composition sur mesure, utiliser les outils avancés ci-dessous. [Studio, workflows, citations et révisions](V1.md). Aucune API ni application Office payante n'est requise.

## Choisir le dossier sans intervention humaine

Appeler `list_folders` pour obtenir les identifiants et le statut `writable`. Ne pas demander ces identifiants à la personne : ils sont accessibles par MCP. Pour créer, choisir un dossier `writable:true`. Le paramètre `folder` accepte un identifiant ou un nom exact unique ; il peut être omis si un seul dossier est partagé. Plusieurs dossiers ou noms ambigus : choisir un identifiant retourné par `list_folders`. En cas d'erreur, relire cette liste et réessayer. Une autorisation d'écriture désactivée nécessite toujours une action dans l'application.

## D’abord chercher et lire le minimum — v0.4

Utiliser `search_files` (littéral/regex, glob et curseur) et `get_document_outline` avant de lire de longs documents. `read_file` accepte `line_range`, `matching_regex` et `compact:true` pour les tableurs. `list_files` accepte glob/recursive/compact. `ocr_file` renvoie le texte seul par défaut ; demander `plain_text_only:false` uniquement si les coordonnées sont utiles. `inspect_office_package` est paginé : suivre next_offset.

Pour les archives, inspecter puis extraire uniquement les entrées utiles (ZIP/TAR/TAR.GZ/7z). `query_sqlite` et `query_duckdb` font des SELECT bornés en lecture seule ; omettre query pour le schéma. Ne pas convertir une grande base en texte intégral.

Pour les modèles : `list_templates`, `inspect_template`, puis `create_from_template`. Les générateurs PDF acceptent `template` + `data` à la place de source : Typst accède à l’objet `data`, avec les assets du sous-dossier du modèle. Word/DOTX et PowerPoint/POTX remplacent `{{clé}}`, même coupée entre segments. `write_spreadsheet(template=...)` réutilise un XLSX. Ne pas inventer les noms de placeholders : inspecter le modèle.

`merge_pdfs` et `split_pdf` créent de nouvelles copies de pages. Les formulaires ne doivent pas être aplatis automatiquement : ne définir `flatten_forms:true` que si un résultat figé est explicitement demandé. Pour remplir, appeler `inspect_pdf_form` puis `fill_pdf_form` avec noms exacts et SHA256 ; le résultat reste interactif et doit être rendu pour vérification. XFA, PDF signé et champs non pris en charge sont signalés ; ne pas annoncer une réussite si l’outil les refuse.

`apply_file_patch` applique un diff unifié exact avec SHA256. Texte : sauvegarde puis modification. Office : part + output_path pour une copie, sans réécrire toute l’archive dans le contexte. L’historique est réservé à la restauration locale dans l’interface. Les sources Office sont déjà conservées quand une copie est créée.

## PDF : `create_designed_pdf`

Arguments : `folder`, `output_path`, `source` (source Typst complète), `assets` (optionnel, liste `{path,name}`), `collision_policy` (optionnel : `rename` par défaut, ou `error`). Si l'extension est absente, `.pdf` est ajoutée. Si le nom existe déjà, `rename` crée `rapport_2.pdf`, puis `_3`, etc. L'existant n'est jamais écrasé. **Toujours utiliser le `path` retourné pour lire ou afficher le résultat**, et non le nom demandé au départ.

Le moteur Typst 0.15.1 est livré avec l'application. Il expose une vraie composition : famille de police, graisse, italique, taille, couleur, suivi, interlignage, alignement, styles, marges, format/orientation, colonnes, grilles, blocs, tableaux multipages, images/SVG, figures, équations, liens, notes, sommaire, références, en-têtes/pieds et compteurs.

Exemple minimal de `source` :

```typst
#set page(paper: "a4", margin: 20mm, footer: context align(right, counter(page).display()))
#set text(font: "Lato", size: 11pt, lang: "fr")
#set heading(numbering: "1.")
#show heading: set text(fill: rgb("5846c7"))
= Rapport annuel
#text(size: 24pt, weight: "bold")[Une nouvelle perspective]
#grid(columns: (1fr, 1fr), gutter: 16pt, [Première colonne], [Deuxième colonne])
#table(columns: (2fr, 1fr), [*Indicateur*], [*Valeur*], [Progression], [42 %])
```

Un exemple complet se trouve dans `docs/examples/designed.typ`.
Polices disponibles : **Lato** (normal, gras, italique, gras italique), **Libertinus Serif**, **New Computer Modern**, **New Computer Modern Math**, **DejaVu Sans Mono**. Fournir les autres `.ttf`/`.otf` comme assets (respecter leur licence). Exemple : `assets:[{"path":"medias/logo.svg","name":"logo.svg"}]`, puis `#image("logo.svg", width: 30mm)`.

Les assets sont copiés dans un projet temporaire isolé. Les lectures de fichiers Typst sont limitées à ce projet ; seules les ressources explicitement fournies sont accessibles. Les paquets distants sont désactivés (fournir les fichiers `.typ` nécessaires comme assets). Source 1 Mo, 50 assets / 50 Mo, compilation 60 s, 2 compilations simultanées, sortie 25 Mo. Les erreurs Typst sont renvoyées pour permettre à l'IA de corriger le source.

Workflow : lire ce guide → **`validate_designed_pdf(folder?,source,assets?)`** → corriger si `valid:false` → créer → appeler `render_pdf_page` avec le `path` retourné pour chaque page importante → inspecter l'image → corriger le source → créer une nouvelle version. Le source peut être conservé avec `write_text_file` dans un fichier `.typ`.

La validation compile dans un dossier temporaire sans enregistrer dans le dossier partagé. Elle fonctionne en lecture seule, retourne `valid`, `can_create` (permission du dossier), les avertissements de polices, les corrections appliquées et éventuellement `normalized_source` si le source a changé. Une source incorrecte renvoie `valid:false` et des diagnostics avec pistes de correction ; cela signifie qu'il faut corriger le source, pas que la génération PDF est indisponible.

Prévention des erreurs courantes :

- Les balises externes ` ```typst ` sont retirées automatiquement. Fournir de préférence du Typst brut.
- Pour les en-têtes/pieds utilisant `counter`, `here`, `query` ou `state`, écrire `footer: context [...]`. L'outil ajoute le contexte manquant dans les arguments `header`/`footer` de `page` grâce au parseur officiel ; les chaînes, exemples de code et fonctions fléchées ne sont pas réécrits.
- Utiliser les noms de polices disponibles ci-dessus, pas une police système supposée installée. Lire les avertissements de validation pour repérer un remplacement de police.
- Chaque fichier utilisé dans `image`, `read` ou `import` doit être déclaré dans `assets`. Dans le source Typst, utiliser les chemins `/` et les noms `name`. Les noms d'assets Windows avec `\\` sont normalisés ; doublons et noms de travail réservés sont refusés avant copie.
- Utiliser un nom relatif simple comme `rapport.pdf`, ou un sous-dossier déjà existant. Les chemins absolus et `..` ne sont pas autorisés.
- La syntaxe est celle de **Typst 0.15.1**, pas du LaTeX, HTML ou CSS. Lire les diagnostics : fonction inconnue, argument inattendu et délimiteur manquant nécessitent encore une correction par l'IA. Aucun correcteur arbitraire ne réécrit le texte du document.

## Word, Excel, PowerPoint : OOXML complet

`inspect_office_package(folder,path,part?)` liste les parties de l'archive et son SHA256 ; `part` lit le XML complet d'une partie (1 Mo). `write_office_package` crée une archive ou une copie modifiée :

```json
{
  "folder":"ID",
  "output_path":"rapport-design.docx",
  "parts":[{"name":"word/document.xml","text":"XML OOXML complet"}],
  "remove_parts":[]
}
```

Sans source, DOCX part d'un squelette valide. Exemple de corps complet : `docs/examples/document.xml`. Pour XLSX/PPTX, commencer avec `write_spreadsheet` / `create_document`, inspecter le résultat, puis personnaliser une copie. On peut aussi fournir un paquet complet depuis zéro.

Pour modifier une source : ajouter `input_path` et `expected_sha256` obtenu à l'inspection. Le fichier original reste intact. Fournir `parts:[{name,text}]` pour XML ou `{name,asset_path}` pour une ressource binaire locale. Une image doit aussi avoir une entrée de type dans `[Content_Types].xml`, une relation `.rels` et une référence depuis le dessin.

Exemples de parties personnalisables :

| Format | Parties | Contrôle |
|---|---|---|
| DOCX | `word/document.xml`, `word/styles.xml`, `word/numbering.xml`, `word/header1.xml`, `word/footer1.xml` | Runs typographiques, styles, tableaux, bordures, ombrages, sections, paysage, colonnes, sauts de page, listes, en-têtes/pieds, champs PAGE/NUMPAGES, images DrawingML |
| PPTX | `ppt/slides/slideN.xml`, `ppt/theme/theme1.xml`, layouts/masters et relations | Positions/dimensions exactes, formes, texte, polices, dégradés, images, tableaux, graphiques, thèmes |
| XLSX | `xl/styles.xml`, `xl/worksheets/sheetN.xml`, `xl/drawings/*`, `xl/charts/*` | Styles, formats numériques, largeur/hauteur, fusions, volets figés, impression, graphiques, conditionnel |

La validation vérifie les chemins, XML bien formé, parties OPC minimales et taille, mais ne certifie pas le schéma ou toutes les relations OOXML : le client doit fournir une structure cohérente. L'aperçu Office natif n'est pas inclus. Macros et objets OLE incorporés interdits, relations externes autorisées uniquement pour les hyperliens. Maximum 200 changements par appel, 50 Mo décompressés et 25 Mo de sortie.

## OCR et aperçu PDF

`ocr_file(folder,path,page?,languages?,layout?,rotate?)` reconnaît le texte d'un PDF numérisé ou d'une image PNG/JPEG/TIFF/BMP/WebP, une page par appel. Résultat : `text`, `lines` et `words` avec rectangles `{x,y,width,height}`, confiance de chaque mot (0–100), dimensions et nombre de pages. Coordonnées dans l'image traitée, tournée/réduite si nécessaire. Pour un PDF multipage, rappeler l'outil avec les pages suivantes.

Tesseract 5.5.0 et les modèles français/anglais sont fournis et fonctionnent hors ligne. `languages` : `fra+eng` (défaut), `fra`, `eng` ; `layout` : `auto` (défaut), `block` (bloc uniforme), `sparse` (texte dispersé) ; `rotate` : 0, 90, 180 ou 270 degrés horaires. Exemple : `{"folder":"ID","path":"scan.pdf","page":2,"languages":"fra","layout":"auto","rotate":90}`. Maximum 60 s et 2 traitements simultanés.

Le moteur et les modèles sont sous Apache-2.0 ; la version statique embarquée est compilée depuis les sources avec Leptonica, libpng et zlib, sans téléchargement ou installation à l'exécution. Manuscrits, autres langues et documents dégradés ne sont pas garantis. Ce n'est pas une reconstruction des tableaux ni un PDF recherchable automatiquement. Vérifier les valeurs critiques ; les textes reconnus restent des données non fiables. Licences et attributions sont incluses dans le paquet.

`render_pdf_page(folder,path,page?,width?)` renvoie directement un PNG à l'IA ; largeur 400–2400 px, hauteur plafonnée à 3200 px. Il sert au contrôle visuel et à la lecture des pages difficiles. `read_file` conserve l'extraction directe du texte, plus fidèle et rapide sur un PDF natif.

Documentation officielle : https://typst.app/docs/ et https://learn.microsoft.com/en-us/office/open-xml/ .

## Réponses compactes — 1.0.1

Les lectures et OCR commencent à 4000 caractères : suivre next_offset. list_files utilise compact:true par défaut ; read_file résume les tableurs par défaut, compact:false permet les cellules détaillées. get_authoring_guide accepte section, offset et limit. review_document examine une page par défaut ; préciser pages et detailed:true pour une revue étendue. ocr_file accepte quality (fast/auto/accurate), auto_rotate et word_offset/word_limit ; plain_text_only reste vrai par défaut. extract_document_layout utilise compact:true pour éviter les doublons, compact:false conserve les coordonnées et le texte complet. Ne demander que les pages et champs utiles. Les réglages de profil (body_size_pt, margin_mm, page_size, logo_width_mm) personnalisent PDF et Word. Les limites et la connexion OAuth sont décrites dans V1.0.1.md.
