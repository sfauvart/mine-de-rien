# L'enfant prépare sa dictée de la semaine en l'écoutant dans le jeu

> **En attente.** Cette spec a été écrite quand l'agent était en Python. Depuis le passage à
> Rust ([spec](../open/2026-11-agent-en-rust.md)), ses chemins `src/mine_de_rien/….py`, ses
> commandes `uv run pytest` et son choix d'une « bibliothèque Python qui écrit du Vorbis »
> ne tiennent plus : la reprendre par `/feature` avant d'en livrer la première phase.

> **Reprise de session** : lire « Pourquoi », « Décisions actées » et « Statut »,
> puis aller à la première phase non terminée.

## Pourquoi

L'enfant a un TDAH et une dyslexie. À l'école, ses dictées sont des dictées à trous : la
phrase est imprimée, il écrit les mots manquants. La maîtresse donne le texte ou les mots à
préparer à l'avance. Deux dictées corrigées, relevées le 2026-11-10 :

| Dictée | Note | Ce qui a été raté |
|---|---|---|
| n°5 | 13/14 | « campagne », repris. Tous les verbes en *-ent* sont justes. |
| n°6 | 12/20 | lettres finales muettes (« plu », « troi », « vint ») ; pluriel des noms (« persone », et *mètres*, *tours* corrigés avec aide) ; consonnes doubles et accents (« persone », « anne ») ; nombre en lettres (« huit-cent-vint huite ») ; « preste » pour « presque ». |

Mine de rien ne sait rien faire de ces dictées aujourd'hui :

- aucune compétence de français : le registre ne connaît que la division et l'anglais
  (`src/mine_de_rien/exercises/registre.py:28-31`) ;
- aucun son : le seul `playsound` est la cloche du jeu
  (`src/mine_de_rien/games/coffre_du_soir.py:272-275`), et le serveur n'a pas de pack de
  ressources (`docker-compose.yml`) ;
- une clé de coffre n'accepte ni majuscule ni apostrophe (`src/mine_de_rien/saisie.py:34-36`),
  donc ni « Dubaï » ni « s'élève » ;
- la lecture de photo ne recopie que des calculs et des mots anglais
  (`src/mine_de_rien/photo.py:48-54`) ;
- tous les coffres partent dès la demande, l'un après l'autre
  (`src/mine_de_rien/orchestrateur.py:139-155`) : rien ne répartit un travail sur une semaine ;
- les essais ratés sont invisibles : la table `tentatives` existe
  (`src/mine_de_rien/storage/sqlite.py:42-48`) mais rien n'y écrit. On ne peut donc ni montrer
  aux parents ce que l'enfant a écrit, ni faire revenir un mot raté.

**Cible** : un parent envoie une fois le texte de la dictée de vendredi (tapé ou
photographié) ; chaque jour jusqu'à jeudi, à sa première connexion, l'enfant trouve quatre
coffres dont il entend la phrase lue par une voix locale et tape le mot manquant sur
l'enclume ; les parents voient sur Telegram ce qu'il a écrit, et un mot raté revient deux
jours plus tard.

## Décisions actées

