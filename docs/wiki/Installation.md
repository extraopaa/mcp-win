# Installation sur Windows

MCP Win est destiné à **Windows x64**. La distribution est portable : vous extrayez une archive et lancez l'application, sans installer Rust, Node.js, Python ni Microsoft Office.

## Version v0.4

Télécharger [la release v0.4.0 Windows x64](https://github.com/extraopaa/mcp-win/releases/tag/v0.4.0), extraire toute l’archive, puis ouvrir **MCP Win.exe** en conservant tous les fichiers et engines à côté. Rust, Node.js, Python et Office ne sont pas nécessaires sur le PC utilisateur.

Les anciennes archives v0.3.2 ne contiennent pas les nouvelles protections. Pour compiler depuis les sources, voir [Développement](Developpement.md).

L'application repose sur **Tauri 2** et nécessite le runtime système **Microsoft Edge WebView2**, habituellement déjà présent sur Windows. Si l'interface ne s'ouvre pas, vérifiez ce prérequis.

## Ce qui est inclus

- Le serveur MCP et l'interface Windows ;
- les clients cloudflared et `frpc` intégrés pour un relais personnel ;
- les moteurs de génération et rendu de documents ;
- Tesseract et les modèles OCR **français** et **anglais**.

Aucune API d'IA payante n'est requise par MCP Win. Votre assistant IA reste un service distinct, avec ses propres règles d'accès et de facturation.

## Premiers réglages utiles

Avant de démarrer un partage distant, consultez **Connexion extérieure** et utilisez le jeton Bearer obligatoire. Un dossier nouvellement ajouté est en **lecture seule** : il n'autorise pas la création de fichiers tant que vous n'avez pas explicitement activé l'écriture.

Pour compiler l'application depuis les sources, voir [Développement](./Developpement.md).

[← Accueil](./Home.md) · [Premiers pas →](./Premiers-pas.md)
