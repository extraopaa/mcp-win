# Créer des documents avec MCP Win

MCP Win peut produire de véritables **PDF, DOCX, XLSX et PPTX** sur l'ordinateur de l'utilisateur. Le choix de l'outil dépend du niveau de présentation requis.

## Quel outil utiliser ?

| Besoin | Outil conseillé |
| --- | --- |
| Rapport PDF simple à sections | `create_document` |
| PDF soigné : couleurs, colonnes, tableaux, en-têtes/pieds, images | `validate_designed_pdf` puis `create_designed_pdf` |
| Word ou PowerPoint simples | `create_document` |
| Tableur avec cellules et formules | `write_spreadsheet` |
| Remplacement ciblé d'un texte Word/PowerPoint | `replace_office_text` |
| Styles, thèmes, en-têtes, images ou graphiques Office avancés | `inspect_office_package` puis `write_office_package` |
| Texte, Markdown, CSV, JSON ou SVG | `write_text_file` |

## Exemple : créer un Word simple

Après avoir découvert un dossier autorisé en écriture avec `list_folders`, l'assistant peut appeler `create_document` :

```json
{
  "folder": "IDENTIFIANT_OU_NOM_UNIQUE",
  "path": "rapport.docx",
  "title": "Rapport de projet",
  "sections": [
    {
      "heading": "Synthèse",
      "text": "Compte rendu du travail réalisé.",
      "bullets": ["Objectifs", "Résultats", "Prochaines étapes"]
    }
  ]
}
```

L'extension `.pdf` ou `.pptx` utilise les autres générateurs structurés de `create_document`.

## PDF avancé : workflow conseillé

1. Appeler `get_authoring_guide` pour connaître la syntaxe Typst et les limites de la version embarquée (**Typst 0.15.1**).
2. Écrire une source Typst complète avec les polices disponibles, notamment **Lato**.
3. Lancer `validate_designed_pdf` pour recueillir `valid`, `can_create`, diagnostics et éventuelles corrections. Corriger la source en cas de `valid:false`.
4. Lancer `create_designed_pdf` sur un dossier autorisé en écriture, éventuellement avec des assets explicites (images, fontes, fichiers Typst).
5. Utiliser **le chemin `path` renvoyé** par l'outil : en cas de collision, le nom peut être suffixé.
6. Inspecter les pages importantes via `render_pdf_page`. Si besoin, corriger et générer une nouvelle version.

Exemple de source Typst minimale :

```typst
#set page(paper: "a4", margin: 20mm, footer: context align(right, counter(page).display()))
#set text(font: "Lato", size: 11pt, lang: "fr")
= Rapport
Voici une synthèse.

#table(columns: (2fr, 1fr), [*Indicateur*], [*Valeur*], [Avancement], [75 %])
```

## Fichiers Office avancés

Les `.docx`, `.xlsx` et `.pptx` sont des archives **OOXML**. `inspect_office_package` permet d'étudier leurs parties ; `write_office_package` écrit des parties XML ou binaires dans un paquet nouveau ou une copie. Pour modifier une source, utiliser son `expected_sha256` ; l'original reste intact.

**Attention :** le contrôle porte notamment sur le XML et les parties minimales, pas sur la conformité exhaustive de toutes les relations OOXML. Tester les documents complexes dans un lecteur Office.

## Limites et détails

Un PDF Typst peut inclure des assets explicites ; les téléchargements de paquets distants sont désactivés. Le générateur standard impose des limites sur les sections et les pages. Les formules Excel enregistrées par MCP Win **ne sont pas recalculées** sur place.

Lire la [documentation technique complète](../AUTHORING.md), les [exemples Typst](../examples/designed.typ) et la [référence des outils](../MCP-TOOLS.md).

[← Outils](./Outils-MCP-et-formats.md) · [OCR et aperçus →](./OCR-et-apercus.md)
