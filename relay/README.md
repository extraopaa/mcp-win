# Relais personnel open source

Ce mode nécessite une machine publique contrôlée par vous. frp et Caddy sont gratuits et sous licence Apache-2.0 ; un nom de domaine et une machine publique peuvent avoir leurs propres coûts. Le mode tunnel public de l'application n'exige pas cette infrastructure.

1. Sur le serveur public, installer [frps 0.71.1](https://github.com/fatedier/frp/releases/tag/v0.71.1) correspondant à son système, et [Caddy](https://github.com/caddyserver/caddy). Le frpc Windows fourni dans l'application utilise la même version de frp.
2. Copier `frps.toml`, remplacer le jeton par un secret long et lancer `frps -c frps.toml`. L'écoute de contrôle est sur 7000, et le routage HTTP sur 127.0.0.1:8080.
3. Faire pointer le DNS `mcp.votre-domaine.fr` vers cette machine. Adapter le `Caddyfile`, puis lancer `caddy run --config Caddyfile`. Le certificat public est obtenu automatiquement auprès de Let's Encrypt lorsque les ports et le DNS sont correctement configurés. Le serveur Caddy n'est pas installé sur le PC client et n'est pas inclus dans son exécutable.
4. Autoriser sur le serveur les ports TCP 7000, 80 et 443. Le port 8080 reste local. Utiliser un service système pour maintenir frps et Caddy démarrés.
5. Dans MCP Win : mode **Mon propre relais**, hôte de frps, port 7000, même jeton frp, URL `https://mcp.votre-domaine.fr`. Ajouter les dossiers et démarrer.

Le jeton frp protège l'inscription du client auprès du relais. Le jeton MCP est distinct et protège les lectures de documents. frpc active TLS pour sa liaison avec frps ; le HTTPS public est terminé par Caddy. Le serveur de relais peut donc voir le trafic. Les exemples n'implémentent pas OAuth ni une authentification par certificat client du serveur frps. Pour une production sensible, configurer aussi la vérification du certificat TLS frps et l'isolation réseau selon votre infrastructure.
