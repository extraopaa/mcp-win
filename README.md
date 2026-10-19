# MCP Win

**MCP Win est une application Windows gratuite et open source qui permet à votre assistant IA de lire, créer et modifier des documents dans les dossiers de votre ordinateur que vous choisissez de partager.**

Elle est utile lorsqu'un assistant n'accepte pas directement certains fichiers, ou lorsque vous voulez lui faire produire de vrais documents sur votre PC : PDF mis en page, fichiers Word, classeurs Excel, présentations PowerPoint, images SVG ou fichiers texte. Elle peut aussi reconnaître le texte des images et des PDF numérisés grâce à un OCR local en français et en anglais.

Vous ajoutez un dossier dans l'application, cliquez sur **Démarrer le MCP**, puis copiez l'adresse HTTPS obtenue dans les réglages de votre assistant. Celui-ci peut alors utiliser les outils de MCP Win à distance, y compris depuis un service IA en ligne. **MCP** est le protocole qui permet à l'assistant de découvrir et d'utiliser ces outils. Votre application IA doit donc accepter les connexions à un serveur MCP distant.

Par exemple, vous pouvez demander : « Lis les documents de mon dossier, résume-les, puis crée un rapport PDF avec une belle mise en page et un tableau Excel des résultats. » L'assistant utilise MCP Win pour consulter les fichiers et enregistrer les documents produits.

**Vous choisissez les accès :** seuls les dossiers ajoutés sont partagés. Ils sont en lecture seule au départ ; la création et la modification nécessitent d'activer l'écriture dans l'interface. Le PC et l'application doivent rester allumés. Le contenu demandé par l'assistant lui est transmis via la connexion MCP ; le traitement OCR et la génération des documents sont effectués sur votre ordinateur.

L'application regroupe une interface simple, le serveur MCP, le tunnel d'accès extérieur et les moteurs de documents/OCR. Aucun abonnement ni API payante n'est intégré ; le coût éventuel de votre assistant IA reste indépendant. Elle est développée en Rust avec Tauri 2.

