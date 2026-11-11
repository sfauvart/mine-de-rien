package minederien.voix;

import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Pattern;

/**
 * Les sons du dossier partagé avec l'agent : du PCM brut, mono, 16 bits petit-boutiste,
 * 48 000 Hz, découpé en trames de 20 ms comme le chat vocal les attend.
 *
 * <p>Cette classe ne dépend ni de Paper ni du chat vocal : elle se teste seule.
 */
public final class Sons {
    /** Échantillons d'une trame : 20 ms à 48 000 Hz. */
    public static final int TRAME = 960;

    /** Une minute de son : bien plus qu'une phrase, assez peu pour tenir en mémoire. */
    public static final long TAILLE_MAX = 60L * 48_000 * 2;

    private static final Pattern NOM = Pattern.compile("[a-z0-9-]{1,64}\\.pcm");

    private final Path dossier;

    public Sons(Path dossier) {
        this.dossier = dossier;
    }

    /** Un son refusé ; le message part tel quel dans la réponse de la commande. */
    public static final class Refus extends Exception {
        public Refus(String message) {
            super(message);
        }
    }

    /** Vrai pour un nom seul, sans dossier : {@code [a-z0-9-]+\.pcm}. */
    public static boolean nomPermis(String nom) {
        return nom != null && NOM.matcher(nom).matches();
    }

    /**
     * Lit un son du dossier partagé. Le nom est contrôlé avant d'ouvrir quoi que ce soit, et
     * un lien symbolique n'est pas suivi : rien ne se lit hors du dossier.
     */
    public List<short[]> lire(String nom) throws Refus {
        if (!nomPermis(nom)) {
            throw new Refus("nom de fichier hors gabarit");
        }
        Path fichier = dossier.resolve(nom);
        if (!Files.isRegularFile(fichier, LinkOption.NOFOLLOW_LINKS)) {
            throw new Refus("fichier introuvable");
        }
        try {
            if (Files.size(fichier) > TAILLE_MAX) {
                throw new Refus("fichier trop long");
            }
            return enTrames(Files.readAllBytes(fichier));
        } catch (IOException e) {
            throw new Refus("fichier illisible");
        }
    }

    /** Découpe du PCM 16 bits en trames ; la dernière est complétée par du silence. */
    public static List<short[]> enTrames(byte[] octets) throws Refus {
        int echantillons = octets.length / 2;
        if (echantillons == 0) {
            throw new Refus("fichier vide");
        }
        ByteBuffer lecture = ByteBuffer.wrap(octets).order(ByteOrder.LITTLE_ENDIAN);
        List<short[]> trames = new ArrayList<>((echantillons + TRAME - 1) / TRAME);
        for (int debut = 0; debut < echantillons; debut += TRAME) {
            short[] trame = new short[TRAME];
            int fin = Math.min(debut + TRAME, echantillons);
            for (int i = debut; i < fin; i++) {
                trame[i - debut] = lecture.getShort(2 * i);
            }
            trames.add(trame);
        }
        return trames;
    }
}
