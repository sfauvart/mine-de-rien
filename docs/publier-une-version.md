# Publier une version

Un tag git `vX.Y.Z` poussé sur GitHub publie l'image
`ghcr.io/sfauvart/mine-de-rien` pour amd64 et arm64, avec trois tags : `X.Y.Z`, `X.Y` et
`latest`. L'image contient le binaire de l'agent, datapack de cette version compris, et le
plugin Paper `mine-de-rien-voix.jar` de la même version, construit et testé par la même
chaîne (étape Maven de `docker/agent.Dockerfile`). Rien d'autre.
Rien n'est publié sur un simple push.

Le workflow (`.github/workflows/publier.yml`) refuse de publier :

- si `cargo fmt --check`, `cargo clippy` ou `cargo test` échoue ;
- si le tag ne correspond pas à la version déclarée dans le dépôt (`tests/version.rs`).

## Étapes

1. Choisir le numéro : `Z` pour une correction, `Y` pour une nouveauté, `X` pour un
   changement qui demande d'intervenir sur une installation existante.
2. Le reporter à trois endroits :
   - `version` dans `Cargo.toml`, seule source de la version : c'est elle qu'affiche
     `mine-de-rien --version` ;
   - `<version>` du plugin dans `plugin/pom.xml` : agent, datapack et plugin sortent
     ensemble ;
   - le tag de `ghcr.io/sfauvart/mine-de-rien` dans `docker-compose.yml`, services `agent`
     et `livraison`. C'est ce fichier que les installations téléchargent depuis le tag : il
     doit désigner l'image que le tag publie.
3. Vérifier en local, puis commiter sur `main` :

   ```bash
   cargo test                                      # met aussi à jour la version dans Cargo.lock
   MDR_TAG=vX.Y.Z cargo test --test version        # le tag correspond-il à la version déclarée ?
   git commit -am "chore: version X.Y.Z"
   ```

4. Tagguer et pousser :

   ```bash
   git push origin main
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

5. Suivre le workflow « Publier l'image » dans l'onglet **Actions** du dépôt GitHub.

## L'image de la voix

Piper et son serveur HTTP vivent dans une image à part,
`ghcr.io/sfauvart/mine-de-rien-voix`, dont le numéro est celui de Piper : elle ne change
pas d'une version de l'agent à l'autre. Un tag git `voix-X.Y.Z` la publie pour amd64 et
arm64 (`.github/workflows/publier-voix.yml`) ; le workflow refuse si `X.Y.Z` n'est pas la
version figée dans `docker/voix.Dockerfile`.

Pour changer de version de Piper : la modifier dans `docker/voix.Dockerfile`, reporter le
numéro sur l'image `mine-de-rien-voix` de `docker-compose.yml` et de
`docker/essai.compose.yml`, écouter une dictée sur le serveur d'essai, commiter, puis :

```bash
git tag voix-X.Y.Z
git push origin voix-X.Y.Z
```

Piper est sous licence GPL : l'image le redistribue tel qu'il est publié sur PyPI, sans
modification, et l'agent ne lui parle que par HTTP.

## Changer de version de Minecraft ou du chat vocal

Le plugin Simple Voice Chat est épinglé dans `docker-compose.yml` (`MODRINTH_PROJECTS`) et
dans `docker/essai.compose.yml`, et son API dans `plugin/pom.xml` (`voicechat.api.version`,
avec `paper.version`). Les changer ensemble, rejouer `/essai-en-jeu`, et mettre à jour la
version du mod annoncée dans `docs/rejoindre-le-serveur.md` : le jeu de l'enfant doit porter
le même numéro que le serveur.

## Visibilité du paquet

Le paquet `mine-de-rien` a pris la visibilité publique du dépôt dès la première
publication (v0.1.0) : rien à régler. Si un téléchargement anonyme est un jour refusé
(`unauthorized`), le réglage est dans GitHub → profil → **Packages** → `mine-de-rien` →
**Package settings** → **Change visibility**.

## Vérifier une version publiée

Depuis une machine non connectée à ghcr.io :

```bash
docker buildx imagetools inspect ghcr.io/sfauvart/mine-de-rien:X.Y.Z   # linux/amd64 et linux/arm64
docker run --rm ghcr.io/sfauvart/mine-de-rien:X.Y.Z mine-de-rien --version
```

## Se tromper de version

Un tag refusé par le workflow n'a rien publié : le supprimer, corriger, recommencer.

```bash
git tag -d vX.Y.Z
git push origin :refs/tags/vX.Y.Z
```

Une version publiée n'est pas réécrite : on en publie une nouvelle.
