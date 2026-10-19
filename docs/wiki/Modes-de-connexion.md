# Modes de connexion et relais

Dans **Connexion extérieure**, MCP Win propose trois modes. Le serveur MCP tourne sur votre PC ; les modes définissent comment un assistant peut l'atteindre.

| Mode | Usage | Adresse |
| --- | --- | --- |
| **Tunnel public** (`localtunnel`) | Démarrage rapide, sans compte ni configuration de box | URL HTTPS temporaire attribuée par le relais |
| **Mon propre relais** (`frp`) | Utiliser votre infrastructure et votre domaine | URL HTTPS du domaine configuré |
| **Local uniquement** | Tests sur le même ordinateur | `http://127.0.0.1:8787/mcp` si le port local est 8787 |

Le **port MCP local** est configurable ; la valeur par défaut est **8787**. Quel que soit le mode, utiliser la vraie adresse affichée et copiée par MCP Win.

## Tunnel public

Le relais par défaut est `https://localtunnel.me`. Le client localtunnel est écrit en Rust et établit des connexions sortantes. Le relais peut devenir indisponible, bloquer certaines connexions, imposer une page intermédiaire ou réattribuer une URL après expiration. Dans ce dernier cas, arrêter puis redémarrer le MCP.

L'en-tête personnalisé `bypass-tunnel-reminder: true` figure dans l'exemple JSON fourni par l'application ; certains clients IA ne prennent pas en charge les en-têtes arbitraires.

**Confidentialité :** le HTTPS public se termine chez le relais, qui peut voir les données et éventuels jetons. La liaison localtunnel entre le relais et le PC n'est pas chiffrée de bout en bout.

## Relais personnel avec frp

Le client `frpc` version **0.71.1** est embarqué. Le serveur distant nécessite **frps**, une machine publique et un serveur HTTPS de votre choix (exemple : **Caddy**).

1. Sur votre serveur, installer `frps` 0.71.1 et Caddy.
2. Adapter [`relay/frps.toml`](../../relay/frps.toml) avec un **jeton frp fort**, puis lancer frps.
3. Configurer votre DNS et adapter [`relay/Caddyfile`](../../relay/Caddyfile) pour exposer un domaine HTTPS et transmettre `/mcp`.
4. Ouvrir sur le serveur les ports **7000, 80 et 443** ; garder le port **8080** local selon l'exemple.
5. Dans MCP Win, renseigner l'hôte frps, le port, le jeton frp et l'URL publique HTTPS, puis démarrer.

Le **jeton frp** sert à connecter le client au relais ; le **jeton MCP** sert, s'il est activé, à contrôler l'accès aux outils. Ce sont deux secrets différents. Le relais personnel n'apporte pas automatiquement de chiffrement de bout en bout ; vérifiez les certificats TLS et la configuration réseau selon votre usage.

Consultez [le guide de déploiement du relais](../../relay/README.md) et [Sécurité et confidentialité](./Securite-et-confidentialite.md).

[← Connexion à une IA](./Connexion-IA.md) · [Sécurité →](./Securite-et-confidentialite.md)
