# Chaque coffre du soir réserve une surprise : des cadeaux tirés au hasard

> **Reprise de session** : lire « Pourquoi », « Décisions actées » et « Statut »,
> puis aller à la première phase non terminée.

## Pourquoi

Tous les coffres donnent la même chose. La récompense est une valeur unique de
`config.toml` (`src/config.rs:214-229`, 3 diamants par défaut), recopiée telle quelle sur
chaque exercice mis en file (`src/orchestrateur.rs:537-538`, `:169-170`, `:1261-1262`,
`src/cli.rs:611-612`) et posée dans une seule case du coffre
(`src/games/coffre_du_soir.rs:306-309`). Le module le dit lui-même : « identique pour tous
les coffres » (`src/games/coffre_du_soir.rs:70`).

Avec une séance de dictée quotidienne, l'enfant ouvre plusieurs coffres par jour : au
troisième lot de 3 diamants, il sait ce qu'il y a dedans avant d'ouvrir. Pour un enfant
avec un TDAH, la surprise compte autant que le butin : c'est elle qui donne envie d'aller
au coffre suivant.

**Cible** : chaque coffre contient plusieurs cadeaux différents, tirés au hasard dans une
liste que les parents règlent dans `config.toml`, où chaque cadeau a un poids (les
courants sortent souvent, les rares de temps en temps) ; les parents lisent sur Telegram ce
que l'enfant a gagné quand il ouvre.

## Décisions actées

| Sujet | Décision |
|---|---|
| Source des cadeaux | Une liste `cadeaux` dans `[coffre]` de `config.toml`. Chaque entrée : `objet`, `nombre`, `poids`, et un `nom` facultatif pour Telegram. |
| Tirage | Pondéré, sans remise : deux fois plus de chances pour un poids double, jamais deux fois le même cadeau dans un coffre. |
| Nombre de cadeaux par coffre | Réglage `[coffre] cadeaux_par_coffre`, 2 par défaut. |
| Liste par défaut | Livrée dans l'agent, utilisée quand `config.toml` ne donne pas de liste. Proposition plus bas, à corriger à la relecture. |
| Ce que voient les parents | Le message « Coffre n°N ouvert… » se termine par les cadeaux gagnés. Rien à la mise en file, rien dans `/file`. |
| Ce que voit l'enfant | Rien de nouveau dans le chat : la surprise est dans le coffre. |
| Moment du tirage (hypothèse) | À la première pose du coffre, pas à la mise en file. Le résultat est écrit en base : un coffre disparu puis reposé contient les mêmes cadeaux, l'enfant ne peut pas « relancer les dés ». Une liste modifiée dans `config.toml` vaut donc aussi pour les coffres déjà en file. |
| Anciennes clés (hypothèse) | `recompense` et `recompense_nombre` restent acceptées pour ne pas empêcher l'agent de démarrer, mais ne sont plus lues : un avertissement au démarrage dit de les retirer. Après la mise à jour, le hasard s'applique donc sans toucher au `config.toml` du homelab. |
| Bornes (hypothèse) | `cadeaux_par_coffre` entre 1 et 5 (les cases 12 à 16 de la rangée du milieu, remplies à partir de la case 13 ; la preuve garde la case 11). La liste doit contenir au moins `cadeaux_par_coffre` entrées. `nombre` entre 1 et 64, `poids` entier entre 1 et 100. |
| Un objet, une entrée (hypothèse) | Le même `objet` ne peut pas figurer deux fois dans la liste : c'est ce qui garantit des cadeaux différents dans un coffre. |
| Coffres déjà posés ou ouverts (hypothèse) | La migration recopie leur ancienne récompense comme unique cadeau : rien ne change pour eux. |

### La liste par défaut proposée

