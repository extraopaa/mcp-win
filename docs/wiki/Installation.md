# Installation sur Windows

MCP Win est destiné à **Windows x64**. La distribution est portable : vous extrayez une archive et lancez l'application, sans installer Rust, Node.js, Python ni Microsoft Office.

## Installer la version publiée

1. Ouvrez [la dernière version sur GitHub](https://github.com/extraopaa/mcp-win/releases/latest).
2. Téléchargez le paquet Windows x64 proposé, puis **extrayez toute l'archive** dans un dossier de votre choix.
3. Conservez les fichiers et le dossier `engines/` à côté de l'exécutable. Les moteurs PDF, Typst et OCR sont fournis avec le paquet.
4. Lancez **MCP Win.exe**.
5. Dans **Vue d'ensemble**, utilisez **Ajouter un dossier** puis **Démarrer le MCP**.

L'application repose sur **Tauri 2** et nécessite le runtime système **Microsoft Edge WebView2**, habituellement déjà présent sur Windows. Si l'interface ne s'ouvre pas, vérifiez ce prérequis.

## Ce qui est inclus

- Le serveur MCP et l'interface Windows ;
- le tunnel public écrit en Rust et le client `frpc` intégré pour un relais personnel ;
- les moteurs de génération et rendu de documents ;
- Tesseract et les modèles OCR **français** et **anglais**.

Aucune API d'IA payante n'est requise par MCP Win. Votre assistant IA reste un service distinct, avec ses propres règles d'accès et de facturation.

## Premiers réglages utiles

Avant de démarrer un partage distant, consultez **Connexion extérieure** et activez de préférence **Exiger un jeton d'accès**. Un dossier nouvellement ajouté est en **lecture seule** : il n'autorise pas la création de fichiers tant que vous n'avez pas explicitement activé l'écriture.

Pour compiler l'application depuis les sources, voir [Développement](./Developpement.md).

[← Accueil](./Home.md) · [Premiers pas →](./Premiers-pas.md)
