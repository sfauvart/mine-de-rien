# Architecture et décisions

## Décisions prises au premier commit

1. **Minecraft Java Edition + serveur Paper auto-hébergé.** Seule combinaison sans compte
   supplémentaire ni abonnement, avec le jeu de commandes le plus riche (tag `Lock`,
   `data merge`, advancements, `worldborder`). Geyser + Floodgate pour les clients Bedrock.
   Minecraft Education exclue (compte scolaire Microsoft obligatoire).
2. **Pont sans plugin pour commencer.** Commandes via RCON, chat et connexions lus dans
   `logs/latest.log`. Un plugin Paper viendra seulement si un jeu exige des événements fins
   (ouverture de coffre, pose de bloc).
3. **Le LLM n'est jamais la source de vérité.** Générateurs déterministes par compétence,
   réponse calculée. Le LLM habille l'énoncé et lit les photos du cahier. Un modèle local de
   7 à 8 milliards de paramètres suffit alors.
4. **Trois adaptateurs.** Jeu, canal, LLM. Chaque adaptateur externe a une alternative
   auto-hébergée : Telegram → Matrix ou page web locale ; Anthropic → API compatible OpenAI
   (llama.cpp, Ollama, LM Studio).
5. **Pseudonymisation vers tout LLM distant.** Seul l'alias configuré et le texte de
   l'exercice partent.
6. **SQLite local** pour compétences, tentatives et rapports. Licence MIT.

## Ordre de construction

| Version | Contenu | Dépendance technique |
|---|---|---|
| 0.1 | Coffre du soir, Telegram, saisie d'exercices, rapport simple ([spec](specs/open/2026-11-coffre-du-soir.md)) | RCON, datapack |
| 0.2 | Arbre de progrès comme format unique de rapport, bordure du monde | datapack |
| 0.3 | Bartholomé | lecture du chat dans le log |
| 0.4 | Chantiers | scan de zone, peut-être plugin Paper |
| 0.5 | Canal web local, adaptateur Bedrock natif si des contributeurs le veulent | |

## Flux d'un devoir

```
parent (Telegram) ──texte ou photo──▶ canal ──▶ orchestrateur
                                                   │ choisit une compétence + un générateur
                                                   │ calcule l'exercice et la réponse
                                                   │ demande au LLM un habillage (optionnel)
                                                   ▼
                                               jeu (RCON) ──▶ panneau + coffre + butin
                                                   ▲
                                               détection de réussite (inventaire, chat)
                                                   │
                                     stockage SQLite ──▶ rapport ──▶ canal ──▶ parents
```