Des objets utiles à un enfant qui construit et explore, rien qui casse le jeu (pas
d'élytres, pas de totem). Identifiants vérifiés sur Minecraft Java 26.3 (Paper) : chacun a
été posé dans un coffre du serveur d'essai et relu.

| Cadeau | `objet` | `nombre` | `poids` |
|---|---|---|---|
| diamant | `minecraft:diamond` | 3 | 10 |
| émeraude | `minecraft:emerald` | 5 | 10 |
| lingot de fer | `minecraft:iron_ingot` | 8 | 10 |
| lingot d'or | `minecraft:gold_ingot` | 4 | 8 |
| carotte dorée | `minecraft:golden_carrot` | 8 | 8 |
| gâteau | `minecraft:cake` | 1 | 8 |
| éclat d'améthyste | `minecraft:amethyst_shard` | 6 | 6 |
| laisse | `minecraft:lead` | 2 | 5 |
| étiquette | `minecraft:name_tag` | 1 | 5 |
| selle | `minecraft:saddle` | 1 | 4 |
| longue-vue | `minecraft:spyglass` | 1 | 3 |
| disque de musique | `minecraft:music_disc_cat` | 1 | 2 |
| pomme dorée | `minecraft:golden_apple` | 1 | 2 |

Avec ces poids, sur 2 cadeaux par coffre, un cadeau rare (poids 2 ou 3) sort environ une
fois sur six.

Dans `config.toml` :

```toml
[coffre]
cadeaux_par_coffre = 2
niveaux_offerts = 3
cadeaux = [
  { objet = "minecraft:diamond", nombre = 3, poids = 10, nom = "diamant" },
  { objet = "minecraft:saddle",  nombre = 1, poids = 4,  nom = "selle" },
  # ...
]
```

### Ce que ces décisions renversent

- Spec du coffre du soir (`docs/specs/done/2026-11-coffre-du-soir.md:48`) : « Récompense :
  valeur par défaut dans `config.toml` (3 diamants), identique pour tous les coffres ».
  Elle devient une liste tirée au hasard.
- L'oracle Python (`tests/donnees/oracle/config.json`, `stockage.json`,
  `coffre_du_soir.json`, `commandes_parents.json`) fige `recompense` / `recompense_nombre`
  et la commande `item replace … container.13 with minecraft:diamond 3`. Les tests
  `oracle_python` concernés casseront : changement de comportement voulu, à assumer dans
  les commits des phases 1 et 2 en mettant à jour les relevés.

## Hors périmètre

- Régler les cadeaux depuis Telegram, ou par exercice.
- Un nombre tiré dans une fourchette (« 1 à 5 diamants ») : `nombre` est fixe par entrée.
- Objets enchantés ou à composants (épée affûtée, livre enchanté) : `objet` reste un
  identifiant simple, comme aujourd'hui (`src/config.rs:12-13`).
- Annoncer le cadeau à l'enfant dans le chat, ou fêter un cadeau rare (son, feu d'artifice).
- Des listes différentes selon la matière ou la difficulté de l'exercice.
- Garantir un cadeau rare au bout de N coffres : le tirage n'a pas de mémoire.
- Les autres jeux : seul le coffre du soir (et la dictée, qui est le même coffre) change.

## Conséquences acceptées

- **Identifiant mal écrit.** `minecraft:diamant` passe la vérification de forme mais le
  serveur refuse la commande : la case reste vide. C'est déjà vrai aujourd'hui avec
  `recompense`. La liste par défaut est vérifiée sur le serveur d'essai ; une liste écrite
  par les parents ne l'est pas.
- **Contournement.** Le tirage est fait une fois et gardé en base : casser, faire
  disparaître ou attendre le coffre ne change pas son contenu. L'enfant ne connaît les
  cadeaux qu'en ouvrant, donc en ayant répondu.
- **Coffre non vidé.** Avec plusieurs cadeaux, l'enfant en laissera plus souvent un dans le
  coffre ; le suivant attend qu'il soit vide (règle existante, inchangée).

## Vie privée

Rien. Le tirage est local, la liste des cadeaux ne part que vers le serveur Minecraft
(RCON) et vers les parents (Telegram, canal déjà configuré). Aucun LLM n'est concerné.

## Statut

**Branche** : `feat/cadeaux-au-hasard`, pas encore créée, `/next` l'ouvre à la première phase.

- [x] **Phase 1 : la liste et le tirage**
- [x] **Phase 2 : les cadeaux dans le coffre**
- [ ] **Phase 3 : les cadeaux dits aux parents, et la doc**