| Sujet | Décision |
|---|---|
| Forme du jeu | Le coffre du soir, inchangé dans son principe : un trou = un coffre, la clé est le mot manquant. Pas de nouveau jeu. |
| Ce que l'enfant entend | La **phrase entière**, lue par une voix de synthèse. Jamais le mot isolé : hors de sa phrase, « plus » ou « tours » se prononcent autrement. |
| Ce que l'enfant voit | Dans le chat, la phrase avec le trou (`_____`, toujours cinq tirets : la longueur ne dit rien du mot). Sur le panneau, « Dictée : » suivi du mot d'avant, du trou et du mot d'après. Jamais le mot lui-même, jamais une graphie fausse, même la sienne. |
| Réécouter | Un objet « Écouteur » donné avec la clé : un clic droit rejoue la phrase tout de suite et réaffiche la phrase à trou. La phrase est aussi lue à l'annonce du coffre et au rappel de connexion. |
| Voix | Synthèse **locale**, par un adaptateur `adapters/voix/`. Première implémentation : Piper, voix française, dans un conteneur du compose. Aucune voix distante. |
| Transport du son | Pack de ressources du serveur, construit par l'agent et servi par lui en HTTP sur le réseau de la maison. Un son par phrase, en deux vitesses (normale et lente). Les fichiers portent des numéros, jamais de mots. |
| Mise à jour du pack | Le pack est reconstruit à chaque dictée confirmée. Un coffre de dictée n'est posé que si l'enfant s'est connecté **après** la construction du pack qui contient son son. |
| Choix des trous | Python tire au hasard dans **tout le texte**, comme la maîtresse. Le parent peut imposer des mots, qui sortent en premier. Les mots déjà ratés pèsent plus lourd. Le LLM ne choisit rien. |
| Mots candidats | Tout mot d'au moins deux lettres, apostrophe et trait d'union compris (« s'élève », « huit-cent-vingt-huit »). Exclus : les mots de plus de 50 caractères, ceux qui contiennent un chiffre, et ceux dont la majuscule porte un accent (« Émirats » : difficile à taper). |
| Écriture de la clé | Le mot **exactement comme dans le texte**, majuscule comprise : « Dans » en début de phrase, « Dubaï », « tours ». Aucune règle pour deviner les noms propres. L'apostrophe typographique devient l'apostrophe du clavier. |
| Mode d'emploi | Pour un coffre de dictée : « écris le mot comme dans la phrase, avec sa majuscule s'il en a une ». La description de la clé ne dit plus « en minuscules », c'est le chat qui le précise selon le coffre. |
| Saisie par le parent | `/dictee vendredi <texte>` ou une photo avec « dictée vendredi » en légende. Le jour est un nom de jour (le prochain) ou `JJ/MM`. Une liste de mots sans texte : `/dictee vendredi mots : plus, trois, personnes` ; chaque mot est alors lu seul. |
| Rôle du LLM | Sur une photo, le LLM **local** recopie le texte imprimé, rien d'autre. Le texte tapé ne passe par aucun LLM. |
| Confirmation | Mine de rien renvoie le texte lu, découpé en phrases, et un **aperçu audio** de toute la dictée. Le parent compare à la feuille, écoute, puis `/oui` ou `/non`. C'est la seule barrière contre une faute de transcription, qui deviendrait une clé fausse. |
| Séance | Chaque jour, à la première connexion de l'enfant, 4 coffres de dictée entrent dans la file (réglable de 3 à 5). Du lendemain de la confirmation à la veille de la dictée. Un jour sans connexion n'est pas rattrapé : pas de dette. |
| Essais ratés | L'agent relit le nom de la clé toutes les 3 secondes. Chaque nouveau nom différent de la réponse est une tentative ratée, écrite dans `tentatives`. Vaut pour tous les coffres, maths et anglais compris. |
| Mot raté | Un mot raté au moins une fois revient au plus tôt deux jours après, en priorité. Un mot réussi du premier coup deux fois de suite sort du tirage. Après le jour de la dictée, un coffre par séance peut reprendre un mot encore raté d'une dictée de moins de 30 jours. |
| Aide de Pixel | Trois paliers pour un coffre de dictée : la phrase relue lentement, puis le nombre de lettres, puis la première lettre. Jamais le mot. |
| Rapport | Le rapport Telegram montre les graphies : « personnes : persone, puis juste ». |
| Fuseau | « Chaque jour » se compte dans le fuseau de la maison, nouvelle clé de configuration. Le stockage reste en UTC. |

Hypothèses prises sans poser la question, à corriger dans ce fichier si elles sont fausses :

- l'enfant joue à la maison, sur le même réseau que le serveur : son jeu peut télécharger
  le pack à l'adresse locale de la machine ;
- la feuille donnée par la maîtresse est imprimée et porte le texte complet, sans trous ;
- une séance de dictée s'ajoute aux coffres demandés par `/coffre`, à la suite dans la file.

### Ce que ces décisions renversent

- `docs/idees.md`, idée 1 : « On ne voit pas les tentatives ratées, seulement le temps de
  résolution. » La phase 6 les rend visibles.
- Spec `2026-11-photo-du-cahier` : « Accepter majuscules […] en lisant le nom de la clé […]
  relève du chantier "essais ratés" » et « réserve nommée et réutilisable : écartée ». Ce
  chantier lit le nom de la clé, et une dictée est une réserve gardée jusqu'à son jour. La
  règle « toujours en minuscules » reste vraie pour l'anglais et les maths.
- `docs/architecture.md`, décision 4, « trois adaptateurs » : la voix en fait un quatrième.
- Le coffre du soir posait tout de suite ce qu'on lui demandait : la séance quotidienne
  introduit le premier travail planifié.

## Hors périmètre

- Voix du parent enregistrée en message vocal : plus chaleureuse, mais elle ferait passer
  une voix de la famille par Telegram. À rouvrir si la synthèse déçoit.
- Dictée manuscrite sur papier, corrigée par photo : refusée dès `docs/idees.md`.
- Bartholomé pour l'orthographe : montrer des mots mal écrits à un enfant dyslexique.
- Générateurs d'accords au pluriel et de nombres en lettres, sans texte de la maîtresse :
  utiles, mais ce sont d'autres compétences, une spec à part.
- Dictionnaire français pour signaler au parent un mot douteux dans la transcription.
- Feuille manuscrite ou feuille déjà trouée.
- Jouer hors de la maison : le pack n'est servi que sur le réseau local.
- Plugin Paper pour pousser le pack à un joueur connecté : la reconnexion suffit.
- Rattraper une séance manquée, ou régler le nombre de coffres depuis Telegram.
- Trou de plusieurs mots à la suite, ponctuation à placer.

## Vie privée

- **Vers Telegram** : le texte de la dictée ou sa photo, envoyés par le parent (cadrer sans
  le prénom, comme pour les fiches). En retour, le texte lu, l'aperçu audio en voix de
  synthèse et, c'est nouveau, **ce que l'enfant a tapé** sur la clé, dans les notifications
  et le rapport. Jamais son pseudo.
- **Vers un LLM** : la photo, au seul LLM local. Rien vers un LLM distant ; avec un LLM
  distant, la photo est refusée et le texte tapé reste possible.
- **Vers la voix** : le texte de la dictée, au conteneur local de synthèse. Aucun service
  de synthèse distant n'est proposé.
- **Sur le réseau de la maison** : le pack de sons, téléchargeable sans mot de passe par
  toute machine du réseau. Il ne contient que des phrases de dictée lues, numérotées.
- **Sur le disque** : le texte des dictées, les sons, les tentatives. Pas de photo.

## Statut

**Branche** : `feat/dictee-audio`, pas encore créée, `/next` l'ouvre à la première phase.

- [ ] **Phase 1 : Une voix locale lit une phrase**
- [ ] **Phase 2 : Le jeu joue un son fabriqué par l'agent**
- [ ] **Phase 3 : Du texte aux trous**
- [ ] **Phase 4 : Le coffre de dictée**
- [ ] **Phase 5 : La dictée depuis Telegram**
- [ ] **Phase 6 : Lire ce que l'enfant a tapé**
- [ ] **Phase 7 : La séance de chaque jour**

Les phases 1, 3 et 6 sont indépendantes. La phase 2 dépend de la 1, la phase 4 des phases 2
et 3, la phase 5 de la 4, la phase 7 des phases 5 et 6. Les phases 1 et 2 portent le risque
technique : si la voix est mauvaise ou si le pack ne se recharge pas, on s'arrête là et on
revoit la spec avant d'écrire le reste.

## Conception

Toutes les commandes Minecraft et les réglages de pack de cette spec sont **à vérifier sur
la version du serveur** (26.3) par `/essai-en-jeu` avant d'être tenus pour vrais.

### Phase 1 : Une voix locale lit une phrase

**Contexte.** Rien ne produit de son. Minecraft ne lit que de l'Ogg Vorbis. La qualité de
la voix française décide de tout le chantier : une phrase mal prononcée est une dictée
fausse. Piper est sous licence GPL, le projet sous MIT : il tourne donc dans son propre
conteneur, appelé par HTTP, jamais importé.

**Fichiers.**
- `src/mine_de_rien/adapters/voix/base.py`, `piper.py`, `__init__.py` (nouveaux) : contrat
  `Voix.dire(texte, lent) -> bytes` (WAV) et `ErreurVoix`.
- `src/mine_de_rien/son.py` (nouveau) : WAV vers Ogg Vorbis mono.
- `src/mine_de_rien/config.py`, `config.example.toml` : section `[voix]` (adaptateur, url,
  nom de la voix, vitesses normale et lente).
- `docker-compose.yml` : service `voix`, profil `voix`.
- `src/mine_de_rien/cli.py` : `dire "<phrase>" [--lent] --sortie fichier.ogg`.
- `tests/test_voix.py`, `tests/test_son.py`.

**Conception.**
- La phase commence par un essai à l'oreille, avant tout code définitif : les voix
  françaises de Piper (`fr_FR-siwis-medium`, `fr_FR-upmc-medium`, `fr_FR-tom-medium`) sur
  les phrases des dictées 5 et 6. À écouter de près : « la plus haute tour », « huit-cent-
  vingt-huit mètres », « trois tours Eiffel », « Elles rient aux éclats ». Le parent choisit
  la voix ; le choix et les défauts entendus vont dans « Retours de jeu ».
- Le serveur HTTP de Piper et son image Docker sont à choisir pendant cet essai (serveur
  HTTP fourni par Piper, ou serveur compatible `/v1/audio/speech`). L'adaptateur cache ce
  choix derrière `dire`.
- Vitesse lente : le paramètre de durée de Piper, pas un ralentissement du fichier, qui
  déformerait la voix.
- Conversion : par une bibliothèque Python qui écrit du Vorbis, de préférence à `ffmpeg`
  dans l'image de l'agent, si elle tient dans une image raisonnable. À trancher ici.
- Si aucune voix n'est jugée assez bonne, la phase s'arrête et le dit : on rouvre « voix du
  parent » dans « Hors périmètre ».

**Vérification automatique.**
- `uv run pytest` : avec `httpx.MockTransport`, la requête porte le texte et la vitesse ;
  une erreur du serveur lève `ErreurVoix` ; un WAV de test converti est un Ogg Vorbis mono
  valide (en-tête `OggS`, codec Vorbis).
- À la main : `uv run mine-de-rien dire "Dans la ville de Dubaï s'élève la plus haute tour
  jamais construite." --sortie /tmp/essai.ogg` produit un fichier qu'un lecteur audio joue,
  en moins de 10 secondes.

### Phase 2 : Le jeu joue un son fabriqué par l'agent

**Contexte.** Un son personnalisé n'existe pour le jeu que dans un pack de ressources. Le
serveur annonce une adresse de pack, le jeu de l'enfant le télécharge à la connexion. Sans
empreinte `sha1` annoncée, le jeu est censé retélécharger le pack à chaque connexion : c'est
ce qui permet de le changer sans redémarrer le serveur. **C'est l'hypothèse à prouver.**

**Fichiers.**
- `src/mine_de_rien/pack.py` (nouveau) : construit le zip (`pack.mcmeta`,
  `assets/mine_de_rien/sounds.json`, `assets/mine_de_rien/sounds/dictee/<n>.ogg`), écrit de
  façon atomique dans `/data`.
- `src/mine_de_rien/service_pack.py` (nouveau) : petit serveur HTTP qui ne sert que ce zip.
- `src/mine_de_rien/config.py`, `config.example.toml` : section `[pack]` (port, adresse
  publique vue par le jeu de l'enfant).
- `docker-compose.yml`, `docker/essai.compose.yml` : port publié pour le pack, variables du
  serveur pour l'adresse du pack et son caractère obligatoire.
- `datapacks/mine_de_rien/data/mine_de_rien/function/dictee/jouer.mcfunction` (nouveau).
- `src/mine_de_rien/cli.py` : `pack <fichier.ogg>…` construit un pack d'essai.
- `docs/installation.md`, `docs/rejoindre-le-serveur.md` : accepter le pack, curseur
  « Voix » des options de son, adresse locale de la machine.
- `tests/test_pack.py`.

**Conception.**
- Format du pack de ressources de la 26.3 : à relever sur le serveur d'essai (ce n'est pas
  le 121 du datapack). Le test de version existant (`tests/test_verifier_version.py`) doit
  couvrir aussi ce numéro.
- Chaque son est déclaré en flux (`"stream": true`) et joué dans la catégorie `voice`.
- Commandes à vérifier :

      playsound mine_de_rien:dictee.<n> voice <pseudo> ~ ~ ~ 1 1
      stopsound <pseudo> voice
      data modify storage mine_de_rien:dictee son set value "mine_de_rien:dictee.<n>"
      function mine_de_rien:dictee/jouer with storage mine_de_rien:dictee

  `jouer.mcfunction` est une fonction à macro : elle coupe le son en cours puis joue
  `$(son)` pour le joueur qui l'exécute.
- Variables de l'image `itzg/minecraft-server` pour l'adresse du pack et l'obligation de
  l'accepter : noms à vérifier dans sa documentation.
- **Repli** si le jeu garde l'ancien pack en cache : l'adresse du pack porte un numéro de
  version et le serveur doit être redémarré quand elle change. Dans ce cas, revenir à la
  spec avant de continuer : la phase 5 devra dire quand redémarrer.

**Vérification automatique.**
- `uv run pytest` : le zip contient les trois sortes de fichiers, `sounds.json` déclare un
  événement par son, aucun nom de fichier ne contient autre chose qu'un numéro ; le service
  HTTP ne sert que le zip (toute autre adresse : 404).
- `/essai-en-jeu` : le serveur démarre avec l'adresse du pack ; `playsound` d'un son du
  pack ne renvoie pas d'erreur de syntaxe ; la fonction à macro s'exécute.

**Vérification en jeu.** Sur le serveur d'essai. Construire un pack avec une phrase
(`mine-de-rien pack`), se connecter : le jeu propose le pack, l'accepter. Lancer le
`playsound` par `rcon-cli` : la phrase s'entend, et le curseur « Voix » (Options, Musique et
sons) règle son volume. Construire un second pack avec une autre phrase sous le même numéro,
**se déconnecter et se reconnecter sans redémarrer le serveur** : c'est la nouvelle phrase
qui s'entend. Noter le résultat dans « Retours de jeu » : il décide du repli.

