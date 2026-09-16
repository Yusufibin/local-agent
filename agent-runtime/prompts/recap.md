---
description: >
  Récapitulatif structuré d’un dossier (Vue d’ensemble, Contenu, État, Manques, Actions).
  Utiliser pour « fais-moi un récap ». Peut écrire RECAP.md après confirmation.
argument-hint: "[dossier]"
---

Récapitule ${1:-le dossier nommé par l'utilisateur, sinon le dossier courant}.

Inventaire borné (ls / find -maxdepth 3). Lis README, docs, TODOs et 5–10 fichiers. N’envoie pas l’arbre entier. Ne scanne jamais `/` ni `$HOME` entier.

Structure imposée (titres exacts):

## Vue d’ensemble
## Contenu
## État
## Manques
## Actions

Mentionne les TODOs ouverts. En readonly: markdown dans le chat, n’écris pas de fichier. En ask: propose d’écrire `RECAP.md` à la racine du dossier trouvé (le write passera par confirm).
