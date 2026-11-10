# L'enfant prépare sa dictée de la semaine en l'écoutant dans le jeu

> **Reprise de session** : lire « Pourquoi », « Décisions actées » et « Statut »,
> puis aller à la première phase non terminée.

Écrite le 2026-11-10 quand l'agent était en Python, reprise le même jour après son passage
à Rust ([spec](../done/2026-11-agent-en-rust.md)). Ce que l'enfant et les parents vivent n'a
pas changé ; les chemins, les commandes et les choix de bibliothèques sont ceux du crate.

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
  (`src/exercises/registre.rs:19-32`) ;
- aucun son : le seul `playsound` est la cloche du jeu (`src/games/coffre_du_soir.rs:422`),
  et le serveur n'a pas de pack de ressources (`docker-compose.yml`) ;
- une clé de coffre n'accepte ni majuscule ni apostrophe (`src/saisie.rs:57-60`), donc ni
  « Dubaï » ni « s'élève » ;
- la lecture de photo ne recopie que des calculs et des mots anglais
  (`src/photo.rs:47-51`) ;
- tous les coffres partent dès la demande, l'un après l'autre
  (`src/orchestrateur.rs:191-229`) : rien ne répartit un travail sur une semaine ;
- les essais ratés sont invisibles : la table `tentatives` existe
  (`src/storage/sqlite.rs:42-48`) mais rien n'y écrit. On ne peut donc ni montrer aux
  parents ce que l'enfant a écrit, ni faire revenir un mot raté ;
