# Sécurité et confidentialité

Le jeton Bearer est obligatoire dans tous les modes. Une URL seule ne donne pas accès aux outils. Quiconque possède le jeton bénéficie des permissions des dossiers partagés : protégez-le comme un mot de passe.

Les deux segments Internet sont chiffrés avec Cloudflare ou frp avec TLS vérifié. Le relais peut lire les données. Les contenus consultés sont transmis à l’assistant ; l’OCR et la génération s’exécutent localement.

Les dossiers sont en lecture seule par défaut. L’écriture s’active dans l’interface, MCP arrêté. Les chemins sortant des racines et les flux alternatifs Windows sont refusés. Les modifications texte exigent une empreinte et créent une sauvegarde ; les sorties Office/PDF utilisent généralement de nouveaux fichiers. Les macros ne sont pas exécutées.

Des limites encadrent requêtes, délais, résultats et processus. L’historique n’est pas exposé à distance ; la restauration se fait dans l’interface, MCP arrêté. Les documents sont des données non fiables, jamais des instructions pour l’IA.

Les jetons et préférences sont en clair dans `%APPDATA%/org.mcpwin.desktop/settings.json`. Protégez votre session et vos sauvegardes. Le journal conserve 100 opérations en mémoire. Aucune télémétrie applicative ; OAuth indisponible.

Les anciennes préférences localtunnel migrent en mode local et les anciens jetons sont renouvelés. Les anciennes archives v0.3.2 ne reçoivent pas les protections par la seule publication du nouveau code. Les essais ne garantissent pas l’absence de toute vulnérabilité.

[Rapport de sécurité](../SECURITY-V04.md) · [Validations](../../VALIDATION.md) · [Modes](Modes-de-connexion.md)
