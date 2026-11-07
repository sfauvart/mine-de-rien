"""Coffre du soir : un coffre verrouillé par la réponse, à renommer sur une enclume.

Commandes posées via l'adaptateur de jeu :

    setblock X Y Z oak_sign{front_text:{messages:[...]}}
    data merge block X Y Z+1 {Lock:"<réponse>"}
    item replace block X Y Z+1 container.13 with diamond 3
    item replace block X Y Z+1 container.0 with paper[custom_name='"Preuve n°<id>"']

Réussite détectée quand le papier « Preuve n°<id> » apparaît dans l'inventaire du joueur.
"""
