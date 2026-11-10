# Zone de l'atelier : aucun joueur en survie ne doit pouvoir casser le coffre ni poser
# d'entonnoir dessous. Aucune coordonnée ici : la zone se mesure depuis chaque marqueur
# `mdr_atelier`, posé par l'agent au centre de l'atelier (au pied du coffre).
#
# La zone est une boîte : 6 blocs autour du marqueur, et 9 vers le bas. Une sphère de 6 ne
# suffit pas par en dessous : les yeux d'un joueur sont 1,62 bloc au-dessus de ses pieds, et
# depuis 6 blocs sous le coffre il atteindrait encore le bloc qui le porte.
tag @a remove mdr_dans_zone
execute at @e[type=minecraft:marker,tag=mdr_atelier] positioned ~-6 ~-9 ~-6 run tag @a[dx=12,dy=15,dz=12] add mdr_dans_zone

# Seuls les joueurs en survie passent en aventure : créatif et spectateur ne sont jamais touchés.
execute as @a[tag=mdr_dans_zone,gamemode=survival] run function mine_de_rien:atelier/entrer
execute as @a[tag=!mdr_dans_zone,tag=mdr_aventure,gamemode=adventure] run function mine_de_rien:atelier/sortir

# Un joueur passé en créatif dans la zone n'est plus à nous : on oublie sa marque.
tag @a[tag=mdr_aventure,gamemode=!adventure] remove mdr_aventure

# Écouteur des dictées : un clic droit relit la phrase tout de suite, sans attendre l'agent.
execute as @a[scores={mdr_ecoute=1..}] at @s run function mine_de_rien:dictee/redire
