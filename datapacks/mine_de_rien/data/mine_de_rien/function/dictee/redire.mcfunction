# Clic droit avec l'Écouteur : réaffiche la phrase du coffre de dictée avec son trou. Le son,
# lui, est rejoué au même clic par le plugin mine-de-rien-voix, par le chat vocal.
# L'agent écrit `phrase` dans le stockage mine_de_rien:dictee à la pose du coffre et l'efface
# à l'ouverture : sans coffre de dictée, l'Écouteur ne fait rien.
scoreboard players set @s mdr_ecoute 0
# Le même objet sans notre marque (un vrai champignon biscornu sur un bâton) n'est pas un Écouteur.
execute unless items entity @s weapon.* minecraft:warped_fungus_on_a_stick[minecraft:custom_data={mdr_ecouteur:1b}] run return fail
execute unless data storage mine_de_rien:dictee phrase run return fail
tellraw @s [{text:"Dictée : ",color:"gold"},{nbt:"phrase",storage:"mine_de_rien:dictee",color:"white"}]
