# Validation locale v0.4.0-preview.1 — 19 octobre 2026

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
