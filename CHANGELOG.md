# Changelog

Les versions stables et leurs changements sont consignés ici. Les préversions et les travaux non publiés ne constituent pas des versions finales.

## À venir

- Compilation OCR : conservation de l’argument CMake `CMAKE_POLICY_VERSION_MINIMUM=3.5` sous PowerShell 7, pour les constructions GitHub Actions.

## [1.0.0 — Orbite](https://github.com/extraopaa/mcp-win/releases/tag/v1.0.0) · 20 octobre 2026

### Ajouts

- **Studio documentaire** : profils visuels, logo, couleurs, police, organisation, coordonnées et pied de page.
- **Documents professionnels par défaut** : PDF, Word et PowerPoint avec titres, tableaux, encadrés, images et pagination ; présentation des nouveaux classeurs Excel.
- **Index persistant SQLite FTS5** et cache d’extraction sur disque ; recherche par extraits avec chemin, page ou feuille et empreinte vérifiée.
- **Citations vérifiées** lors de la génération des documents.
- **Contrôle qualité** avant enregistrement et outil de revue avec aperçus des pages PDF.
- **Édition par éléments** : blocs et cellules des documents générés, paragraphes et cellules Office, lignes de texte.
- **Révisions locales** : comparaison, aperçu PDF avant/après, application ou refus dans l’interface, sauvegarde et annulation.
- **Workflows durables** : étapes avec références aux résultats, progression, pause, reprise et annulation groupée des livrables.
- **OCR structuré** : blocs, titres estimés, champs et tableaux avec confiance, export CSV/XLSX.
- **Mises à jour intégrées** : recherche automatique, téléchargement, vérification Ed25519/SHA-256, remplacement du paquet portable et redémarrage.
- Dix nouveaux outils MCP, soit **42 outils** au total.

### Interface

- Nouvelle interface avec six vues, navigation latérale et Studio documentaire.
- Nouveau logo dans l’application et les icônes Windows.
- Liens vers GitHub et la documentation ouverts dans le navigateur système.
- Suppression des slogans, des badges décoratifs et des textes répétitifs.

### Fiabilité et sécurité

- Refus des références périmées et des modifications lorsque la source a changé.
- Conservation des styles OOXML pour les remplacements ciblés et des types de cellules Excel.
- Journaux de reprise pour les workflows et les révisions interrompues.
- Profils visuels et empreintes des images conservés avec le modèle de composition d’un document.
- Stockage privé exclu des dossiers accessibles par MCP.
- Updater limité aux releases stables signées du dépôt, avec validation des chemins du ZIP et restauration en cas d’échec de remplacement.
- Clé privée de publication conservée hors du dépôt et du paquet.

### Compatibilité

- Windows x64 avec WebView2 ; distribution portable comprenant les moteurs.
- Les utilisateurs de v0.x doivent installer une première fois le paquet v1 complet pour bénéficier de l’updater.
- Les réglages locaux sont conservés. Les connexions exigent toujours un jeton Bearer.
- L’OCR de tableaux reste heuristique ; la qualité Office est contrôlée structurellement. Voir [les limites](docs/V1.md).

## [0.4.0](https://github.com/extraopaa/mcp-win/releases/tag/v0.4.0) · 19 octobre 2026

### Ajouts

- Recherche récursive avec regex, filtres glob, pagination et extraits.
- Plans des documents, lectures ciblées et aperçu compact des tableurs.
- Fusion/extraction de pages PDF et inspection/remplissage des formulaires AcroForm.
- Inspection, extraction sélective et création ZIP/TAR/7z.
- Interrogation SQLite et DuckDB en lecture seule.
- Modèles Typst et Office, patchs ciblés et historique de restauration local.

### Sécurité

- Suppression du tunnel localtunnel non chiffré ; intégration de Cloudflare Quick Tunnel.
- Authentification Bearer obligatoire, y compris en mode local ; migration des anciennes préférences et rotation des anciens jetons.
- Vérification TLS pour frp, contrôles d’accès HTTP, bornes de ressources et isolation de certaines opérations dans des sous-processus.
- Copie des headers JSON et de la valeur Bearer depuis l’application.

## [0.3.2 — Création PDF plus fiable](https://github.com/extraopaa/mcp-win/releases/tag/v0.3.2) · 19 octobre 2026

- Prévalidation Typst sans écrire dans un dossier partagé.
- Diagnostics, avertissements de polices et corrections des erreurs de contexte de pagination les plus courantes.
- Gestion des collisions de noms par création d’une nouvelle copie.

## [0.3.1 — Découverte des dossiers](https://github.com/extraopaa/mcp-win/releases/tag/v0.3.1) · 19 octobre 2026

- Sélection automatique lorsqu’un seul dossier est partagé.
- Découverte des dossiers récupérable et erreurs d’ambiguïté explicites, sans contourner les droits d’écriture.

## [0.3.0 — Mise en page avancée et OCR](https://github.com/extraopaa/mcp-win/releases/tag/v0.3.0) · 19 octobre 2026

- Composition PDF avec Typst et rendu avec Pdfium.
- OCR local Tesseract français/anglais.
- Inspection et écriture des paquets Office OOXML.

## [0.2.0 — PDF et Office](https://github.com/extraopaa/mcp-win/releases/tag/v0.2.0) · 19 octobre 2026

- Génération PDF, DOCX, XLSX et PPTX.
- Modification des fichiers texte et Office avec permissions et empreinte de version.

## [0.1.0 — Windows x64](https://github.com/extraopaa/mcp-win/releases/tag/v0.1.0) · 19 octobre 2026

- Première application native Rust/Tauri et serveur MCP HTTP.
- Partage de dossiers, lecture de documents et aperçu SVG.

---

[Politique de version et noms des versions majeures](docs/RELEASES.md) · [Toutes les releases](https://github.com/extraopaa/mcp-win/releases)
