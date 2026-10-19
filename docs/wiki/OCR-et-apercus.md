# OCR, images et aperçus

MCP Win contient un moteur d'OCR **Tesseract 5.5.0**, avec les modèles **français** et **anglais**. La reconnaissance s'exécute localement sur le PC Windows.

## Reconnaître le texte d'un scan

L'outil `ocr_file` accepte un **PDF numérisé** ou une image PNG, JPEG, TIFF, BMP ou WebP. Il traite **une page par appel**. Options principales :

| Paramètre | Valeurs |
| --- | --- |
| `folder`, `path` | Dossier autorisé et chemin relatif du fichier |
| `page` | Page à reconnaître, si nécessaire |
| `languages` | `fra+eng` (défaut), `fra` ou `eng` |
| `layout` | `auto` (défaut), `block` ou `sparse` |
| `rotate` | `0`, `90`, `180` ou `270` degrés horaires |

Exemple :

```json
{"folder":"IDENTIFIANT_OU_NOM_UNIQUE","path":"scan.pdf","page":2,"languages":"fra","layout":"auto","rotate":90}
```

La réponse comprend le texte et, lorsqu'ils sont disponibles, lignes, mots, coordonnées de rectangles et niveaux de confiance. Les coordonnées se rapportent à l'image effectivement traitée après rotation/réduction.

**Limites :** l'écriture manuscrite, les langues non fournies et les documents très dégradés ne sont pas garantis. Les tableaux ne sont pas reconstruits automatiquement et l'OCR ne transforme pas, à lui seul, le PDF en PDF recherchable. Vérifiez les montants et autres valeurs sensibles.

## Prévisualiser un document

- `render_pdf_page` convertit une page PDF en image PNG pour contrôle visuel ; la largeur peut être réglée de **400 à 2400 px** (hauteur plafonnée).
- `render_svg` convertit un SVG en PNG, sans exécuter de scripts ni charger des ressources externes.
- `read_file` reste préférable pour un **PDF natif** dont le texte est directement extractible.

Le rendu des pages permet notamment de vérifier tableaux, marges, titres et pagination après `create_designed_pdf`.

Voir le [guide avancé](../AUTHORING.md).

[← Création](./Creation-de-documents.md) · [Dépannage →](./Depannage.md)
