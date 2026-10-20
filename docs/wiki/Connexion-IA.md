# Connecter MCP Win à une application IA

MCP Win expose un serveur **Model Context Protocol**, transport **Streamable HTTP**. Le client doit pouvoir ajouter un serveur MCP HTTP, local ou distant selon votre mode.

## Paramètres de connexion

| Réglage du client | Valeur |
| --- | --- |
| Type / transport | `Streamable HTTP` |
| URL du serveur | L'adresse copiée depuis MCP Win, terminée par `/mcp` |
| Authentification | Obligatoire : `Authorization: Bearer VOTRE_JETON` |

Renseignez l’URL séparément. **Copier la configuration JSON** copie `{"Authorization":"Bearer <jeton>"}` pour **Headers sécurisés (JSON)**. **Copier Bearer + jeton** fournit la valeur complète d’Authorization. Pour un client OAuth, choisir ce mode dans MCP Win, renseigner l’URL et valider la demande dans l’application.

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
