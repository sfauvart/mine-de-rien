# L'enfant entend la dictée et Pixel tout de suite, sans que son jeu se recharge

> **Reprise de session** : lire « Pourquoi », « Décisions actées » et « Statut »,
> puis aller à la première phase non terminée.

Écrite le 2026-11-11, le jour où la dictée audio a été close
([spec](../done/2026-11-dictee-audio.md)) et publiée en 0.2.0.

## Pourquoi

La dictée se joue, mais ses sons voyagent dans un pack de ressources, et Minecraft ne
recharge un pack qu'à la connexion. Retours du parent, partie jouée le 2026-11-11 :

- se déconnecter et se reconnecter à la main pour avoir les sons « est pénible » ;
- le renvoi automatique livré ensuite (commande `transfer`) fonctionne, mais « ça ne va pas
  plaire à l'enfant, ça donne un comportement agaçant ».

Ce que le code fait aujourd'hui pour vivre avec cette limite :

- un coffre de dictée n'est posé que si l'enfant s'est connecté après la fabrication du pack
  (`src/orchestrateur.rs:235-265`) ; sinon l'agent renvoie son jeu vers le serveur, ou lui
  demande de se reconnecter (`src/orchestrateur.rs:389-395`, `src/games/dictee.rs:89-97`) ;
- l'agent fabrique un zip et le sert en HTTP sur le réseau de la maison (`src/pack.rs`,
  `src/service_pack.rs`, port 8765 et `RESOURCE_PACK` de `docker-compose.yml`) ;
- le son est joué par `playsound` dans une fonction du datapack
  (`datapacks/mine_de_rien/data/mine_de_rien/function/dictee/jouer.mcfunction`) : seul un
  son déjà dans le pack peut être joué. Rien ne peut donc être dit à la volée ;
- Pixel n'écrit que dans le chat (`src/games/pixel.rs`). Pour un enfant dyslexique, chaque
  indice est un texte de plus à lire.