### Phase 3 : Du texte aux trous

**Contexte.** La part déterministe du chantier, sans jeu ni réseau. Suivre
`/nouvel-exercice`. La réponse n'est pas calculée mais **découpée** : c'est un mot du texte
fourni par le parent, à sa position.

**Fichiers.**
- `src/mine_de_rien/exercises/francais/__init__.py`, `dictee.py` (nouveaux).
- `src/mine_de_rien/saisie.py` : `cle_dictee_valide`, à côté de `cle_valide`.
- `tests/test_dictee.py`.

**Conception.**
- `decouper(texte) -> list[Phrase]` : phrases coupées sur `.`, `!`, `?`, `…` ; chaque
  phrase garde son texte et ses mots avec leur position. Une liste de mots (`mots : a, b`)
  donne une phrase par mot.
- `candidats(phrases)` : les mots qui peuvent devenir un trou (règles du tableau).
- `tirer(phrases, n, rng, poids, imposes)` : `n` trous sans remise, les mots imposés
  d'abord, puis au hasard selon `poids` (un nombre par mot, 1 par défaut ; la phase 7 le
  calcule). Jamais deux trous dans la même phrase tant qu'une autre phrase est libre.
- Chaque trou devient un `Exercice` : compétence `francais/dictee`, réponse = le mot,
  énoncé = « Dictée : <mot d'avant> _____ <mot d'après> » (les voisins sont retirés si
  l'énoncé ne tient pas sur le panneau), et dans `donnees` le numéro de phrase, la position
  et la phrase à trou pour le chat.
- `cle_dictee_valide` : lettres avec accents, majuscules permises, apostrophe droite et
  trait d'union à l'intérieur, 50 caractères au plus.
- Pas d'entrée au registre : `/coffre dictee 3` n'a pas de sens sans dictée enregistrée.

**Vérification automatique.** `uv run pytest`, sur 500 graines et sur les textes des
dictées 5 et 6 :
- chaque énoncé tient sur le panneau, chaque clé passe `cle_dictee_valide` ;
- la phrase à trou, une fois le trou remplacé par la clé, redonne la phrase d'origine ;
- la clé n'apparaît ni dans l'énoncé ni à la place du trou ;
- « Dans » garde sa majuscule ; « s'élève », « huit-cent-vingt-huit » et « la » sont des
  trous possibles ; « Émirats » (majuscule accentuée) et « à » (une lettre) ne le sont
  jamais ;
- un mot imposé absent du texte est signalé ; un mot imposé présent sort toujours ;
- avec un poids de 10 sur un mot, il sort nettement plus souvent qu'un autre.

### Phase 4 : Le coffre de dictée

**Contexte.** Brancher les phases 2 et 3 sur le coffre du soir. Suivre `/nouveau-jeu`. Le
coffre lui-même ne change pas : verrou, preuve, détection. Ce qui change est autour : la
phrase lue, la phrase à trou dans le chat, l'Écouteur, le mode d'emploi, l'aide de Pixel.

**Fichiers.**
- `src/mine_de_rien/games/coffre_du_soir.py` : annonce, rappel, équipement et rangement
  selon la compétence du coffre ; description de la clé sans « en minuscules ».
- `src/mine_de_rien/games/dictee.py` (nouveau) : commandes propres à la dictée.
- `datapacks/mine_de_rien/…/function/tick.mcfunction`, `dictee/redire.mcfunction`,
  `load` : objectif de score et détection du clic droit.
- `src/mine_de_rien/exercises/indices.py`, `games/pixel.py` : paliers d'un coffre de dictée.
- `src/mine_de_rien/storage/sqlite.py` : migration 4, `source` admet `dictee`.
- `src/mine_de_rien/cli.py` : `dictee-essai "<texte>" [-n 2]` : synthétise, construit le
  pack et met des coffres en file, sans Telegram.
- `tests/test_coffre_du_soir.py`, `tests/test_pixel.py`, `tests/test_indices.py`,
  `tests/test_stockage.py`.

**Conception.**
- À la pose, l'agent écrit dans le stockage `mine_de_rien:dictee` le son et la phrase à
  trou du coffre actif ; au rangement et au retrait, il les efface.
- L'Écouteur : un objet à clic droit détecté par un score (`minecraft.used:…`), avec un nom
  et une apparence à lui. À chaque clic, le datapack rejoue le son et réaffiche la phrase à
  trou depuis le stockage, sans attendre le tour de 3 secondes de l'agent. Sans coffre de
  dictée actif, il ne fait rien. Donné avec la clé, s'il n'en a pas déjà un.
- Annonce d'un coffre de dictée : message, phrase à trou, mode d'emploi de dictée, son.
  Au rappel de connexion : pareil.
- Condition de pose : la connexion en cours de l'enfant a commencé après la construction du
  pack. Sinon le coffre attend, et les parents sont prévenus une fois (« il doit se
  reconnecter pour entendre la dictée »).
- Indices : palier 1, le son lent ; palier 2, « Le mot a N lettres » ; palier 3, « Il
  commence par X et a N lettres ». Le filtre `contient_la_reponse` s'applique comme
  ailleurs.
- La clé déjà dans l'inventaire de l'enfant porte l'ancienne description : la remplacer à
  la première pose après la mise à jour.
- Contournements, à écrire dans l'en-tête du module comme pour le coffre du soir : la
  feuille de la maîtresse est à la maison (c'est de la préparation, le parent la garde s'il
  veut) ; le pack sur le disque du jeu ne contient que des sons numérotés ; le volume
  « Voix » à zéro rend le coffre muet mais pas plus facile ; essayer au hasard coûte
  toujours un niveau par essai.

**Vérification automatique.**
- `uv run pytest` : les commandes d'un coffre de dictée posent le verrou avec la clé à
  majuscule et apostrophe correctement échappée ; l'annonce contient la phrase à trou et
  jamais la clé ; le rangement efface le stockage ; un coffre de maths n'écrit rien dans le
  stockage et garde « en minuscules » ; les trois paliers ne contiennent jamais le mot ; une
  base en version 3 migre sans rien perdre.
- `/essai-en-jeu` : un coffre verrouillé par « s'élève », puis par « Dubaï », est accepté et
  `data get block` restitue la clé à l'identique ; le score de l'Écouteur monte au clic
  droit (à constater en jeu) ; la fonction `redire` s'exécute sans erreur.

**Vérification en jeu.** Sur le serveur d'essai : `mine-de-rien dictee-essai "Dans la ville
de Dubaï s'élève la plus haute tour jamais construite." -n 2`, puis se reconnecter. À
l'atelier : la phrase s'entend à l'annonce, le chat montre la phrase avec `_____`, le
panneau commence par « Dictée : ». Clic droit avec l'Écouteur : la phrase est relue aussitôt.
Sur l'enclume, taper le mot avec une faute : le coffre reste fermé. Le taper juste, avec sa
majuscule ou son apostrophe : il s'ouvre. Écrire « Pixel, aide-moi » trois fois : lecture
lente, nombre de lettres, première lettre.

### Phase 5 : La dictée depuis Telegram

**Contexte.** Le parent n'a encore aucun moyen d'envoyer une dictée. Le canal ne sait
envoyer que du texte (`src/mine_de_rien/adapters/channel/base.py:18-25`).

**Fichiers.**
- `src/mine_de_rien/storage/sqlite.py` : migration 5, tables `dictees` (jour, texte, mots
  imposés, statut, date du pack) et `dictee_phrases`.
- `src/mine_de_rien/photo.py` : consigne et interprétation pour un texte de dictée.
- `src/mine_de_rien/adapters/channel/base.py`, `telegram.py` : envoi d'un fichier audio.
- `src/mine_de_rien/orchestrateur.py` : `/dictee`, photo légendée « dictée … », `/oui`,
  `/non`, aide.
- `src/mine_de_rien/dictees.py` (nouveau) : de la dictée confirmée aux sons et au pack.
- `docs/installation.md`.
- `tests/test_orchestrateur.py`, `tests/test_photo.py`, `tests/test_telegram.py`,
  `tests/test_stockage.py`, `tests/donnees/photo/dictee-*.json`.

**Conception.**
- Commandes : `/dictee <jour> <texte>`, `/dictee <jour> mots : a, b, c`, `/dictee` seul
  (liste des dictées en cours, avec leur numéro), `/dictee annuler <n>`,
  `/dictee mots <n> a b c` (mots imposés, vérifiés contre le texte).
- Photo : une légende qui commence par « dictée » (accent facultatif) l'envoie vers la
  lecture de texte ; sinon c'est une fiche, comme aujourd'hui. Consigne au LLM sans aucun
  exemple, comme pour les fiches : recopier le texte imprimé, mot pour mot, sans rien
  corriger ni compléter. La phase commence par un essai sur une vraie feuille de la
  maîtresse ; si le texte n'est pas recopié à l'identique, la photo sort du périmètre et le
  texte tapé reste la seule voie.
- Réponse au parent : les phrases numérotées, le nombre de mots candidats, le jour retenu
  en toutes lettres avec sa date, puis l'aperçu audio (toutes les phrases à la suite, un
  seul fichier) et « Compare à la feuille et écoute : /oui ou /non ». Envoi de l'audio par
  Telegram : format accepté à vérifier (message vocal, sinon document).
