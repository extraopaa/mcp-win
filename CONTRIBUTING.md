# Contribuer à MCP Win

Les contributions sont les bienvenues : correction de bugs, nouveaux formats, accessibilité et tests.

## Environnement Windows

Installer Rust stable MSVC, Git, Node.js, CMake, les outils C++ Windows et le runtime WebView2. Depuis la racine du dépôt :

```powershell
./scripts/build-office.ps1
./scripts/fetch-frp.ps1
./scripts/fetch-engines.ps1
./scripts/fetch-cloudflared.ps1
./scripts/fetch-duckdb.ps1
cargo fmt --check
cargo test --locked
cargo run --locked
```

Les téléchargements frp, Cloudflare, DuckDB, Typst, Pdfium et modèles OCR sont épinglés et vérifiés par SHA-256. Les moteurs OCR/PDF sont requis pour les tests avancés. Le frontend principal reste en HTML/CSS/JS. Les cinq éditeurs Rust nécessitent la compilation WASM automatisée par `build-office.ps1` ; voir [le guide de la suite](docs/OFFICE.md).

Pour les tests facultatifs de l'interface native, installer Node.js puis `npm ci --prefix tools/dev`. Construire le paquet avec `./scripts/package.ps1`, puis lancer `./scripts/run-native-check.ps1`. Ces tests utilisent uniquement un document synthétique et restaurent les préférences précédentes. Ils ouvrent temporairement un tunnel public et nécessitent donc Internet. Les tests standard n'utilisent aucun relais public.

Pour une pull request, décrire le problème, le changement et les vérifications effectuées. Ne pas ajouter de jetons, documents personnels, exécutables compilés ou captures contenant des chemins locaux. Le projet et les nouvelles dépendances doivent rester open source, sans API payante obligatoire.

## Vérifications v0.4

`cargo test --no-default-features --locked` couvre les fonctions documentaires, archives, SQL, formulaires et accès HTTP. L’essai Internet Cloudflare est ignoré par défaut ; le lancer explicitement uniquement avec les fixtures synthétiques : `cargo test cloudflare::tests::public_tunnel_auth_and_read -- --ignored`.

`node scripts/security-ui-smoke.cjs` vérifie l’interface avec données simulées (authentification, presse-papiers des headers/Bearer, historique/restauration, rendu aux dimensions minimales). Aucun tunnel ni préférence personnelle modifiée.

Après les tests Rust et un build release : `./scripts/check-v04-workers.ps1` vérifie les vrais sous-processus de production sur la fixture AcroForm de target/qa-v04. Les sorties sont isolées dans un nouveau sous-dossier de test. `./scripts/package.ps1 -OutputDirectory dist-v04` prépare le paquet local complet, sans publication.

Les exécutables et moteurs sont distribués séparément des sources ; ne pas les ajouter aux commits.

## Versions et mises à jour signées

Les versions majeures suivent la [convention de nommage](docs/RELEASES.md). Chaque publication met à jour le [changelog](CHANGELOG.md). La procédure de signature est décrite dans [UPDATES.md](docs/UPDATES.md) ; aucune clé privée ne doit entrer dans le dépôt. Les vérifications locales de l’updater utilisent `./scripts/check-updater.ps1` avec une fixture synthétique et la clé locale du mainteneur.
