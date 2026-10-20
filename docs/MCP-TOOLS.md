# Outils MCP et génération de fichiers

Base v0.4 : [nouveaux outils et exemples](V0.4.md). Recherche, lectures ciblées, PDF, archives, SQL, modèles, patchs et historique. Les en-têtes sécurisés se copient séparément de l’adresse du serveur.

Pour le contrôle de typographie/mise en page et l'OCR, voir [AUTHORING.md](AUTHORING.md) ou appeler `get_authoring_guide`. Création avancée : `validate_designed_pdf`, `create_designed_pdf`, `inspect_office_package`, `write_office_package`, `render_pdf_page`, `ocr_file`. Valider le source avant création pour corriger les erreurs Typst sans échouer lors de l'enregistrement. En cas de nom occupé, la création PDF ajoute un suffixe ; toujours relire le `path` retourné.

**Le jeton Bearer est obligatoire dans tous les modes en v0.4.** **Les dossiers sont en lecture seule à l'ajout.** Pour autoriser une génération ou une modification, arrêter le MCP puis cliquer sur **Lecture seule** à côté du dossier : le bouton devient **Écriture autorisée**. Redémarrer ensuite le serveur. Cette autorisation appartient au dossier et ne peut pas être activée par un outil MCP distant.

## Catalogue des 42 outils v1

| Usage | Outils |
| --- | --- |
| Studio v1 | `list_brand_profiles`, `index_folder`, `search_index`, `inspect_document_elements`, `edit_document_elements`, `start_workflow`, `workflow_status`, `control_workflow`, `extract_document_layout`, `review_document` |
| Découverte et lecture | `list_folders`, `list_files`, `search_files`, `get_document_outline`, `read_file`, `read_file_bytes` |
| Aperçu et OCR | `render_svg`, `render_pdf_page`, `ocr_file` |
| Création et modification | `write_text_file`, `create_document`, `write_spreadsheet`, `replace_office_text`, `create_binary_file`, `apply_file_patch` |
| Mise en page avancée | `get_authoring_guide`, `validate_designed_pdf`, `create_designed_pdf`, `inspect_office_package`, `write_office_package` |
| PDF existants | `merge_pdfs`, `split_pdf`, `inspect_pdf_form`, `fill_pdf_form` |
| Archives | `inspect_archive`, `extract_archive`, `create_archive` |
| Bases en lecture seule | `query_sqlite`, `query_duckdb` |
| Modèles | `list_templates`, `inspect_template`, `create_from_template` |

Les schémas du serveur donnent les paramètres exacts. Voir [V0.4](V0.4.md) pour les exemples et limites. Commencer par une recherche ou un plan puis lire les extraits utiles. L’OCR renvoie du texte simple par défaut ; `plain_text_only:false` active les métadonnées détaillées.

Voir [les exemples et limites v1](V1.md). `create_document` utilise une mise en page professionnelle par défaut ; `raw:true` est réservé à une demande explicite.

## Formats pris en charge

| Format | Lire | Créer | Modifier |
| --- | --- | --- | --- |
| PDF | Texte, page par page | Document A4, titres, paragraphes, listes, pagination | Fusion, extraction de pages et formulaires AcroForm |
| DOCX | Corps et tableaux sous forme de texte | Titres, paragraphes, listes | Remplacement de texte, dans une nouvelle copie |
| XLSX | Feuilles et valeurs mises en cache | Feuilles, cellules texte/nombre/booléen/formule | Cellules modifiées, dans une nouvelle copie |
| PPTX | Texte par diapositive, dans l'ordre du document | Titre et diapositives de contenu | Remplacement de texte, dans une nouvelle copie |
| XLS, XLSB, XLSM, ODS | Feuilles et valeurs mises en cache | Non | Non |
| Texte, Markdown, CSV, TSV, JSON, SVG, XML, code | UTF-8 | Oui | Oui, avec empreinte SHA-256 |
| Autres formats binaires | Octets en base64 | À partir d'octets valides fournis par le client | Pas de conversion automatique |

Les formats binaires ont une structure propre : donner une extension ne suffit pas. Les générateurs dédiés produisent cette structure pour PDF/DOCX/XLSX/PPTX. `create_binary_file` permet d'enregistrer d'autres types si le client fournit déjà leurs octets valides, sans prétendre savoir les concevoir ou les convertir automatiquement.

Les générateurs ne lancent ni Microsoft Office, ni scripts, ni macros, ni API d'IA. Les dépendances sont open source. L'IA connectée choisit le contenu et appelle les outils.

