# Validation du 19 octobre 2026

- Compilation Windows MSVC en mode release.
- Onze tests automatisés réussis : accès HTTP avec et sans authentification, limites des chemins, permissions et empreintes d'écriture, générations et relectures DOCX/PPTX/XLSX, remplacements Office sur copie, types des cellules et formules, PDF multipage avec accents, SVG, transfert TCP et fermeture de frpc.
- Test Internet explicite réussi : lecture d'un fichier synthétique via le relais public localtunnel et une adresse HTTPS temporaire.
- Test de l'exécutable Tauri réussi via l'URL publique : activation de l'écriture sur le dossier synthétique, génération PDF/DOCX/PPTX/XLSX, récupération binaire, modification texte avec SHA-256 et arrêt. L'en-tête Authorization est absent et le JSON copié ne le contient pas.
- Le PDF de test (six pages) a été rendu et inspecté : titres, accents, marges, paragraphes conservés et numérotation. Les archives Office et leurs XML sont validés et relus ; leur rendu dans Microsoft Office n'a pas été automatisé.
- Interface native vérifiée dans la WebView : IPC Rust, onglets, champs par mode, validation du démarrage, absence de débordement horizontal. Captures visuellement inspectées.
- `cargo fmt --check` et vérification syntaxique JavaScript réussis.

Aucun document personnel n'a été utilisé pour les tests publics. Les tunnels de test ont été fermés et les préférences précédentes restaurées. Le test frp utilise un serveur de test local ; le déploiement HTTPS d'un relais personnel sur un serveur tiers n'a pas été réalisé.

La disponibilité ultérieure du relais public, la compatibilité avec chaque client IA et l'obtention d'une adresse permanente ne sont pas garanties. L'application est sans authentification par défaut, propose un jeton Bearer optionnel et ne fournit pas OAuth. Les PDF scannés nécessitent encore un OCR.
