---
name: recap-dossier
description: >
  Produit un récapitulatif structuré d’un dossier (contenu, état,
  trous, prochaines actions) et peut écrire RECAP.md. Utiliser pour
  "fais-moi un récap", "où en est le dossier X", "ce qu’il manque".
---

# recap-dossier

Procédure:

1. Inventaire borné: `ls` puis `find -maxdepth 3` dans le dossier cible (pas l’arbre entier).
2. Lire README, docs, TODOs, et 5–10 fichiers représentatifs. Échantillonner. Ne pas dumper tout le tree.
3. Structure imposée du récap (titres exacts):
   - Vue d’ensemble
   - Contenu
   - État
   - Manques
   - Actions
4. En `readonly`: imprimer le markdown dans le chat, ne pas écrire de fichier.
5. En `ask`: proposer d’écrire `RECAP.md` à la racine du dossier (le write passera par confirm).

Ne jamais lire `.env` ni des clés.