## Découverte et lecture

- `list_folders` : identifiants des dossiers, noms et autorisation `writable`.
- `list_files` : Parcours récursif, glob, mode compact et pagination, 500 entrées maximum par segment.
- `read_file` : texte, métadonnées et `sha256`. `page` désigne une page PDF, une diapositive PowerPoint ou une feuille de calcul ; la première vaut 1. `offset` et `limit` segmentent le texte.
- `read_file_bytes` : octets en base64, jusqu'à 1 Mo par segment. Continuer avec `next_offset` pour reconstituer un fichier généré.
- `render_svg` : aperçu PNG retourné à un modèle compatible avec les images.

## Créer un PDF, un Word ou un PowerPoint

Appeler `create_document`. L'extension de `path` sélectionne le générateur ; le fichier doit être nouveau.

```json
{
  "folder": "IDENTIFIANT_DU_DOSSIER",
  "path": "rapport.docx",
  "title": "Rapport de projet",
  "sections": [
    {
      "heading": "Avancement",
      "text": "Le projet peut maintenant produire plusieurs formats.",
      "bullets": ["Documents Word", "Documents PDF", "Présentations PowerPoint"]
    }
  ]
}
```

Utiliser `rapport.pdf` pour un PDF ou `presentation.pptx` pour des diapositives. Les longs contenus PowerPoint sont répartis sur plusieurs diapositives. Les PDF utilisent la police libre Lato, avec les caractères couverts par cette police (notamment latin, grec et cyrillique). Les autres caractères sont refusés plutôt qu'affichés comme carrés vides.

Limites : 100 sections, 200 Ko de contenu textuel, 100 pages PDF, 150 diapositives PowerPoint, 90 caractères par titre PowerPoint. Pour contrôler les styles, tableaux et médias, utiliser Typst ou les outils OOXML du guide avancé.

## Créer ou modifier un classeur Excel

Appeler `write_spreadsheet`. Chaque cellule comporte une feuille, une adresse telle que `A1`, un `kind` (`text`, `number`, `boolean`, `formula`) et une `value` sous forme de chaîne.

```json
{
  "folder": "IDENTIFIANT_DU_DOSSIER",
  "output_path": "budget.xlsx",
  "cells": [
    {"sheet": "Budget", "cell": "A1", "kind": "text", "value": "Poste"},
    {"sheet": "Budget", "cell": "B2", "kind": "number", "value": "125.50"},
    {"sheet": "Budget", "cell": "C2", "kind": "formula", "value": "SUM(B2,25)"}
  ]
}
```

Pour une modification, ajouter `input_path` et son `expected_sha256`, obtenu par `read_file`. `output_path` doit être un nouveau fichier : l'original est conservé. Au maximum 10 000 cellules sont acceptées par appel. Les formules sont stockées mais pas calculées par MCP Win. Vérifier les mises en forme et fonctions complexes dans Excel ou LibreOffice.

## Modifier un texte, CSV, JSON ou SVG

`write_text_file` reçoit `folder`, `path` et `content`. Un nouveau fichier ne nécessite pas d'empreinte. Pour remplacer un fichier existant, ajouter `expected_sha256` avec la valeur reçue à la lecture : si le fichier a changé, la modification est refusée. Les JSON/XML/SVG sont validés avant l'écriture. Le contenu texte est limité à 2 Mo.

## Modifier un DOCX ou PPTX

`replace_office_text` reçoit `folder`, `path`, `output_path`, `expected_sha256`, `find`, `replacement` et `expected_occurrences`. Les éléments non concernés de l'archive sont conservés et l'original reste intact. Le remplacement vise les segments textuels du corps Word ou des diapositives. Un mot découpé entre plusieurs segments de mise en forme doit être remplacé segment par segment. Les en-têtes, pieds de page, notes et graphiques ne sont pas édités.

## Autres fichiers

`create_binary_file` enregistre `data_base64` dans un nouveau `path`, dans un dossier autorisé en écriture. Le contenu base64 est limité à 3 000 000 caractères, soit environ 2 Mo d'octets. Le fichier n'est ni interprété ni exécuté. `read_file_bytes` permet de récupérer n'importe quel format pris en charge par le stockage local, sans conversion.

Toutes les sorties sont limitées à 25 Mo. Les écritures sont préparées dans le dossier cible puis finalisées sans fichier partiel ; les créations refusent les écrasements. Les chemins absolus, sorties hors dossier et flux alternatifs Windows sont refusés.
