# Lit un son de dictée au joueur qui exécute la fonction, à la place de celui en cours.
# Fonction à macro : `son` est le nom du son, que l'agent écrit dans un stockage.
#
#   data modify storage mine_de_rien:dictee son set value "mine_de_rien:dictee.3"
#   execute as <pseudo> at @s run function mine_de_rien:dictee/jouer with storage mine_de_rien:dictee
#
# Catégorie « voice » : c'est le curseur « Voix » des options de son qui règle le volume.
stopsound @s voice
$playsound $(son) voice @s ~ ~ ~ 1 1
