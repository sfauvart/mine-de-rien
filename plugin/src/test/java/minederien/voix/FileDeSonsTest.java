package minederien.voix;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import org.junit.jupiter.api.Test;

class FileDeSonsTest {
    /** Un lecteur qui note ce qu'on lui confie et ne finit un son que sur demande. */
    private static final class FauxLecteur implements FileDeSons.Lecteur {
        final List<Short> joues = new ArrayList<>();
        final List<Runnable> fins = new ArrayList<>();
        int arrets;
        boolean enPanne;

        @Override
        public Runnable jouer(Iterator<short[]> trames, Runnable aLaFin) {
            if (enPanne) {
                return null;
            }
            joues.add(trames.next()[0]);
            fins.add(aLaFin);
            return () -> arrets++;
        }
    }

    private static List<short[]> son(int marque) {
        return List.of(new short[] {(short) marque});
    }

    @Test
    void unNouveauSonAttendLaFinDuPrecedent() {
        FauxLecteur lecteur = new FauxLecteur();
        FileDeSons file = new FileDeSons(lecteur);
        file.ajouter(son(1));
        file.ajouter(son(2));
        file.ajouter(son(3));
        assertEquals(List.of((short) 1), lecteur.joues);
        assertEquals(2, file.enAttente());

        lecteur.fins.get(0).run();
        assertEquals(List.of((short) 1, (short) 2), lecteur.joues);
        lecteur.fins.get(1).run();
        lecteur.fins.get(2).run();
        assertEquals(List.of((short) 1, (short) 2, (short) 3), lecteur.joues);
        assertFalse(file.joue());
    }

    @Test
    void couperVideLaFileEtArreteLeSonEnCours() {
        FauxLecteur lecteur = new FauxLecteur();
        FileDeSons file = new FileDeSons(lecteur);
        file.ajouter(son(1));
        file.ajouter(son(2));
        file.couper();
        assertEquals(1, lecteur.arrets);
        assertEquals(0, file.enAttente());
        assertFalse(file.joue());

        // La fin du son coupé arrive après coup : elle ne relance rien, même si un autre son
        // est parti entre-temps.
        file.ajouter(son(3));
        lecteur.fins.get(0).run();
        assertTrue(file.joue());
        assertEquals(List.of((short) 1, (short) 3), lecteur.joues);
        file.ajouter(son(4));
        assertEquals(1, file.enAttente());
    }

    @Test
    void unSonQuiNePartPasLaisseLaPlaceAuSuivant() {
        FauxLecteur lecteur = new FauxLecteur();
        FileDeSons file = new FileDeSons(lecteur);
        lecteur.enPanne = true;
        file.ajouter(son(1));
        assertFalse(file.joue());
        lecteur.enPanne = false;
        file.ajouter(son(2));
        assertEquals(List.of((short) 2), lecteur.joues);
    }
}
