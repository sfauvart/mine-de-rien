---
name: nouvel-exercice
description: Ajoute un générateur d'exercice Mine de rien pour une compétence de CM2 (maths, anglais, français, autres matières) : module déterministe dans src/exercises/, réponse calculée par le code Rust, niveaux de difficulté, format de réponse tapable sur une enclume, tests de propriétés sur des centaines de graines. À utiliser dès qu'on ajoute ou modifie un générateur, y compris pendant une phase de /next.
---

# Ajouter un générateur d'exercice

Un générateur transforme une graine aléatoire et un niveau en un `Exercice` :
une compétence, un énoncé neutre, **la** réponse attendue, et les données brutes
qui serviront à l'habillage. C'est la seule pièce du projet qui décide de ce qui
est juste. Un générateur faux, c'est un enfant corrigé à tort, et il ne fera plus
confiance au jeu.

## Règles qui ne se discutent pas

- **La réponse est calculée par le générateur.** Jamais par un LLM, jamais recopiée d'une
  sortie de modèle, jamais devinée depuis l'énoncé. Le LLM habillera l'énoncé plus
  tard, à partir de `donnees`, et ne verra pas la réponse s'il n'en a pas besoin.
- **Déterministe pour une graine donnée.** Tout tirage passe par l'`Alea` reçu
  (`src/alea.rs`). Aucun autre hasard, aucune date, aucun appel réseau.
- **Une seule écriture de la réponse.** Dans le coffre du soir, Minecraft compare
  le nom de l'objet à la clé caractère pour caractère, sans la tolérance de
  `normaliser`. Si une réponse peut s'écrire de plusieurs façons (`3,5` et `3.5`,
  `1/2` et `0,5`, `twelve` et `12`), soit le générateur évite ce cas, soit
  l'énoncé impose le format (« réponds en chiffres », « avec une virgule »). Le
  dire dans le commentaire d'en-tête du module.
- **Tapable sur une enclume par un enfant.** 50 caractères au plus, idéalement
  moins de 10. Pas de symbole absent d'un clavier français standard, pas d'espace
  en tête ou en fin.
- **Contenu de langue relu par un adulte.** Pour l'anglais ou le français, les
  listes de mots, verbes et phrases vivent dans des fichiers de données versionnés
  (`src/exercises/<matiere>/donnees/*.toml`, embarqués par `include_str!`), relus par un
  parent. Un LLM peut aider à
  les rédiger hors ligne, jamais les produire à la volée.

## 1. Situer la compétence

- Nommer la compétence par son attendu de fin de CM2 (repères annuels de
  progression du cycle 3, publiés par le ministère sur Eduscol), pas par un nom
  de code. Le commentaire d'en-tête du module cite l'attendu en une phrase.
- Identifiant : `<matiere>/<slug>`, en minuscules avec tirets, par exemple
  `maths/fractions-comparaison` ou `anglais/heure`. Il servira de clé dans
  SQLite, dans les rapports et dans l'arbre de progrès : le choisir stable.
- Vérifier qu'il n'existe pas déjà : `grep -rn 'COMPETENCE' src/exercises/`.

## 2. Écrire le module

`src/exercises/<matiere>/<slug>.rs`, sur le modèle de
`src/exercises/maths/division.rs` : une constante `COMPETENCE` et une fonction
`generer(alea: &mut Alea, niveau: u32) -> Exercice`. Déclarer le module dans le `mod.rs`
de la matière, et l'ajouter au registre (`src/exercises/registre.rs`) si les parents
doivent pouvoir le demander par son nom.

- **Niveau 1** : l'attendu dans sa forme la plus simple, réponse courte et
  unique. C'est celui du coffre du soir.
- **Niveau 2** et suivants : seulement si l'attendu a une vraie progression
  (diviseur à deux chiffres, reste non nul). Un niveau qui ne change que la
  taille des nombres n'en vaut pas la peine.
- **Construire à rebours** quand c'est possible : tirer la réponse d'abord, puis
  fabriquer l'énoncé qui y mène. C'est ce qui garantit une division qui tombe
  juste ou une fraction déjà irréductible.
- `donnees` porte tout ce qu'un habillage pourrait vouloir citer (les nombres,
  le mot, l'unité), sans phrase toute faite.
- L'`enonce` est neutre et compréhensible sans habillage : c'est lui qui
  s'affiche si le LLM est absent ou en panne.

## 3. Tester

`tests/<matiere>_<slug>.rs`. Des tests de **propriétés** sur au moins
200 graines par niveau (`Alea::graine(n)`), pas trois exemples choisis à la main :

- la réponse vérifie la relation mathématique ou linguistique attendue,
  recalculée indépendamment dans le test ;
- `ex.accepte(ex.reponse)` est vrai, et une réponse voisine fausse est refusée ;
- la réponse respecte le format (longueur, caractères autorisés, écriture
  unique) ;
- deux appels avec la même graine donnent le même exercice ;
- les valeurs restent dans les bornes de l'attendu (pas de nombre à 7 chiffres
  en CM2).

```bash
cargo test --test <matiere>_<slug>
cargo clippy --all-targets -- -D warnings
```

## 4. Relier

- Si le datapack d'arbre de progrès existe (`datapacks/mine_de_rien/`), ajouter
  le nœud de la compétence. Sinon, ne rien créer : ce sera le travail de la spec
  qui l'introduit.
- Montrer dans le rapport trois exercices générés par niveau (graines 1, 2, 3),
  énoncé et réponse, pour qu'un parent juge d'un coup d'œil si c'est du CM2.
