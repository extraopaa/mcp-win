# Dépannage

Ce guide reprend les problèmes les plus courants et les limites connues de MCP Win 0.3.2.

| Symptôme | Vérification ou correction |
| --- | --- |
| L'application ne démarre pas | Utiliser le paquet Windows x64 **complet**, extrait avec son dossier `engines/` ; vérifier WebView2. |
| Aucune URL publique n'apparaît | Vérifier que **Démarrer le MCP** est actif et que le tunnel est disponible ; essayer **Local uniquement** pour isoler le problème. |
| L'assistant ne voit aucun outil | Vérifier le transport **Streamable HTTP**, l'URL copiée se terminant par `/mcp` et les exigences d'authentification du client. |
| Le client refuse une page d'avertissement du relais public | Essayer `bypass-tunnel-reminder: true` si le client permet les en-têtes personnalisés, ou utiliser un relais personnel. |
| L'adresse publique ne répond plus | Le relais public fournit une adresse temporaire ; arrêter et redémarrer le MCP puis copier la nouvelle URL. |
| L'assistant réclame un identifiant de dossier | Lui demander d'appeler `list_folders` ; un nom exact unique ou l'omission de `folder` (un seul dossier) peut convenir. |
| Un outil refuse l'écriture | Arrêter le MCP, passer le dossier à **Écriture autorisée**, redémarrer et vérifier `writable: true`. |
| Une modification est rejetée à cause du SHA-256 | Relire le fichier pour obtenir son dernier `sha256` ; ne pas réutiliser une ancienne empreinte. |
| Un PDF Typst échoue | Appeler `validate_designed_pdf`, lire les diagnostics, corriger la source, les assets ou le contexte des compteurs, puis recommencer. |
| Le PDF produit porte un autre nom | `create_designed_pdf` ajoute par défaut un suffixe en cas de collision : lire le `path` retourné. |
| L'OCR manque du texte | Vérifier la bonne page, la langue (`fra`/`eng`), le mode de lecture et la rotation ; ne pas supposer les résultats exacts. |
| Une formule Excel n'affiche pas de résultat calculé | MCP Win stocke les formules mais **ne les calcule pas** : ouvrir le classeur dans un tableur adapté. |
| Un DOCX/PPTX ne conserve pas tout le style après remplacement | Le remplacement textuel agit sur les segments pris en charge ; pour une édition avancée, utiliser les outils OOXML puis valider le document dans Office. |

## Diagnostic par étapes

1. Vérifier l'état du serveur dans **Vue d'ensemble**.
2. Vérifier le mode et les paramètres dans **Connexion extérieure**.
3. Consulter **Activité** (journal en mémoire, limité à 100 opérations).
4. Tester `list_folders` et `list_files` avant une tâche d'écriture ou de génération.
5. Utiliser les diagnostics renvoyés par les outils eux-mêmes.
6. Si le problème persiste, ouvrir une [issue GitHub](https://github.com/extraopaa/mcp-win/issues) en décrivant la version, le mode de connexion, les étapes et un exemple synthétique — **sans jeton ni fichier confidentiel**.

Les tests documentés dans [VALIDATION.md](../../VALIDATION.md) ne garantissent pas la disponibilité future du relais public ni la compatibilité avec tous les clients IA.

[← Sécurité](./Securite-et-confidentialite.md) · [FAQ →](./FAQ.md)
