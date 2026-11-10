---
name: next
description: Livre la phase suivante d'une spec Mine de rien : lit docs/specs/open/, prend la première phase non terminée, se place sur la branche de la spec, la code, fait tourner ses vérifications automatiques (cargo test, clippy, serveur d'essai), coche le statut, commit, puis s'arrête avec le scénario à jouer en jeu. À invoquer explicitement (/next) après qu'une spec a été validée par /feature.
---

# Livrer la phase suivante

Une phase, un commit, un arrêt. C'est le rythme, et il n'est pas négociable
tacitement : enchaîner deux phases parce que « la deuxième est petite »
supprime la porte de test. Ici la porte a deux battants : la vérification
automatique, que `/next` fait, et la partie jouée, que fait le parent.

## 1. Retrouver où on en est

Chercher dans `docs/specs/open/`, et seulement là (`done/` est l'archive), la
spec qui a des phases non cochées.

- **Aucune** → il n'y a rien à reprendre. Le dire, et proposer `/feature`. Si
  `open/` contient une spec dont toutes les phases sont cochées, le dire aussi :
  elle attend `/close`, pas du code.
- **Plusieurs** → demander laquelle. Ne pas deviner.
- **Une** → c'est la spec active.

Lire **« Reprise de session », « Pourquoi », « Décisions actées », « Hors
périmètre » et « Vie privée »** avant la phase elle-même. Le « pourquoi » est ce
qui permet de trancher les micro-décisions que la spec ne descend pas.

Vérifier les dépendances annoncées dans « Statut » : si la phase visée dépend
d'une phase non terminée, le dire plutôt que de contourner.

## 2. Se placer sur la branche de la spec

La ligne « Branche » de « Statut » dit où le travail s'écrit. **Toute phase se
livre sur cette branche**, jamais sur `main` ni sur la branche où on se trouve
par hasard.

```bash
git status --porcelain          # doit être vide, hors fichiers non suivis
git switch <branche>            # si elle existe
git switch -c <branche> main    # sinon : elle naît de main, avec la spec
```

- **Arbre de travail sale** (fichiers suivis modifiés) → s'arrêter et le dire.
  Ni `stash` ni commit de rattrapage en silence.
- **Pas de ligne « Branche »** → demander le nom, l'écrire dans la spec, et
  continuer.
- **Branche existante mais en retard sur `main`** → le signaler dans le rapport
  de fin. Ne pas rebaser ni fusionner sans qu'on le demande.

Dire sur quelle branche on est avant d'écrire la première ligne.

## 3. Coder cette phase, et rien d'autre

Si la phase ajoute un générateur d'exercice, suivre `/nouvel-exercice`. Si elle
ajoute une mise en scène dans le jeu, suivre `/nouveau-jeu`. Ces skills portent
les règles qui ne se discutent pas (réponse calculée par le générateur, échappement des
commandes, garde-fous anti-contournement).

Le « Hors périmètre » est contraignant. Un problème réel croisé en chemin mais
hors périmètre se **note** dans le rapport de fin, il ne se corrige pas au
passage.

Les règles de `CLAUDE.md` s'appliquent à chaque ligne, en particulier : le LLM
ne calcule jamais une réponse, et rien de nouveau ne sort de la maison sans que
la section « Vie privée » de la spec l'ait prévu. Une phase qui découvre qu'elle
a besoin d'un appel réseau non prévu s'arrête et le dit.

Si la conception de la spec se révèle fausse une fois dans le code : s'arrêter
et le dire. Une spec irréalisable est une information, pas un obstacle à
contourner en silence.

## 4. Vérifier

Toujours :

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Puis les **vérifications automatiques écrites dans la phase**. Si la phase
touche au jeu, cela passe par `/essai-en-jeu` sur le serveur d'essai, jamais sur
le monde réel de l'enfant.

Rapporter les résultats tels quels. Un test qui échoue se montre avec sa
sortie ; une vérification sautée se dit, avec sa raison (serveur d'essai
indisponible, EULA non acceptée…). « Ça devrait marcher » n'est pas un résultat.

## 5. Cocher, commiter

La case de « Statut » se coche **dans le même commit** que le code de la phase.
Si `/essai-en-jeu` a validé des commandes que la spec marquait « à vérifier »,
retirer la mention dans la spec, dans ce même commit.

```
feat(<zone>): <sujet en français, au présent>

<Le pourquoi, ce que ça renverse, ce qu'on a écarté. Pas la paraphrase du diff.>

Spec: <slug>, phase N
```

- `<zone>` reprend l'arborescence : `exercices`, `jeux`, `jeu`, `canal`, `llm`,
  `stockage`, `datapack`, `docker`.
- Le trailer `Spec: <slug>, phase N` est obligatoire : sans issues, c'est lui
  que `/close` cherche pour prouver qu'une phase a bien été livrée.

Ne pas pousser sans qu'on le demande, et ne pas quitter la branche de la spec :
le test en jeu se fait dessus.

## 6. S'arrêter pour le test en jeu

Rendre la main avec :

- la branche sur laquelle c'est commité, et si elle vient d'être créée ;
- ce que la phase a livré, en deux ou trois lignes ;
- les vérifications automatiques et leur résultat ;
- **le scénario à jouer**, recopié de la spec et précisé : comment lancer le
  serveur, où se téléporter, quel geste faire, ce qui doit se passer en jeu et
  sur Telegram. Assez concret pour être suivi par quelqu'un qui n'a pas lu le
  code ;
- ce qui a été noté en chemin et laissé de côté ;
- la phase suivante, nommée, sans la commencer. Si c'était la dernière, dire que
  la spec est complète et proposer `/close` une fois le jeu testé. `/next` ne
  déplace jamais une spec vers `done/`.