- le canal ne sait envoyer que du texte (`src/adapters/channel/base.rs:23-33`).

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
| Voix | Synthèse **locale**, par un adaptateur `src/adapters/voix/`. Première implémentation : Piper, voix française, dans son propre conteneur du compose, appelé par HTTP. Aucune voix distante. |
| Format du son | Minecraft ne lit que l'Ogg Vorbis. La conversion se fait **dans le binaire** : l'image de l'agent ne contient rien d'autre que lui, donc ni `ffmpeg` ni interpréteur de commandes. |
| Transport du son | Pack de ressources du serveur, construit par l'agent et servi par lui en HTTP sur le réseau de la maison. Un son par phrase, en deux vitesses (normale et lente). Les fichiers portent des numéros, jamais de mots. |
| Mise à jour du pack | Le pack est reconstruit à chaque dictée confirmée. Un coffre de dictée n'est posé que si l'enfant s'est connecté **après** la construction du pack qui contient son son. |
| Choix des trous | Le code tire au hasard dans **tout le texte**, comme la maîtresse. Le parent peut imposer des mots, qui sortent en premier. Les mots déjà ratés pèsent plus lourd. Le LLM ne choisit rien. |
| Mots candidats | Tout mot d'au moins deux lettres, apostrophe et trait d'union compris (« s'élève », « huit-cent-vingt-huit »). Exclus : les mots de plus de 50 caractères, ceux qui contiennent un chiffre, et ceux dont la majuscule porte un accent (« Émirats » : difficile à taper). |
| Écriture de la clé | Le mot **exactement comme dans le texte**, majuscule comprise : « Dans » en début de phrase, « Dubaï », « tours ». Aucune règle pour deviner les noms propres. L'apostrophe typographique devient l'apostrophe du clavier. |
| Mode d'emploi | Pour un coffre de dictée : « écris le mot comme dans la phrase, avec sa majuscule s'il en a une ». La description de la clé ne dit plus « en minuscules », c'est le chat qui le précise selon le coffre. |
| Saisie par le parent | `/dictee vendredi <texte>` ou une photo avec « dictée vendredi » en légende. Le jour est un nom de jour (le prochain) ou `JJ/MM`. Une liste de mots sans texte : `/dictee vendredi mots : plus, trois, personnes` ; chaque mot est alors lu seul. |
| Rôle du LLM | Sur une photo, le LLM **local** recopie le texte imprimé, rien d'autre. Le texte tapé ne passe par aucun LLM. |
| Confirmation | Mine de rien renvoie le texte lu, découpé en phrases, et un **aperçu audio** de toute la dictée. Le parent compare à la feuille, écoute, puis `/oui` ou `/non`. C'est la seule barrière contre une faute de transcription, qui deviendrait une clé fausse. |
| Séance | Chaque jour, à la première connexion de l'enfant, 4 coffres de dictée entrent dans la file (réglable de 3 à 5). Du lendemain de la confirmation à la veille de la dictée. Un jour sans connexion n'est pas rattrapé : pas de dette. |
| Essais ratés | L'agent relit le nom de la clé toutes les 3 secondes. Chaque nouveau nom différent de la réponse est une tentative ratée, écrite dans `tentatives`. Vaut pour tous les coffres, maths et anglais compris. |
| Mot raté | Un mot raté au moins une fois revient au plus tôt deux jours après, en priorité. Un mot réussi du premier coup deux fois de suite sort du tirage. Tant qu'une dictée est en cours, un coffre par séance peut reprendre un mot encore raté d'une dictée passée de moins de 30 jours. |
| Aide de Pixel | Trois paliers pour un coffre de dictée : la phrase relue lentement, puis le nombre de lettres, puis la première lettre. Jamais le mot. |
| Rapport | Le rapport Telegram montre les graphies : « personnes : persone, puis juste ». |
| Fuseau | « Chaque jour » se compte dans le fuseau de la maison, nouvelle clé de configuration. Le stockage reste en UTC. La table des fuseaux est embarquée dans le binaire : l'image n'a pas celle du système. |
| Oracle | Les fichiers de `tests/donnees/oracle/` figent le comportement de la 0.1.2. Ce chantier en change deux points voulus (description de la clé, notification d'ouverture) : les cas touchés sont corrigés à la main dans la phase qui les change, et le commit le dit. Tout autre test `oracle_python` qui casse est un défaut. |

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
- `docs/architecture.md`, décision 4, et `CLAUDE.md`, « Arborescence » : trois adaptateurs
  (jeu, canal, LLM). La voix en fait un quatrième.
- Spec `2026-11-agent-en-rust` : « Telegram : `teloxide`, limité au client `Bot` et à
  l'interrogation longue », en pratique `sendMessage` et `getFile`. La phase 5 ajoute l'envoi
  d'un fichier. « `chrono` sans fuseaux » : la phase 7 ajoute les fuseaux.
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
- Un outil audio ajouté à l'image de l'agent (`ffmpeg`), ou un second conteneur de
  conversion : l'image reste le binaire seul.
- Corriger les deux défauts hérités du Python (espace après une parenthèse, double
  téléportation) : un autre chantier.

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
- **Dépendances** : chaque crate ajouté (audio, zip, fuseaux, serveur HTTP) est contrôlé
  comme le demande `CLAUDE.md` : aucune connexion de son propre chef.

## Statut

**Branche** : `feat/dictee-audio`, pas encore créée, `/next` l'ouvre à la première phase.

- [x] **Phase 1 : Une voix locale lit une phrase**
- [x] **Phase 2 : Le jeu joue un son fabriqué par l'agent**
- [x] **Phase 3 : Du texte aux trous**
- [ ] **Phase 4 : Le coffre de dictée**
- [ ] **Phase 5 : La dictée depuis Telegram**
- [ ] **Phase 6 : Lire ce que l'enfant a tapé**
- [ ] **Phase 7 : La séance de chaque jour**

Les phases 1, 3 et 6 sont indépendantes. La phase 2 dépend de la 1, la phase 4 des phases 2
et 3, la phase 5 de la 4, la phase 7 des phases 5 et 6. Les phases 1 et 2 portent le risque
technique : si la voix est mauvaise, si le Vorbis ne se compile pas dans l'image ou si le
pack ne se recharge pas, on s'arrête là et on revoit la spec avant d'écrire le reste.

## Conception

Toutes les commandes Minecraft et les réglages de pack de cette spec sont **à vérifier sur
la version du serveur** (26.3) par `/essai-en-jeu` avant d'être tenus pour vrais.

### Règles communes à toutes les phases

**Vérification automatique, toujours.**

    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test

**Habitudes du crate, à tenir.**

- Un adaptateur est un trait asynchrone objet (`async-trait`, `?Send`), partagé par
  `Rc<dyn …>` sur le runtime mono-fil, comme `AdaptateurJeu`, `Canal` et `Llm`. Les tests
  lui substituent un faux qui implémente le trait.
- Les appels HTTP sortants passent par `reqwest` (0.12, une seule version dans le binaire :
  `cargo tree -d` le vérifie). Les tests tournent contre un petit serveur HTTP local écrit
  dans le test, comme `tests/llm.rs`.
- Les longueurs se comptent en caractères (`chars()`), jamais en octets : la limite de 50
  de l'enclume, la largeur du panneau, la première lettre d'un mot (« é », « œ »).
- Le crate `regex` n'a ni regard avant ni regard arrière : le découpage en phrases et en
  mots s'écrit sans.
- Pas de `unwrap()` hors des tests et des constantes : une erreur de voix, de pack ou de
  fuseau est une variante d'erreur que la boucle journalise avant de repartir.
- Le tirage passe par `Alea` (`src/alea.rs`), avec une graine dans les tests.
- Un crate qui compile du C doit passer la construction croisée `musl` de
  `docker/agent.Dockerfile`, pour amd64 et arm64, comme SQLite aujourd'hui.

### Phase 1 : Une voix locale lit une phrase

**Contexte.** Rien ne produit de son. La qualité de la voix française décide de tout le
chantier : une phrase mal prononcée est une dictée fausse. Piper est sous licence GPL, le
projet sous MIT : il tourne dans son propre conteneur, appelé par HTTP, jamais lié au
binaire. La sous-commande `dire` existe déjà et écrit dans le chat (`src/cli.rs:45-46`) :
celle de la voix s'appelle `voix`.

**Fichiers.**
- `src/adapters/voix/base.rs`, `piper.rs`, `mod.rs` (nouveaux) : trait `Voix`, avec
  `dire(texte, lent) -> Result<Vec<u8>, ErreurVoix>` (un WAV), et `creer(config)`.
- `src/son.rs` (nouveau) : WAV vers Ogg Vorbis mono.
- `src/config.rs`, `config.example.toml`, `config.essai.example.toml` : section `[voix]`
  (adaptateur, url, nom de la voix, vitesses normale et lente). Absente : pas de dictée,
  le reste de l'agent marche comme avant.
- `docker/voix.Dockerfile` (nouveau) : Piper et son serveur HTTP, versions figées.
- `docker-compose.yml` : service `voix` (profil `voix`, réseau sans sortie) et service
  ponctuel `voix-telecharger` (profil `voix-installation`). `docker/essai.compose.yml` :
  service `voix`, port 5055 du poste.
- `docs/installation.md`, `.env.example` : télécharger la voix, lancer le conteneur.
- `src/cli.rs` : `voix "<phrase>" [--lent] --sortie fichier.ogg`.
- `Cargo.toml` : l'encodeur Vorbis.
- `CLAUDE.md`, `docs/architecture.md` : le quatrième adaptateur.
- `tests/voix.rs`, `tests/son.rs`.

**Conception.**
- La phase commence par un essai à l'oreille, avant tout code définitif : les voix
  françaises de Piper (`fr_FR-siwis-medium`, `fr_FR-upmc-medium`, `fr_FR-tom-medium`) sur
  les phrases des dictées 5 et 6. À écouter de près : « la plus haute tour », « huit-cent-
  vingt-huit mètres », « trois tours Eiffel », « Elles rient aux éclats ». Le parent choisit
  la voix ; le choix et les défauts entendus vont dans « Retours de jeu ».
- Serveur retenu : celui que fournit Piper (`python -m piper.http_server`, paquet
  `piper-tts[http]` 1.8.0), dans une image construite par le projet, faute d'image
  officielle. `POST /synthesize` avec `text`, `voice` et `length_scale` rend un WAV PCM
  16 bits mono à 22 050 Hz. L'adaptateur cache ce choix derrière `dire`.
- Vitesse lente : le paramètre de durée de Piper, pas un ralentissement du fichier, qui
  déformerait la voix.
- Encodeur Vorbis retenu : `vorbis_rs` 0.5, qui embarque l'encodeur de référence en C
  (licence BSD), sans sa fonction de tirage du numéro de flux : le numéro est fixe, deux
  conversions du même WAV donnent le même fichier. Les encodeurs en Rust pur trouvés sur
  crates.io sont en version 0.0 ou 0.1 : écartés sans essai. Le WAV se lit à la main, sans
  `hound`. Reste à prouver en phase 2 : que Minecraft lit le fichier.
- Si aucun encodeur ne passe la construction de l'image, la phase s'arrête et le dit : la
  ligne « outil audio dans l'image » de « Hors périmètre » est alors à rediscuter.
- Si aucune voix n'est jugée assez bonne, même arrêt : on rouvre « voix du parent ».

**Vérification automatique.** Les commandes communes, plus :
- contre un faux serveur HTTP : la requête porte le texte et la vitesse ; une erreur ou un
  délai dépassé rend `ErreurVoix` ; un WAV de test converti commence par `OggS`, annonce le
  codec Vorbis et un seul canal ; un WAV mal formé est une erreur, pas une panique ;
- `docker buildx build --platform linux/amd64,linux/arm64 -f docker/agent.Dockerfile .`
  réussit avec la nouvelle dépendance, et la taille de l'image est notée à côté des 7 Mo
  d'aujourd'hui ;
- à la main : `cargo run -- voix "Dans la ville de Dubaï s'élève la plus haute tour jamais
  construite." --sortie /tmp/essai.ogg` produit un fichier qu'un lecteur audio joue, en
  moins de 10 secondes.

### Phase 2 : Le jeu joue un son fabriqué par l'agent

**Contexte.** Un son personnalisé n'existe pour le jeu que dans un pack de ressources. Le
serveur annonce une adresse de pack, le jeu de l'enfant le télécharge à la connexion. Sans
empreinte `sha1` annoncée, le jeu retélécharge le pack à chaque connexion : c'est ce qui
permet de le changer sans redémarrer le serveur. Prouvé en jeu le 2026-11-10.
L'image de l'agent n'a ni `/tmp` ni système : le pack s'écrit dans `/data`, son seul dossier
inscriptible.

**Fichiers.**
- `src/pack.rs` (nouveau) : construit le zip (`pack.mcmeta`,
  `assets/mine_de_rien/sounds.json`, `assets/mine_de_rien/sounds/dictee/<n>.ogg`), écrit de
  façon atomique à côté de la base (fichier temporaire voisin, puis renommage).
- `src/service_pack.rs` (nouveau) : petit serveur HTTP qui ne sert que ce zip.
- `src/config.rs`, `config.example.toml`, `config.essai.example.toml` : section `[pack]`
  (port, interface d'écoute). L'adresse vue par le jeu de l'enfant n'est pas ici : c'est le
  serveur Minecraft qui l'annonce, elle se règle donc dans `.env` (`PACK_URL`).
- `docker-compose.yml`, `docker/essai.compose.yml`, `.env.example` : port publié pour le
  pack, adresse du pack donnée au serveur.
- `datapacks/mine_de_rien/data/mine_de_rien/function/dictee/jouer.mcfunction` (nouveau,
  embarqué dans le binaire à la compilation comme le reste du datapack).
- `src/cli.rs` : `pack <fichier.ogg>…` construit un pack d'essai, `servir-pack` le sert
  seul ; `lancer` démarre le service quand `[pack]` est configuré.
- `Cargo.toml` : `zip`, pour les tests seulement.
- `docs/installation.md`, `docs/rejoindre-le-serveur.md`, `datapacks/README.md` : accepter
  le pack, curseur « Voix » des options de son, adresse locale de la machine, format du
  pack de ressources.
- `tests/pack.rs`, `tests/service_pack.rs`, `tests/datapack.rs`.

**Conception.**
- Format du pack de ressources de la 26.3 : **97**, relevé dans le `version.json` du
  serveur d'essai (`pack_version.resource_major` ; le 121 du datapack y est
  `data_major`). Constante `FORMAT` de `src/pack.rs`, citée dans `datapacks/README.md` à
  côté de l'autre, à ajuster ensemble quand `MC_VERSION` change.
- Zip : archive sans compression écrite à la main (l'Ogg ne se compresse pas), sans date,
  donc identique à l'octet pour les mêmes sons. Aucune dépendance de plus dans le binaire ;
  les tests la relisent avec le crate `zip`, indépendant de notre écriture.
- Service HTTP : écrit à la main sur `tokio::net::TcpListener`, sans `hyper`. Il ne répond
  qu'à `GET` et `HEAD` sur une seule adresse ; tout le reste reçoit 404. Le fichier est
  relu à chaque requête. Il s'arrête avec l'agent (`SIGTERM`).
- Un pack vide est écrit au lancement s'il n'y en a pas : le jeu trouve toujours un pack à
  l'adresse annoncée, même avant la première dictée.
- **Le pack n'est pas obligatoire** pour le joueur, contrairement à ce que prévoyait la
  première écriture de cette phase : obligatoire, il empêcherait l'enfant de rejoindre le
  serveur chaque fois que l'agent est arrêté. Refusé ou introuvable, les dictées sont
  muettes, pas plus faciles.
- Chaque son est déclaré en flux (`"stream": true`) et joué dans la catégorie `voice`.
- Commandes, acceptées par Paper 26.3 le 2026-11-10, sans joueur connecté (le son
  lui-même reste à entendre en jeu) :

      playsound mine_de_rien:dictee.<n> voice <pseudo> ~ ~ ~ 1 1
      stopsound <pseudo> voice
      data modify storage mine_de_rien:dictee son set value "mine_de_rien:dictee.<n>"
      execute as <pseudo> at @s run function mine_de_rien:dictee/jouer with storage mine_de_rien:dictee

  `jouer.mcfunction` est une fonction à macro : elle coupe le son en cours puis joue
  `$(son)` pour le joueur qui l'exécute. Sans `son` dans le stockage, elle échoue sans rien
  faire (« Missing argument son »).
- Variables de l'image `itzg/minecraft-server`, vérifiées dans `server.properties` :
  `RESOURCE_PACK` (adresse) et `RESOURCE_PACK_ENFORCE` (`FALSE`). Pas de
  `RESOURCE_PACK_SHA1` : c'est son absence qui doit faire retélécharger le pack.
- Le compose d'essai n'a pas de service `agent` : le pack y est servi par `cargo run` sur
  le poste, et le jeu, lancé sur le même poste, le prend sur `localhost`.
- **Repli** si le jeu garde l'ancien pack en cache : l'adresse du pack porte un numéro de
  version et le serveur doit être redémarré quand elle change. Dans ce cas, revenir à la
  spec avant de continuer : la phase 5 devra dire quand redémarrer.

**Vérification automatique.** Les commandes communes, plus :
- le zip contient les trois sortes de fichiers, `sounds.json` déclare un événement par son,
  aucun nom de fichier ne contient autre chose qu'un numéro ; une écriture interrompue ne
  laisse jamais un zip tronqué à l'adresse servie ;
- le service HTTP ne sert que le zip (toute autre adresse : 404), et rend la main à l'arrêt ;
- `tests/datapack.rs` : le datapack recopié contient la nouvelle fonction ;
- `/essai-en-jeu` : le serveur démarre avec l'adresse du pack ; `playsound` d'un son du
  pack ne renvoie pas d'erreur de syntaxe ; la fonction à macro s'exécute.

**Vérification en jeu.** Sur le serveur d'essai. Fabriquer une phrase (`cargo run -- voix`),
construire un pack avec elle (`cargo run -- pack`), le servir (`cargo run -- servir-pack`),
puis se connecter : le jeu propose le pack, l'accepter. Lancer le `playsound` par
`rcon-cli` : la phrase s'entend, et le curseur « Voix » (Options, Musique et sons) règle
son volume. Construire un second pack avec une autre phrase sous le même numéro, **se
déconnecter et se reconnecter sans redémarrer le serveur** : c'est la nouvelle phrase qui
s'entend. Noter le résultat dans « Retours de jeu » : il décide du repli.

### Phase 3 : Du texte aux trous

**Contexte.** La part déterministe du chantier, sans jeu ni réseau. Suivre
`/nouvel-exercice`. La réponse n'est pas calculée mais **découpée** : c'est un mot du texte
fourni par le parent, à sa position.

**Fichiers.**
- `src/exercises/francais/mod.rs`, `dictee.rs` (nouveaux), `src/exercises/mod.rs`.
- `src/saisie.rs` : `cle_dictee_valide`, à côté de `cle_valide` (`:68-71`).
- `src/alea.rs` : tirage pondéré.
- `tests/dictee.rs`, `tests/saisie.rs`.

**Conception.**
- `decouper(texte) -> Vec<Phrase>` : phrases coupées sur `.`, `!`, `?`, `…` ; chaque
  phrase garde son texte et ses mots avec leur position. Une liste de mots (`mots : a, b`)
  donne une phrase par mot.
- `candidats(&phrases)` : les mots qui peuvent devenir un trou (règles du tableau).
- `tirer(&phrases, n, &mut alea, &poids, &imposes)` : `n` trous sans remise, les mots
  imposés d'abord, puis au hasard selon `poids` (un nombre par mot, 1 par défaut ; la phase
  7 le calcule). Jamais deux trous dans la même phrase tant qu'une autre phrase est libre.
  Un mot imposé absent du texte est une erreur rendue à l'appelant.
- Chaque trou devient un `Exercice` (`src/exercises/base.rs`) : compétence
  `francais/dictee`, réponse = le mot, énoncé = « Dictée : <mot d'avant> _____ <mot
  d'après> » (les voisins sont retirés si `lignes_panneau` refuse l'énoncé), et dans
  `donnees` le numéro de phrase, la position et la phrase à trou pour le chat.
- `cle_dictee_valide` : lettres avec accents, majuscules permises, apostrophe droite et
  trait d'union à l'intérieur, 50 caractères au plus.
- Pas d'entrée au registre : `/coffre dictee 3` n'a pas de sens sans dictée enregistrée, et
  la signature `Generateur` (`fn(&mut Alea, u32)`) ne porte pas de texte.

**Vérification automatique.** Les commandes communes ; sur 500 graines et sur les textes
des dictées 5 et 6 :
- chaque énoncé tient sur le panneau, chaque clé passe `cle_dictee_valide` ;
- la phrase à trou, une fois le trou remplacé par la clé, redonne la phrase d'origine ;
- la clé n'apparaît ni dans l'énoncé ni à la place du trou ;
- « Dans » garde sa majuscule ; « s'élève », « huit-cent-vingt-huit » et « la » sont des
  trous possibles ; « Émirats » (majuscule accentuée) et « à » (une lettre) ne le sont
  jamais ;
- un mot imposé absent du texte est signalé ; un mot imposé présent sort toujours ;
- avec un poids de 10 sur un mot, il sort nettement plus souvent qu'un autre ; avec un
  poids de 0, jamais ;
- `cle_valide` ne change pas : ses cas d'oracle passent tels quels.

### Phase 4 : Le coffre de dictée

**Contexte.** Brancher les phases 2 et 3 sur le coffre du soir. Suivre `/nouveau-jeu`. Le
coffre lui-même ne change pas : verrou, preuve, détection. Ce qui change est autour : la
phrase lue, la phrase à trou dans le chat, l'Écouteur, le mode d'emploi, l'aide de Pixel.
La boucle voit le coffre par le trait `Coffre` (`src/games/coffre_du_soir.rs:246-265`).

**Fichiers.**
- `src/games/coffre_du_soir.rs` : annonce, rappel, équipement et rangement selon la
  compétence du coffre ; description de la clé sans « en minuscules » (`:130-138`).
- `src/games/dictee.rs` (nouveau), `src/games/mod.rs` : commandes propres à la dictée,
  en-tête de module avec les contournements.
- `datapacks/mine_de_rien/…/function/tick.mcfunction`, `dictee/redire.mcfunction`, une
  fonction de chargement et son étiquette `load` : objectif de score et clic droit.
- `src/exercises/indices.rs`, `src/games/pixel.rs` : paliers d'un coffre de dictée.
- `src/saisie.rs` : `Source::Dictee` (`:28-49`).
- `src/storage/sqlite.rs` : migration 4, `source` admet `dictee` ; `VERSION_SCHEMA` à 4.
- `src/cli.rs` : `dictee-essai "<texte>" [-n 2]` : synthétise, construit le pack et met
  des coffres en file, sans Telegram.
- `tests/coffre_du_soir.rs`, `tests/pixel.rs`, `tests/indices.rs`, `tests/stockage.rs`,
  `tests/commun/mod.rs`, et les cas d'oracle qui citent la description de la clé.

**Conception.**
- À la pose, l'agent écrit dans le stockage `mine_de_rien:dictee` le son et la phrase à
  trou du coffre actif ; au rangement et au retrait, il les efface.
- L'Écouteur : un objet à clic droit détecté par un score (`minecraft.used:…`), avec un nom
  et une apparence à lui. À chaque clic, le datapack rejoue le son et réaffiche la phrase à
  trou depuis le stockage, sans attendre le tour de 3 secondes de l'agent. Sans coffre de
  dictée actif, il ne fait rien. Donné avec la clé, s'il n'en a pas déjà un.
- Annonce d'un coffre de dictée : message, phrase à trou, mode d'emploi de dictée, son.
  Au rappel de connexion : pareil. Un coffre de maths ou d'anglais garde ses messages, au
  caractère près, sauf la description de la clé.
- Condition de pose : la connexion en cours de l'enfant a commencé après la construction du
  pack. Sinon le coffre attend, et les parents sont prévenus une fois (« il doit se
  reconnecter pour entendre la dictée »). Sans `[voix]` ou sans `[pack]`, un coffre de
  dictée n'est jamais créé.
- Indices : palier 1, le son lent, avec une phrase fixe de Pixel ; palier 2, « Le mot a N
  lettres » ; palier 3, « Il commence par X et a N lettres ». `indices()` rend du texte
  (`src/exercises/indices.rs:780`) : le palier 1 est une action de plus dans Pixel, à
  dessiner ici. Le filtre `contient_la_reponse` s'applique comme ailleurs.
- La clé déjà dans l'inventaire de l'enfant porte l'ancienne description : la remplacer à
  la première pose après la mise à jour.
- Migration 4 : la contrainte `CHECK` de `source` oblige à recréer la table, comme la
  migration 3. Les bases `tests/donnees/bases/v1` à `v3` doivent arriver en version 4 sans
  rien perdre.
- Contournements, à écrire dans l'en-tête du module comme pour le coffre du soir : la
  feuille de la maîtresse est à la maison (c'est de la préparation, le parent la garde s'il
  veut) ; le pack sur le disque du jeu ne contient que des sons numérotés ; le volume
  « Voix » à zéro rend le coffre muet mais pas plus facile ; essayer au hasard coûte
  toujours un niveau par essai.

**Vérification automatique.** Les commandes communes, plus :
- les commandes d'un coffre de dictée posent le verrou avec la clé à majuscule et
  apostrophe correctement échappée ; l'annonce contient la phrase à trou et jamais la clé ;
  le rangement efface le stockage ; un coffre de maths n'écrit rien dans le stockage et son
  mode d'emploi dans le chat dit toujours « en minuscules » ; les trois paliers ne
  contiennent jamais le mot ; les trois bases de test migrent en version 4 ;
- les tests `oracle_python` passent, une fois corrigée à la main la seule description de la
  clé dans leurs fichiers ;
- `/essai-en-jeu` : un coffre verrouillé par « s'élève », puis par « Dubaï », est accepté et
  `data get block` restitue la clé à l'identique ; la fonction `redire` s'exécute sans
  erreur.

**Vérification en jeu.** Sur le serveur d'essai : `cargo run -- --config config.essai.toml
dictee-essai "Dans la ville de Dubaï s'élève la plus haute tour jamais construite." -n 2`,
`lancer` dans un autre terminal, puis se reconnecter. À l'atelier : la phrase s'entend à
l'annonce, le chat montre la phrase avec `_____`, le panneau commence par « Dictée : ».
Clic droit avec l'Écouteur en main : la phrase est relue aussitôt. Sur l'enclume, taper le
mot avec une faute : le coffre reste fermé. Le taper juste, avec sa majuscule ou son
apostrophe : il s'ouvre. Écrire « Pixel, aide-moi » trois fois dans le chat (touche `T`) :
lecture lente, nombre de lettres, première lettre.

### Phase 5 : La dictée depuis Telegram

**Contexte.** Le parent n'a encore aucun moyen d'envoyer une dictée, et le canal ne sait
envoyer que du texte. Le canal n'a pas de tâche de fond : pendant un traitement long, les
autres messages attendent chez Telegram (écart assumé du passage à Rust). La fabrication
des sons d'une dictée est un traitement long.

**Fichiers.**
- `src/storage/sqlite.rs` : migration 5, tables `dictees` (jour, texte, mots imposés,
  statut, date du pack) et `dictee_phrases`.
- `src/photo.rs` : consigne et interprétation pour un texte de dictée.
- `src/adapters/channel/base.rs`, `telegram.rs` : `Canal::envoyer_audio`.
- `src/orchestrateur.rs` : `/dictee`, photo légendée « dictée … », `/oui`, `/non`, aide,
  accusé de réception.
- `src/dictees.rs` (nouveau) : de la dictée confirmée aux sons et au pack.
- `Cargo.toml` : ce que l'envoi de fichier demande à `teloxide`, sans second `reqwest`.
- `docs/installation.md`.
- `tests/orchestrateur.rs`, `tests/photo.rs`, `tests/telegram.rs`, `tests/stockage.rs`,
  `tests/donnees/photo/dictee-*.json`.

**Conception.**
- Commandes : `/dictee <jour> <texte>`, `/dictee <jour> mots : a, b, c`, `/dictee` seul
  (liste des dictées en cours, avec leur numéro), `/dictee annuler <n>`,
  `/dictee mots <n> a b c` (mots imposés, vérifiés contre le texte).
- Photo : une légende qui commence par « dictée » (accent facultatif, comparée par
  `normaliser`) l'envoie vers la lecture de texte ; sinon c'est une fiche, comme
  aujourd'hui. Consigne au LLM sans aucun exemple, comme pour les fiches
  (`src/photo.rs:45-46`) : recopier le texte imprimé, mot pour mot, sans rien corriger ni
  compléter. La phase commence par un essai sur une vraie feuille de la maîtresse ; si le
  texte n'est pas recopié à l'identique, la photo sort du périmètre et le texte tapé reste
  la seule voie. Seules des réponses du modèle entrent dans le dépôt, jamais la photo.
- Réponse au parent : accusé de réception immédiat (« Je prépare la dictée… »), puis les
  phrases numérotées, le nombre de mots candidats, le jour retenu en toutes lettres avec sa
  date, l'aperçu audio (toutes les phrases à la suite, un seul fichier) et « Compare à la
  feuille et écoute : /oui ou /non ». Envoi de l'audio : par `sendDocument` ou `sendAudio`
  de `teloxide`, selon ce que Telegram accepte de jouer pour de l'Ogg Vorbis. À vérifier
  sur le vrai Telegram.
- `/oui` : la dictée est enregistrée, les sons des deux vitesses sont produits, le pack est
  reconstruit avec les sons de toutes les dictées en cours. Aucun coffre n'est créé ici :
  c'est la phase 7. En attendant, `/dictee seance <n>` met tout de suite une séance en
  file, et restera utile pour une séance de plus à la demande.
- La dictée en attente de `/oui` vit en mémoire, une par parent, 30 minutes, à côté de
  `LectureEnAttente` (`src/orchestrateur.rs:303`). `/oui` et `/non` servent les deux.
- Une dictée dont le jour est passé depuis 30 jours est retirée du pack.
- Le jour se calcule avec la date du fuseau de la maison. Tant que la phase 7 n'a pas
  apporté le fuseau, la date UTC sert, et le test de 23 h 30 attend la phase 7.

**Vérification automatique.** Les commandes communes, plus, avec une voix et une horloge
simulées : « vendredi » un mardi donne le vendredi de la semaine, un vendredi donne le
suivant ; un jour passé est refusé ; un texte sans mot candidat est refusé ; `/oui`
enregistre la dictée et reconstruit le pack ; `/non` n'écrit rien ; une photo légendée
« dictée vendredi » avec un LLM distant est refusée sans appel ; une photo sans le mot
« dictée » suit l'ancien chemin, et les cas d'oracle des commandes de parents passent sans
retouche hors du texte de l'aide ; un mot imposé absent du texte est refusé ; `/dictee
seance` crée des coffres de source `dictee` ; contre le faux serveur de l'API Bot, l'audio
part en un seul envoi, au bon parent ; les bases de test migrent en version 5.

**Vérification en jeu.** Avec le bot d'essai : envoyer `/dictee vendredi` suivi du texte de
la dictée 6. Vérifier les phrases, écouter l'aperçu dans Telegram, `/oui`. Taper
`/dictee seance 1`, se reconnecter au serveur d'essai : quatre coffres dans `/file`, le
premier à l'atelier avec sa phrase lue. Recommencer avec la photo d'une feuille de dictée,
puis `/non`.

### Phase 6 : Lire ce que l'enfant a tapé

**Contexte.** Indépendante de la dictée, utile à tous les coffres. La clé est un objet
marqué (`mdr_cle`, `src/games/coffre_du_soir.rs:140-142`) : son nom est lisible dans les
données du joueur. La surveillance d'un coffre actif est dans
`src/orchestrateur.rs:144-189`.

**Fichiers.**
- `src/games/coffre_du_soir.rs` : lecture du nom de la clé, nouvelle méthode du trait
  `Coffre`.
- `src/orchestrateur.rs` : une tentative par nouveau nom, pendant qu'un coffre est actif.
- `src/storage/sqlite.rs` : écriture et lecture des `tentatives` (pas de migration, la
  table existe).
- `src/rapport.rs` : graphies dans le rapport et dans la notification d'ouverture.
- `tests/coffre_du_soir.rs`, `tests/orchestrateur.rs`, `tests/rapport.rs`,
  `tests/commun/mod.rs`, et les cas d'oracle touchés.

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
- Rien de ce que l'enfant a tapé n'est réaffiché dans le jeu, ni écrit dans le journal de
  l'agent.
- Notification d'ouverture : « Coffre n°12 ouvert en 4 min, sans aide (Dictée : la _____
  haute → plus). Essais : plu. » Sans essai raté, la notification reste celle
  d'aujourd'hui, au caractère près.
- Oracle : une commande de plus part à chaque tour de surveillance. Les scénarios
  transcrits (`Rejeu`, `tests/commun/mod.rs`) qui refusent toute commande en trop sont
  corrigés à la main pour cette lecture, et pour elle seule.

**Vérification automatique.** Les commandes communes, plus :
- nom inchangé, aucune tentative ; « plu » puis « plus » donnent une tentative fausse et
  une juste ; le nom présent à la pose n'est pas compté ; le rapport liste les graphies ; un
  coffre de maths est suivi de la même façon ; une graphie tapée n'apparaît pas dans le
  journal capturé ;
- `/essai-en-jeu` : la commande renvoie le nom d'une clé renommée, et une réponse
  reconnaissable quand la clé n'a pas de nom ou n'est pas dans l'inventaire.

**Vérification en jeu.** Sur le serveur d'essai, un coffre `/coffre 348 ÷ 12`. Renommer la
clé « 28 », la prendre, attendre 5 secondes, puis « 29 » et ouvrir. Telegram annonce
l'ouverture avec « Essais : 28 », et `/rapport` le montre.

### Phase 7 : La séance de chaque jour

**Contexte.** Dernière brique : plus personne ne déclenche rien. S'appuie sur les dictées
enregistrées (phase 5) et sur les tentatives (phase 6). `chrono` est compilé sans fuseaux
et l'image n'a pas de base de fuseaux.

**Fichiers.**
- `src/seance.rs` (nouveau) : décide s'il faut une séance, et laquelle.
- `src/orchestrateur.rs` : appel à la connexion de l'enfant.
- `src/storage/sqlite.rs` : migration 6, date de la dernière séance par dictée ; lecture de
  l'historique d'un mot.
- `src/config.rs`, `config.example.toml`, `config.essai.example.toml` : section `[dictee]`
  (coffres par séance, de 3 à 5, 4 par défaut ; `fuseau = "Europe/Paris"`).
- `Cargo.toml` : `chrono-tz`, qui embarque la table des fuseaux.
- `src/dictees.rs` : le jour de `/dictee` passe au fuseau de la maison.
- `src/rapport.rs` : mots acquis et mots à retravailler par dictée.
- `tests/seance.rs`, `tests/orchestrateur.rs`, `tests/rapport.rs`, `tests/config.rs`.

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
- Un fuseau inconnu est refusé à la lecture de la configuration, avec les autres refus.
- L'horloge reste injectée (`maintenant`, `src/storage/sqlite.rs:140`).

**Vérification automatique.** Les commandes communes, plus, avec une horloge simulée : une
connexion lundi crée 4 coffres, une seconde connexion lundi n'en crée pas, mardi en crée 4
autres ; le jour de la dictée et après, rien ; un jour sans connexion n'est pas rattrapé ;
un mot raté lundi ne sort pas mardi et sort mercredi ; un mot réussi deux fois ne sort
plus ; à 23 h 30 heure de Paris, le jour compté est celui de Paris, pas celui d'UTC, en été
comme en hiver ; les coffres de `/coffre` déjà en file restent devant ; les bases de test
migrent en version 6 ; `docker run --rm <image> mine-de-rien --version` marche toujours et
la taille de l'image est notée.

**Vérification en jeu.** Sur le serveur d'essai, après une dictée confirmée la veille (ou
dont la date de confirmation a été reculée à la main dans la base d'essai) : se connecter,
quatre coffres arrivent sans rien taper sur Telegram. Rater exprès un mot, ouvrir les
quatre, se déconnecter et se reconnecter : aucune nouvelle séance. `/rapport` montre le mot
raté et sa graphie.

## Retours de jeu

*(à remplir au fil des phases : voix choisie, encodeur retenu, rechargement du pack, taille
de l'image, écarts constatés.)*

**Phase 1.**
- Voix choisie par le parent le 2026-11-10, à l'écoute des trois voix françaises de Piper
  sur les dictées 5 et 6 : **`fr_FR-siwis-medium`, à la vitesse 1,4**. Les deux autres
  (`upmc`, `tom`) « sonnent trop synthétique ». La vitesse 1,4, essayée comme vitesse lente,
  devient la vitesse normale ; la vitesse lente du premier indice passe à 1,8, que le parent
  n'a pas encore écoutée.
- Piper lit une dictée entière en une à deux secondes sur le poste ; une phrase lue et
  convertie par `mine-de-rien voix` prend 0,1 à 0,2 s. À la vitesse 1,4, la première phrase
  de la dictée 6 dure 4,6 s, contre 3,3 s au débit naturel de la voix.
- Le fichier produit est reconnu par `ffprobe` (Vorbis, mono, 22 050 Hz, environ 50 kb/s,
  29 Ko pour 3,3 s) et décodé sans erreur par `ffmpeg`. Minecraft ne l'a pas encore lu.
- Image de l'agent, construite pour les deux architectures sans émulation, comme avant :
  8,04 Mo pour amd64 et 6,56 Mo pour arm64 (7,09 et 5,82 Mo sans l'encodeur). Le binaire
  statique de l'image amd64 lit et convertit une phrase (Vorbis mono à 22 050 Hz).
- Le serveur de Piper sait télécharger des voix (`/download`, `/all-voices`). L'agent
  n'appelle pas ces routes, et le compose place le conteneur sur un réseau `internal` :
  vérifié, l'agent le joint et lui n'atteint pas Internet. Le téléchargement de la voix
  est un service ponctuel à part, lancé à la main une fois.
- Le compose d'essai, lui, publie la voix sur `127.0.0.1:5055` pour `cargo run`, sur le
  réseau ordinaire : c'est le poste du parent, pas le serveur de la maison.
- `voix: null` aurait cassé l'oracle de la configuration : la section absente ne se
  sérialise pas, l'oracle n'est pas touché.
- La sous-commande refuse sans `[voix]` : « Section [voix] absente de la configuration. »

**Phase 2.**
- Essai sur Paper 26.3 (serveur d'essai, sans joueur) : `playsound mine_de_rien:dictee.1
  voice …` et `stopsound … voice` répondent « No player was found », sans erreur de
  syntaxe ; le stockage s'écrit et se relit ; la fonction à macro s'exécute avec le stockage
  ou avec `{son:"…"}`, et échoue proprement sans argument.
- Un vrai pack de deux phrases (58 Ko) est relu sans erreur par `unzip -t`, par `zipfile`
  de Python et par `jar` de Java, le plus proche de ce que fait Minecraft.
- Le service, lancé par `servir-pack` : le pack téléchargé est identique à l'octet ; la
  base voisine et une adresse en `../` reçoivent 404 ; `SIGTERM` l'arrête aussitôt.
- **Non prouvé, faute de joueur** : que le jeu propose le pack, qu'il joue le son, et
  surtout qu'il le retélécharge à la reconnexion sans redémarrage du serveur. C'est la
  partie à jouer, et elle décide du repli.
- Partie jouée, 2026-11-10, premier essai : aucune proposition de pack, et « Unable to play
  unknown soundEvent: mine_de_rien:dictee.1 » dans le journal du jeu. Cause : dans
  `servers.dat` de l'instance XMCL, les deux serveurs portent `acceptTextures = 0`, soit
  « Packs de ressources du serveur : Désactivés ». Le jeu refuse alors le pack en silence.
  `docs/rejoindre-le-serveur.md` le dit maintenant. Le serveur, lui, avertit au démarrage :
  « You specified a resource pack without providing a sha1 hash. Pack will be updated on
  the client only if you change the name of the pack » : le rechargement à la reconnexion
  reste donc à prouver, et le repli est plausible.
- Partie jouée, 2026-11-10, second essai, packs de serveur réglés sur « Activés » : le pack
  se charge, la phrase s'entend par `playsound` et par la fonction du datapack. Après
  inversion des deux sons et reconnexion, **sans redémarrer le serveur**, c'est bien la
  nouvelle phrase qui s'entend : l'hypothèse tient, le repli n'est pas nécessaire, malgré
  l'avertissement du serveur.
- Le parent a demandé d'enchaîner les phases 3 à 7 et de tout jouer à la fin : elles sont
  livrées sans leur partie jouée, une par commit.
- `lancer` démarre le service quand `[pack]` existe ; le port 8765 est publié par le
  compose même sans `[pack]`, personne n'y répond alors.

**Phase 3.**
- Un trou est une `Position` (phrase, mot), pas un mot : « tour » et « tours », ou deux
  « la », sont des trous distincts. Les poids de la phase 7 se donnent par position.
- Les mots imposés se comparent sans tenir compte de la casse (« Personnes » impose
  « personnes »). Un mot imposé de poids nul ne sort pas : c'est ce qui laissera reposer un
  mot raté la veille.
- Le voisin identique au mot (« Nous nous aimons ») n'est pas affiché sur le panneau : il
  donnerait la réponse.
- Les apostrophes sont ramenées à celle du clavier dans tout le texte, pas seulement dans
  la clé : la phrase affichée et la clé restent cohérentes.
- Une coupe de phrase se trompe sur une abréviation (« M. Dupont ») : le parent le voit
  dans les phrases numérotées de la confirmation (phase 5).
- Le port RCON du modèle `config.essai.example.toml` avait été changé par erreur en 25567
  (le port du jeu de l'essai) dans un commit de la phase 2 : remis à 25576.
