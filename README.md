undefined
## Vérifier une modification

```bash
pnpm install --frozen-lockfile
pnpm check
pnpm test
pnpm build
pnpm test:rust
```

GitHub Actions exécute les contrôles frontend et les tests Rust sous Linux et Windows. Les tests ordinaires utilisent un sidecar simulé et ne contactent aucun modèle.

Le chat conserve les brouillons tant que l’envoi n’est pas confirmé, affiche les erreurs avec une action de reprise et confirme la transmission des approbations. L’historique se consulte par pages de 200 messages, avec retour aux messages récents.

Les messages affichent les titres, listes, liens, blocs de code copiables et diffs. Le HTML brut reste du texte.

Les [évaluations de l’agent](evaluations/README.md) couvrent 20 tâches. Elles mesurent les résultats, la durée et le coût dans un parcours dédié.
