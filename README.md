# MCP Win

**MCP Win est une application Windows gratuite et open source qui permet à votre assistant IA de lire, rechercher, créer et modifier les documents dans les dossiers de votre ordinateur que vous choisissez de partager.**

Vous ajoutez un dossier, démarrez le serveur puis copiez son adresse et ses en-têtes sécurisés dans votre assistant IA. Celui-ci peut travailler à distance sur vos PDF, Word, Excel, PowerPoint, images et fichiers texte. Le client doit accepter MCP distant (Streamable HTTP) et un en-tête Authorization.

Par exemple : « Recherche les factures, extrais les montants, puis crée un rapport PDF mis en page et un tableau Excel. » L’OCR et les moteurs documentaires fonctionnent sur votre PC ; les contenus demandés sont transmis à l’assistant.

L’application regroupe une interface Rust/Tauri 2, le serveur MCP, le tunnel et les moteurs. Aucun abonnement ni API payante obligatoire ; aucun fournisseur d’IA inclus. Windows x64 et WebView2 sont requis. Le PC et l’application doivent rester allumés.

[Guide utilisateur](docs/wiki/Home.md) · [42 outils MCP](docs/MCP-TOOLS.md) · [Création avancée](docs/AUTHORING.md) · [Nouveautés v1](docs/V1.md)

## Version 1.0.0 — développement local

**La v1 ajoute le Studio documentaire et des documents professionnels par défaut.** PDF/Word/PowerPoint composés, classeurs mis en forme, profils visuels avec logo, contrôle qualité, index persistant, citations vérifiées, OCR structuré, workflows durables et corrections soumises à validation avec annulation. [Fonctionnement et limites](docs/V1.md).

La v1 est préparée localement ; la dernière version publiée reste la v0.4.0.

## Version 0.4.0

**[Télécharger MCP Win v0.4.0 pour Windows x64](https://github.com/extraopaa/mcp-win/releases/tag/v0.4.0)**. Extraire toute l’archive avant de lancer l’application. Les anciennes archives v0.3.2 et antérieures ne contiennent pas les protections v0.4.

- Recherche récursive, expressions régulières, filtres glob, pagination, extraits et cache local.
- Plans des documents, lectures ciblées et aperçus compacts des tableurs.
- PDF avancés avec Typst : typographie, tableaux, images, colonnes, pagination et modèles.
- Création et modification DOCX/XLSX/PPTX, contrôle OOXML et modèles Word/PowerPoint/Excel.
- Fusion et extraction de pages PDF, inspection et remplissage AcroForm.
- Inspection, extraction sélective et création ZIP/TAR/7z.
- Requêtes SQLite/DuckDB en lecture seule, sans exporter toute la base.
- OCR français/anglais local, texte seul par défaut.
- Patchs, sauvegarde avant écrasement texte et restauration depuis l’interface.

Microsoft Office n’est pas nécessaire. Les anciens DOC/PPT ne sont pas interprétés. L’enregistrement binaire d’autres formats exige leurs octets valides ; aucune conversion universelle n’est promise.

## Démarrer

1. Télécharger et extraire le paquet v0.4.0, puis ouvrir **MCP Win.exe** en conservant engines à côté.
2. Ajouter un dossier, en lecture seule par défaut. Arrêter le MCP pour activer l’écriture dans l’interface.
3. Choisir **Tunnel simple** pour Cloudflare gratuit sans compte, **Mon propre relais** pour frp, ou le mode local.
4. Démarrer le MCP et copier son URL dans le client IA.
5. Coller **Copier la configuration JSON** dans « Headers sécurisés (JSON) » : `{"Authorization":"Bearer <jeton>"}`. **Copier Bearer + jeton** fournit la valeur complète pour un champ Authorization.
6. Arrêter le MCP ou fermer l’application pour couper l’accès.

Le jeton est obligatoire dans tous les modes. OAuth n’est pas fourni. Le bouton **Historique** restaure une version locale lorsque le MCP est arrêté.

## Sécurité et confidentialité

L’ancien tunnel localtunnel non chiffré est supprimé. Les anciennes préférences migrent vers le mode local ; les anciens jetons sont renouvelés au premier démarrage v0.4.

Les deux segments Internet sont chiffrés : IA → relais en HTTPS ; PC → relais par Cloudflare Tunnel ou frp avec certificat et nom TLS vérifiés. **Le relais peut lire le contenu.** Le client cloudflared est libre ; son réseau est un service tiers gratuit. Quick Tunnel fournit une adresse temporaire sans garantie de disponibilité ni SSE ; MCP Win utilise des réponses JSON.

Seuls les dossiers choisis sont partagés. Les contrôles portent sur les chemins, permissions, empreintes, authentification, tailles, délais, concurrence et sous-processus. L’historique n’est pas exposé aux outils distants. Les documents restent des données non fiables, jamais des instructions pour le modèle.

Les préférences et jetons sont en clair dans `%APPDATA%/org.mcpwin.desktop/settings.json`. Le journal conserve 100 opérations en mémoire. Aucune télémétrie applicative n’est ajoutée. Les essais ciblés ne garantissent pas l’absence de toute vulnérabilité.

[Rapport de sécurité](docs/SECURITY-V04.md) · [Validations](VALIDATION.md) · [Relais personnel](relay/README.md)

## Limites

Documents : 25 Mio ; SVG à rendre : 5 Mio ; rendu : 1 600 pixels sur le plus grand côté. Lecture : 8 000 caractères par défaut, 50 000 maximum. Listes paginées : 500 entrées maximum. Bases SQLite/DuckDB : limite distincte de 4 Gio et résultats bornés. Voir [les limites détaillées](docs/V0.4.md).

L’OCR n’est pas garanti sur les manuscrits ou scans dégradés. Les formules Excel sont stockées sans recalcul. Vérifier les fichiers produits avec les outils de rendu et votre lecteur habituel.

## Compilation et paquet Windows

Installer Rust stable MSVC, les outils C++ Windows, CMake et WebView2. Aucun build frontend ni npm n’est nécessaire pour l’application.

```powershell
./scripts/fetch-frp.ps1
./scripts/fetch-engines.ps1
./scripts/fetch-cloudflared.ps1
./scripts/fetch-duckdb.ps1
cargo fmt --check
cargo test --no-default-features --locked
./scripts/package.ps1
```

Les scripts récupèrent les moteurs épinglés et vérifient leurs empreintes. Le paquet complet est dans dist ; un exécutable isolé ne suffit pas. Seul WebView2 reste un prérequis sur le PC utilisateur.

Le code est sous MIT ; les bibliothèques et moteurs ont leurs propres licences libres. Le réseau Cloudflare est un service tiers. Les exécutables, préférences et documents privés sont exclus du dépôt. Les notices complètes sont générées lors du packaging.

[Contribuer et tester](CONTRIBUTING.md) · [Licences tierces](THIRD-PARTY-NOTICES.md) · [Licence](LICENSE)