La phase 2 a besoin de la liste et du tirage de la phase 1 ; la phase 3 lit en base ce que
la phase 2 y écrit. Après la phase 1 seule, le comportement en jeu n'a pas changé.

## Conception

### Phase 1 : la liste et le tirage

**Contexte.** `[coffre]` est la structure `Coffre` de `src/config.rs:214-250`, avec
`deny_unknown_fields`. `Alea` (`src/alea.rs`) sait déjà tirer un indice selon des poids
(`pondere`, `src/alea.rs:41-43`) et se rejoue avec une graine. Cette phase n'est branchée
sur rien : le coffre pose toujours `recompense`.

**Fichiers.** `src/config.rs`, nouveau `src/games/cadeaux.rs`, `src/games/mod.rs`,
`tests/config.rs`, nouveau `tests/cadeaux.rs`.

**Conception.**
- `Cadeau { objet, nombre, poids, nom: Option<String> }` (`nombre` et `poids` valent 1
  s'ils ne sont pas écrits) et, dans `Coffre`, `cadeaux:
  Vec<Cadeau>` (défaut : la liste proposée) et `cadeaux_par_coffre: i64` (défaut 2).
- `Coffre::verifier` refuse, avec un message pour la personne qui installe : objet de forme
  invalide (`OBJET`), `nombre` hors 1..=64, `poids` hors 1..=100, objet en double (avec ou sans `minecraft:`),
  `cadeaux_par_coffre` hors 1..=5, liste plus courte que `cadeaux_par_coffre`, `nom` vide ou
  de plus de 30 caractères (comptés avec `chars()`).
- `cadeaux::tirer(liste, combien, &mut Alea) -> Vec<CadeauTire>` (`objet`, `nombre`,
  `nom`) : `combien` tirages avec `Alea::pondere`, le poids d'un cadeau déjà sorti mis à
  zéro. Aucun `unwrap` : une liste trop courte rend ce qu'elle peut.
- `recompense` et `recompense_nombre` restent dans `Coffre` à cette phase.

**Vérification automatique.** `cargo test`, `cargo clippy --all-targets -- -D warnings`,
`cargo fmt --check`. Les tests montrent :
- sur 500 graines, toujours `combien` cadeaux, tous différents, tous issus de la liste ;
- la même graine donne le même tirage ;
- sur 10 000 tirages d'un cadeau, un poids 10 sort entre 4 et 6 fois plus qu'un poids 2 ;
- chaque refus de `verifier` a son test, la liste par défaut passe `verifier` ;
- `oracle_python` de la config reste vert sans retouche : aux valeurs par défaut, les deux
  nouvelles clés ne s'écrivent pas (même règle que `[voix]` et `[dictee]`).

### Phase 2 : les cadeaux dans le coffre

**Contexte.** La récompense voyage aujourd'hui dans `Demande`
(`src/storage/sqlite.rs:349-353`), est écrite à la mise en file (`:526-537`) et posée par
`commandes_pose` (`src/games/coffre_du_soir.rs:306-309`) dans la case 13 ; la preuve est en
case 11. Le schéma en est à la version 5 (`MIGRATIONS`, `src/storage/sqlite.rs:52`).
Charger `/nouveau-jeu` et `/essai-en-jeu`.

**Fichiers.** `src/storage/sqlite.rs`, `src/games/coffre_du_soir.rs`,
`src/orchestrateur.rs`, `src/cli.rs`, `src/config.rs`, les quatre `config*.toml` d'exemple
et locaux, les tests qui construisent une `Demande` (`tests/stockage.rs`, `rapport.rs`,
`paisible.rs`, `orchestrateur.rs`, `seance.rs`, `pixel.rs`, `parole.rs`),
`tests/coffre_du_soir.rs`, les relevés d'oracle touchés.

**Conception.**
- Migration 6 : colonne `cadeaux TEXT NOT NULL DEFAULT '[]'` (JSON, liste de
  `{objet, nombre, nom}`). Pour les lignes `pose` et `ouvert`, elle est remplie avec
  l'ancienne récompense ; les lignes `en_file` gardent `[]`, « pas encore tiré ». Les
  colonnes `recompense` et `recompense_nombre` sont retirées (`ALTER TABLE … DROP COLUMN`,
  accepté par le SQLite embarqué par `rusqlite`).
- `Demande` perd `recompense` et `recompense_nombre` ; `ExerciceStocke` gagne `cadeaux`.
  La configuration du coffre quitte du même coup `CommandesParents` et les séances : seule
  la boucle de surveillance en a besoin (`Orchestrateur::avec_cadeaux`).
- À la pose, si `cadeaux` est vide, l'orchestrateur tire (`cadeaux::tirer`, avec son
  `Alea`), écrit en base, puis pose. Un coffre reposé relit la base et ne retire pas.
- `commandes_pose` : un `item replace block C container.<case> with <objet> <nombre>` par
  cadeau, après le verrou, avant la preuve. Les cases se remplissent dans l'ordre 13, 14,
  12, 15, 16 : un cadeau seul reste en case 13, comme l'ancienne récompense. Vérifié sur
  Minecraft Java 26.3 (Paper).
- `Coffre` (config) : `recompense` et `recompense_nombre` deviennent facultatives et
  ignorées ; présentes, elles déclenchent un avertissement au démarrage (« ces clés ne
  servent plus, voir `cadeaux` »). Elles disparaissent des fichiers d'exemple, remplacées
  par la liste commentée.
- En-tête de `coffre_du_soir.rs` (commandes, paragraphe « Récompense ») mis à jour.

**Vérification automatique.** Les trois commandes cargo, plus :
- une base en version 5 avec un coffre en file, un posé, un ouvert migre : le posé et
  l'ouvert ont leur ancienne récompense comme cadeau, celui en file a `[]` ;
- avec une graine fixée, la pose émet autant d'`item replace` que `cadeaux_par_coffre`,
  dans des cases distinctes de 12 à 16, verrou posé avant ;
- un coffre disparu puis reposé émet les mêmes cadeaux, sans nouveau tirage ;
- un `config.toml` avec les anciennes clés se charge ;
- `/essai-en-jeu` : chaque objet de la liste par défaut est posé dans un coffre du serveur
  d'essai et relu avec `data get block` ; trois coffres posés à la suite n'ont pas tous le
  même contenu.

**Vérification en jeu.** Sur le serveur d'essai, résoudre trois coffres d'affilée :
chacun contient deux cadeaux différents au milieu du coffre, à droite du papier « Preuve »,
et les trois coffres ne se ressemblent pas. Le détail des gestes sera donné par `/next`.

### Phase 3 : les cadeaux dits aux parents, et la doc

**Contexte.** Le message d'ouverture est construit dans `surveiller`
(`src/orchestrateur.rs:321-329`) : « Coffre n°12 ouvert en 4 min, sans aide (348 ÷ 12 →
29). ». Les cadeaux tirés sont en base depuis la phase 2.

**Fichiers.** `src/orchestrateur.rs`, `src/games/cadeaux.rs`, `tests/orchestrateur.rs`,
`tests/cadeaux.rs`, `docs/installation.md` (ligne 74),
`tests/donnees/oracle/commandes_parents.json` si le relevé couvre ce message.

**Conception.**
- `cadeaux::dire(&[CadeauTire]) -> String` : « 3 × diamant, 1 × selle ». Le `nom` de la
  config s'il existe, sinon l'identifiant sans `minecraft:`. Pas d'accord au pluriel.
- Le message d'ouverture se termine par « Cadeaux : 3 × diamant, 1 × selle. », après les
  essais. Un coffre sans cadeau connu n'ajoute rien.
- `docs/installation.md` : `[coffre]` décrit la liste, les poids, `cadeaux_par_coffre`, et
  dit de retirer les deux anciennes clés. Le `README.md` pointe déjà vers cette spec.

**Vérification automatique.** Les trois commandes cargo ; les tests montrent le message
complet avec deux cadeaux, avec et sans `nom`, et un coffre migré (un seul cadeau).

**Vérification en jeu.** Sur le serveur d'essai avec Telegram réglé, ouvrir un coffre : le
message reçu liste exactement ce qui se trouvait dans le coffre.