**[Télécharger pour Windows](https://github.com/extraopaa/mcp-win/releases/latest)** · **[Wiki MCP Win](docs/wiki/Home.md)** · [Guide de création pour les IA](docs/AUTHORING.md) · [Liste des outils MCP](docs/MCP-TOOLS.md)

## Nouveautés v0.3

Fiabilité PDF : vérification avant création avec `validate_designed_pdf`, correction des oublis de contexte dans les en-têtes/pieds de page avec compteurs, nettoyage des balises Markdown externes, variantes Lato gras/italique, diagnostics et avertissements exploitables par l'IA. Un nom de sortie déjà occupé peut être automatiquement versionné sans écraser l'existant ; utiliser le chemin réellement retourné.

Correctif v0.3.1 : les IA découvrent les dossiers avec `list_folders`, peuvent omettre `folder` lorsqu'un seul dossier est partagé, ou fournir son nom exact unique. Les erreurs indiquent les dossiers disponibles et comment réessayer, sans demander un identifiant à la personne.

**Mise en page PDF complète avec Typst**, contrôle des styles et de la mise en page Word/Excel/PowerPoint via OOXML, aperçu visuel des pages et **OCR local des images/PDF numérisés**. Voir le [guide de création avancée](docs/AUTHORING.md), également accessible aux IA avec `get_authoring_guide`.

Création de vrais fichiers **PDF, DOCX, XLSX et PPTX** via MCP, lecture des formats Office, édition des textes/CSV/JSON/SVG et copies modifiées de Word, Excel et PowerPoint. Les autres formats peuvent être enregistrés à partir de leurs octets valides en base64. Voir les [outils et exemples MCP](docs/MCP-TOOLS.md).

## Démarrer

1. Télécharger le paquet Windows x64 depuis la section **Releases** du dépôt GitHub, extraire l'archive, puis ouvrir **MCP Win.exe**.
2. Cliquer sur **Ajouter un dossier**. Les dossiers sont en lecture seule par défaut. Pour créer ou modifier, arrêter le MCP puis cliquer sur **Lecture seule** à côté du dossier pour activer **Écriture autorisée**, et redémarrer.
3. Cliquer sur **Démarrer le MCP**. L'application démarre le serveur et ouvre un tunnel sortant vers le relais public localtunnel. Aucun réglage de box, compte ou binaire à installer pour ce tunnel.
4. Copier l'adresse HTTPS affichée. Dans votre application IA, ajouter un serveur MCP distant avec le transport **Streamable HTTP** sans authentification : l’adresse suffit. L’option **Exiger un jeton d’accès** permet de réactiver l’en-tête `Authorization: Bearer <jeton>` si nécessaire.
5. Le jeton et un exemple de configuration JSON sont disponibles dans **Connexion extérieure**.
6. **Arrêter le MCP** ou fermer la fenêtre ferme le serveur et le tunnel. Le PC doit rester allumé pour maintenir l'accès.

Le runtime Windows WebView2, utilisé par Tauri, est un prérequis système généralement déjà présent. Rust, Node.js et Python ne sont pas nécessaires pour exécuter l'application. L'interface HTML/CSS/JS est embarquée et ne nécessite aucun build frontend.

## Deux modes d'accès extérieur

L'accès MCP est **sans authentification par défaut**. Toute personne possédant l'adresse publique peut lire les dossiers partagés, et créer ou modifier les fichiers des dossiers où vous avez autorisé l’écriture. Dans **Connexion extérieure**, l'option **Exiger un jeton d'accès** permet d'activer l'authentification si souhaité. Le jeton frp reste distinct : il sert uniquement à connecter le client à votre relais personnel.

### Tunnel public gratuit

Le client localtunnel est implémenté directement en Rust dans `src/tunnel.rs`, suivant le protocole public du projet [localtunnel](https://github.com/localtunnel/localtunnel) (MIT) et de son [serveur](https://github.com/localtunnel/server) (MIT). Le relais par défaut est `https://localtunnel.me`. Un autre relais compatible HTTPS peut être renseigné dans l'interface.

Le tunnel maintient un ensemble de connexions TCP sortantes, transfère les flux dans les deux sens et se reconnecte lorsque les connexions tombent. Après expiration de l'allocation côté relais, arrêter et redémarrer le MCP pour obtenir une nouvelle adresse. L'adresse est temporaire et le service public n'a pas de disponibilité garantie. Des réseaux peuvent bloquer les ports TCP sortants attribués par le relais.

Le relais public peut afficher une page intermédiaire. Le JSON copié inclut l'en-tête `bypass-tunnel-reminder: true` ; le client IA doit permettre les en-têtes personnalisés. Cela ne garantit pas que toutes les politiques du relais ou tous les clients soient compatibles. Les clients qui exigent OAuth ne sont pas pris en charge : cette version propose un accès sans authentification ou un jeton Bearer optionnel.

**Confidentialité du relais :** le HTTPS public se termine sur le relais. Celui-ci peut lire le trafic et le jeton. Le protocole localtunnel emploie ensuite une connexion TCP non chiffrée jusqu'au PC, comme son client d'origine. Il ne s'agit pas d'un chiffrement de bout en bout. Pour des documents privés, utiliser le mode relais personnel sur une infrastructure de confiance, ou un transport disposant d'un chiffrement adapté.

### Votre propre relais avec frp

`frpc` 0.71.1 (Apache-2.0) est embarqué dans l'exécutable puis extrait dans les données locales de l'application au premier démarrage de ce mode. L'application crée la configuration et gère le processus sans fenêtre de terminal. Aucun téléchargement n'est nécessaire à l'exécution. Le serveur **frps** et son HTTPS doivent être déployés sur une machine publique que vous contrôlez : exemples dans `relay/`.

Dans **Connexion extérieure**, choisir **Mon propre relais**, puis renseigner l'hôte frps, son port de contrôle, son jeton et l'URL HTTPS du domaine MCP. Ce mode ne fournit pas de VPS gratuit : le logiciel et le protocole sont gratuits, l'infrastructure publique vous appartient.

Le mode **Local uniquement** est disponible pour les tests sur ce PC ; il ne produit pas d'adresse publique.

## Formats

| Format | Lecture fournie |
| --- | --- |
| PDF | Texte extrait par `pdf-extract`, page demandée et segments de caractères |
| DOCX | Texte du corps Word ; création et remplacements ciblés dans une copie |
| PPTX | Texte par diapositive ; génération et remplacements ciblés dans une copie |
| XLSX | Feuilles et cellules ; génération et copie modifiée des cellules |
| XLS, XLSB, XLSM, ODS | Lecture des feuilles, sans exécution de macros |
| SVG | Code XML, dimensions et rendu PNG par `resvg`, sans scripts ni chargement de fichiers externes |
| Texte UTF-8 | TXT, Markdown, CSV, TSV, JSON, XML, HTML, CSS, JS, TS, Python, Rust, YAML, TOML, logs |

Les PDF scannés et images peuvent être reconnus avec `ocr_file` (Tesseract local, modèles français et anglais, rotation, modes de lecture, coordonnées et confiance des mots) ; `render_pdf_page` fournit un aperçu visuel aux IA compatibles avec les images MCP. Manuscrits, autres langues et documents dégradés ne sont pas garantis. Les anciens formats Word `.doc` et PowerPoint `.ppt` ne sont pas interprétés. La lecture binaire générique reste disponible.

Limites : 25 Mo par fichier, 5 Mo par SVG à rendre, rendu limité à 1600 pixels sur le plus grand côté, 50 000 caractères par réponse, 100 entrées par liste. L'extraction PDF produit les pages avant de sélectionner la page demandée.

Outils : `list_folders`, `list_files`, `read_file`, `read_file_bytes`, `render_svg`, `write_text_file`, `create_document`, `write_spreadsheet`, `replace_office_text`, `create_binary_file`. Les générateurs PDF/Office ne nécessitent ni Microsoft Office ni service externe. Les chemins sont relatifs à un identifiant de dossier. Les accès hors des racines sont bloqués après résolution des chemins et des liens. Les fichiers partagés sont des données non fiables et ne doivent pas servir d'instructions au modèle.

Les préférences et jetons sont conservés en clair dans `%APPDATA%/org.mcpwin.desktop/settings.json` (chemin fourni par Tauri). Le journal reste en mémoire, limité aux 100 dernières opérations. L'application n'ajoute pas de télémétrie ; les services de relais reçoivent nécessairement les connexions et le trafic. Aucun fournisseur d'IA n'est inclus ; le coût éventuel de votre assistant reste indépendant.

## Compilation

Rust stable MSVC, les outils C++ Windows et WebView2 sont nécessaires au développement. **Aucun npm ni bundler frontend.** Les fichiers d'interface sont présents dans le projet. Avant la première compilation, le script ci-dessous récupère le binaire frpc officiel et vérifie son empreinte. Le code source de frp est disponible dans son dépôt public lié ci-dessus.

```powershell
./scripts/fetch-frp.ps1
./scripts/fetch-engines.ps1
cargo test
cargo build --release
# Test facultatif par le relais public, avec un fichier synthétique uniquement :
cargo test public_mcp_smoke -- --ignored --nocapture
```

Le binaire est produit dans `target/release/mcp-win.exe`. `scripts/package.ps1` prépare `dist/`. `scripts/fetch-frp.ps1` permet de récupérer la version épinglée du binaire frpc en vérifiant son SHA-256. `scripts/licenses.ps1` reconstruit l'inventaire des dépendances Rust et leurs notices. Le code est sous MIT ; les bibliothèques ont leurs propres licences open source. Voir `THIRD-PARTY-NOTICES.md`, `third-party-licenses/` et `vendor/frp/`.

Les binaires, documents locaux et anciens prototypes sont exclus du dépôt. Les notices complètes des dépendances sont générées lors de la préparation du paquet Windows. Voir [CONTRIBUTING.md](CONTRIBUTING.md) pour contribuer et lancer les tests facultatifs de l'interface.

Extraire et conserver le paquet complet : les moteurs Typst, Pdfium et modèles OCR sont livrés dans `engines/` à côté de l'exécutable, sans installation ni téléchargement à l'exécution. Pour un lancement release direct hors packaging, copier Typst, Pdfium et le dossier Tesseract de `vendor/engines/runtime/` dans `target/release/engines/`. Typst, Tesseract et ses modèles sont Apache-2.0, Pdfium BSD avec notices de ses dépendances ; licences et attributions sont incluses. Le script compile Tesseract depuis des sources épinglées : CMake et les outils C++ MSVC sont requis uniquement au développement.