Recherche du 2026-11-11 (sources lues, rien d'essayé en jeu) :

- le mod **Simple Voice Chat** est publié pour Minecraft 26.3 : plugin serveur
  `bukkit-2.6.27` (Paper compris) et mod client Fabric `2.6.27+26.3`, tous deux du
  2026-10-28 et marqués « beta » ; la dernière version « release » du plugin est la 2.6.25 ;
- son API permet à un plugin de jouer du son à **un seul joueur** (canal statique) et, plus
  tard, de recevoir son micro ;
- aucun plugin existant ne joue un fichier à un seul joueur sur Paper 26.3 : AudioPlayer
  n'existe que pour un serveur Fabric et son son est entendu de tous les joueurs proches ;
  PurpleMusic s'arrête à Minecraft 1.21.9 ;
- OpenAudioMc ne s'héberge pas entièrement à la maison (compte chez un tiers).

**Cible** : l'enfant, resté connecté, entend la phrase de sa dictée et les messages de Pixel
dans la seconde, par le chat vocal du jeu ; le pack de sons, son service HTTP et le renvoi
automatique n'existent plus.

## Décisions actées

| Sujet | Décision |
|---|---|
| Transport du son | Le chat vocal de **Simple Voice Chat**, auto-hébergé : un plugin sur le serveur Paper, un mod Fabric dans le jeu de l'enfant. Seul transport : pas de repli. |
| Plugin maison | Un petit plugin Paper **en Java**, dans le dépôt (`plugin/`), qui joue à un seul joueur un son que l'agent lui désigne. Il ne décide rien : ni quoi dire, ni quand. Aucune dépendance embarquée ; l'API de Paper et celle du chat vocal sont fournies par le serveur. |
| De l'agent au plugin | L'agent écrit le son dans un dossier partagé avec le serveur, puis envoie une commande par RCON. Le plugin n'ouvre aucun port et n'appelle personne. |
| Format échangé | PCM brut, mono, 16 bits, 48 000 Hz : ce que le chat vocal attend. L'agent rééchantillonne la voix de Piper (22 050 Hz) lui-même, sans outil externe. |
| Qui peut commander | La console et RCON seulement. Un joueur, même opérateur, ne peut pas faire jouer un son : la commande ne sert pas à entendre la phrase d'un autre coffre. |
| Ce qui est dit | La phrase de dictée (pose du coffre, rappel de connexion, Écouteur, indice lent de Pixel), et **les messages que Pixel et l'agent écrivent à l'enfant dans le chat** : annonces de coffre, indices, réponses de Pixel, « C'est la bonne réponse ! ». |
| Ce qui n'est jamais dit | La phrase à trou telle qu'affichée (« _____ » ne se lit pas), le mode d'emploi répété à chaque coffre, et tout texte qui contiendrait la réponse : la règle est celle du chat, la voix ne dit rien que le chat n'écrit pas. |
| Le texte reste | Tout ce qui est dit est aussi écrit dans le chat, comme aujourd'hui. La voix s'ajoute, elle ne remplace rien : sans chat vocal, le jeu reste jouable, sauf la dictée. |
| Voix | La même pour tout : Piper, voix et vitesse de `[voix]`. Les messages du chat sont fabriqués à la volée (0,1 à 0,2 s par phrase, mesuré en phase 1 de la dictée). |
| Un son à la fois | Un nouveau son attend la fin du précédent ; la phrase de dictée demandée par l'Écouteur passe devant et coupe ce qui joue. |
| Écouteur | Le clic droit reste immédiat : c'est le plugin qui le voit et rejoue la phrase du coffre, que l'agent lui a désignée à la pose. Le datapack garde l'affichage de la phrase à trou dans le chat. |
| Enfant sans chat vocal | L'agent demande au plugin si le jeu de l'enfant est relié au chat vocal. Sinon : aucun coffre de dictée n'est posé, les coffres de calcul continuent, l'enfant lit « Demande à un parent d'installer le chat vocal », et les parents reçoivent une notification, une fois par jour. |
| Micro de l'enfant | Le plugin ne l'écoute pas. Les groupes du chat vocal sont désactivés. Le guide dit de passer l'étape du micro à l'installation. |
| Pack de sons | Supprimé avec tout ce qui le porte : `src/pack.rs`, `src/service_pack.rs`, la section `[pack]`, la sous-commande `servir-pack`, le port 8765, `RESOURCE_PACK`, `ACCEPTS_TRANSFERS`, le renvoi automatique, la condition « connecté après le pack ». Une configuration qui porte encore `[pack]` démarre quand même. |
| Aperçu Telegram | Inchangé : le parent reçoit toujours `dictee-apercu.ogg`. L'encodeur Ogg Vorbis reste dans le binaire pour lui seul. |
| Livraison du plugin | Le fichier `.jar` est construit par la même chaîne que l'image et livré **dans l'image de l'agent**, à côté du binaire. Le service ponctuel qui recopie le datapack recopie aussi le plugin. Même version pour l'agent, le datapack et le plugin. |
| Versions | Le plugin Simple Voice Chat est épinglé dans le compose à une version précise. Le guide dit d'épingler aussi `MC_VERSION` : le mod du jeu de l'enfant doit correspondre à la version de Minecraft. |

Hypothèses prises sans poser la question, à corriger dans ce fichier si elles sont fausses :

- l'enfant joue avec XMCL, où ajouter Fabric et un mod à une instance se fait à la souris ;
- un seul enfant écoute à la fois (le pseudo de la configuration), comme partout ailleurs ;
- les dictées déjà confirmées sont relues par Piper à la demande : leurs sons au format du
  pack, gardés dans la base, ne servent plus qu'à l'aperçu ;
- Pixel garde la voix de la dictée ; une seconde voix pour le distinguer est une idée pour
  plus tard.

### Ce que ces décisions renversent

- `docs/architecture.md`, décision 2 : « Pont sans plugin pour commencer. […] Un plugin
  Paper viendra seulement si un jeu exige des événements fins. » Il vient pour le son.
- `CLAUDE.md` : « L'agent est écrit en Rust : un seul crate, un seul binaire. » L'agent le
  reste ; le dépôt gagne un second programme, en Java, qui tourne dans le serveur.
- `README.md` : « Aucun plugin requis pour commencer », et « l'image ne contient que le
  binaire ». La dictée exige deux plugins et un mod ; l'image porte aussi le `.jar`.
- Spec `2026-11-dictee-audio` : « Transport du son : pack de ressources », « Mise à jour du
  pack », « Format du son : Ogg Vorbis » pour le jeu, la phase 10 entière, et le hors
  périmètre « Plugin Paper pour pousser le pack ».
- Spec `2026-11-dictee-audio`, phase 4 : « Pas de cloche à l'annonce d'une dictée » reste
  vrai, mais l'annonce elle-même est maintenant dite avant la phrase.
- `docs/rejoindre-le-serveur.md` : « Packs de ressources du serveur : Activés » n'est plus
  nécessaire.

## Hors périmètre

- **L'enfant parle à Pixel au micro** : une spec à part (reconnaissance vocale locale,
  conversation). Le plugin est écrit pour qu'on puisse y ajouter l'écoute du micro, rien de
  plus.
