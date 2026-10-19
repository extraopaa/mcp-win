# cloudflared embarqué

Client officiel Cloudflare Tunnel, licence Apache-2.0 (LICENSE).
Version épinglée : **2026.10.1**, Windows amd64.
SHA-256 : `4d2490e74082c5d48dbfedaf92576fa5bd1702379ab88eb7faa0cfbe8dc891b6`.
Source : https://github.com/cloudflare/cloudflared/tree/2026.10.1
Binaire et empreinte : https://github.com/cloudflare/cloudflared/releases/tag/2026.10.1

`scripts/fetch-cloudflared.ps1` télécharge et vérifie le binaire. Celui-ci est distribué dans engines/cloudflared.exe ; son empreinte est aussi vérifiée avant exécution. Pas de mise à jour automatique ni de connexion à un compte. Aucun en-tête de requête ou jeton MCP n’est passé au processus.

Le logiciel client est libre ; le réseau de relais Cloudflare est un service tiers. Quick Tunnel est gratuit, sans compte, avec adresse temporaire, sans SLA ni SSE. Le jeton MCP reste obligatoire et Cloudflare peut voir le contenu transmis.
