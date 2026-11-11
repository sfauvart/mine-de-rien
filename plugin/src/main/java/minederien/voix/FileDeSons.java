package minederien.voix;

import java.util.ArrayDeque;
import java.util.Deque;
import java.util.Iterator;
import java.util.List;

/**
 * Les sons qui attendent leur tour pour un joueur : un seul joue à la fois, le suivant part
 * quand le précédent se termine.
 *
 * <p>La file ne sait pas jouer : elle confie chaque son à un {@link Lecteur}, qui la prévient
 * à la fin. Toutes ses méthodes s'appellent depuis le fil principal du serveur.
 */
public final class FileDeSons {
    /** Ce qui joue un son, et sait l'arrêter. */
    public interface Lecteur {
        /**
         * Commence à jouer ; {@code aLaFin} est appelé une fois le son terminé ou arrêté.
         * Renvoie de quoi l'arrêter, ou {@code null} si le son n'a pas pu partir.
         */
        Runnable jouer(Iterator<short[]> trames, Runnable aLaFin);
    }

    private final Lecteur lecteur;
    private final Deque<List<short[]>> attente = new ArrayDeque<>();
    private Runnable arret;
    /** Change à chaque son lancé ou coupé : la fin d'un son coupé ne lance pas le suivant. */
    private int tour;

    public FileDeSons(Lecteur lecteur) {
        this.lecteur = lecteur;
    }

    /** Met un son dans la file ; il part tout de suite si rien ne joue. */
    public void ajouter(List<short[]> trames) {
        attente.addLast(trames);
        if (arret == null) {
            suivant();
        }
    }

    /** Vide la file et arrête le son en cours. */
    public void couper() {
        attente.clear();
        tour++;
        Runnable enCours = arret;
        arret = null;
        if (enCours != null) {
            enCours.run();
        }
    }

    /** Sons en attente, sans compter celui qui joue. */
    public int enAttente() {
        return attente.size();
    }

    public boolean joue() {
        return arret != null;
    }

    private void suivant() {
        while (arret == null && !attente.isEmpty()) {
            int ceTour = ++tour;
            arret = lecteur.jouer(attente.removeFirst().iterator(), () -> fini(ceTour));
        }
    }

    private void fini(int ceTour) {
        if (ceTour != tour) {
            return;
        }
        arret = null;
        suivant();
    }
}