- Garder le pack de sons en repli pour une famille sans mod : écarté par le parent.
- Remplacer Paper par un serveur Fabric pour utiliser AudioPlayer : son entendu de tous,
  et tout le projet à revoir.
- OpenAudioMc, ou une page web locale qui joue le son hors du jeu.
- Que les joueurs se parlent entre eux par le chat vocal : les groupes sont désactivés, la
  parole de proximité n'est ni réglée ni documentée.
- Lire à voix haute ce que l'enfant écrit, les panneaux, le tableau de la dictée ou les
  descriptions d'objets.
- Une voix différente pour Pixel, ou un réglage du volume depuis Telegram.
- Bedrock, et les clients sans mod (lanceur officiel sans Fabric) : la dictée y est muette.
- Jouer hors de la maison : le port du chat vocal n'est publié que sur le réseau local.

## Vie privée

- **Vers l'extérieur, à l'installation seulement** : le serveur télécharge le plugin Simple
  Voice Chat sur Modrinth au démarrage, comme il télécharge déjà Paper ; le parent
  télécharge le mod et Fabric pour le jeu de l'enfant. Rien pendant le jeu.
- **Vérifié en phase 1, dans les fichiers** (plugin `voicechat-bukkit-2.6.27.jar`, mod
  `voicechat-fabric-2.6.27+26.3.jar`) : ni l'un ni l'autre ne contient d'adresse internet,
  de client HTTP ni de bStats. Le serveur d'essai, démarré avec les deux plugins, ne garde
  aucune connexion sortante ouverte. Le journal du jeu, lui, reste à relire à la partie
  jouée de la phase 1.
- **Sur le réseau de la maison** : un port UDP de plus (24454), entre le jeu de l'enfant et
  le serveur. Le service HTTP du pack (port 8765) disparaît.
- **Le micro de l'enfant** : le mod sait l'envoyer au serveur. Mine de rien ne l'écoute pas
  et ne l'enregistre pas ; un autre joueur admis sur le serveur et équipé du mod pourrait
  l'entendre à proximité. Le guide fait passer l'étape du micro, et le dit.
- **Vers la voix** : le texte des messages de Pixel et des annonces part au conteneur local
  de Piper, en plus du texte des dictées. Ces messages peuvent venir du LLM configuré ; ce
  chantier n'envoie rien de plus à un LLM.
- **Sur le disque** : des sons de quelques secondes dans le dossier partagé, effacés par
  l'agent une fois joués (messages) ou la dictée passée (phrases). Jamais la voix de l'enfant.
- **Dépendances** : le plugin n'embarque aucune bibliothèque. Côté Rust, aucun crate ajouté
  n'est prévu ; s'il en faut un pour rééchantillonner, il est contrôlé comme le demande
  `CLAUDE.md`.

## Statut

**Branche** : `feat/voix-en-direct`.

- [x] **Phase 1 : Le serveur dit une phrase au seul enfant**
- [x] **Phase 2 : La dictée passe par la voix en direct**
- [ ] **Phase 3 : Le plugin est livré avec l'image**
- [ ] **Phase 4 : Le pack de sons disparaît**
- [ ] **Phase 5 : Pixel et les annonces se font entendre**

