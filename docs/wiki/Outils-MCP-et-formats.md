# Outils MCP et formats

Les outils sont exposés au client IA connecté par **Streamable HTTP**. Leur disponibilité et leur description exacte proviennent du serveur de la version installée.

## Inventaire des outils

| Outil | Rôle |
| --- | --- |
| `list_folders` | Découvrir les dossiers autorisés et leur droit `writable` |
| `list_files` | Parcourir le contenu d'un dossier |
| `read_file` | Extraire du texte, consulter des métadonnées et le `sha256` |
| `read_file_bytes` | Lire un fichier sous forme de segments base64 |
| `render_svg` | Produire un aperçu PNG d'un SVG |
| `write_text_file` | Créer ou modifier texte, Markdown, CSV, JSON, SVG, etc. |
| `create_document` | Créer un PDF, un DOCX ou un PPTX à partir de sections structurées |
| `write_spreadsheet` | Créer ou copier un classeur XLSX et renseigner ses cellules |
| `replace_office_text` | Remplacer du texte dans une nouvelle copie DOCX/PPTX |
| `create_binary_file` | Enregistrer un fichier à partir d'octets base64 valides |
| `get_authoring_guide` | Fournir à l'assistant le guide de création avancée |
| `validate_designed_pdf` | Prévalider une source Typst sans enregistrer un document partagé |
| `create_designed_pdf` | Composer un PDF avancé avec Typst, styles et assets |
| `render_pdf_page` | Afficher une page PDF sous forme d'image PNG |
| `ocr_file` | Reconnaître du texte dans des scans et images |
| `inspect_office_package` | Inspecter les parties internes OOXML d'un Office |
| `write_office_package` | Créer ou copier un paquet Office avec personnalisation OOXML |

## Formats pris en charge

| Format | Capacités |
| --- | --- |
| **PDF** | Extraction de texte, lecture d'une page, création classique ou avancée, rendu PNG, OCR des scans |
| **DOCX** | Lecture du corps, création, remplacement ciblé dans une copie, modification OOXML avancée |
| **XLSX** | Lecture de feuilles et cellules, création, copie modifiée de cellules, OOXML avancé ; formules **non calculées** |
| **PPTX** | Lecture par diapositive, création, remplacement dans une copie, OOXML avancé |
| **XLS / XLSB / XLSM / ODS** | Lecture des valeurs de feuilles ; pas d'exécution des macros |
| **SVG** | Lecture du XML, aperçu PNG et écriture contrôlée |
| **TXT / MD / CSV / TSV / JSON / XML / HTML / code et fichiers UTF-8** | Lecture, création et modification de texte |
| **Autres fichiers** | Lecture d'octets et création à partir d'un contenu binaire déjà valide, sans conversion automatique |

Les anciens `.doc` et `.ppt` ne sont pas interprétés. Un fichier binaire ne devient pas un vrai PDF ou Office par le simple changement de son extension.

## Ressources de référence

- [Outils et paramètres avec exemples JSON](../MCP-TOOLS.md).
- [Mise en page avancée, OCR, limites et XML Office](../AUTHORING.md).
- [Exemples de création](./Creation-de-documents.md).

[← Accueil](./Home.md) · [Création de documents →](./Creation-de-documents.md)
