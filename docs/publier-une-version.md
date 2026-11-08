# Publier une version

Un tag git `vX.Y.Z` poussé sur GitHub publie l'image
`ghcr.io/sfauvart/mine-de-rien` pour amd64 et arm64, avec trois tags : `X.Y.Z`, `X.Y` et
`latest`. L'image contient l'agent et le datapack de cette version. Rien n'est publié sur
un simple push.

Le workflow (`.github/workflows/publier.yml`) refuse de publier :

- si `ruff check` ou `pytest` échoue ;
- si le tag ne correspond pas aux versions déclarées dans le dépôt
  (`scripts/verifier_version.py`).

## Étapes

1. Choisir le numéro : `Z` pour une correction, `Y` pour une nouveauté, `X` pour un
   changement qui demande d'intervenir sur une installation existante.
2. Le reporter à trois endroits :
   - `version` dans `pyproject.toml` ;
   - `__version__` dans `src/mine_de_rien/__init__.py` ;
   - le tag de `ghcr.io/sfauvart/mine-de-rien` dans `docker-compose.yml`, services `agent`
     et `datapack`. C'est ce fichier que les installations téléchargent depuis le tag : il
     doit désigner l'image que le tag publie.
3. Vérifier en local, puis commiter sur `main` :

   ```bash
   uv lock                                         # met à jour la version dans uv.lock
   uv run pytest
   uv run python scripts/verifier_version.py vX.Y.Z
   git commit -am "chore: version X.Y.Z"
   ```

4. Tagguer et pousser :

   ```bash
   git push origin main
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

5. Suivre le workflow « Publier l'image » dans l'onglet **Actions** du dépôt GitHub.

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