- `/oui` : la dictée est enregistrée, les sons des deux vitesses sont produits, le pack est
  reconstruit avec les sons de toutes les dictées en cours. Aucun coffre n'est créé ici :
  c'est la phase 7. En attendant, `/dictee seance <n>` met tout de suite une séance en
  file, et restera utile pour une séance de plus à la demande.
- Une dictée dont le jour est passé depuis 30 jours est retirée du pack.

**Vérification automatique.** `uv run pytest` : « vendredi » un mardi donne le vendredi de
la semaine, un vendredi donne le suivant ; un jour passé est refusé ; un texte sans mot
candidat est refusé ; `/oui` enregistre la dictée et reconstruit le pack (voix simulée) ;
`/non` n'écrit rien ; une photo légendée « dictée vendredi » avec un LLM distant est
refusée sans appel ; une photo sans le mot « dictée » suit l'ancien chemin ; un mot imposé
absent du texte est refusé ; `/dictee seance` crée des coffres de source `dictee`.

**Vérification en jeu.** Avec le bot d'essai : envoyer `/dictee vendredi` suivi du texte de
la dictée 6. Vérifier les phrases, écouter l'aperçu, `/oui`. Taper `/dictee seance 1`, se
reconnecter au serveur d'essai : quatre coffres dans `/file`, le premier à l'atelier avec sa
phrase lue. Recommencer avec la photo d'une feuille de dictée, puis `/non`.