La phase 1 porte tout le risque : si le son n'arrive pas, arrive en retard ou si le mod
gêne l'enfant, on s'arrête là et on revoit la spec. La phase 2 dépend de la 1, la 3 de la
2, la 4 de la 3 (on ne retire le pack qu'une fois le plugin livrable). La phase 5 ne
dépend que de la 1. **Aucune version n'est taguée entre la phase 2 et la phase 4** : la
branche y est dans un état que le compose de production ne sait pas servir.

## Conception

Les commandes Minecraft et les appels d'API cités dans les phases 2 à 5 sont **à vérifier
sur la version du serveur** (Paper 26.3, Simple Voice Chat 2.6.27) tant que `/essai-en-jeu`
ne les a pas exécutés. Ceux de la phase 1 l'ont été le 2026-11-11 (Paper 26.3 build 248) :
le plugin se charge, s'inscrit auprès du chat vocal, et `mdrvoix` répond par RCON. Ce qui
demande un joueur (le son lui-même, `relié`) attend la partie jouée.

### Phase 1 : Le serveur dit une phrase au seul enfant

**Contexte.** Prouver la chaîne entière sur le serveur d'essai, sans toucher à la dictée :
Piper → agent → fichier → plugin → chat vocal → casque du parent. Rien n'existe encore côté
Java dans le dépôt.

**Fichiers.** `plugin/` (nouveau : `pom.xml`, `plugin.yml`, deux ou trois classes Java, un
test), `docker/essai.compose.yml` (Simple Voice Chat, port UDP, dossier partagé, plugin
maison), `src/son.rs` (rééchantillonnage), un nouveau module pour la diffusion,
`src/config.rs`, `src/cli.rs` (sous-commande d'essai), `config.essai.example.toml`,
`docker/essai-plugins/` (réglages du chat vocal),
`docs/rejoindre-le-serveur.md` (installer le mod), skill `/essai-en-jeu`.

**Conception.**

- Le plugin s'appelle `mine-de-rien-voix`. Il dépend du plugin `voicechat` et obtient son
  API par le service Bukkit `BukkitVoicechatService`. Il se construit avec Maven dans un
  conteneur (`maven:…-eclipse-temurin-25`) : aucun JDK à installer sur le poste.
- Une commande, refusée à tout autre que la console et RCON :

  ```text
  mdrvoix jouer <pseudo> <fichier>      met le son dans la file du joueur
  mdrvoix couper <pseudo>               vide la file et arrête le son en cours
  mdrvoix etat <pseudo>                 répond « relié », « sans chat vocal » ou « absent »
  ```

  `jouer` et `couper` répondent `ok` ou `refusé : <raison>`. La commande porte une
  permission donnée aux seuls opérateurs (la console en est un) : un joueur ordinaire ne la
  voit pas, et le plugin la refuse à un joueur opérateur.
- `<fichier>` est un nom seul, `[a-z0-9-]+\.pcm`, cherché dans le dossier partagé et nulle
  part ailleurs. Tout autre nom est refusé avant d'ouvrir quoi que ce soit.
- Le plugin lit le son en mémoire au moment de la commande : pour un message, l'agent
  efface le fichier dès que le serveur a répondu.
- Le son est joué par un canal statique dont l'enfant est la seule cible
  (`createStaticAudioChannel`, `addTarget`, `createAudioPlayer` avec l'encodeur Opus de
  l'API, trames de 960 échantillons). Le canal porte une catégorie de volume « Mine de
  rien », réglable par l'enfant dans le mod.
- Côté agent : `Pcm` (`src/son.rs:21`) gagne un rééchantillonnage vers 48 000 Hz, écrit à
  la main. La qualité se juge à l'oreille dans cette phase ; si elle déçoit, un crate dédié
  est envisagé et contrôlé.
- Un trait de diffusion, à côté de `Coffre`, avec un faux pour les tests : `dire(son)`,
  `couper()`, `relie() -> bool`. La vraie implémentation écrit le fichier dans le dossier
  partagé (nouvelle clé `[voix] dossier_du_jeu`) puis envoie la commande par RCON.
- Sous-commande d'essai : `mine-de-rien parler "<texte>"` fait lire le texte par Piper et
  le fait entendre au pseudo configuré. (`dire`, prévu d'abord, existe déjà : il écrit dans
  le chat.)
- Réglages du chat vocal dans le compose d'essai : groupes désactivés, port **24464**/udp
  publié (24454, le port d'usage, est laissé au vrai serveur). L'adresse annoncée au jeu
  (`voice_host`) est laissée vide si cela suffit sur le réseau local ; sinon elle devient
  un réglage documenté.

**Vérification automatique.**

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` : le
  rééchantillonnage garde la durée (à un échantillon près) et ne sature pas ; le nom de
  fichier et le pseudo sont contrôlés avant d'entrer dans une commande ; la commande tient
  sous 1 446 octets.
- Test Java du plugin : lecture d'un fichier PCM en trames, refus des noms hors gabarit.
- `/essai-en-jeu`, sans joueur : le serveur démarre avec les deux plugins, sans erreur dans
  son journal ; `mdrvoix etat <pseudo>` répond « absent » ; `mdrvoix jouer` sur un fichier
  inexistant ou un nom en `../` répond un refus, sans exception.
- Le journal du serveur et celui du jeu sont relus pour toute connexion sortante après
  l'installation (voir « Vie privée »).

**Vérification en jeu.** Le parent installe Fabric et le mod dans son instance XMCL en
suivant `docs/rejoindre-le-serveur.md`, se connecte au serveur d'essai, passe l'assistant
du micro. Dans un terminal : `cargo run -- --config config.essai.toml parler "Bonjour, je
suis Pixel."`. La phrase doit s'entendre en moins d'une seconde, sans écran de chargement,
nette et à bon volume. La relancer pendant qu'elle joue : la seconde attend la fin de la
première. À noter dans « Retours de jeu » : le délai ressenti, la qualité, ce que
l'assistant du mod a demandé, et si l'icône ou les messages du mod gênent.

### Phase 2 : La dictée passe par la voix en direct

**Contexte.** La dictée joue ses phrases par `playsound`, depuis l'agent
(`src/games/dictee.rs:158`) et depuis le datapack pour l'Écouteur. La phase 1 a donné un
moyen de dire un son tout de suite. Suivre `/nouveau-jeu`.

**Fichiers.** `src/games/dictee.rs`, `src/games/coffre_du_soir.rs`, `src/orchestrateur.rs`,
`src/dictees.rs`, `src/games/pixel.rs` (indice lent), `plugin/` (Écouteur),
`datapacks/mine_de_rien/data/mine_de_rien/function/dictee/`, `tests/coffre_de_dictee.rs`,
`tests/coffre_du_soir.rs`, `tests/commun/`.

**Conception.**

- À la pose d'un coffre de dictée, l'agent s'assure que les deux sons de la phrase (normal
  et lent) sont dans le dossier partagé, au format du chat vocal ; s'ils manquent, il les
  redemande à Piper. Les fichiers portent des numéros, jamais de mots.
- La phrase est dite à la pose, au rappel de connexion et à l'indice lent de Pixel, par le
  trait de diffusion.
- L'agent désigne au plugin la phrase du coffre posé (`mdrvoix phrase <pseudo> <fichier>`,
  et sans fichier pour l'oublier à l'ouverture ou à l'annulation). Sur un clic droit avec
  l'Écouteur (objet marqué `mdr_ecouteur`), le plugin coupe ce qui joue et rejoue cette
  phrase. Le datapack ne joue plus rien : il garde le compteur de clics et l'affichage de
  la phrase à trou.
- La condition de pose devient « le jeu de l'enfant est relié au chat vocal »
  (`mdrvoix etat`), à la place de « connecté après le pack ». Sinon : pas de coffre de
  dictée, message à l'enfant, notification aux parents une fois par jour.
- Le pack n'est plus attendu par le chemin de la dictée ; son code reste en place jusqu'à
  la phase 4, et il est encore réécrit à chaque dictée confirmée, sans que rien ne le lise.

**Précisé en codant.**

- Le lien au chat vocal se fait quelques secondes après la connexion. L'agent attend donc
  20 secondes avant de conclure qu'il manque : pendant ce délai la dictée attend, et le
  rappel d'un coffre déjà posé aussi.
- Enfant absent : la dictée n'est pas posée, elle attend son retour, ses sons prêts, et
  garde sa place dans la file. (Avec le pack, elle était posée en son absence.)
- Un coffre de dictée déjà posé devant un enfant sans chat vocal reste actif : le rappel
  part écrit, l'enfant et les parents sont prévenus, et la file attend derrière lui. Le
  parent peut l'annuler.
- Voix en panne (Piper ne répond pas) : la dictée attend, les coffres de calcul continuent,
  les parents sont prévenus une fois par jour ; rien n'est demandé à l'enfant.
- Les sons d'une phrase (`phrase-N.pcm`, normal et lent) sont effacés à l'ouverture ou à
  l'annulation du coffre. Le plugin oublie la phrase désignée quand le serveur redémarre :
  l'agent la redésigne à chaque annonce et à chaque rappel.
- Le plugin reconnaît l'Écouteur à son type et à la marque `mdr_ecouteur` de ses données.
  Un clic maintenu ne relance pas la phrase plus d'une fois par seconde.

**Vérification automatique.**

- `cargo test` : avec la fausse diffusion, la phrase est dite à la pose, au rappel et à
  l'indice lent (son lent) ; rien n'est dit pour un coffre de calcul ; sans chat vocal, le
  coffre de dictée attend, celui de calcul est posé, les parents sont prévenus une seule
  fois dans la journée ; plus aucun `playsound mine_de_rien:` dans les commandes envoyées.
- Tests `oracle_python` : aucun fichier d'oracle ne change.
- `/essai-en-jeu`, sans joueur : `dictee-essai` puis `lancer` ; le dossier partagé contient
  les sons de la phrase ; le coffre attend, faute de joueur relié. (Le 2026-11-11, non joué
  tel quel : le monde d'essai portait un coffre actif du parent. Vérifiés à la place :
  `mdrvoix phrase` par RCON, avec et sans fichier, et le démarrage de `lancer`.)

**Vérification en jeu.** Sur le serveur d'essai, connecté avec le mod : envoyer une
dictée **en restant connecté** (`dictee-essai`, ou Telegram : `/dictee vendredi <texte>`,
`/oui`, `/dictee seance 3`). Le coffre doit apparaître et la phrase s'entendre sans aucune
coupure du jeu. Clic droit avec l'Écouteur : la phrase est redite aussitôt. Demander de
l'aide à Pixel : la phrase est relue lentement. Ouvrir le coffre : le suivant lit sa
propre phrase. Puis se connecter avec une instance sans le mod : aucun coffre de dictée,
le message dans le chat, la notification sur Telegram.

### Phase 3 : Le plugin est livré avec l'image

**Contexte.** Jusqu'ici le plugin ne vit que sur le serveur d'essai, construit à la main.
Le compose de production et l'image publiée (`docker/agent.Dockerfile`,
`.github/workflows/publier.yml`) ne le connaissent pas.

**Fichiers.** `docker/agent.Dockerfile`, `docker-compose.yml`, `.github/workflows/publier.yml`,
`src/cli.rs` (recopie du plugin), `tests/version.rs`, `docs/installation.md`,
`docs/publier-une-version.md`, `docs/rejoindre-le-serveur.md`, `README.md`,
`config.example.toml`, `.env.example`.

**Conception.**

- Le Dockerfile gagne une étape qui construit le `.jar` ; l'image finale contient le
  binaire et ce fichier, rien d'autre. Le plugin porte la version de `Cargo.toml`, et
  `tests/version.rs` vérifie que les deux concordent.
- Le service ponctuel du compose recopie datapack et plugin avant le démarrage du serveur
  (une sous-commande de l'agent, sans interpréteur de commandes dans l'image).
- Compose de production : Simple Voice Chat épinglé, port 24454/udp, dossier partagé entre
  l'agent (écriture) et le serveur (lecture seule), réglages du chat vocal.
- Le guide d'installation explique, geste par geste : le mod dans le jeu de l'enfant,
  l'étape du micro à passer, le volume « Mine de rien », épingler `MC_VERSION`, et ce qui
  se passe quand Minecraft sort une version que le mod ne couvre pas encore.

**Vérification automatique.**

- Les trois commandes `cargo`, et le test de version avec le plugin.
- Construction de l'image pour amd64 et arm64, sans émulation pour l'agent ; taille notée.
- Un compose de production lancé sur un dossier vide et des ports libres : le serveur
  démarre avec les deux plugins, `mdrvoix etat` répond par RCON.

**Vérification en jeu.** Aucune : rien ne change pour le joueur depuis la phase 2.

### Phase 4 : Le pack de sons disparaît

**Contexte.** Depuis la phase 2 le pack ne sert plus, mais son code, son service et ses
réglages sont encore là, ainsi que le renvoi automatique de la phase 10 de la dictée.

**Fichiers.** `src/pack.rs` et `src/service_pack.rs` (supprimés), `src/config.rs`,
`src/cli.rs`, `src/orchestrateur.rs`, `src/games/dictee.rs`, `src/games/coffre_du_soir.rs`,
le datapack (`dictee/jouer.mcfunction`), les deux fichiers compose, `config.example.toml`,
`config.essai.example.toml`, `.env.example`, `docs/installation.md`,
`docs/rejoindre-le-serveur.md`, `docs/architecture.md`, `README.md`, `CLAUDE.md`, les tests
du pack.

**Conception.** Tout ce que liste la ligne « Pack de sons » des décisions est retiré. Une
configuration qui contient encore `[pack]` se charge : la section est ignorée, et l'agent
le dit une fois au démarrage. `docs/architecture.md` reçoit un paragraphe daté sur
l'arrivée du plugin et du mod, et `CLAUDE.md` décrit `plugin/`.

**Vérification automatique.**

- Les trois commandes `cargo` ; plus aucune occurrence de `resource-pack`, `transfer`,
  `8765` ni `adresse_du_jeu` hors des specs closes.
- Une configuration avec `[pack]` se charge, avec son avertissement ; l'oracle de
  configuration n'est pas touché.
- `/essai-en-jeu` : le serveur d'essai, recréé, n'annonce plus de pack de ressources.

**Vérification en jeu.** Rejouer le scénario de la phase 2 sur le serveur d'essai recréé :
la dictée s'entend, et le jeu ne propose plus aucun pack à la connexion.

### Phase 5 : Pixel et les annonces se font entendre

**Contexte.** Tout ce que l'agent dit à l'enfant passe par des messages du chat
(`src/games/pixel.rs`, `src/games/coffre_du_soir.rs`). La phase 1 permet de dire un texte
quelconque. Indépendante des phases 2 à 4.

**Fichiers.** `src/games/pixel.rs`, `src/games/coffre_du_soir.rs`, `src/orchestrateur.rs`,
`src/config.rs`, `config.example.toml`, `tests/pixel.rs`, `tests/coffre_du_soir.rs`,
`docs/installation.md`.

**Conception.**

- Un seul point de passage : quand l'agent écrit à l'enfant un message de la liste
  ci-dessous, il le fait aussi lire par Piper et le met dans la file. Le texte part dans le
  chat tout de suite, sans attendre la voix.
- Sont dits : l'annonce et le rappel d'un coffre, les réponses et indices de Pixel, « C'est
  la bonne réponse ! », le message d'un coffre ouvert. Ne sont pas dits : le mode d'emploi,
  la phrase à trou, les messages d'erreur techniques.
- Avant la lecture, le texte est débarrassé de ce qui ne se prononce pas (codes de couleur,
  « _____ », symboles d'opération écrits en clair : « 348 ÷ 12 » se lit « 348 divisé par
  12 »). Les règles sont dans le code, pas dans un LLM.
- Si Piper ne répond pas ou si l'enfant n'est pas relié, le message reste écrit et rien
  d'autre ne se passe : jamais d'erreur dans le chat de l'enfant pour une voix manquante.
- Réglage `[voix] dire_le_chat`, vrai par défaut quand `[voix]` existe.
- Les sons des messages sont effacés du dossier partagé une fois joués.

**Vérification automatique.**

- `cargo test` : chaque message de la liste est dit une fois, dans l'ordre ; le mode
  d'emploi ne l'est pas ; rien de ce qui est dit ne contient la réponse d'un coffre, sur
  des centaines de graines ; « ÷ », « × » et « _____ » sont réécrits ; une voix en panne ne
  change ni les messages du chat ni l'état du coffre.
- Tests `oracle_python` : les commandes et messages du chat ne changent pas.
- `/essai-en-jeu`, sans joueur : un coffre de calcul posé, puis une demande d'aide à
  Pixel ; le dossier partagé reçoit les sons puis se vide.

**Vérification en jeu.** Sur le serveur d'essai, avec le mod : demander un coffre de
calcul. L'annonce doit s'entendre. Écrire « Pixel, aide-moi » dans le chat : l'indice est
écrit et dit. Renommer la clé avec une majuscule de trop : « C'est la bonne réponse ! »
s'entend. À juger : est-ce que la voix aide ou bavarde trop, et les énoncés de calcul
sont-ils lus correctement. Noter l'avis de l'enfant dans « Retours de jeu ».

## Retours de jeu

- **Phase 1, 2026-11-11.** Le parent a installé Fabric et le mod, s'est connecté au serveur
  d'essai et a lancé `parler` : « ça fonctionne, j'ai bien la voix qui arrive dans le jeu ».
  Délai, qualité, assistant du micro et icône du mod : pas de remarque à ce stade. Le
  journal du jeu n'a pas été relu pour les connexions sortantes.

*(à compléter au fil des phases.)*
