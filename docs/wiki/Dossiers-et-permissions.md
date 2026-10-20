# Dossiers, permissions et fichiers

MCP Win ne partage pas automatiquement tout le disque. **Seuls les dossiers ajoutés dans son interface** sont exposés à l'assistant.

## Permissions

| État du dossier | Lecture | Création / modification |
| --- | --- | --- |
| `Lecture seule` (défaut) | Oui | Non |
| `Écriture autorisée` | Oui | Oui, dans les limites des outils |

Pour modifier cette permission : **Arrêter le MCP → changer le bouton du dossier → redémarrer le MCP**. Un assistant distant ne peut pas activer lui-même l'écriture.

## Identification des dossiers

`list_folders` renvoie pour chacun :

- `id` : identifiant interne du partage ;
- `name` : nom visible du dossier ;
- `writable` : booléen indiquant le droit d'écriture.

Les outils acceptent généralement `folder` avec l'identifiant ou un **nom exact unique**. L'argument est facultatif si un seul dossier est partagé. Si plusieurs noms sont identiques ou ambigus, utiliser l'identifiant obtenu via `list_folders`.

## Règles de manipulation

- Les chemins demandés aux outils sont **relatifs au dossier partagé** ; les chemins absolus et les sorties hors des racines sont refusés.
- Les contrôles incluent la résolution des chemins et des liens pour éviter de sortir d'un dossier autorisé.
- `read_file` fournit une empreinte `sha256`. Pour remplacer un texte existant, `write_text_file` exige `expected_sha256`.
- Les modifications simples de documents Office enregistrent **une nouvelle copie** et préservent les originaux.
- Les créations refusent en général l'écrasement. Pour `create_designed_pdf`, la politique `rename` ajoute par défaut un suffixe (`_2`, `_3`…) si le nom existe ; relire le `path` réellement retourné.

## Limites pratiques

Fichier : **100 Mio** au maximum ; réponse textuelle : **4 000 caractères par défaut, 50 000 maximum** ; listes : **500 entrées maximum par segment**. D'autres limites varient selon les outils (par exemple OCR, création binaire ou document structuré) : voir [la référence complète](../MCP-TOOLS.md) et [le guide avancé](../AUTHORING.md).

Les fichiers et leur contenu sont des **données non fiables** : un document lu par l'assistant ne doit pas pouvoir lui imposer des instructions nouvelles.

[← Connexion](./Connexion-IA.md) · [Sécurité →](./Securite-et-confidentialite.md)