### Phase 6 : Lire ce que l'enfant a tapé

**Contexte.** Indépendante de la dictée, utile à tous les coffres. La clé est un objet
marqué (`mdr_cle`, `coffre_du_soir.py:129-130`) : son nom est lisible dans les données du
joueur.

**Fichiers.**
- `src/mine_de_rien/games/coffre_du_soir.py` : lecture du nom de la clé.
- `src/mine_de_rien/orchestrateur.py` : une tentative par nouveau nom, pendant qu'un coffre
  est actif.
- `src/mine_de_rien/storage/sqlite.py` : écriture et lecture des `tentatives`.
- `src/mine_de_rien/rapport.py` : graphies dans le rapport et dans la notification
  d'ouverture.
- `tests/test_coffre_du_soir.py`, `tests/test_orchestrateur.py`, `tests/test_rapport.py`.

**Conception.**
- Commande à vérifier, qui lit le nom de la clé où qu'elle soit dans l'inventaire :

      data get entity <pseudo> Inventory[{components:{"minecraft:custom_data":{mdr_cle:1b}}}].components."minecraft:custom_name"

- À la pose, le nom courant de la clé est retenu comme point de départ : c'est la réponse
  du coffre précédent, pas un essai.
- À chaque tour, si le nom a changé : une ligne dans `tentatives`, juste ou fausse par
  comparaison exacte avec la réponse. Un même nom n'est compté qu'une fois de suite.
