# Validation du 19 octobre 2026

- Compilation Windows MSVC en mode release.
- Sept tests automatisés : segmentation Unicode et chemins, PDF réel, SVG et PNG, serveur MCP HTTP avec authentification optionnelle, lecture sans en-tête Authorization, transfert TCP du tunnel, relais frp réel et fermeture du processus via un Job Windows.
- Test Internet explicite réussi : lecture d'un fichier synthétique via le relais public localtunnel et une adresse HTTPS temporaire.
- Test de l'exécutable Tauri réussi : clic sur Démarrer, ouverture du tunnel Rust, appel MCP depuis l'URL publique, lecture du fichier synthétique et arrêt.
- Interface native vérifiée dans la WebView : IPC Rust, onglets, champs par mode, validation du démarrage, absence de débordement horizontal. Captures visuellement inspectées.
- `cargo fmt --check` et vérification syntaxique JavaScript réussis.

Aucun document personnel n'a été utilisé pour les tests publics. Les tunnels de test ont été fermés et les préférences précédentes restaurées. Le test frp utilise un serveur de test local ; le déploiement HTTPS d'un relais personnel sur un serveur tiers n'a pas été réalisé.

La disponibilité ultérieure du relais public, la compatibilité avec chaque client IA et l'obtention d'une adresse permanente ne sont pas garanties. L'application est sans authentification par défaut, propose un jeton Bearer optionnel et ne fournit pas OAuth. Les PDF scannés nécessitent encore un OCR.
