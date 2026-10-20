# Mises à jour

MCP Win recherche une version stable au démarrage, puis toutes les six heures pendant son exécution. **Mises à jour** dans la barre latérale permet une recherche immédiate. La vérification contacte GitHub ; aucun fichier partagé ni jeton MCP n’est transmis.

Lorsqu’une version est disponible, arrêter le serveur puis cliquer sur **Installer et redémarrer**. Le paquet complet est téléchargé, vérifié et installé dans le dossier de l’application. Le dossier doit être accessible en écriture. Les préférences, dossiers partagés, profils, index et historiques sont conservés. Le serveur reste arrêté après redémarrage.

## Vérification

- HTTPS, domaines de redirection limités à GitHub et ses serveurs d’assets.
- Manifeste signé Ed25519, clé publique intégrée dans l’exécutable ; aucune option pour ignorer une signature invalide.
- Version stable strictement supérieure, plateforme Windows x64, URL du dépôt fixée.
- Taille et SHA-256 de l’archive revérifiés par le processus d’installation après téléchargement.
- Refus des chemins absolus, traversées, liens, jonctions, doublons et noms Windows réservés.
- Remplacement limité aux fichiers du paquet applicatif. Extraction préalable et restauration si une opération de remplacement échoue.

Les fichiers retirés d’une nouvelle release ne sont pas supprimés automatiquement. En cas de coupure brutale pendant l’installation, conserver le dossier `.mcp-update-*` : son sous-dossier `backup` et son journal permettent une récupération manuelle. Il ne s’agit pas d’un système de fichiers transactionnel résistant à toute coupure de courant.

Le mécanisme ne passe pas par un outil MCP distant. Il ne demande pas d’élévation administrateur et ne lance pas de script téléchargé. La signature protège les mises à jour ; elle ne constitue pas une signature Authenticode Windows.

## Publier une version compatible

La dernière release GitHub stable doit contenir trois assets :

1. `mcp-win-vVERSION-windows-x64.zip` : paquet portable complet.
2. `update.json` : version, cible, URL exacte, taille et SHA-256 du ZIP.
3. `update.json.sig` : signature Ed25519 du contenu exact de `update.json`, encodée en base64.

Les releases antérieures sans manifeste signé ne sont pas installées automatiquement. La première version contenant l’updater doit être installée manuellement. Tant qu’aucun manifeste n’est publié, aucune mise à jour n’est proposée.

Après avoir changé les versions Cargo/Tauri et compilé le paquet :

```powershell
./scripts/package.ps1
./scripts/prepare-update.ps1 -SourceDirectory dist
```

Le script crée les assets dans `release-assets/` et ne publie rien. Joindre ces trois fichiers à la release stable dont le tag vaut `vVERSION`. Publier le manifeste et sa signature avec le paquet correspondant. Les préversions sont exclues.

## Clé de signature

La clé privée du mainteneur est conservée hors du dépôt et du paquet, dans `%USERPROFILE%/.mcp-win-release/updater.key`. La clé publique seule est versionnée dans `assets/updater-public-key.txt`. **Sauvegarder la clé privée dans un emplacement sûr** : sa perte empêche de signer de futures mises à jour pour les versions déjà distribuées. Ne jamais la joindre à une release, un rapport, un commit ou une archive.

Pour un fork, créer sa propre paire de clés, adapter l’URL du dépôt dans le code et reconstruire l’application :

```powershell
cargo run --example sign-update -- generate CHEMIN_PRIVE assets/updater-public-key.txt
```

Le générateur refuse d’écraser une clé existante. Le signataire vérifie que la clé privée correspond à la clé publique du dépôt. La clé locale est un secret de publication ; les utilisateurs finaux n’en ont pas besoin.