- Limites connues, à écrire dans le module : deux essais en moins de 3 secondes n'en font
  qu'un ; un essai fait avec un autre objet que la clé n'est pas vu ; un essai resté sur
  l'enclume sans être pris non plus.
- Rien de ce que l'enfant a tapé n'est réaffiché dans le jeu.
- Notification d'ouverture : « Coffre n°12 ouvert en 4 min, sans aide (Dictée : la _____
  haute → plus). Essais : plu. »

**Vérification automatique.**
- `uv run pytest` : nom inchangé, aucune tentative ; « plu » puis « plus » donnent une
  tentative fausse et une juste ; le nom présent à la pose n'est pas compté ; le rapport
  liste les graphies ; un coffre de maths est suivi de la même façon.
- `/essai-en-jeu` : la commande renvoie le nom d'une clé renommée, et une réponse
  reconnaissable quand la clé n'a pas de nom ou n'est pas dans l'inventaire.

**Vérification en jeu.** Sur le serveur d'essai, un coffre `/coffre 348 ÷ 12`. Renommer la
clé « 28 », la prendre, attendre 5 secondes, puis « 29 » et ouvrir. Telegram annonce
l'ouverture avec « Essais : 28 », et `/rapport` le montre.

### Phase 7 : La séance de chaque jour

