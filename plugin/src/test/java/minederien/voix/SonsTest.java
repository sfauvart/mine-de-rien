package minederien.voix;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class SonsTest {
    /** Du PCM 16 bits petit-boutiste dont l'échantillon i vaut i. */
    private static byte[] pcm(int echantillons) {
        ByteBuffer octets = ByteBuffer.allocate(2 * echantillons).order(ByteOrder.LITTLE_ENDIAN);
        for (int i = 0; i < echantillons; i++) {
            octets.putShort((short) i);
        }
        return octets.array();
    }

    @Test
    void unFichierSeLitEnTramesDeVingtMillisecondes(@TempDir Path dossier) throws Exception {
        Files.write(dossier.resolve("phrase-1.pcm"), pcm(2 * Sons.TRAME + 10));
        List<short[]> trames = new Sons(dossier).lire("phrase-1.pcm");
        assertEquals(3, trames.size());
        for (short[] trame : trames) {
            assertEquals(Sons.TRAME, trame.length);
        }
        assertEquals(0, trames.get(0)[0]);
        assertEquals(Sons.TRAME - 1, trames.get(0)[Sons.TRAME - 1]);
        assertEquals(Sons.TRAME, trames.get(1)[0]);
        // La dernière trame est complétée par du silence.
        assertEquals(2 * Sons.TRAME + 9, trames.get(2)[9]);
        assertEquals(0, trames.get(2)[10]);
        assertEquals(0, trames.get(2)[Sons.TRAME - 1]);
    }

    @Test
    void lesOctetsSontLusEnPetitBoutiste() throws Exception {
        List<short[]> trames = Sons.enTrames(new byte[] {0x34, 0x12, (byte) 0xff, (byte) 0xff});
        short[] attendu = new short[Sons.TRAME];
        attendu[0] = 0x1234;
        attendu[1] = -1;
        assertArrayEquals(attendu, trames.get(0));
    }

    @Test
    void unFichierVideEstRefuse(@TempDir Path dossier) throws IOException {
        Files.write(dossier.resolve("vide.pcm"), new byte[0]);
        assertThrows(Sons.Refus.class, () -> new Sons(dossier).lire("vide.pcm"));
    }

    @Test
    void unFichierAbsentEstRefuse(@TempDir Path dossier) {
        assertThrows(Sons.Refus.class, () -> new Sons(dossier).lire("absent.pcm"));
    }

    @Test
    void lesNomsHorsGabaritSontRefuses() {
        assertTrue(Sons.nomPermis("phrase-12-lent.pcm"));
        assertTrue(Sons.nomPermis("7.pcm"));
        for (String nom : new String[] {
            "../secret.pcm", "..", "a/b.pcm", "/etc/passwd", "a\\b.pcm", "phrase.ogg", "phrase.pcm.tmp",
            ".pcm", "Phrase.pcm", "été.pcm", "a b.pcm", "a.pcm ", "a..pcm", "a.pcm/..", "", "a".repeat(65) + ".pcm"
        }) {
            assertFalse(Sons.nomPermis(nom), nom);
        }
        assertFalse(Sons.nomPermis(null));
    }

    @Test
    void rienNeSeLitHorsDuDossier(@TempDir Path racine) throws Exception {
        Path dossier = Files.createDirectory(racine.resolve("sons"));
        Files.write(racine.resolve("dehors.pcm"), pcm(10));
        Sons sons = new Sons(dossier);
        assertThrows(Sons.Refus.class, () -> sons.lire("../dehors.pcm"));
        // Un lien symbolique au bon nom n'est pas suivi.
        Files.createSymbolicLink(dossier.resolve("lien.pcm"), racine.resolve("dehors.pcm"));
        assertThrows(Sons.Refus.class, () -> sons.lire("lien.pcm"));
        // Un dossier au bon nom non plus.
        Files.createDirectory(dossier.resolve("dossier.pcm"));
        assertThrows(Sons.Refus.class, () -> sons.lire("dossier.pcm"));
    }
}
