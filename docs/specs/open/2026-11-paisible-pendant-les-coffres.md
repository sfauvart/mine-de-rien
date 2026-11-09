# Tant qu'il reste un coffre à faire, les monstres laissent l'enfant tranquille

> **Reprise de session** : lire « Pourquoi », « Décisions actées » et « Statut »,
> puis aller à la première phase non terminée.

## Pourquoi

Un coffre attend l'enfant à l'atelier, mais rien ne le protège sur le chemin. S'il fait
nuit, les zombies l'interrompent pendant qu'il cherche son calcul et manipule l'enclume.
Sur le serveur réel, la difficulté est `easy` (`server.properties`) : les monstres
apparaissent normalement.

La seule parade aujourd'hui est manuelle : un parent passe le serveur en paisible par
RCON, pour toute la durée, et perd les monstres aussi quand il n'y a aucun devoir. Le
mode créatif n'est pas une solution : la zone de l'atelier ne s'applique qu'aux joueurs
en survie (`datapacks/mine_de_rien/data/mine_de_rien/function/tick.mcfunction:12`).

La boucle de surveillance sait déjà tout ce qu'il faut : elle tourne toutes les
3 secondes (`orchestrateur.py:152`), connaît le coffre actif (`orchestrateur.py:85`) et
la file (`storage/sqlite.py:249-255`).

**Cible** : le serveur est en paisible dès qu'il reste un coffre à faire, et revient tout
seul à sa difficulté habituelle quand il n'y en a plus. L'enfant est prévenu dans le chat
à chaque bascule.

## Décisions actées

| Sujet | Décision |
|---|---|
| Quand le serveur est paisible | Dès qu'un coffre est actif ou en file, à condition que l'atelier existe dans ce monde. Pas de va-et-vient entre deux coffres. |
| Limite de durée | Aucune : paisible tant que le coffre n'est pas résolu. |
| Message à l'enfant | Une ligne dans le chat à chaque bascule, s'il est connecté. Rien sur Telegram. |
| Mécanisme | État relu dans le monde à chaque tour : `difficulty` lit la valeur, l'agent ne la change que si elle diffère de l'attendue. Rien en base. |
| Difficulté hors coffre (hypothèse) | Réglage `[paisible] difficulte_normale`, `easy` par défaut, soit la valeur actuelle du serveur réel. |
| Activation (hypothèse) | `[paisible] actif = true` par défaut, comme Pixel. `false` rend la difficulté aux parents. |
| Atelier absent (hypothèse) | Aucun coffre ne peut exister dans le monde : difficulté normale, même si la file n'est pas vide. |
| Message manqué (hypothèse) | Une bascule survenue enfant déconnecté n'est pas annoncée plus tard. Le rappel de connexion existant parle du coffre. |

## Hors périmètre

- Limiter la durée du mode paisible, ou le réserver aux abords de l'atelier : écarté, la
  difficulté vaut pour tout le serveur et aucune limite n'a été retenue.
- Prévenir les parents sur Telegram des bascules : `/file` dit déjà ce qui reste à faire.
- Un réglage par joueur : la difficulté est globale, les autres joueurs du serveur sont
  concernés aussi.
- Une commande Telegram pour changer la difficulté à la main.

## Conséquences acceptées

- **Contournement.** Un enfant qui préfère la tranquillité peut laisser son coffre
  non résolu, ou ne pas le vider pour bloquer la file. Accepté : les parents voient les
  coffres en attente avec `/file`, et on ajoutera une limite si le cas se présente.
- **Réglage manuel écrasé.** Avec `actif = true`, un `difficulty` tapé à la main est
  corrigé au tour suivant, en 3 secondes. Pour garder un serveur paisible en permanence,
  mettre `actif = false`. À dire dans la doc.
- **Le passage en paisible supprime les monstres présents**, y compris ceux d'un élevage
  ou nommés (à vérifier en jeu), et la faim ne descend plus.

## Vie privée

Rien ne sort de la maison. Le pseudo n'apparaît que dans les commandes RCON locales.

## Statut

**Branche** : `feat/paisible-pendant-les-coffres`, pas encore créée, `/next` l'ouvre à la
première phase.

- [ ] **Phase 1 : la difficulté suit les coffres**
- [ ] **Phase 2 : l'enfant est prévenu, la doc suit**

La phase 2 dépend de la phase 1, qui lui fournit le moment exact de la bascule.

## Conception

### Phase 1 : la difficulté suit les coffres

**Contexte.** À chaque tour, l'agent compare la difficulté du serveur à celle attendue.
Comparer à chaque tour plutôt que réagir aux événements couvre sans code de plus
`/annuler`, un redémarrage du serveur ou de l'agent, et un changement de monde.