**Contexte.** Dernière brique : plus personne ne déclenche rien. S'appuie sur les dictées
enregistrées (phase 5) et sur les tentatives (phase 6).

**Fichiers.**
- `src/mine_de_rien/seance.py` (nouveau) : décide s'il faut une séance, et laquelle.
- `src/mine_de_rien/orchestrateur.py` : appel à la connexion de l'enfant.
- `src/mine_de_rien/storage/sqlite.py` : migration 6, date de la dernière séance par
  dictée ; lecture de l'historique d'un mot.
- `src/mine_de_rien/config.py`, `config.example.toml` : section `[dictee]` (coffres par
  séance, de 3 à 5, 4 par défaut) et fuseau de la maison.
- `src/mine_de_rien/rapport.py` : mots acquis et mots à retravailler par dictée.
- `tests/test_seance.py`, `tests/test_orchestrateur.py`, `tests/test_rapport.py`.

**Conception.**
- Déclencheur : l'enfant vient de se connecter, aucune séance de dictée n'a été créée
  aujourd'hui (jour du fuseau de la maison), et une dictée est en cours (aujourd'hui est
  après le jour de sa confirmation et avant le jour de la dictée).
- Poids d'un mot pour `tirer` (phase 3), calculé depuis les tentatives :
  - jamais demandé : 1 ;
  - raté à sa dernière apparition, il y a deux jours ou plus : 10 ;
  - raté hier ou aujourd'hui : 0, il attend ;
  - réussi du premier coup deux fois de suite : 0, acquis.
