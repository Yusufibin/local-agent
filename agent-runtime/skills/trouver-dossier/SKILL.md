---
name: trouver-dossier
description: >
  Localise un dossier ou un projet sur la machine à partir d’un nom
  approximatif (ex. "dossier de Fred"). Utiliser quand l’utilisateur
  désigne un dossier sans chemin. Chercher uniquement dans les workspace
  roots via find/fd/grep. Ne jamais scanner / ou $HOME entier.
---

# trouver-dossier

Procédure:

1. Normaliser le nom (minuscule, accents ignorés). Ex. `fred` → `*fred*`.
2. Chercher **uniquement à l’intérieur des workspace roots** (jamais `/`, jamais `$HOME` entier):
   `find <each-root> -maxdepth 5 -iname '*nom*' -type d`
   (ou `fd` borné au même root).
3. Si 0 résultat: élargir `maxdepth` progressivement (6, 8, 10) **toujours dans les roots**, puis demander confirmation à l’utilisateur.
4. Si plusieurs hits: lister chemins + mtimes et demander confirmation (quel dossier ?).
5. Une fois choisi: travailler uniquement sous ce chemin.

Ne jamais lancer `find /` ni un scan récursif de `$HOME`.