**Fichiers.** `games/paisible.py` (nouveau), `config.py`, `orchestrateur.py`, `cli.py`,
`tests/test_paisible.py` (nouveau), `tests/test_orchestrateur.py`, `tests/test_config.py`.

**Conception.**

- `config.py` : dataclass `Paisible(actif: bool = True, difficulte_normale: str = "easy")`.
  `difficulte_normale` doit valoir `easy`, `normal` ou `hard`, sinon `ValueError` au
  démarrage. `peaceful` est refusé : il annulerait la feature.
- `games/paisible.py` : `ModePaisible(jeu, difficulte_normale)`, avec
  `async def appliquer(self, coffre_a_faire: bool) -> str | None`. La méthode lit la
  difficulté, ne fait rien si c'est la bonne, sinon la change, vérifie la réponse du
  serveur (une réponse refusée lève `ErreurCoffre`) et renvoie la nouvelle difficulté.
- `Orchestrateur` reçoit `paisible: ModePaisible | None = None`. `_tour`
  (`orchestrateur.py:79`) l'appelle en toute fin, **y compris après un `AtelierAbsent`**
  (le `return` de `orchestrateur.py:91` aujourd'hui l'en empêcherait), avec
  `coffre_a_faire = atelier présent et (depot.actif() ou depot.suivant())`, relus après
  la pose ou l'ouverture du tour.
- Une erreur RCON remonte à `tourner`, qui réessaie au tour suivant.
- `cli.py` ne crée `ModePaisible` que si `config.paisible.actif`.

Commandes, **à vérifier sur la version du serveur** par `/essai-en-jeu`. Lecture et
passage en paisible ont été observés sur Paper 26.3 lors d'un essai jetable, le retour
à `easy` et la réponse quand rien ne change ne l'ont pas été :

```
difficulty                 → « The difficulty is Peaceful »
difficulty peaceful        → « The difficulty has been set to Peaceful »
```

**Vérification automatique.**

- `uv run pytest` et `uv run ruff check` passent.
- Avec un faux jeu : coffre actif → paisible ; file non vide sans actif → paisible ;
  rien à faire → normal ; atelier absent avec file non vide → normal ; difficulté déjà
  correcte → aucune commande d'écriture ; `actif = false` → aucune commande `difficulty`.
- `/essai-en-jeu` : les trois commandes ci-dessus sont acceptées, avec relevé exact des
  réponses, et la difficulté survit à un redémarrage du serveur d'essai.

**Vérification en jeu.** Serveur d'essai, de nuit, en survie, l'enfant connecté :

1. Sans coffre, attendre qu'un monstre apparaisse près du joueur (touche `F3` non
   nécessaire, il suffit de le voir).
2. Depuis Telegram, envoyer `/coffre 348 ÷ 12` : en quelques secondes, le monstre disparaît.
3. Ouvrir le coffre en suivant le mode d'emploi du chat : en quelques secondes, les
   monstres peuvent réapparaître.
4. Envoyer `/coffre division 2`, résoudre le premier : pas de monstre entre les deux coffres.

### Phase 2 : l'enfant est prévenu, la doc suit

**Contexte.** `ModePaisible.appliquer` renvoie la difficulté quand elle vient de changer.
Rien ne change si elle était déjà bonne, donc aucun message à un simple redémarrage.

**Fichiers.** `games/paisible.py`, `orchestrateur.py`, `tests/test_paisible.py`,
`config.example.toml`, `docs/installation.md`, `docs/architecture.md`.

**Conception.**

- `ModePaisible` reçoit le pseudo, et `appliquer` un argument `en_ligne`. S'il vient de
  changer la difficulté et que l'enfant est connecté, il envoie via `jeu.dire` :
  - en passant en paisible : « Les monstres s'endorment tant qu'un coffre t'attend. » ;
  - en revenant à la normale : « Plus de coffre : les monstres sont de retour ! ».
- Le message de passage en paisible part après l'annonce du coffre, déjà envoyée plus
  tôt dans le tour. Aucun chiffre ni réponse dans ces phrases.
- `config.example.toml` : section `[paisible]` commentée.
- `docs/installation.md` : ce que voit l'enfant, le réglage, et l'avertissement sur le
  `difficulty` manuel écrasé.

**Vérification automatique.**

- `uv run pytest` et `uv run ruff check` passent.
- Avec un faux jeu : un message à chaque bascule enfant connecté, aucun enfant
  déconnecté, aucun quand la difficulté était déjà la bonne.
- Aucun message ne contient de réponse d'exercice.

**Vérification en jeu.** Serveur d'essai, le joueur connecté :

1. Envoyer `/coffre 348 ÷ 12` : après l'annonce du coffre, le chat affiche que les
   monstres s'endorment.
2. Ouvrir le coffre : le chat affiche que les monstres sont de retour.
3. Se déconnecter, envoyer un coffre, se reconnecter : le rappel du coffre apparaît, sans
   message sur les monstres.
