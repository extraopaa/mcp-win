# Modes de connexion

| Mode | Usage | Protection |
| --- | --- | --- |
| Tunnel simple | Cloudflare Quick Tunnel sans compte, URL temporaire | HTTPS et tunnel chiffré, jeton obligatoire |
| Mon propre relais | Votre infrastructure frp/Caddy | HTTPS public, TLS frp avec CA et nom vérifiés, jeton MCP distinct |
| Local uniquement | Même PC, généralement http://127.0.0.1:8787/mcp | Écoute loopback et jeton obligatoire |

Le client cloudflared est libre ; le réseau Cloudflare est un service tiers gratuit qui peut lire le trafic. L’adresse est temporaire, sans garantie de disponibilité ni SSE. MCP Win utilise les réponses JSON. Localtunnel est supprimé sans repli automatique.

Le jeton frp connecte le tunnel ; le jeton MCP protège les outils. Le mode frp exige aussi une CA de confiance et un nom TLS correspondant au certificat.

[Relais personnel](../../relay/README.md) · [Sécurité](Securite-et-confidentialite.md) · [Connexion IA](Connexion-IA.md)
