# Premiers pas

Ce tutoriel suppose que vous avez lancé [MCP Win sur Windows](./Installation.md).

## 1. Ajouter un dossier

Dans **Vue d'ensemble**, cliquez sur **Ajouter un dossier** et choisissez un répertoire contenant des documents que vous acceptez de partager avec votre assistant.

- **Lecture seule** est activé au départ : l'assistant peut consulter les fichiers, mais pas les créer ou les modifier.
- Pour permettre une production de documents, **arrêtez d'abord le MCP**, puis cliquez sur le bouton **Lecture seule** du dossier pour passer à **Écriture autorisée**.
- Redémarrez ensuite le MCP. Les droits d'écriture ne peuvent pas être accordés à distance par un outil MCP.

Privilégiez un dossier de travail dédié, sans secrets ni documents personnels inutiles.

## 2. Choisir le mode de connexion

Dans **Connexion extérieure**, le mode par défaut est **Tunnel public**. Il offre une URL HTTPS temporaire. Vous pouvez aussi sélectionner **Mon propre relais** ou **Local uniquement**.

**Conseil :** cochez **Exiger un jeton d'accès** avant d'exposer des dossiers sur Internet. Conservez le jeton secret ; l'URL seule ne protège pas un serveur configuré sans authentification.

## 3. Démarrer le serveur

Cliquez sur **Démarrer le MCP**. Attendez que l'application affiche l'adresse de connexion. Cliquez sur **Copier l'adresse** ; l'URL de l'outil MCP comprend le chemin `/mcp`.

Le serveur et le tunnel s'arrêtent lorsque vous cliquez sur **Arrêter le MCP** ou lorsque vous fermez l'application. Le PC doit rester allumé pour accepter les appels distants.

## 4. Ajouter le serveur à votre assistant IA

Votre application IA doit prendre en charge les **serveurs MCP distants** avec transport **Streamable HTTP**. Ajoutez l'URL copiée et, si activé, le jeton Bearer. Les réglages varient d'un client à l'autre ; voir [Connexion à une IA](./Connexion-IA.md).

## 5. Tester

Demandez à l'assistant :

> « Utilise MCP Win. Appelle d'abord `list_folders`, puis `list_files` pour me montrer les fichiers disponibles. »

Pour écrire, autorisez l'écriture dans un dossier, puis demandez :

> « Crée un fichier `notes.md` avec un bref résumé des documents. Utilise le dossier inscriptible retourné par `list_folders`. »

Si vous possédez plusieurs dossiers, laissez l'assistant lire les identifiants via `list_folders`. Ne cherchez pas à fournir vous-même ces identifiants.

[← Installation](./Installation.md) · [Connexion à une IA →](./Connexion-IA.md)
