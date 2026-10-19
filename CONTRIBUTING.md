# Contribuer à MCP Win

Les contributions sont les bienvenues : correction de bugs, nouveaux formats, accessibilité et tests.

## Environnement Windows

Installer Rust stable MSVC, les outils C++ Windows et le runtime WebView2. Depuis la racine du dépôt :

```powershell
./scripts/fetch-frp.ps1
cargo fmt --check
cargo test --locked
cargo run --locked
```

Le téléchargement frp est épinglé en version et vérifié par SHA-256. Le frontend HTML/CSS/JS ne nécessite aucun build ni dépendance npm.

Pour les tests facultatifs de l'interface native, installer Node.js puis `npm ci --prefix tools/dev`. Construire le paquet avec `./scripts/package.ps1`, puis lancer `./scripts/run-native-check.ps1`. Ces tests utilisent uniquement un document synthétique et restaurent les préférences précédentes. Ils ouvrent temporairement un tunnel public et nécessitent donc Internet. Les tests standard n'utilisent aucun relais public.

Pour une pull request, décrire le problème, le changement et les vérifications effectuées. Ne pas ajouter de jetons, documents personnels, exécutables compilés ou captures contenant des chemins locaux. Le projet et les nouvelles dépendances doivent rester open source, sans API payante obligatoire.
