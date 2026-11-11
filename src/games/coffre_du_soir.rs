//! Coffre du soir : un coffre verrouillé par la réponse, à renommer sur une enclume.
//!
//! **Le désir.** Des diamants dans un coffre, à l'atelier près de sa base. Le coffre ne s'ouvre
//! qu'en tenant un objet dont le nom est la réponse : le calcul est la clé, pas une barrière
//! posée devant.
//!
//! **Commandes**, vérifiées sur Minecraft Java 26.3 (Paper), `C` coffre, `P` panneau, `E`
//! enclume (voir `atelier.rs`) :
//!
//! ```text
//!     data modify block C lock set value {components:{"minecraft:custom_name":"29"}}
//!     item replace block C container.13 with minecraft:diamond 3
//!     item replace block C container.11 with minecraft:paper[minecraft:custom_name="Preuve n°12",
//!         minecraft:custom_data={mdr_coffre:12}]
//!     setblock P minecraft:oak_sign[rotation=0]{front_text:{messages:[...]},is_waxed:1b}
//!     setblock E minecraft:anvil[facing=east]      # enclume neuve, elle s'abîme à l'usage
//!     give <pseudo> minecraft:trial_key[minecraft:item_name="Clé du coffre",
//!         minecraft:lore=[...],minecraft:custom_data={mdr_cle:1b}] 1
//!     xp add <pseudo> 3 levels
//!     execute at <pseudo> run playsound minecraft:block.bell.use master <pseudo> ~ ~ ~ 1 1
//! ```
//!
//! L'atelier est relu dans le monde à chaque tour de surveillance (`actualiser_atelier`). S'il
//! n'y en a pas, tout ce qui a besoin de sa position refuse avec `AtelierAbsent`, sans rien poser.
//!
//! Le verrou est posé avant d'y mettre la récompense : à aucun moment le coffre n'est ouvert
//! avec quelque chose dedans.
//!
//! **La clé** : une clé (`trial_key`) dont la description explique les gestes, donnée avec les
//! niveaux si l'enfant n'en a plus une sur lui. C'est elle qu'il renomme : il n'a pas à deviner
//! quel objet poser sur l'enclume. Elle sert d'un coffre à l'autre, renommée à chaque fois
//! (un simple renommage coûte toujours un niveau, pas plus). N'importe quel autre objet renommé
//! ouvre aussi le coffre : le verrou ne regarde que le nom. Le mode d'emploi est répété dans le
//! chat à chaque annonce et à chaque rappel, car l'enclume ne s'explique pas d'elle-même : le
//! champ du nom reste grisé tant que rien n'est posé à gauche, et il n'y a pas de bouton pour
//! valider, on clique sur le résultat pour le prendre.
//!
//! **Casse et espaces.** Le verrou compare lettre à lettre, l'enfant non : « Plus », « plus »
//! et « plus » suivi d'une espace valent la même réponse. À chaque tour, l'agent lit le nom de
//! la clé ; s'il vaut la réponse (`meme_cle`), le verrou prend ce nom tel qu'il est écrit, et
//! l'enfant lit « C'est la bonne réponse ! » avec un tintement. Il attend donc ce message, au
//! plus 3 secondes, avant d'ouvrir. Une réponse écrite exactement ouvre sans attendre.
//!
//! **Phrase d'une dictée.** Voir `dictee.rs` : elle s'entend par le chat vocal. Une voix en
//! panne ne casse pas le coffre : la phrase à trou reste écrite dans le chat.
//!
//! **Ce qui est aussi dit à voix haute** (`diffusion::Parole`), en plus d'être écrit :
//! l'annonce d'un coffre, son rappel à la connexion (avec l'énoncé d'un calcul, « 348 ÷ 12 »
//! lu « 348 divisé par 12 »), et « C'est la bonne réponse ! ». Jamais le mode d'emploi, répété
//! à chaque coffre, ni la phrase à trou d'une dictée, ni un texte qui donnerait la réponse.
//! Pour une dictée, l'annonce est dite avant la phrase.
//!
//! **Détection**, toutes les 3 secondes quand l'enfant est connecté :
//!
//! ```text
//!     clear <pseudo> minecraft:paper[minecraft:custom_data={mdr_coffre:12}] 0
//! ```
//!
//! compte sans retirer (« Found 1 matching item(s) on player ... »). Le papier n'existe que dans
//! le coffre : l'avoir sur soi prouve qu'on l'a ouvert.
//!
//! **Récompense** : la valeur de `[coffre]` dans `config.toml`, identique pour tous les coffres.
//!
//! **Rangement** après ouverture : la preuve est retirée de l'inventaire, le panneau disparaît,
//! le verrou saute. Ce qui reste dans le coffre n'est pas effacé : le coffre suivant attend
//! qu'il soit vide, pour ne jamais détruire une récompense oubliée.
//!
//! **Rapport aux parents** : coffre posé, ouvert avec la durée depuis la pose, disparu et
//! reposé. Jamais le pseudo de l'enfant.
//!
//! **Contournements.**
//! - *Casser le coffre* : zone en mode aventure autour de l'atelier (datapack). Un coffre qui
//!   disparaît quand même (creeper, créatif) est reposé et l'événement noté. Limite connue :
//!   une explosion fait tomber le contenu, preuve comprise, et compte donc comme une ouverture.
//! - *Opérateur, créatif* : l'enfant ne doit pas être opérateur sur le serveur réel (voir
//!   `docs/installation.md`). En créatif, la zone ne s'applique pas.
//! - *Répondre au hasard* : chaque renommage coûte un niveau ; 3 niveaux sont offerts par coffre,
//!   au-delà c'est son expérience à lui.
//! - *Lire la réponse* : elle n'est que dans les données du bloc, invisibles sans `/data`
//!   (réservé aux opérateurs). Le chat de l'agent ne la donne jamais.
//! - *Faire résoudre par un autre* : possible, le rapport dit « ouvert », pas « résolu seul ».

