# Suite bureautique intégrée — travail non publié

Cette branche ajoute cinq éditeurs locaux à MCP Win. Ils ne sont pas inclus dans le téléchargement stable v1.0.1.

| Entrée de la barre latérale | Éditeur | Format de travail principal |
| --- | --- | --- |
| Texte | Traitement de texte avec ruban et mise en page | DOCX |
| Tableur | Cellules, formules et feuilles | XLSX |
| Présentation | Diapositives et objets | PPTX |
| PDF | Pages, annotations, formulaires et outils PDF | PDF |
| Composition | Mise en page avec blocs, pages et calques | DesignCraft / IDML à l’import |

Un clic ouvre l’éditeur dans la partie droite de l’application. Les documents restent ouverts dans des onglets ; la barre latérale permet de revenir à chaque type d’éditeur. **Nouveau**, **Ouvrir**, **Enregistrer** et **Enregistrer sous** utilisent les dialogues de fichiers Windows. Jusqu’à douze documents peuvent être ouverts simultanément, avec une limite de 100 Mio par fichier.

Les moteurs Rust sont compilés en WebAssembly et embarqués dans Tauri. Aucun site web externe, compte bureautique ou Microsoft Office n’est nécessaire pour éditer les documents. Les échanges entre l’interface et les éditeurs restent locaux. L’OCR français/anglais utilise le Tesseract déjà fourni dans MCP Win, et non les modèles OCR d’ArtCraft. Enregistrez le PDF avant de lancer la reconnaissance ; le texte reconnu est présenté dans une fenêtre avec copie.

## Enregistrement

Les formats que le moteur sait seulement importer sont proposés dans un format éditable à l’enregistrement : XLS/XLSB/ODS vers XLSX, certaines présentations vers PPTX et les imports de composition vers le format de travail DesignCraft. Le dialogue demande où enregistrer la copie et conserve l’original.

Les fichiers sont écrits par le backend natif, avec les sauvegardes et la vérification d’empreinte existantes. Si un autre programme modifie le fichier après son ouverture, l’enregistrement refuse de l’écraser silencieusement. Un document ouvert depuis un dossier partagé conserve les droits de ce dossier : la lecture seule impose d’enregistrer une copie ailleurs.

La fermeture d’un document modifié propose de l’enregistrer, de conserver le document ouvert ou d’abandonner les changements. La fermeture de l’application et l’installation d’une mise à jour tiennent également compte des documents modifiés.

## Portée et limites

Ces éditeurs complètent les 42 outils MCP existants. Cette intégration n’ajoute pas d’outil distant donnant accès à toutes les commandes internes des moteurs. L’IA continue à utiliser les outils documentaires existants sur les dossiers partagés.

La fidélité d’import/export dépend du moteur et du document. L’intégration ne garantit pas la compatibilité intégrale avec Word, Excel, PowerPoint ou InDesign, les macros, les extensions propriétaires ou les services en ligne des applications d’origine. Les dialogues et certaines fonctions spécialisées des moteurs peuvent conserver des libellés anglais. Les cinq moteurs augmentent la taille du paquet et la mémoire consommée : fermer les onglets inutilisés sur les machines modestes.

## Compiler et vérifier

Installer Rust MSVC, Git, Node.js, les outils C++ Windows, CMake et WebView2. `scripts/build-office.ps1` récupère les cinq sources aux révisions de `tools/office-web/sources.json`, applique l’adaptation reproductible de `scripts/office-brand.cjs`, installe la cible WASM et le CLI wasm-bindgen épinglé, puis génère `ui/office/runtime`. Aucun exécutable bureautique externe n’est à installer.

```powershell
./scripts/build-office.ps1
cargo test --locked
node scripts/office-web-smoke.cjs
./scripts/package.ps1 -OutputDirectory dist-office
./scripts/run-v1-native-check.ps1 -PackageDirectory dist-office -ValidationScript office-native-smoke.cjs
```

Les tests d’interface nécessitent `npm ci --prefix tools/dev`. Le test natif emploie uniquement des fichiers synthétiques et restaure les paramètres préexistants. Il ne crée aucun tunnel public. Les sources récupérées, les artefacts WASM et les paquets compilés ne sont pas versionnés.

Les notices des moteurs et de leurs dépendances sont générées par `scripts/office-licenses.cjs` et incluses dans le paquet : [inventaire des licences](OFFICE-LICENSES.md). Les droits et notices des auteurs sont conservés ; l’interface adaptée utilise l’identité MCP Win.
