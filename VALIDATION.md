# Validation locale v1.0.1 — 21 octobre 2026

- Rust : 45 tests réussis sur 46 ; le test Internet Cloudflare reste ignoré. Les tests incluent les accès HTTP hostiles, OAuth/PKCE, consentement local, rotation et rejeu des jetons, révocation, fichiers de plus de 25 Mio et logos de plus de 2 Mio.
- Personnalisation : géométrie Letter du PDF, dimensions/marges Word et taille de police OOXML vérifiées.
- OCR : cache et reconnaissance après rotation vérifiés sur des images synthétiques ; confiance et relecture signalées.
- Interface : six vues, états OAuth/jeton, progression de mise à jour, presse-papiers, permissions, CSP, absence de débordement à 900/1180/1440 pixels.
- Tauri release : 42 outils, création PDF professionnelle, aperçu, index/recherche, révision et annulation, workflows, import d’un grand logo ; parcours OAuth réel HTTP avec validation dans l’interface et révocation immédiate vérifié.
- Updater : installation native synthétique signée, vérification du paquet, remplacement portable, conservation des préférences et redémarrage réussis.
- PDF professionnel et écran OAuth inspectés visuellement. Aucune validation avec un assistant tiers réel ; aucune publication v1.0.1 effectuée.

[Changements et limites 1.0.1](docs/V1.0.1.md).

---
# Validation locale v1.0.0 — 20 octobre 2026

- 42 outils MCP, dont 10 nouveaux outils du Studio documentaire.
- Suite Rust : 40 tests réussis, 0 échec ; 1 test Internet Cloudflare séparé non relancé pour la v1.
- Interface : six vues sous CSP strict, profils/logo, index, workflows, comparaison et actions de révision ; largeurs 900/1180/1440 vérifiées.
- Tauri natif release et vrai MCP HTTP local authentifié : création professionnelle, rapport qualité et aperçu PDF, indexation/recherche, inspection des éléments, proposition refusée à l’application tant que le serveur tourne, validation/annulation locale, workflow avec référence de résultat et annulation groupée.
- Cas vérifiés : index périmé exclu, stockage privé non partagé, styles OOXML conservés, types numériques XLSX conservés, chaînes commençant par = restant du texte, conflit de source, sauvegarde/restauration, reprise de workflow sans rejeu, OCR tabulaire avec confiance.
- PDF professionnel rendu en PNG et inspecté visuellement : titres, accents, tableau pleine largeur, encadré et pagination. Office : contrôle structurel et relecture ; pas de validation visuelle dans Microsoft Office ou LibreOffice.
- Updater : signature falsifiée, mauvaise clé, mauvaise origine, préversion, downgrade, traversée ZIP, noms réservés et doublons refusés. Échec d’installation avec restauration testé.
- Installation native synthétique : vrai processus helper, manifeste signé par la clé locale, SHA-256, remplacement du paquet portable, conservation d’un fichier de préférences et relance du nouvel exécutable validés, sans publication ni téléchargement de release.
- Interface updater : recherche, disponibilité, blocage pendant l’activité MCP, téléchargement/installation simulés et affichage d’erreur de signature vérifiés sous CSP strict.
- Paquet Windows x64 local : `dist-v1/MCP Win.exe` avec les moteurs libres. Release V1 — Orbite préparée avec manifeste signé et paquet complet.

[Fonctionnement et limites v1](docs/V1.md). Les contrôles qualité ne garantissent pas l’absence de tout défaut graphique ; les tests de sécurité restent ciblés.

---

# Validation locale v0.4.0 — 19 octobre 2026

- `cargo fmt --check` : réussi.
- `cargo test --no-default-features --locked` : **29 réussis, 0 échec, 1 essai Internet séparé**.
- Interface Playwright/Edge : headers JSON, Bearer, authentification obligatoire, CA frp, historique/restauration, état serveur et dimensions minimales vérifiés.
- Sous-processus réels du binaire release : inspection/remplissage AcroForm, aplatissement sans widgets/AcroForm, création/inspection 7z réussis. Entrée/sortie par canaux privés.
- Test natif Tauri → Cloudflare HTTPS sur dossier fictif : recherche, listes/lectures compactes, fusion/extraction PDF, formulaires, ZIP sélectif, PDF/Office avancés, rendu PNG, OCR et modification avec SHA réussis.
- Préférences existantes sauvegardées puis restaurées par le test natif ; aucun document personnel utilisé.
- Vérification visuelle du formulaire rempli et de l’interface d’historique : texte accentué, case, choix de liste et contrôles affichés correctement.
- Builds/moteurs épinglés : cloudflared 2026.10.1, DuckDB 1.5.6, Typst 0.15.1, Pdfium chromium8100, Tesseract 5.5.0 (fra/eng).
- Paquet local : `dist-v04/`, puis copie dans `dist/`. Les sources sont publi?es s?par?ment du paquet de validation.

