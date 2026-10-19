# Vérification de sécurité v0.4 — 19 octobre 2026

Rapport de vérification du code v0.4.0 publié dans le dépôt. Les anciennes archives de release restent inchangées.

## Failles corrigées

- Suppression du transport TCP localtunnel non chiffré. Pas de repli silencieux vers ce protocole.
- Confirmation effective du démarrage du proxy frp avant affichage « prêt » ; aucune simple temporisation ne suffit.
- frp : TLS avec CA explicitement approuvée et vérification du nom du certificat ; les certificats aléatoires non vérifiés ne sont plus acceptés.
- Jeton MCP obligatoire, distinct du jeton frp. URL publique, paramètre `?token=`, faux Bearer et Basic ne donnent aucun accès.
- Anciennes préférences localtunnel migrées au mode local, sans ouverture réseau automatique. Tous les anciens jetons MCP sont renouvelés au premier démarrage v0.4.
- Serveur lié exclusivement à une adresse loopback ; validation Host et rejet Origin conservés. Refus des identifiants HTTP ambigus.
- Corps HTTP limité à 12 Mio, y compris transfert segmenté ; réception limitée à 15 secondes ; 4 requêtes simultanées et 240 requêtes par minute globalement. GET/DELETE/OPTIONS ne permettent pas de télécharger les fichiers.
- Réponses MCP autorisées marquées no-store/nosniff. Les chemins relatifs restent confinés aux partages ; flux alternatifs Windows `:` refusés, historique local réservé.
- Processus auxiliaires limités à 768 Mio et un processus par job Windows ; arrêt à la fermeture de l’application. Les délais existants Typst/OCR sont conservés.

## Essais contrôlés

Les tests Rust créent des dossiers temporaires avec des données fictives. Le relais frps et la sonde réseau tournent exclusivement sur 127.0.0.1. Aucun test offensif sur un relais tiers ni sur les documents personnels.

- Accès sans jeton, faux jeton, Basic, jeton en query string, double Authorization : refus.
- Mauvais Host et Origin web : refus.
- Traversée `../`, chemins Windows absolus, flux alternatifs, historique, traversée encodée : refus.
- Corps surdimensionné et rafale : HTTP 413 / 429.
- Configuration publique anonyme et ancien tunnel : refus ; migration vérifiée.
- Relais réel de test : mauvais nom TLS puis mauvaise CA rejetés ; certificat approuvé accepté ; échange HTTP aller-retour réussi.
- Sonde TCP sur le segment frpc–frps : présence de trafic vérifiée ; absence des chaînes HTTP, du contenu témoin et du jeton frp en clair.
- Fermeture du job Windows : frpc se termine.
- Régressions génération Office/PDF, rendu et OCR : suite générale Rust.

## Limites et exploitation

Le relais termine le HTTPS public : son administrateur peut lire les données. Il doit être de confiance. Les certificats PEM du dossier tests/fixtures et leurs clés sont publics et strictement réservés aux tests.

Le relais public de production, son DNS, ses certificats, son pare-feu et sa configuration Caddy doivent encore être vérifiés au déploiement. Le mode simple utilise désormais Cloudflare Quick Tunnel sans compte, à la demande du propriétaire : client cloudflared libre Apache-2.0, réseau Cloudflare tiers. Le projet ne fournit pas de chiffrement applicatif rendant le relais aveugle.

Un jeton volé permet les opérations autorisées sur les partages. Le renouveler dans l’interface puis redémarrer révoque l’ancien jeton. Les documents sont des entrées non fiables ; les moteurs natifs et les dépendances doivent être maintenus. Ce contrôle ciblé n’est pas une preuve d’absence de toute vulnérabilité, un audit indépendant ou un test DDoS Internet.

Commande reproductible : `cargo test --no-default-features --locked`.

Contrôle interface : `node scripts/security-ui-smoke.cjs` (Playwright/Edge, données simulées), vérification des contrôles obligatoires et de l’enregistrement CA/nom TLS.

Résultat sur le code final : **19 tests Rust réussis, 0 échec ; 1 test Internet séparé**, plus contrôle de l’interface réussi. Le test permettant auparavant l’accès anonyme a été supprimé ; le serveur refuse désormais toute configuration sans jeton, y compris en local.

## Cloudflare Quick Tunnel

Client officiel 2026.10.1, SHA-256 épinglé au téléchargement et revérifié avant chaque exécution. Aucune mise à jour automatique ; environnement nettoyé et fichier de configuration temporaire explicite pour ignorer les variables/configurations héritées. Hôte local forcé vers 127.0.0.1 ; métriques uniquement sur loopback et diagnostics de gestion désactivés. Le jeton MCP n’est ni passé en ligne de commande ni confié au processus cloudflared par un réglage.

Le démarrage attend une URL HTTPS trycloudflare.com et la confirmation d’une connexion au réseau. Arrêt et annulation terminent le processus via job Windows. Aucune rétrogradation vers localtunnel si Cloudflare échoue.

Restrictions officielles : https://developers.cloudflare.com/tunnel/get-started/quick-tunnels/ (adresse temporaire, pas de SLA, pas de SSE). Réponses MCP JSON ; compatibilité des clients à vérifier individuellement. Le réseau Cloudflare peut voir le contenu.

Essai Internet exécuté séparément : **Cloudflare HTTPS réussi**, refus sans jeton et avec jeton erroné, lecture JSON autorisée du seul fichier synthétique. Aucun dossier personnel partagé pendant cet essai. Un premier essai immédiat a échoué sur la propagation DNS ; la vérification laisse désormais le temps à l’adresse temporaire de devenir joignable.

## Fonctions documentaires ajoutées à la version

Les archives et la manipulation/remplissage de PDF sont traités dans un sous-processus limité à 768 Mio, un processus et 30 secondes. Les chemins restent résolus dans les partages ; aucune extraction globale ni écrasement d’archive n’est autorisé. Les champs AcroForms canoniques, valeurs de widgets et apparences sont vérifiés après écriture ; les documents signés ou structures ambiguës sont refusés. L’aplatissement PDF est explicite et vérifié sans widgets/AcroForm restants.

SQLite est ouvert en lecture seule/immutable, avec authorizer, limites et interruption ; DuckDB est lancé en mode sûr irréversible, lecture seule et sans init utilisateur. Les essais de lecture de fichiers hors base, d’accès réseau, d’extensions et de mutation sont refusés. Les bases à journal actif sont refusées.

L’historique est réservé à l’interface locale. Une restauration requiert le MCP arrêté, conserve l’état remplacé et n’ouvre aucun nouvel outil d’écriture distant. Les patchs exigent une empreinte source à jour. Les données de modèles Typst transitent par JSON temporaire, sans évaluation du contenu comme code.

Validation des ajouts : **29 tests Rust réussis**, contrôle Playwright des headers et de l’historique réussi, sous-processus de production testés sur fixtures AcroForm/PDF/7z. Le test Internet Cloudflare reste séparé de la suite locale et avait été exécuté avec succès lors du durcissement du tunnel.

Les instructions et résultats des sous-processus documentaires passent par stdin/stdout privés, sans fichier de requête mutable dans TEMP. Un partage du dossier temporaire ne permet donc pas de réécrire les racines ou permissions envoyées au worker.

Test natif final Tauri → Cloudflare HTTPS v0.4 réussi : recherche, lectures compactes, fusion/extraction/remplissage PDF, archives sélectives, génération, OCR et écritures avec empreinte. Le test utilise exclusivement un partage fictif, puis restaure les préférences existantes.
