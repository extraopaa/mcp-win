# FAQ — questions fréquentes

### MCP Win est-il gratuit ?

Oui, le logiciel est libre et gratuit. Les coûts éventuels d'un **assistant IA**, d'un nom de domaine ou d'une machine louée pour un relais personnel sont indépendants de MCP Win.

### Dois-je installer Microsoft Office ou Python ?

Non pour utiliser le paquet Windows publié. Les outils de génération ne nécessitent ni Office, ni Python. Pour compiler le projet, il faut l'environnement Rust/Windows décrit dans [Développement](./Developpement.md).

### Puis-je utiliser n'importe quel assistant IA ?

Uniquement si l'application prend en charge un **serveur MCP** avec transport **Streamable HTTP**, selon le mode réseau choisi. Certains clients exigent OAuth ou ne permettent pas les en-têtes personnalisés ; leur compatibilité n'est pas garantie.

### Est-ce que l'assistant peut lire tout mon PC ?

Non : seuls les dossiers ajoutés sont partagés. Toute personne disposant du jeton peut utiliser les droits des dossiers partagés ; le jeton est obligatoire. Voir [Sécurité](./Securite-et-confidentialite.md).

### L'IA peut-elle décider d'activer l'écriture ?

Non. L'autorisation s'active dans l'interface Windows, avec le serveur arrêté. Elle est propre à chaque dossier.

### Où l'IA doit-elle trouver l'identifiant du dossier ?

Elle doit appeler `list_folders`. Elle peut aussi utiliser un nom exact unique ou omettre `folder` si un seul dossier est partagé.

### Mes documents sont-ils envoyés sur Internet ?

Si un assistant distant lit un fichier par MCP, **son contenu lui est transmis**. La génération et l'OCR s'exécutent sur le PC. Les relais externes peuvent également voir les flux qu'ils transportent.

### Peut-on utiliser MCP Win sans Internet ?

Le mode **Local uniquement** peut desservir un client MCP sur le même ordinateur. Les outils locaux de documents/OCR ne requièrent pas d'API externe à l'exécution ; le fonctionnement d'un assistant IA donné peut, lui, exiger Internet.

### Le tunnel public fournit-il un domaine permanent ?

Non. Le service public est fourni sans garantie de disponibilité ni de permanence de l'URL. Pour votre domaine, consultez [Modes de connexion](./Modes-de-connexion.md).

### MCP Win peut-il éditer un PDF existant ?

La v0.4 fusionne les PDF, extrait des pages et remplit les champs AcroForm pris en charge. Elle lit, génère, prévisualise et OCRise aussi les PDF, sans proposer une édition arbitraire de tous les objets graphiques existants.

### L'OCR reconnaît-il l'écriture manuscrite ?

Pas de manière garantie. Les modèles fournis ciblent le français et l'anglais ; la qualité dépend du document.

### Où signaler un problème ?

Dans les [issues GitHub](https://github.com/extraopaa/mcp-win/issues), en retirant toute donnée sensible. Consultez d'abord le [dépannage](./Depannage.md).

[← Accueil](./Home.md) · [Sommaire](./_Sidebar.md)
