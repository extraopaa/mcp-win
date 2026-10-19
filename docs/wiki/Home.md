# Wiki MCP Win

**MCP Win** est une application Windows libre et gratuite permettant à un assistant IA compatible **Model Context Protocol (MCP)** de lire des fichiers et de créer des documents dans les dossiers que vous autorisez sur votre ordinateur.

Ce wiki documente la **version 0.3.2** du dépôt. Pour les changements récents, consultez également le [README principal](../../README.md) et les [versions publiées](https://github.com/extraopaa/mcp-win/releases).

> **À savoir avant de commencer :** une connexion extérieure est **sans authentification par défaut**. Toute personne disposant de l'URL peut accéder aux dossiers partagés selon leurs permissions. Pour une utilisation distante, activez de préférence **Exiger un jeton d'accès**, et ne partagez que les dossiers nécessaires.

## Démarrage en trois étapes

1. [Installer MCP Win](./Installation.md) sur Windows et ouvrir l'application.
2. [Partager un dossier et démarrer le serveur](./Premiers-pas.md). Les dossiers sont initialement en lecture seule.
3. [Connecter un assistant IA](./Connexion-IA.md) avec l'adresse MCP fournie par l'application.

**Exemple de demande à l'assistant :** « Liste les dossiers disponibles, lis les documents du dossier partagé, puis prépare un rapport PDF et un classeur récapitulatif. »

## Parcours de lecture

| Vous souhaitez… | Consultez |
| --- | --- |
| Découvrir l'interface et le premier échange | [Premiers pas](./Premiers-pas.md) |
| Brancher une application IA | [Connexion à une IA](./Connexion-IA.md) |
| Choisir ce qu'une IA peut lire ou écrire | [Dossiers et permissions](./Dossiers-et-permissions.md) |
| Découvrir les commandes disponibles | [Outils MCP et formats](./Outils-MCP-et-formats.md) |
| Produire un PDF, Word, Excel ou PowerPoint | [Création de documents](./Creation-de-documents.md) |
| Extraire le texte de scans ou prévisualiser | [OCR et aperçus](./OCR-et-apercus.md) |
| Choisir un tunnel ou un relais personnel | [Modes de connexion](./Modes-de-connexion.md) |
| Comprendre les risques de partage | [Sécurité et confidentialité](./Securite-et-confidentialite.md) |
| Corriger un dysfonctionnement | [Dépannage](./Depannage.md) |
| Compiler ou contribuer au projet | [Développement](./Developpement.md) |
| Trouver une réponse rapide | [FAQ](./FAQ.md) |

## Comment ça fonctionne

```text
Assistant IA compatible MCP
          │
          │ Streamable HTTP (éventuellement via Internet)
          ▼
  Tunnel public / relais personnel / accès local
          │
          ▼
  Serveur MCP Win sur votre PC Windows
          │
          ├─ Dossiers explicitement partagés
          ├─ Lecture et édition contrôlées de fichiers
          ├─ Génération PDF / DOCX / XLSX / PPTX
          └─ Aperçu PDF / SVG et OCR français-anglais
```

Le programme traite localement la génération et l'OCR. Lorsqu'un assistant appelle un outil de lecture, **le contenu demandé est transmis à cet assistant**. Le PC et MCP Win doivent rester allumés pour qu'une connexion distante fonctionne.

**Documentation technique d'origine :** [guide de création](../AUTHORING.md) · [référence des outils](../MCP-TOOLS.md) · [contribution](../../CONTRIBUTING.md).

---

[Installer →](./Installation.md) · [Sommaire](./_Sidebar.md)
