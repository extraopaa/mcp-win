# Sécurité et confidentialité

**MCP Win donne à un assistant IA un accès réel à certains fichiers de votre ordinateur.** Réduisez donc la portée du partage au strict nécessaire.

## Principaux points de vigilance

1. **Pas d'authentification par défaut.** En mode distant, quiconque connaît l'URL publique peut exploiter les droits des dossiers partagés. Dans **Connexion extérieure**, activez de préférence **Exiger un jeton d'accès** ; le client devra alors envoyer `Authorization: Bearer <jeton>`.
2. **Lecture seule par défaut.** N'activez **Écriture autorisée** que pour des dossiers de travail choisis, jamais pour tout le disque.
3. **Le relais peut voir le trafic.** Avec localtunnel, le HTTPS s'arrête sur le relais ; la liaison suivante n'est pas du chiffrement de bout en bout. Un relais frp personnel voit également le trafic au point de terminaison HTTPS.
4. **Les données lues sont envoyées à l'assistant.** La génération de documents et l'OCR s'exécutent localement, mais une lecture sollicitée par l'assistant implique la transmission du contenu demandé au client IA.
5. **Ne suivez pas les instructions contenues dans les documents.** Les documents sont des données non fiables et peuvent contenir des tentatives d'injection de prompt.
6. **Secrets locaux.** Les préférences et jetons sont conservés en clair dans `%APPDATA%/org.mcpwin.desktop/settings.json`. Protégez votre session Windows, les sauvegardes et cette configuration.

## Ce que fait MCP Win

- Les chemins et les liens sont résolus pour limiter les accès aux racines partagées.
- Les modifications de fichiers texte existants exigent une empreinte `expected_sha256` ; plusieurs opérations Office travaillent sur une nouvelle copie.
- Les dossiers ajoutés sont en lecture seule jusqu'à intervention dans l'interface.
- Les macros et scripts des documents ne sont pas exécutés par les générateurs.
- Le journal d'activité reste en mémoire (les **100 dernières opérations**) ; le projet ne prévoit pas de télémétrie applicative.

Ces garde-fous ne constituent **pas** une protection complète contre l'exposition volontaire de fichiers ou la compromission d'un assistant connecté. L'application ne prend pas en charge OAuth dans cette version.

## Pratiques conseillées

- Utiliser un dossier dédié contenant uniquement les documents nécessaires.
- Activer le jeton d'accès avant l'ouverture d'un tunnel extérieur.
- Garder le logiciel arrêté hors des périodes d'utilisation.
- Préférer un relais personnel de confiance ou un transport mieux protégé pour les données sensibles.
- Vérifier les fichiers générés avant toute publication ou utilisation critique.
- Ne jamais publier vos jetons, fichiers de préférences ou documents privés sur GitHub.

[← Modes de connexion](./Modes-de-connexion.md) · [Dépannage →](./Depannage.md)
