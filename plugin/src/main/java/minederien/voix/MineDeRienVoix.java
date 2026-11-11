package minederien.voix;

import de.maxhenkel.voicechat.api.BukkitVoicechatService;
import de.maxhenkel.voicechat.api.VoicechatConnection;
import de.maxhenkel.voicechat.api.VoicechatPlugin;
import de.maxhenkel.voicechat.api.VoicechatServerApi;
import de.maxhenkel.voicechat.api.VolumeCategory;
import de.maxhenkel.voicechat.api.audiochannel.AudioPlayer;
import de.maxhenkel.voicechat.api.audiochannel.StaticAudioChannel;
import de.maxhenkel.voicechat.api.events.EventRegistration;
import de.maxhenkel.voicechat.api.events.VoicechatServerStartedEvent;
import de.maxhenkel.voicechat.api.opus.OpusEncoder;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import org.bukkit.command.Command;
import org.bukkit.command.CommandSender;
import org.bukkit.command.ConsoleCommandSender;
import org.bukkit.command.RemoteConsoleCommandSender;
import org.bukkit.entity.Player;
import org.bukkit.event.EventHandler;
import org.bukkit.event.Listener;
import org.bukkit.event.player.PlayerQuitEvent;
import org.bukkit.plugin.IllegalPluginAccessException;
import org.bukkit.plugin.java.JavaPlugin;

/**
 * Joue à un seul joueur, par le chat vocal, un son que l'agent a déposé dans le dossier
 * partagé. Le plugin ne décide rien : ni quoi dire, ni quand. Il n'ouvre aucun port,
 * n'appelle personne et n'écoute aucun micro.
 *
 * <pre>
 * mdrvoix jouer &lt;pseudo&gt; &lt;fichier&gt;   met le son dans la file du joueur
 * mdrvoix couper &lt;pseudo&gt;            vide la file et arrête le son en cours
 * mdrvoix etat &lt;pseudo&gt;              répond « relié », « sans chat vocal » ou « absent »
 * </pre>
 */
public final class MineDeRienVoix extends JavaPlugin implements VoicechatPlugin, Listener {
    /** Le réglage de volume que l'enfant voit dans le mod. Seize caractères au plus. */
    static final String CATEGORIE = "mine_de_rien";

    static final String RELIE = "relié";
    static final String SANS_CHAT_VOCAL = "sans chat vocal";
    static final String ABSENT = "absent";

    /** Le dossier partagé, quand il n'est pas celui du plugin. */
    static final String VARIABLE_DOSSIER = "MDR_VOIX_DOSSIER";

    private final Map<UUID, FileDeSons> files = new HashMap<>();
    private Sons sons;
    /** Donnée par le chat vocal quand son serveur a démarré, sur un autre fil. */
    private volatile VoicechatServerApi api;

    @Override
    public void onEnable() {
        String reglage = System.getenv(VARIABLE_DOSSIER);
        Path dossier = reglage == null || reglage.isBlank()
                ? getDataFolder().toPath().resolve("sons")
                : Path.of(reglage);
        sons = new Sons(dossier);
        getLogger().info("Sons lus dans " + dossier);

        BukkitVoicechatService service =
                getServer().getServicesManager().load(BukkitVoicechatService.class);
        if (service == null) {
            getLogger().severe("Le plugin voicechat (Simple Voice Chat) ne répond pas : aucun son ne sera joué.");
        } else {
            service.registerPlugin(this);
        }
        getServer().getPluginManager().registerEvents(this, this);
    }

    @Override
    public void onDisable() {
        files.values().forEach(FileDeSons::couper);
        files.clear();
    }

    // --- Chat vocal ---

    @Override
    public String getPluginId() {
        return "mine_de_rien_voix";
    }

    @Override
    public void registerEvents(EventRegistration inscription) {
        inscription.registerEvent(VoicechatServerStartedEvent.class, this::chatVocalDemarre);
    }

    private void chatVocalDemarre(VoicechatServerStartedEvent evenement) {
        VoicechatServerApi serveur = evenement.getVoicechat();
        VolumeCategory categorie = serveur.volumeCategoryBuilder()
                .setId(CATEGORIE)
                .setName("Mine de rien")
                .setDescription("La voix qui lit les dictées et les messages")
                .build();
        serveur.registerVolumeCategory(categorie);
        api = serveur;
    }

