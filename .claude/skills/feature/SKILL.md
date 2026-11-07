---
name: feature
description: Démarre un chantier Mine de rien par le rituel questions → spec → validation, sans écrire de code. Produit une spec versionnée dans docs/specs/open/, découpée en phases autonomes avec leurs critères de vérification (automatiques et en jeu), et la commite sur main. À invoquer explicitement (/feature) quand on ouvre un chantier, pas pour un correctif isolé.
---

# Démarrer une feature

Le but est qu'on soit d'accord **par écrit** sur ce qu'on va faire avant qu'une
ligne soit écrite, et que cet écrit survive à la fin de la session.

**Tu ne codes rien pendant cette skill.** Aucun `Edit` ni `Write` en dehors de la
spec et des liens qui la citent. Si l'envie de « juste montrer un bout » se
présente, elle appartient à la spec, pas à l'éditeur.

Le projet n'a pas de forge ni d'issues : **la spec est la seule source de
vérité** sur ce qui est prévu, fait ou abandonné. Elle doit donc tout porter.

## 1. Se renseigner avant de demander

Une question dont la réponse est dans le dépôt fait perdre du temps. Avant de
poser quoi que ce soit :

- lire `CLAUDE.md`, puis `docs/architecture.md` (décisions prises, ordre de
  construction) et `docs/idees.md` (les jeux et ce qui a été refusé) ;
- lister `docs/specs/open/` : un chantier déjà ouvert peut recouvrir la demande ;
- `git log --oneline -20` pour voir ce qui vient d'être livré ;
- lire le code que la demande touche, assez pour citer des `fichier.py:ligne`.

## 2. Poser les questions qui changent le travail

**Trois à cinq, pas davantage.** Une question mérite d'être posée si deux
réponses plausibles conduisent à un travail différent. Tout le reste se tranche
seul, et s'annonce dans la spec comme une hypothèse.

`AskUserQuestion` quand les options sont fermées, la prose quand la question est
ouverte.

Les axes qui, sur ce projet, cachent le plus souvent un désaccord :

- **La portée négative.** Qu'est-ce qu'on ne fait *pas* ? Quelle matière, quel
  jeu, quel canal reste hors du chantier ?
- **Ce que voit l'enfant, ce que voient les parents.** Qui reçoit quoi, dans le
  jeu et sur Telegram, et à quel moment.
- **Ce qui sort de la maison.** Le chantier ajoute-t-il un appel réseau, un
  fournisseur, une donnée envoyée à un LLM distant ? Si oui, quelle alternative
  locale, et qu'est-ce qui est pseudonymisé ?
- **Le contournement.** Un enfant de 10 ans cherchera le raccourci : casser le
  bloc, lire la réponse ailleurs, répondre au hasard. Comment le jeu y résiste,
  ou pourquoi ce n'est pas grave.
- **La vérification.** À quoi on verra que c'est fini : ce qu'un test
  automatique ou le serveur d'essai prouve, et ce qui ne se juge qu'en jouant.

## 3. Écrire la spec

Dans `docs/specs/open/AAAA-MM-<slug>.md`. `open/` contient les specs en cours,
`done/` celles closes par `/close`.

```markdown
# <Titre : ce que la feature change pour l'enfant ou les parents, pas son nom de code>

> **Reprise de session** : lire « Pourquoi », « Décisions actées » et « Statut »,
> puis aller à la première phase non terminée.

## Pourquoi

Le problème, avec des preuves : `fichier.py:120-134`, un comportement observé
en jeu, un attendu du programme de CM2. Pas de généralités. Si le problème ne
peut pas être montré, il n'est peut-être pas encore compris.

**Cible** : une phrase qui dit l'état d'arrivée.

## Décisions actées

| Sujet | Décision |
|---|---|
| … | … |

### Ce que ces décisions renversent

Les points de `docs/architecture.md` ou d'une spec antérieure qui sont
contredits, et pourquoi. Omettre la section s'il n'y en a pas.

## Hors périmètre

Ce que cette feature ne fera pas, et ce qui a été envisagé puis écarté. Une
ligne chacun.

## Vie privée

Ce qui sort de la maison avec ce chantier, vers qui, et sous quelle forme.
« Rien » est une réponse valable, et la plus fréquente.

## Statut

**Branche** : `feat/<slug>`, pas encore créée, `/next` l'ouvre à la première phase.

- [ ] **Phase 1 : <nom>**
- [ ] **Phase 2 : <nom>**

Les dépendances entre phases, en une phrase.

## Conception

### Phase 1 : <nom>

**Contexte.** Ce qu'il faut savoir pour la reprendre à froid.
**Fichiers.** Ceux qui bougent.
**Conception.** Le comment, au niveau où une décision se discute.
**Vérification automatique.** Les commandes à lancer (`uv run pytest`, scénario
`/essai-en-jeu`) et ce qu'elles doivent montrer.
**Vérification en jeu.** Le scénario que le parent jouera : où aller, quel geste
faire, ce qui doit se passer. Omettre si la phase ne touche pas au jeu.
```

Cinq règles de fond :

- **La spec nomme sa branche.** `feat/<slug>` par défaut, avec le slug du
  fichier. Toute phase livrée par `/next` s'écrit sur cette branche ; c'est
  `/next` qui la crée à la première phase, depuis `main`.
- **Une phase est autonome.** Elle se reprend dans une session neuve, sans avoir
  lu la conversation qui l'a produite.
- **Une phase tient dans un commit relisible.** Un nouveau générateur, un
  nouveau jeu, un adaptateur : une phase chacun. Si une phase demande les deux
  skills `/nouvel-exercice` et `/nouveau-jeu`, elle est probablement trop grosse.
- **Les critères de vérification s'écrivent maintenant**, pas au moment de
  livrer. C'est ce qui empêche « est-ce que c'est fini ? » de devenir un
  jugement au doigt mouillé.
- **Toute commande Minecraft citée dans la spec est marquée « à vérifier sur la
  version du serveur »** tant que `/essai-en-jeu` ne l'a pas exécutée. La
  syntaxe change d'une version à l'autre.

## 4. Faire valider le document, pas le message

Annoncer la spec écrite et demander une relecture **du fichier**. Les
corrections se font dans le fichier. Ne pas passer à l'étape suivante avant
l'accord.

## 5. Relier et commiter, sur `main`

Si la spec correspond à une ligne de « Ordre de construction » dans
`docs/architecture.md` ou à un jeu du tableau du `README.md`, y ajouter le lien
vers la spec.

Commiter **sur `main`** : la spec précède la branche, et la branche naîtra de
`main` avec la spec dedans. Si on n'est pas sur `main`, le dire avant de
commiter plutôt que de poser la spec sur une branche étrangère.

```
docs(specs): <titre de la feature>

<Pourquoi ce chantier, en quelques lignes.>
```

## 6. S'arrêter

Résumer les phases, la branche prévue, et proposer `/next`. Ne pas enchaîner sur
le code dans la foulée : la spec vient d'être écrite, elle mérite d'être lue à
tête reposée.