- S'il reste moins de mots que de coffres, la séance est plus courte ; s'il n'en reste
  aucun, pas de séance, et les parents reçoivent « tous les mots de la dictée sont acquis ».
- Deux dictées en cours : la séance sert celle dont le jour est le plus proche.
- Révision : hors de toute dictée en cours, rien. Pendant une dictée en cours, un des
  coffres de la séance peut être un mot encore raté d'une dictée passée de moins de 30
  jours, si son son est toujours dans le pack.
- La veille au soir de la dictée, le rapport dit aux parents quels mots restent fragiles.

**Vérification automatique.** `uv run pytest`, avec une horloge simulée : une connexion
lundi crée 4 coffres, une seconde connexion lundi n'en crée pas, mardi en crée 4 autres ;
le jour de la dictée et après, rien ; un jour sans connexion n'est pas rattrapé ; un mot
raté lundi ne sort pas mardi et sort mercredi ; un mot réussi deux fois ne sort plus ; à
23 h 30 heure de Paris, le jour compté est celui de Paris, pas celui d'UTC ; les coffres de
`/coffre` déjà en file restent devant.

**Vérification en jeu.** Sur le serveur d'essai, après une dictée confirmée la veille (ou
dont la date de confirmation a été reculée à la main dans la base d'essai) : se connecter,
quatre coffres arrivent sans rien taper sur Telegram. Rater exprès un mot, ouvrir les
quatre, se déconnecter et se reconnecter : aucune nouvelle séance. `/rapport` montre le mot
raté et sa graphie.
