# Développement, architecture et contribution

Le projet MCP Win est écrit en **Rust** avec **Tauri 2**. L'interface est en HTML/CSS/JavaScript embarqués ; **aucun build frontend npm n'est requis** pour compiler l'application.

## Organisation du dépôt

| Chemin | Responsabilité principale |
| --- | --- |
| `src/main.rs` | Point d'entrée Tauri et commandes de l'interface |
| `src/desktop.rs` | Préférences, dossiers partagés, démarrage/arrêt du serveur |
| `src/server.rs` | Serveur MCP Streamable HTTP et déclaration des outils |
| `src/library.rs` | Accès aux fichiers et contrôles de chemins |
| `src/cloudflare.rs` | Cloudflare Quick Tunnel chiffré |
| `src/generate.rs`, `src/writing.rs` | Génération et écriture de documents |
| `src/pdf_design.rs` | Création/validation de PDF Typst |
| `src/office.rs` | Opérations avancées Office/OOXML |
| `src/ocr.rs` | Reconnaissance de caractères |
| `ui/` | Interface Windows embarquée |
| `relay/` | Exemples pour un relais personnel frps/Caddy |
| `scripts/` | Récupération vérifiée, packaging, validation |
| `docs/` | Documentation technique et exemples |

## Compiler sous Windows

Prérequis de développement : **Rust stable MSVC**, outils **C++ Windows**, **CMake** et **WebView2**. Depuis une console PowerShell à la racine :

```powershell
./scripts/fetch-frp.ps1
./scripts/fetch-engines.ps1
./scripts/fetch-cloudflared.ps1
./scripts/fetch-duckdb.ps1
cargo fmt --check
cargo test --locked
cargo run --locked
cargo build --release
```

Le binaire Rust release apparaît dans `target/release/mcp-win.exe`. Pour le paquet portable complet :

```powershell
./scripts/package.ps1
```

Le script prépare le répertoire `dist/`. En dehors du packaging, les moteurs d'exécution doivent être disposés à côté du binaire dans `engines/`. Les scripts vérifient les téléchargements épinglés (SHA-256).

## Tests et intégration continue

Le workflow [`.github/workflows/windows.yml`](../../.github/workflows/windows.yml) utilise un runner Windows, vérifie le formatage Rust, récupère les moteurs, exécute `cargo test --locked`, prépare le paquet et publie un artefact de CI.

Les tests natifs facultatifs nécessitent Node.js : `npm ci --prefix tools/dev` puis les scripts de contrôle indiqués dans [CONTRIBUTING.md](../../CONTRIBUTING.md). Certains tests réseau ouvrent un tunnel public temporaire et doivent utiliser uniquement des documents synthétiques.

## Contribuer

Consultez [CONTRIBUTING.md](../../CONTRIBUTING.md), les [issues](https://github.com/extraopaa/mcp-win/issues) et les [pull requests](https://github.com/extraopaa/mcp-win/pulls). Une contribution doit décrire le problème, les changements et les vérifications effectuées. N'ajoutez jamais de jetons, données personnelles ou binaires compilés au dépôt.

Le code est distribué sous **licence MIT** ; les moteurs et bibliothèques tiers sont soumis à leurs propres licences, consignées notamment dans [THIRD-PARTY-NOTICES.md](../../THIRD-PARTY-NOTICES.md).

[← Accueil](./Home.md) · [FAQ →](./FAQ.md)