Fonctions et limites : [V0.4](docs/V0.4.md). Sécurité : [rapport](docs/SECURITY-V04.md). Le contrôle ciblé et les tests ne constituent pas une garantie d’absence de toute vulnérabilité.

---

# Validation du 19 octobre 2026

- Compilation Windows MSVC en mode release.
- Dix-sept tests automatisés réussis : accès HTTP avec et sans authentification, découverte des nouveaux outils, résolution des identifiants/noms de dossiers et absence de contournement des permissions, limites des chemins, permissions et empreintes d'écriture, générations et relectures DOCX/PPTX/XLSX, remplacements Office sur copie, types des cellules et formules, PDF multipage avec accents, SVG, transfert TCP et fermeture de frpc. Le test HTTP vérifie aussi un appel sans paramètre folder lorsqu'un seul dossier est partagé.
- v0.3.2 : validation PDF invalide renvoyée en résultat structuré par HTTP, avertissements de polices, prévalidation en lecture seule sans fichier partagé créé, nettoyage Markdown et contexte de pagination par AST, idempotence et préservation des exemples/fonctions fléchées, deux créations simultanées sur le même nom avec suffixes sans écrasement, mode de collision strict et protection des noms d'assets réservés. Le PDF avec les quatre variantes Lato est rendu et visuellement inspecté.
- v0.3 : PDF Typst avec grille, tableau, couleurs, styles et pied de page, rendu Pdfium répété, transformation en image puis PDF numérisé et reconnaissance Tesseract. L'accent français dans « identité » est explicitement vérifié. Styles/sections/tableaux OOXML, copie avec empreinte, XML invalide et chemins interdits sont testés.
- La version statique de Tesseract est compilée depuis les sources épinglées ; ses dépendances Windows ont été inspectées : uniquement KERNEL32.dll. Modèles français et anglais livrés, aucun accès réseau à l'exécution.
- Test Internet explicite réussi : lecture d'un fichier synthétique via le relais public localtunnel et une adresse HTTPS temporaire.
- Test de l'exécutable Tauri v0.3 réussi via l'URL publique : activation de l'écriture sur le dossier synthétique, génération PDF/DOCX/PPTX/XLSX, PDF Typst conçu, aperçu PNG, OCR de l'image, Word stylé par OOXML, inspection des parties, récupération binaire, modification texte avec SHA-256 et arrêt. L'en-tête Authorization est absent et le JSON copié ne le contient pas. Le script natif exige que toute instance déjà ouverte soit fermée.
- Le PDF de test classique (six pages) et le nouveau PDF conçu avec Typst ont été rendus et inspectés : titres, accents, marges, grilles, tableaux et numérotation. Les archives Office et leurs XML sont validés et relus ; leur rendu dans Microsoft Office n'a pas été automatisé.
- Interface native vérifiée dans la WebView : IPC Rust, onglets, champs par mode, validation du démarrage, absence de débordement horizontal. Captures visuellement inspectées.
- `cargo fmt --check` et vérification syntaxique JavaScript réussis.

Aucun document personnel n'a été utilisé pour les tests publics. Les tunnels de test ont été fermés et les préférences précédentes restaurées. Le test frp utilise un serveur de test local ; le déploiement HTTPS d'un relais personnel sur un serveur tiers n'a pas été réalisé.

La disponibilité ultérieure du relais public, la compatibilité avec chaque client IA et l'obtention d'une adresse permanente ne sont pas garanties. L'application est sans authentification par défaut, propose un jeton Bearer optionnel et ne fournit pas OAuth. L'OCR français/anglais est fourni ; les manuscrits, documents dégradés et autres langues ne sont pas garantis. L'édition OOXML vérifie le XML et le paquet minimal mais ne certifie pas toute la cohérence du schéma Office.
