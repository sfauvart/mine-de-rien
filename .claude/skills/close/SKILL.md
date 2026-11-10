---
name: close
description: Clôt une spec Mine de rien : vérifie que chaque phase est cochée et porte son commit « Spec: », fait confirmer que les scénarios en jeu ont été joués, déplace la spec de docs/specs/open/ vers docs/specs/done/, met à jour les liens qui la citent et commit. Ne fusionne pas. À invoquer explicitement (/close [slug]) après avoir joué la dernière phase livrée par /next.
---

# Clore une spec

Une spec close est une spec qu'on ne rouvrira pas : ses phases sont livrées,
vérifiées et jouées, ou abandonnées en toutes lettres. La décision appartient à
celui qui a joué, pas à `/next`. C'est pour ça que le déplacement vers `done/`
est un geste séparé, après la porte de test.

**Aucun code ne s'écrit pendant cette skill.** Si la clôture révèle un manque,
c'est une phase de plus, ou une nouvelle `/feature`.

## 1. Choisir la spec

L'argument, s'il y en a un, est un slug (`/close coffre-du-soir`) : la spec est
`docs/specs/open/*-<slug>.md`.

Sans argument, chercher dans `docs/specs/open/` :

- **Une seule spec dont toutes les phases sont cochées** → c'est elle.
- **Plusieurs, ou aucune entièrement cochée** → demander laquelle. Ne pas
  deviner, et ne pas clore la plus récente par défaut.

## 2. Se placer sur la branche, et dire si elle est fusionnée

La ligne « Branche » de « Statut » dit où on travaille. `git switch` dessus,
arbre de travail propre exigé. Puis :

```bash
git branch --merged main | grep -x "  <branche>"
```

Si la branche **n'est pas fusionnée** dans `main`, le dire dans le rapport de
fin : la clôture est commitée sur la branche, et `main` ne verra la spec dans
`done/` qu'à la fusion. Ce n'est pas bloquant. Ne pas fusionner soi-même.

## 3. Vérifier les phases

Lire « Statut ». Si des phases restent non cochées, demander confirmation avec
`AskUserQuestion` : clore vaut **abandon** de ces phases, et la spec doit le
dire, sous la liste des phases :

```markdown
Close le AAAA-MM-JJ. Phases 4 et 5 abandonnées : <raison, une ligne>.
```

Sans confirmation explicite, s'arrêter là.

Pour chaque phase cochée, retrouver son commit :

```bash
git log main..<branche> --format='%h %s%n%b' | grep -B3 "^Spec: <slug>, phase N$"
```

Une phase cochée **sans** commit `Spec:` correspondant se signale dans le
rapport de fin (livrée hors rituel, trailer oublié). Ne pas réécrire
l'historique pour l'ajouter.

## 4. Confirmer la partie jouée

Pour les phases qui ont une « Vérification en jeu », demander avec
`AskUserQuestion` si les scénarios ont été joués et s'ils se sont passés comme
prévu. Trois issues :

- **Joués, conformes** → continuer.
- **Joués, avec des écarts** → noter les écarts dans la spec, sous une section
  `## Retours de jeu`, une ligne chacun, datée. Si un écart est bloquant,
  s'arrêter et proposer une phase de plus plutôt que de clore.
- **Pas joués** → s'arrêter. Clore une spec dont le jeu n'a pas été essayé,
  c'est archiver une hypothèse.

Si l'utilisateur rapporte ce qu'en a pensé l'enfant, le noter aussi dans
« Retours de jeu ». C'est l'information la plus utile pour la feature suivante.

## 5. Déplacer, relier, dater

```bash
git mv docs/specs/open/AAAA-MM-<slug>.md docs/specs/done/AAAA-MM-<slug>.md
grep -rn "AAAA-MM-<slug>.md" --include=*.md . | grep -v "^./target/"
```

Corriger chaque lien trouvé hors de la spec elle-même. `README.md`,
`docs/architecture.md` et `docs/idees.md` sont les citeurs habituels. Si la spec
livre un jeu du tableau du `README.md`, passer son état de « à faire » à
« livré ».

Ajouter dans « Statut », sous la liste des phases, `Close le AAAA-MM-JJ.` (ou la
ligne d'abandon de l'étape 3, qui la contient déjà).

## 6. Commiter

```
docs(specs): clôt <titre court de la feature>

<Ce que la feature a livré, en deux lignes, et ce qu'on a laissé de côté.>

Spec: <slug>, clôture
```

Ne pas pousser sans qu'on le demande, et rester sur la branche de la spec.

## 7. S'arrêter

Rendre la main avec : la branche et son état de fusion, la spec déplacée, les
liens corrigés, les phases sans commit `Spec:` s'il y en a, les retours de jeu
notés, et ce qui a été abandonné s'il y a lieu. Proposer la fusion dans `main`
sans la faire.