use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::LazyLock;

use async_trait::async_trait;
use chrono::TimeDelta;
use regex::Regex;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::adapters::game::base::AdaptateurJeu;
use crate::adapters::game::rcon::ErreurRcon;
use crate::dictees::Lecteur;
use crate::diffusion::Parole;
use crate::exercises::base::Exercice;
use crate::games::atelier::{Atelier, PAS_CONSTRUIT, lire_atelier, pos};
use crate::games::dictee;
use crate::games::tableau::{self, Morceau};
use crate::journal;
use crate::minecraft::texte::{self, LONGUEUR_MAX, PseudoInvalide, Style};
use crate::storage::sqlite::ExerciceStocke;
use crate::util::{json_python, repr, strip};

static ERREUR_SYNTAXE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<--\[HERE\]|Unknown|Incorrect argument|Expected|Invalid").unwrap()
});

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErreurCoffre {
    /// Le serveur a répondu autre chose que prévu.
    #[error("{0}")]
    Coffre(String),
    /// Ce monde n'a pas d'atelier ; le message va aux parents.
    #[error("{0}")]
    AtelierAbsent(String),
    /// Le serveur n'a pas répondu.
    #[error("{0}")]
    Rcon(#[from] ErreurRcon),
}

#[async_trait(?Send)]
pub trait Notificateur {
    /// Prévient les parents. Le message ne contient jamais le pseudo de l'enfant.
    async fn notifier(&self, message: &str);
}

/// Sans canal Telegram : les notifications s'affichent dans la console.
pub struct NotificateurConsole;

#[async_trait(?Send)]
impl Notificateur for NotificateurConsole {
    async fn notifier(&self, message: &str) {
        println!("{message}");
    }
}

/// Chaîne SNBT entre guillemets doubles (sous-ensemble JSON), échappée.
fn snbt(chaine: &str) -> String {
    json_python(&Value::String(chaine.to_owned()))
}

pub fn predicat_preuve(numero: i64) -> String {
    format!("minecraft:paper[minecraft:custom_data={{mdr_coffre:{numero}}}]")
}

pub const CLE: &str = "minecraft:trial_key";
pub const NOM_CLE: &str = "Clé du coffre";
pub const MODE_EMPLOI_CLE: [&str; 4] = [
    "1. Pose-moi à gauche sur l'enclume",
    "2. Écris la réponse",
    "3. Clique sur moi à droite",
    "4. Tiens-moi en main, ouvre le coffre",
];
pub const MODE_EMPLOI: &str = "Pour l'ouvrir : pose ta clé à gauche sur l'enclume, remplace son \
    nom par la réponse du panneau, écrite en minuscules, clique sur la clé renommée à droite pour \
    la prendre, puis ouvre le coffre en la tenant en main.";

pub fn predicat_cle() -> String {
    format!("{CLE}[minecraft:custom_data={{mdr_cle:1b}}]")
}

fn description_cle() -> String {
    let lore: Vec<String> = MODE_EMPLOI_CLE
        .iter()
        .map(|ligne| json_python(&json!({"text": ligne, "italic": false, "color": "gray"})))
        .collect();
    lore.join(",")
}

/// Lit le nom que porte la clé de l'enfant, où qu'elle soit dans son inventaire : c'est ce
/// qu'il vient d'écrire sur l'enclume. Vérifiée sur Minecraft Java 26.3.
pub fn commande_nom_cle(pseudo: &str) -> Result<String, PseudoInvalide> {
    Ok(format!(
        "data get entity {} Inventory[{{components:{{\"minecraft:custom_data\":{{mdr_cle:1b}}}}}}].components.\"minecraft:custom_name\"",
        texte::verifier_pseudo(pseudo)?
    ))
}

/// Le nom lu dans la réponse du serveur (« … has the following entity data: "plu" »), ou
/// `None` : clé sans nom, absente de l'inventaire, en double, ou enfant déconnecté.
pub fn lire_nom_cle(reponse: &str) -> Option<String> {
    let (_, donnee) = reponse.split_once(" data: ")?;
    let mut caracteres = strip(donnee).chars();
    let guillemet = caracteres.next().filter(|c| matches!(c, '"' | '\''))?;
    let mut nom = String::new();
    while let Some(c) = caracteres.next() {
        match c {
            '\\' => nom.push(caracteres.next()?),
            c if c == guillemet => return (!nom.is_empty()).then_some(nom),
            c => nom.push(c),
        }
    }
    None
}

/// Une clé qui porte la description d'aujourd'hui. Celle des clés données avant la dictée
/// disait « en minuscules ».
pub fn predicat_cle_a_jour() -> String {
    format!(
        "{CLE}[minecraft:custom_data={{mdr_cle:1b}},minecraft:lore=[{}]]",
        description_cle()
    )
}

/// `give` d'une clé neuve, avec son mode d'emploi dans sa description.
pub fn commande_cle(pseudo: &str) -> Result<String, PseudoInvalide> {
    Ok(format!(
        "give {} {CLE}[minecraft:item_name={},minecraft:lore=[{}],minecraft:custom_data={{mdr_cle:1b}}] 1",
        texte::verifier_pseudo(pseudo)?,
        snbt(NOM_CLE),
        description_cle()
    ))
}

/// « 12 min », « 3 h 05 », « 2 jours » : une durée telle qu'on la dit aux parents.
pub fn formater_duree(duree: Option<TimeDelta>) -> String {
    let Some(duree) = duree else {
        return "?".to_owned();
    };
    let minutes = duree.num_seconds().div_euclid(60);
    if minutes < 1 {
        return "moins d'une minute".to_owned();
    }
    if minutes < 60 {
        return format!("{minutes} min");
    }
    let (heures, minutes) = (minutes / 60, minutes % 60);
    if heures < 48 {
        return format!("{heures} h {minutes:02}");
    }
    format!("{} jours", heures / 24)
}

/// Ce que l'enfant lit quand l'agent a reconnu la réponse sur sa clé.
pub const CLE_JUSTE: &str = "C'est la bonne réponse ! Ouvre le coffre en tenant ta clé en main.";

/// Le verrou du coffre : il ne s'ouvre qu'avec un objet qui porte exactement ce nom.
pub fn commande_verrou(a: &Atelier, nom: &str) -> String {
    format!(
        "data modify block {} lock set value {{components:{{\"minecraft:custom_name\":{}}}}}",
        pos(a.coffre()),
        snbt(nom)
    )
}

/// Commandes qui posent le coffre `s` dans l'atelier `a`. Aucune réponse calculée ici.
pub fn commandes_pose(a: &Atelier, s: &ExerciceStocke) -> Result<Vec<String>, ErreurCoffre> {
    // Une dictée se lit sur le tableau (`tableau.rs`), pas sur quatre lignes de panneau.
    let enonce = if dictee::est_dictee(&s.exercice) {
        dictee::PANNEAU
    } else {
        s.exercice.enonce.as_str()
    };
    let Some(lignes) = texte::lignes_panneau(enonce) else {
        return Err(ErreurCoffre::Coffre(format!(
            "énoncé trop long pour le panneau : {}",
            repr(&s.exercice.enonce)
        )));
    };
    let (c, p, e) = (pos(a.coffre()), pos(a.panneau()), pos(a.enclume()));
    let messages: Vec<String> = lignes
        .iter()
        .map(|ligne| snbt(&texte::nettoyer(ligne, LONGUEUR_MAX)))
        .collect();
    let nom_preuve = snbt(&texte::nettoyer(
        &format!("Preuve n°{}", s.id),
        LONGUEUR_MAX,
    ));
    let preuve = format!(
        "minecraft:paper[minecraft:custom_name={nom_preuve},minecraft:custom_data={{mdr_coffre:{}}}]",
        s.id
    );
    let panneau = format!(
        "minecraft:oak_sign[rotation=0]{{front_text:{{messages:[{}]}},is_waxed:1b}}",
        messages.join(",")
    );
    Ok(vec![
        format!(
            "execute unless block {c} minecraft:chest run setblock {c} minecraft:chest[facing=south]"
        ),
        commande_verrou(a, &s.exercice.reponse),
        format!(
            "item replace block {c} container.13 with {} {}",
            s.recompense, s.recompense_nombre
        ),
        format!("item replace block {c} container.11 with {preuve}"),
        format!("setblock {p} {panneau}"),
        format!("setblock {e} minecraft:anvil[facing=east]"),
    ])
}

/// Après ouverture : preuve retirée, panneau enlevé, verrou ôté. Le contenu reste.
pub fn commandes_rangement(
    a: &Atelier,
    pseudo: &str,
    numero: i64,
) -> Result<Vec<String>, PseudoInvalide> {
    Ok(vec![
        format!(
            "clear {} {}",
            texte::verifier_pseudo(pseudo)?,
            predicat_preuve(numero)
        ),
        format!("setblock {} minecraft:air", pos(a.panneau())),
        format!("data remove block {} lock", pos(a.coffre())),
    ])
}

/// Coffre annulé : il n'a pas été gagné, on le vide entièrement.
pub fn commandes_retrait(a: &Atelier) -> Vec<String> {
    vec![
        format!("setblock {} minecraft:air", pos(a.panneau())),
        format!("data remove block {} lock", pos(a.coffre())),
        format!("data modify block {} Items set value []", pos(a.coffre())),
    ]
}

/// Ce que la boucle de surveillance attend d'un coffre : le vrai, ou un faux dans les tests.
#[async_trait(?Send)]
pub trait Coffre {
    /// Tenu pendant un tour de surveillance et pendant /atelier : pas de pose en plein
    /// déplacement.
    fn verrou(&self) -> &Mutex<()>;
    /// L'atelier lu au dernier `actualiser_atelier`.
    fn atelier(&self) -> Option<Atelier>;
    fn verifier_atelier(&self) -> Result<Atelier, ErreurCoffre>;
    async fn joueur_en_ligne(&self) -> Result<bool, ErreurCoffre>;
    async fn actualiser_atelier(&self) -> Result<(), ErreurCoffre>;
    async fn poser(&self, s: &ExerciceStocke) -> Result<(), ErreurCoffre>;
    async fn annoncer(&self, s: &ExerciceStocke) -> Result<(), ErreurCoffre>;
    async fn equiper(&self) -> Result<(), ErreurCoffre>;
    async fn rappeler(&self, s: &ExerciceStocke) -> Result<(), ErreurCoffre>;
    async fn demander_de_vider(&self) -> Result<(), ErreurCoffre>;
    /// Vrai si le jeu de l'enfant est relié au chat vocal : il entendrait une dictée.
    async fn chat_vocal_relie(&self) -> Result<bool, ErreurCoffre>;
    /// Avant la pose d'un coffre de dictée : les sons de sa phrase sont prêts à être joués.
    async fn preparer_la_dictee(&self, s: &ExerciceStocke) -> Result<(), ErreurCoffre>;
    /// Une dictée attend, mais le jeu de l'enfant n'a pas le chat vocal.
    async fn demander_le_chat_vocal(&self) -> Result<(), ErreurCoffre>;
    async fn preuve_trouvee(&self, s: &ExerciceStocke) -> Result<bool, ErreurCoffre>;
    /// Le nom que porte la clé de l'enfant : ce qu'il a écrit sur l'enclume. `None` s'il n'y
    /// a rien à lire.
    async fn nom_de_la_cle(&self) -> Result<Option<String>, ErreurCoffre>;
    /// L'enfant a écrit `nom`, qui vaut la réponse à la casse et aux espaces près : le verrou
    /// prend ce nom-là, et l'enfant est prévenu qu'il peut ouvrir.
    async fn accepter_la_cle(&self, nom: &str) -> Result<(), ErreurCoffre>;
    /// Le texte entier de la dictée, derrière le coffre ; refait s'il y est déjà.
    async fn afficher_tableau(&self, morceaux: &[Morceau]) -> Result<(), ErreurCoffre>;
    async fn effacer_tableau(&self) -> Result<(), ErreurCoffre>;
    async fn coffre_present(&self) -> Result<bool, ErreurCoffre>;
    async fn coffre_vide(&self) -> Result<bool, ErreurCoffre>;
    async fn ranger(&self, s: &ExerciceStocke) -> Result<(), ErreurCoffre>;
}

pub struct CoffreDuSoir {
    pub jeu: Rc<dyn AdaptateurJeu>,
    atelier: Cell<Option<Atelier>>,
    verrou: Mutex<()>,
    pub pseudo: String,
    pub niveaux_offerts: i64,
    /// Nom de Pixel s'il écoute le chat : le mode d'emploi dit qu'on peut l'appeler.
    pub assistant: Option<String>,
    /// Ce qui fait entendre la phrase d'une dictée ; sans lui, les dictées sont muettes.
    lecteur: Option<Rc<Lecteur>>,
    /// Ce qui dit à voix haute les messages écrits à l'enfant ; sans lui, ils restent écrits.
    parole: Option<Rc<Parole>>,
}

const GRIS: [(&str, Style<'static>); 1] = [("color", Style::Texte("gray"))];
const OR: [(&str, Style<'static>); 1] = [("color", Style::Texte("gold"))];
const VERT: [(&str, Style<'static>); 1] = [("color", Style::Texte("green"))];
const BLANC: [(&str, Style<'static>); 1] = [("color", Style::Texte("white"))];

impl CoffreDuSoir {
    pub fn new(
        jeu: Rc<dyn AdaptateurJeu>,
        pseudo: &str,
        niveaux_offerts: i64,
        assistant: Option<&str>,
    ) -> Result<Self, PseudoInvalide> {
        Ok(Self {
            jeu,
            atelier: Cell::new(None),
            verrou: Mutex::new(()),
            pseudo: texte::verifier_pseudo(pseudo)?.to_owned(),
            niveaux_offerts,
            assistant: assistant.map(str::to_owned),
            lecteur: None,
            parole: None,
        })
    }

    /// Avec de quoi dire à voix haute les annonces écrites dans le chat.
    pub fn avec_parole(mut self, parole: Rc<Parole>) -> Self {
        self.parole = Some(parole);
        self
    }

    /// Écrit un message à l'enfant et le retient pour le dire : la voix suit, sans retarder
    /// les lignes suivantes.
    async fn dire_et_retenir(
        &self,
        message: &str,
        style: &[(&str, Style<'_>)],
        coffre: Option<&Exercice>,
    ) -> Result<(), ErreurCoffre> {
        self.jeu.dire(&self.pseudo, message, style).await?;
        if let Some(parole) = &self.parole {
            parole.retenir(message, coffre);
        }
        Ok(())
    }

    /// Dit ce qui a été retenu, une fois toutes les lignes écrites.
    async fn parler(&self) {
        if let Some(parole) = &self.parole {
            parole.dire().await;
        }
    }

    /// Avec de quoi lire les phrases de dictée à l'enfant, par le chat vocal.
    pub fn avec_lecteur(mut self, lecteur: Rc<Lecteur>) -> Self {
        self.lecteur = Some(lecteur);
        self
    }

    /// Le coffre de dictée n'est plus à faire : l'Écouteur redevient muet, les sons de la
    /// phrase sont effacés. Un échec n'empêche pas de ranger le coffre.
    async fn oublier_la_phrase(&self, ex: &Exercice) {
        if let Some(lecteur) = &self.lecteur
            && let Err(e) = lecteur.oublier(ex).await
        {
            journal::avertissement(format!("phrase de dictée non oubliée : {e}"));
        }
    }

    async fn mode_emploi(&self, dictee: bool) -> Result<(), ErreurCoffre> {
        if dictee {
            self.jeu
                .dire(&self.pseudo, dictee::MODE_EMPLOI, &GRIS)
                .await?;
            self.jeu
                .dire(&self.pseudo, dictee::REECOUTER, &GRIS)
                .await?;
        } else {
            self.jeu.dire(&self.pseudo, MODE_EMPLOI, &GRIS).await?;
        }
        if let Some(assistant) = self.assistant.as_deref().filter(|nom| !nom.is_empty()) {
            self.jeu
                .dire(
                    &self.pseudo,
                    &format!(
                        "Bloqué ou loin ? Écris « {assistant}, aide-moi » ou \
                         « {assistant}, ramène-moi au coffre » dans le chat."
                    ),
                    &GRIS,
                )
                .await?;
        }
        Ok(())
    }

    async fn executer(&self, commandes: &[String]) -> Result<(), ErreurCoffre> {
        for commande in commandes {
            let reponse = self.jeu.commande(commande).await?;
            if ERREUR_SYNTAXE.is_match(&reponse) || reponse.contains("not loaded") {
                return Err(ErreurCoffre::Coffre(format!(
                    "le serveur a refusé « {commande} » : {}",
                    strip(&reponse)
                )));
            }
        }
        Ok(())
    }

    async fn teste(&self, commande: &str) -> Result<bool, ErreurCoffre> {
        Ok(strip(&self.jeu.commande(commande).await?).starts_with("Test passed"))
    }

    pub async fn a_une_cle(&self) -> Result<bool, ErreurCoffre> {
        let reponse = self
            .jeu
            .commande(&format!("clear {} {} 0", self.pseudo, predicat_cle()))
            .await?;
        Ok(strip(&reponse).starts_with("Found"))
    }

    /// Une clé seulement s'il n'en a plus : elle sert d'un coffre à l'autre.
    pub async fn donner_cle(&self) -> Result<(), ErreurCoffre> {
        if !self.a_une_cle().await? {
            let commande = commande_cle(&self.pseudo).map_err(ErreurRcon::from)?;
            self.executer(&[commande]).await?;
        }
        Ok(())
    }

    /// Coffre annulé : retiré du jeu. Appelé depuis /annuler, hors de la boucle. `dictee` :
    /// l'exercice du coffre retiré si c'était une dictée, l'Écouteur redevient muet.
    pub async fn retirer(&self, dictee: Option<&Exercice>) -> Result<(), ErreurCoffre> {
        self.actualiser_atelier().await?;
        self.executer(&commandes_retrait(&self.verifier_atelier()?))
            .await?;
        if let Some(ex) = dictee {
            self.executer(&dictee::commandes_effacement()).await?;
            self.effacer_tableau().await?;
            self.oublier_la_phrase(ex).await;
        }
        Ok(())
    }

    /// Devant un coffre de dictée : une clé dont la description ne dit plus « en minuscules »,
    /// et l'Écouteur s'il n'en a pas.
    async fn equiper_pour_la_dictee(&self) -> Result<(), ErreurCoffre> {
        let a_jour = self
            .jeu
            .commande(&format!(
                "clear {} {} 0",
                self.pseudo,
                predicat_cle_a_jour()
            ))
            .await?;
        if !strip(&a_jour).starts_with("Found") {
            let commande = commande_cle(&self.pseudo).map_err(ErreurRcon::from)?;
            self.executer(&[
                format!("clear {} {}", self.pseudo, predicat_cle()),
                commande,
            ])
            .await?;
        }
        let ecouteur = self
            .jeu
            .commande(&format!(
                "clear {} {} 0",
                self.pseudo,
                dictee::predicat_ecouteur()
            ))
            .await?;
        if !strip(&ecouteur).starts_with("Found") {
            let commande = dictee::commande_ecouteur(&self.pseudo).map_err(ErreurRcon::from)?;
            self.executer(&[commande]).await?;
        }
        Ok(())
    }

    /// La phrase à trou dans le chat, le mode d'emploi, puis la phrase lue.
    async fn presenter_la_dictee(
        &self,
        s: &ExerciceStocke,
        titre: &str,
    ) -> Result<(), ErreurCoffre> {
        self.equiper_pour_la_dictee().await?;
        self.dire_et_retenir(titre, &OR, Some(&s.exercice)).await?;
        if let Some(phrase) = dictee::phrase_a_trou(&s.exercice) {
            self.jeu
                .dire(&self.pseudo, &format!("Dictée : {phrase}"), &BLANC)
                .await?;
        }
        self.mode_emploi(true).await?;
        // L'annonce d'abord, puis la phrase : les deux attendent leur tour dans la même file.
        self.parler().await;
        // Une voix en panne ne casse pas le coffre : la phrase à trou est dans le chat.
        if let Some(lecteur) = &self.lecteur {
            // Redit à chaque présentation : un serveur redémarré a oublié la phrase désignée.
            let lue = match lecteur.designer(&s.exercice).await {
                Ok(()) => lecteur.dire(&s.exercice, false).await,
                Err(e) => Err(e),
            };
            if let Err(e) = lue {
                journal::avertissement(format!("phrase de dictée non lue : {e}"));
            }
        }
        Ok(())
    }
}

#[async_trait(?Send)]
impl Coffre for CoffreDuSoir {
    fn verrou(&self) -> &Mutex<()> {
        &self.verrou
    }

    fn atelier(&self) -> Option<Atelier> {
        self.atelier.get()
    }

    fn verifier_atelier(&self) -> Result<Atelier, ErreurCoffre> {
        self.atelier
            .get()
            .ok_or_else(|| ErreurCoffre::AtelierAbsent(PAS_CONSTRUIT.to_owned()))
    }

    async fn joueur_en_ligne(&self) -> Result<bool, ErreurCoffre> {
        let reponse = self.jeu.commande("list").await?;
        let Some((_, noms)) = reponse.rsplit_once(':') else {
            return Ok(false);
        };
        let noms: HashSet<&str> = noms.split(',').map(strip).collect();
        Ok(noms.contains(self.pseudo.as_str()))
    }

    /// Relit la position dans le monde : elle peut changer pendant que l'agent tourne.
    async fn actualiser_atelier(&self) -> Result<(), ErreurCoffre> {
        self.atelier.set(lire_atelier(self.jeu.as_ref()).await?);
        Ok(())
    }

    async fn poser(&self, s: &ExerciceStocke) -> Result<(), ErreurCoffre> {
        let a = self.verifier_atelier()?;
        self.executer(&commandes_pose(&a, s)?).await?;
        let c = pos(a.coffre());
        let pose = self
            .teste(&format!("execute if data block {c} lock"))
            .await?
            && self
                .teste(&format!(
                    "execute if items block {c} container.* {}",
                    predicat_preuve(s.id)
                ))
                .await?;
        if !pose {
            return Err(ErreurCoffre::Coffre(format!(
                "le coffre n°{} n'a pas été posé correctement",
                s.id
            )));
        }
        if dictee::est_dictee(&s.exercice) {
            // Ce que l'Écouteur réaffichera, sans attendre l'agent.
            self.executer(&dictee::commandes_stockage(&s.exercice))
                .await?;
        }
        Ok(())
    }

    /// Message, mode d'emploi et cloche : l'enfant doit être connecté.
    async fn annoncer(&self, s: &ExerciceStocke) -> Result<(), ErreurCoffre> {
        if dictee::est_dictee(&s.exercice) {
            // Pas de cloche : elle couvrirait le début de la phrase.
            return self.presenter_la_dictee(s, dictee::ANNONCE).await;
        }
        self.dire_et_retenir(
            "Un nouveau coffre est apparu à l'atelier !",
            &OR,
            Some(&s.exercice),
        )
        .await?;
        self.mode_emploi(false).await?;
        self.jeu
            .commande(&format!(
                "execute at {0} run playsound minecraft:block.bell.use master {0} ~ ~ ~ 1 1",
                self.pseudo
            ))
            .await?;
        self.parler().await;
        Ok(())
    }

    /// Pour un nouveau coffre : la clé s'il n'en a plus, et les niveaux offerts.
    async fn equiper(&self) -> Result<(), ErreurCoffre> {
        self.donner_cle().await?;
        if self.niveaux_offerts != 0 {
            self.jeu
                .commande(&format!(
                    "xp add {} {} levels",
                    self.pseudo, self.niveaux_offerts
                ))
                .await?;
        }
        Ok(())
    }

    async fn rappeler(&self, s: &ExerciceStocke) -> Result<(), ErreurCoffre> {
        if dictee::est_dictee(&s.exercice) {
            return self.presenter_la_dictee(s, dictee::RAPPEL).await;
        }
        self.donner_cle().await?;
        self.dire_et_retenir(
            &format!("Un coffre t'attend à l'atelier : {}", s.exercice.enonce),
            &OR,
            Some(&s.exercice),
        )
        .await?;
        self.mode_emploi(false).await?;
        self.parler().await;
        Ok(())
    }

    async fn demander_de_vider(&self) -> Result<(), ErreurCoffre> {
        self.jeu
            .dire(
                &self.pseudo,
                "Vide le coffre de l'atelier : le suivant attend qu'il soit vide.",
                &GRIS,
            )
            .await?;
        Ok(())
    }

    async fn chat_vocal_relie(&self) -> Result<bool, ErreurCoffre> {
        let Some(lecteur) = &self.lecteur else {
            return Ok(false);
        };
        lecteur
            .relie()
            .await
            .map_err(|e| ErreurCoffre::Coffre(format!("chat vocal : {e}")))
    }

    async fn preparer_la_dictee(&self, s: &ExerciceStocke) -> Result<(), ErreurCoffre> {
        let Some(lecteur) = &self.lecteur else {
            return Err(ErreurCoffre::Coffre(
                "les dictées demandent [voix] dossier_du_jeu dans la configuration".to_owned(),
            ));
        };
        lecteur
            .preparer(&s.exercice)
            .await
            .map_err(|e| ErreurCoffre::Coffre(format!("phrase de dictée : {e}")))
    }

    async fn demander_le_chat_vocal(&self) -> Result<(), ErreurCoffre> {
        self.jeu
            .dire(&self.pseudo, dictee::SANS_CHAT_VOCAL, &OR)
            .await?;
        Ok(())
    }

    async fn preuve_trouvee(&self, s: &ExerciceStocke) -> Result<bool, ErreurCoffre> {
        let reponse = self
            .jeu
            .commande(&format!(
                "clear {} {} 0",
                self.pseudo,
                predicat_preuve(s.id)
            ))
            .await?;
        Ok(strip(&reponse).starts_with("Found"))
    }

    async fn nom_de_la_cle(&self) -> Result<Option<String>, ErreurCoffre> {
        let commande = commande_nom_cle(&self.pseudo).map_err(ErreurRcon::from)?;
        Ok(lire_nom_cle(&self.jeu.commande(&commande).await?))
    }

    async fn accepter_la_cle(&self, nom: &str) -> Result<(), ErreurCoffre> {
        let a = self.verifier_atelier()?;
        self.executer(&[commande_verrou(&a, nom)]).await?;
        // Le message ne redit pas le mot : rien à en retirer.
        self.dire_et_retenir(CLE_JUSTE, &VERT, None).await?;
        self.jeu
            .commande(&format!(
                "execute at {0} run playsound minecraft:entity.experience_orb.pickup master {0} ~ ~ ~ 1 1",
                self.pseudo
            ))
            .await?;
        self.parler().await;
        Ok(())
    }

    async fn afficher_tableau(&self, morceaux: &[Morceau]) -> Result<(), ErreurCoffre> {
        let a = self.verifier_atelier()?;
        self.executer(&tableau::commandes(&a, morceaux)).await
    }

    async fn effacer_tableau(&self) -> Result<(), ErreurCoffre> {
        self.executer(&[tableau::commande_effacement()]).await
    }

    async fn coffre_present(&self) -> Result<bool, ErreurCoffre> {
        let c = pos(self.verifier_atelier()?.coffre());
        self.teste(&format!("execute if block {c} minecraft:chest"))
            .await
    }

    async fn coffre_vide(&self) -> Result<bool, ErreurCoffre> {
        let c = pos(self.verifier_atelier()?.coffre());
        if !self.coffre_present().await? {
            return Ok(true);
        }
        Ok(!self
            .teste(&format!("execute if data block {c} Items[0]"))
            .await?)
    }

    async fn ranger(&self, s: &ExerciceStocke) -> Result<(), ErreurCoffre> {
        let mut commandes = commandes_rangement(&self.verifier_atelier()?, &self.pseudo, s.id)
            .map_err(ErreurRcon::from)?;
        let est_dictee = dictee::est_dictee(&s.exercice);
        if est_dictee {
            commandes.extend(dictee::commandes_effacement());
        }
        self.executer(&commandes).await?;
        if est_dictee {
            self.oublier_la_phrase(&s.exercice).await;
        }
        Ok(())
    }
}
