# Relais personnel sécurisé — v0.4

Le tunnel localtunnel est désactivé : sa liaison relais–PC était en clair. Ce guide décrit « Mon propre relais » : frps avec TLS vérifié et jeton MCP distinct. Le mode « Tunnel simple » propose aussi Cloudflare sans compte, avec jeton obligatoire. Les logiciels sont libres et gratuits ; le domaine et une machine publique ne sont pas fournis.

1. Installer frps 0.71.1 et Caddy sur votre serveur public (Apache-2.0).
2. Obtenir un certificat TLS frps dont le **SAN** correspond au nom du serveur, signé par votre CA privée ou une CA de confiance. Garder sa clé privée sur le serveur. **Ne jamais utiliser les certificats de tests/fixtures.**
3. Adapter `frps.toml` : certificat, clé et jeton aléatoire de 32 caractères minimum. Le HTTP 8080 reste lié à 127.0.0.1. Exposer 7000 pour frp, 80/443 pour Caddy ; ne pas publier 8080 par le pare-feu ou les conteneurs.
4. Adapter le DNS et le Caddyfile pour activer le HTTPS public.
5. Dans MCP Win, choisir Mon propre relais ; saisir hôte/port, jeton frp, **nom TLS exact**, **certificat CA public PEM** et URL HTTPS. Transférer le certificat CA par un canal de confiance, après vérification avec l’administrateur. Ne jamais copier de clé privée dans l’application.
6. Copier la configuration MCP avec son jeton dans le client IA. L’adresse seule ne donne aucun accès. Le jeton est un secret ; ne pas le placer dans une URL.

Les deux segments Internet sont chiffrés : client IA → Caddy en HTTPS ; frpc → frps en TLS avec vérification de la chaîne et du nom du certificat. Le relais reste une partie de confiance pouvant lire le contenu. Ce système ne rend pas le relais aveugle. Les segments HTTP Caddy–frps et frpc–MCP restent sur les boucles locales respectives.

Sans certificat valide ou jeton, le démarrage échoue. Aucun retour automatique au protocole localtunnel. Les anciennes configurations passent au mode local avec authentification obligatoire.

Documentation primaire : https://gofrp.org/en/docs/features/common/network/network-tls/
