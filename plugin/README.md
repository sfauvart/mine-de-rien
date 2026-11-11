# mine-de-rien-voix

Le plugin Paper de Mine de rien. Il joue à **un seul joueur**, par le chat vocal
([Simple Voice Chat](https://modrinth.com/plugin/simple-voice-chat)), un son que l'agent a
déposé dans un dossier partagé avec le serveur. Il ne décide rien, n'ouvre aucun port,
n'appelle personne et n'écoute aucun micro.

## Commande

Réservée à la console du serveur et à RCON : un joueur, même opérateur, est refusé.

```text
mdrvoix jouer <pseudo> <fichier>      met le son dans la file du joueur
mdrvoix couper <pseudo>               vide la file et arrête le son en cours
mdrvoix etat <pseudo>                 répond « relié », « sans chat vocal » ou « absent »
mdrvoix phrase <pseudo> [fichier]     désigne le son que l'Écouteur rejoue ; sans fichier, l'oublie
```

`jouer`, `couper` et `phrase` répondent `ok`, ou `refusé : <raison>`.

L'**Écouteur** est l'objet que l'agent donne à l'enfant devant un coffre de dictée
(`warped_fungus_on_a_stick` marqué `mdr_ecouteur`). Un clic droit avec lui coupe ce qui joue
et rejoue le son désigné par `phrase`, lu dans le dossier partagé à ce moment-là. Le plugin
oublie les phrases désignées quand le serveur redémarre : l'agent les redésigne à chaque
rappel.

- `<fichier>` est un nom seul, `[a-z0-9-]+\.pcm`, cherché dans le dossier partagé et nulle
  part ailleurs. Format : PCM brut, mono, 16 bits petit-boutiste, 48 000 Hz.
- Pour `jouer`, le son est lu en mémoire au moment de la commande : l'agent peut effacer le
  fichier dès que le serveur a répondu `ok`. Le son de `phrase`, lui, doit rester dans le
  dossier tant que l'Écouteur doit le rejouer.
- Un son à la fois par joueur : un nouveau son attend la fin du précédent.
- Le dossier partagé est `plugins/mine-de-rien-voix/sons`, ou celui de la variable
  d'environnement `MDR_VOIX_DOSSIER`.

## Construire

Sans JDK sur le poste, depuis la racine du dépôt :

```bash
docker run --rm -v "$PWD/plugin":/plugin -v mine-de-rien-m2:/root/.m2 -w /plugin \
  maven:3.9-eclipse-temurin-25 mvn -B package
```

Le fichier produit est `plugin/target/mine-de-rien-voix.jar`. La construction télécharge
l'API de Paper, celle du chat vocal et JUnit ; rien de tout cela n'entre dans le `.jar`, et
le plugin ne télécharge rien une fois sur le serveur.