    private static boolean relie(VoicechatConnection connexion) {
        return connexion != null
                && connexion.isInstalled()
                && connexion.isConnected()
                && !connexion.isDisabled();
    }

    private String etat(Player joueur) {
        if (joueur == null) {
            return ABSENT;
        }
        VoicechatServerApi serveur = api;
        if (serveur == null || !relie(serveur.getConnectionOf(joueur.getUniqueId()))) {
            return SANS_CHAT_VOCAL;
        }
        return RELIE;
    }

    /** Joue un son au seul joueur désigné ; {@code null} s'il n'est plus relié. */
    private Runnable jouer(UUID joueur, Iterator<short[]> trames, Runnable aLaFin) {
        VoicechatServerApi serveur = api;
        if (serveur == null) {
            return null;
        }
        VoicechatConnection connexion = serveur.getConnectionOf(joueur);
        if (!relie(connexion)) {
            return null;
        }
        // Canal statique : pas de position dans le monde, et une seule cible. Un autre joueur,
        // même à côté, n'entend rien.
        StaticAudioChannel canal = serveur.createStaticAudioChannel(UUID.randomUUID());
        if (canal == null) {
            return null;
        }
        canal.addTarget(connexion);
        canal.setCategory(CATEGORIE);
        OpusEncoder encodeur = serveur.createEncoder();
        AudioPlayer lecteur = serveur.createAudioPlayer(
                canal, encodeur, () -> trames.hasNext() ? trames.next() : null);
        lecteur.setOnStopped(() -> {
            if (!encodeur.isClosed()) {
                encodeur.close();
            }
            // Le chat vocal prévient depuis son propre fil : la file ne vit que sur le principal.
            try {
                getServer().getScheduler().runTask(this, aLaFin);
            } catch (IllegalPluginAccessException e) {
                // Le serveur s'arrête : plus rien à enchaîner.
            }
        });
        lecteur.startPlaying();
        return lecteur::stopPlaying;
    }

    @EventHandler
    public void quandUnJoueurPart(PlayerQuitEvent evenement) {
        FileDeSons file = files.remove(evenement.getPlayer().getUniqueId());
        if (file != null) {
            file.couper();
        }
    }

    // --- Commande ---

    @Override
    public boolean onCommand(CommandSender auteur, Command commande, String alias, String[] mots) {
        // Un joueur, même opérateur, ne fait rien jouer : la commande ne sert pas à entendre
        // la phrase d'un coffre sans y être invité.
        if (!(auteur instanceof ConsoleCommandSender) && !(auteur instanceof RemoteConsoleCommandSender)) {
            auteur.sendMessage("refusé : commande réservée à la console");
            return true;
        }
        auteur.sendMessage(repondre(mots));
        return true;
    }

    @Override
    public List<String> onTabComplete(CommandSender auteur, Command commande, String alias, String[] mots) {
        return List.of();
    }

    private String repondre(String[] mots) {
        if (mots.length < 2) {
            return "refusé : mdrvoix jouer|couper|etat <pseudo> [fichier]";
        }
        Player joueur = getServer().getPlayerExact(mots[1]);
        switch (mots[0]) {
            case "etat":
                if (mots.length != 2) {
                    break;
                }
                return etat(joueur);
            case "couper":
                if (mots.length != 2) {
                    break;
                }
                if (joueur != null) {
                    FileDeSons file = files.get(joueur.getUniqueId());
                    if (file != null) {
                        file.couper();
                    }
                }
                return "ok";
            case "jouer":
                if (mots.length != 3) {
                    break;
                }
                // Le nom est contrôlé avant tout le reste : un nom hors gabarit est refusé
                // de la même façon que le joueur soit là ou non.
                if (!Sons.nomPermis(mots[2])) {
                    return "refusé : nom de fichier hors gabarit";
                }
                String etat = etat(joueur);
                if (!RELIE.equals(etat)) {
                    return "refusé : " + etat;
                }
                try {
                    List<short[]> trames = sons.lire(mots[2]);
                    UUID id = joueur.getUniqueId();
                    files.computeIfAbsent(id, cle -> new FileDeSons((t, fin) -> jouer(id, t, fin)))
                            .ajouter(trames);
                    return "ok";
                } catch (Sons.Refus refus) {
                    return "refusé : " + refus.getMessage();
                }
            default:
                break;
        }
        return "refusé : mdrvoix jouer|couper|etat <pseudo> [fichier]";
    }
}
