# Connecter MCP Win à une application IA

MCP Win expose un serveur **Model Context Protocol**, transport **Streamable HTTP**. Le client doit pouvoir ajouter un serveur MCP HTTP, local ou distant selon votre mode.

## Paramètres de connexion

| Réglage du client | Valeur |
| --- | --- |
| Type / transport | `Streamable HTTP` |
| URL du serveur | L'adresse copiée depuis MCP Win, terminée par `/mcp` |
| Authentification par défaut | Aucune (configuration par défaut de MCP Win) |
| Si « Exiger un jeton d'accès » est activé | En-tête `Authorization: Bearer VOTRE_JETON` |
| Avec localtunnel, si le client accepte les en-têtes personnalisés | `bypass-tunnel-reminder: true` |

Les interfaces et formats JSON des différents clients IA ne sont **pas uniformes**. Utilisez le bouton **Copier la configuration JSON** dans **Connexion extérieure**, puis adaptez le résultat au format demandé par votre assistant. Ne supposez pas qu'un client exigeant OAuth pourra fonctionner : MCP Win ne le fournit pas dans cette version.

## Ordre de travail recommandé pour l'assistant

1. Appeler `list_folders` afin de connaître les noms, identifiants et droits `writable`.
2. Choisir un dossier ; le paramètre `folder` peut être omis si **un seul** dossier est partagé, ou renseigné avec un nom exact non ambigu.
3. Appeler `list_files` puis `read_file` pour consulter les données pertinentes.
4. Pour créer, choisir un dossier `writable: true` et utiliser l'outil adapté.
5. Pour modifier un fichier existant, obtenir son empreinte `sha256` et respecter les conditions propres à l'outil.
6. Pour un PDF mis en page, commencer par `get_authoring_guide` et `validate_designed_pdf` ; vérifier ensuite la sortie avec `render_pdf_page`.

### Exemple de demande

> « Utilise les outils de MCP Win. Découvre les dossiers avec `list_folders`, lis les documents pertinents et crée un compte rendu Word dans un dossier autorisé en écriture. Ne remplace pas les originaux. »

## Vérifier la connexion

Si le client découvre les outils mais ne trouve pas un dossier, l'assistant doit relancer `list_folders` plutôt que demander à l'utilisateur un identifiant interne. Si aucun outil n'apparaît, vérifiez que le MCP est démarré, que l'URL se termine par `/mcp` et que le transport est bien Streamable HTTP.

Consultez [Dépannage](./Depannage.md) et [Sécurité](./Securite-et-confidentialite.md).

[← Premiers pas](./Premiers-pas.md) · [Outils MCP →](./Outils-MCP-et-formats.md)
