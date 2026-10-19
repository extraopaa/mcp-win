# frp 0.71.1

Source : https://github.com/fatedier/frp/tree/v0.71.1

Licence : Apache-2.0, copie dans `LICENSE`.

Les binaires ne sont pas versionnés. Depuis la racine, lancer `./scripts/fetch-frp.ps1` avant la première compilation. Le script récupère l'archive officielle Windows x64 et vérifie son SHA-256 épinglé :

```
b90074b2b6a1d792a7a40f085ac6d276bcab2d1bc60201f8a86ca53ce65ce821
```

Le `frpc.exe` obtenu est embarqué dans l'application au moment de la compilation. `frps.exe` est utilisé uniquement pour le test du relais personnel.
