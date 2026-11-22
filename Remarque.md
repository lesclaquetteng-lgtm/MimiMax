# Remarque.md — Analyse du dépôt MimiMax (Aether Client / Aether Launcher)

> Dépôt analysé : https://github.com/lesclaquetteng-lgtm/MimiMax
> Date de l'analyse : 22 novembre 2026

---

## 0. Périmètre et limites (à lire d'abord)

**Ce qui a été lu :**

- la page d'accueil du dépôt et le `README.md`
- `ANALYSE-AMELIORATIONS.md` (1140 lignes, lu jusqu'au §8.7)
- `RAPPORT-FINAL.md` (en entier)
- la liste des fichiers à la racine

**Ce qui n'a pas pu être lu :**

- le **code source** (`aether-client/src`, `launcher/src`, `launcher/gui`) : l'accès GitHub aux sous-dossiers est refusé aux outils automatisés
- les **workflows** `.github/workflows` et la **Pull Request ouverte**
- le contenu des binaires `.exe`

Conséquence : cette analyse porte sur **l'architecture, la documentation, l'hygiène du dépôt et les risques déduits de ce que le projet décrit lui-même**. Chaque remarque est étiquetée :

| Étiquette | Signification |
|---|---|
| ✅ **Constaté** | visible directement sur le dépôt ou dans ses documents |
| 🔎 **À vérifier** | risque plausible d'après la documentation, à confirmer dans le code |

---

## 1. Verdict en bref

Le projet est **sérieux et bien outillé** : un cœur Rust testé (151 tests, clippy à 0 avertissement), un mod Fabric testé (42 tests d'après la dernière révision), des tests E2E pilotant la vraie interface, et surtout une culture de la preuve rare pour un projet de cette taille (chaque correctif est accompagné de sa vérification).

Les faiblesses ne sont pas dans la qualité du code mais dans **l'hygiène du dépôt, la sécurité de la distribution, la cohérence de la documentation et quelques risques liés au périmètre fonctionnel**.

| # | Remarque | Gravité | Effort |
|---|---|---|---|
| R1 | Binaires `.exe` commités dans le dépôt | 🔴 Haute | Faible |
| R2 | Données locales / secrets possibles dans `tools/` et les docs | 🔴 Haute | Faible |
| R3 | Serveur hébergé en `online-mode=false` : exposition si le port est ouvert | 🔴 Haute | Faible |
| R4 | Modules de combat + mode hors-ligne : risque de ban et de réputation | 🟠 Moyenne | Moyen |
| R5 | Jetons Microsoft : coffre DPAPI et migration — ✅ **traité** (§R5) | 🟠 Moyenne | Moyen |
| R6 | Aucune release, aucune signature, aucune mise à jour | 🟠 Moyenne | Moyen |
| R7 | Nom du projet incohérent (MimiMax / Aether) | 🟡 Faible | Faible |
| R8 | Documentation divergente (Electron vs Tauri) et fichiers `.md` trop lourds à la racine | 🟡 Faible | Faible |
| R9 | Chiffres de FPS présentés sans leur contexte | 🟡 Faible | Faible |
| R10 | Dépendance à des versions de mods figées — ✅ **traité** (§R10) | 🟡 Faible | Moyen |
| R11 | Windows uniquement, CI non vérifiée | 🟡 Faible | Moyen |
| R12 | Dette technique — 🟡 **partiellement traitée** : installation faite, reste les points « client » (§R12) | 🟡 Faible | Variable |
| R13 | Interface en anglais uniquement, accessibilité clavier partielle | 🟡 Faible | Moyen |

---

## 2. Points forts à conserver

- **Architecture saine** : le cœur Rust ne contient pas de logique dans l'interface ; tout passe par des documents JSON, donc tout est testable en ligne de commande.
- **Téléchargements robustes** : reprise par `Range`, backoff, vérification SHA-1, protection zip-slip, index d'assets corrompu non cru sur parole.
- **Sonde de login réelle** (`servers probe`) plutôt qu'une heuristique sur le MOTD : on sait avant de lancer le jeu si le serveur accepte un compte hors-ligne.
- **Honnêteté du projet** : les limites sont écrites (« hors-ligne ≠ sécurisé », « aucun contournement d'`online-mode=true` »).
- **Sécurité Electron correcte** : `contextBridge`, pas de `nodeIntegration`, navigation externe en HTTPS uniquement, tests de régression (`npm run test:security`).
- **Bugs graves trouvés ET corrigés avec preuve** : preset JVM `balanced` qui empêchait le démarrage, crash au démarrage dû à la config appliquée avant `minecraft.options`, BOM UTF-8 qui faisait perdre les réglages, `Settings` sans `#[serde(default)]`.

---

## 3. Remarques détaillées et solutions

### R1 — Binaires `.exe` commités à la racine · 🔴 ✅ Constaté

**Constat.** `MimiMax.exe` et `Aether-Launcher.exe` sont versionnés à la racine. Les documents mentionnent aussi un dossier `dist/` avec un installateur NSIS et un exe portable.

**Risques.**

- Impossible de prouver que le binaire correspond au code source.
- Chaque nouvelle version alourdit l'historique Git de plusieurs Mo, définitivement.
- Les empreintes SHA-256 publiées dans `dist/LISEZ-MOI.txt` sont dans **le même dépôt** que les binaires : elles ne prouvent rien si le dépôt est compromis.
- Les exe non signés déclenchent SmartScreen et des faux positifs antivirus, qui nuisent à la confiance.

**Solutions.**

1. Retirer les binaires du suivi Git et les ignorer :

```bash
git rm --cached MimiMax.exe Aether-Launcher.exe
git rm -r --cached dist/ 2>/dev/null || true
printf '\n*.exe\n/dist/\n/launcher/target/\n/launcher/desktop/target/\n/aether-client/build/\n/aether-client/.gradle/\nnode_modules/\n' >> .gitignore
git add .gitignore && git commit -m "Retire les binaires du suivi Git"
```

2. Publier les binaires via **GitHub Releases**, construits par GitHub Actions (voir l'annexe A), avec le fichier `SHA256SUMS` en pièce jointe.
3. Pour alléger l'historique déjà alourdi :

```bash
pip install git-filter-repo
git filter-repo --invert-paths --path MimiMax.exe --path Aether-Launcher.exe --path dist/
# puis force-push : à faire UNE fois, en prévenant les éventuels contributeurs
```

4. À moyen terme : signature de code Windows (voir R6).

---

### R2 — Données locales et secrets possibles dans le dépôt · 🔴 🔎 À vérifier

**Constat.** Le README décrit `tools/` comme « scripts et références locaux utilisés pendant le développement ». Les documents mentionnent `tools/aether-home` (une vraie installation avec `accounts.json`, `settings.json`, logs de session), `tools/ui-shots`, des captures `tools/*.png` et des logs `tools/tauri-bundle*.log`.

Par ailleurs, `ANALYSE-AMELIORATIONS.md` contient des chemins personnels (`C:\Users\ee\Aether`, `%LOCALAPPDATA%\...`) et le matériel de la machine de test.

**Risques.**

- Si `tools/aether-home` est versionné, il peut contenir des **identifiants de compte** et, si une connexion Microsoft y a déjà été faite, des **jetons d'accès/rafraîchissement**.
- Même supprimé, un secret reste lisible dans l'historique Git.

**Solutions.**

1. Chercher les secrets dans tout l'historique, pas seulement dans la version actuelle :

```bash
# gitleaks
gitleaks detect --source . --log-opts="--all" -v
# ou trufflehog
trufflehog git file://. --since-commit "$(git rev-list --max-parents=0 HEAD)"
```

2. Vérifier explicitement les fichiers à risque :

```bash
git ls-files | grep -Ei 'accounts|token|session|\.env|aether-home|settings\.json|home\.txt'
git log --all --oneline -- tools/aether-home
```

3. Si un jeton Microsoft a déjà été commité : **le révoquer** (compte Microsoft → sécurité → applications autorisées), puis purger l'historique avec `git filter-repo`.
4. Ajouter à `.gitignore` : `tools/aether-home/`, `tools/ui-shots/`, `tools/*.log`, `tools/*.png`, `**/accounts.json`, `**/home.txt`.
5. Activer **GitHub Secret Protection / push protection** (gratuit sur les dépôts publics) dans *Settings → Code security*.
6. Anonymiser les chemins personnels dans les `.md` (`C:\Users\<vous>\...`).

---

### R3 — Hébergement local en `online-mode=false` : exposition réseau · 🔴 🔎 À vérifier

**Constat.** La page Servers lance un serveur Fabric local en `online-mode=false` « en un clic », administré par RCON.

**Risque.** Un serveur hors-ligne accepte **n'importe quel pseudo**. Si le port 25565 est joignable depuis l'extérieur (box ouverte, UPnP, tunnel), n'importe qui peut se connecter sous le pseudo d'un opérateur et obtenir les droits `op`. Le rapport prévoit d'ailleurs « ouverture de port assistée (UPnP) » et « tunnel playit.gg » comme améliorations futures : sans garde-fous, elles aggraveraient le problème. Le mot de passe RCON est l'autre point sensible.

**Solutions (valeurs par défaut à imposer à la création d'un hôte).**

```properties
# server.properties généré par le launcher
online-mode=false
white-list=true
enforce-whitelist=true
server-ip=127.0.0.1          # LAN/local uniquement par défaut
enable-rcon=true
rcon.password=<généré aléatoirement, 32 caractères, jamais affiché en clair>
broadcast-rcon-to-ops=false
op-permission-level=2
```

- Ajouter dans l'interface un bouton « Autoriser le réseau local » distinct, avec un avertissement clair : *« N'importe qui qui atteint ce serveur peut usurper n'importe quel joueur. »*
- Ne **jamais** proposer l'ouverture UPnP sans liste blanche activée.
- Faire écouter RCON uniquement sur `127.0.0.1` et refuser de démarrer si le port RCON est exposé.
- Test à ajouter : un test d'intégration qui lit le `server.properties` généré et échoue si `white-list` est faux ou si `rcon.password` est vide.

---

### R4 — Modules de combat et mode hors-ligne · 🟠 ✅ Constaté

**Constat.** Le mod embarque des modules de type *Kill Aura*, *Auto Crystal*, *Anti Knockback*, *No Fall*, *Safe Walk*, *Auto Jump*, *Auto Tool*. Le README compare pourtant le projet à Lunar, Badlion et Feather, qui n'en contiennent pas. Le rapport reconnaît lui-même que le mode « fair play » n'est « pas encore implémenté ».

**Risques.**

- Ces modules sont interdits par la quasi-totalité des serveurs publics : **bannissement du compte** de l'utilisateur.
- Combinés au compte hors-ligne, ils ouvrent la porte à des usages abusifs sur des serveurs tiers, ce qui expose le projet à des signalements.
- Message de positionnement confus : « client d'optimisation et de confort » d'un côté, outils de triche de l'autre.

**Solutions.**

1. **Séparer** : sortir les modules de combat dans un second jar (`aether-combat.jar`), non installé par défaut, ou au minimum derrière un réglage « Modules avancés » désactivé par défaut.
2. **Exploiter la sonde existante** : le launcher sait déjà si un serveur est `premium` ou `offline`. Passer au jeu un indicateur de mode et verrouiller les modules de combat quand le serveur n'est pas celui de l'utilisateur :

```text
# Launcher → JVM
-Daether.fairplay=true
```

```java
// Côté mod : refus d'activation en mode fair play
public boolean canEnable() {
    return !(category == ModuleCategory.COMBAT
             && Boolean.getBoolean("aether.fairplay"));
}
```

3. **Documenter** chaque module (page `docs/FEATURES.md`) avec une colonne « Autorisé sur serveurs publics ? ».
4. **Clarifier le README** : une section « Usage responsable » (serveurs privés, solo, serveurs qui l'autorisent) et la phrase *« Utiliser ces modules sur un serveur qui les interdit peut entraîner un bannissement. »*

---

### R5 — Stockage des jetons Microsoft · 🟠 ✅ Traité

> **État au 22 novembre 2026 — ✅ traité**, prouvé par des tests exécutés.
>
> `launcher/src/accounts.rs` ne sérialise ni `accessToken` ni `refreshToken` (`skip_serializing`) et
> son `Debug` les remplace par `<redacted>`. Les jetons vivent dans `accounts.credentials`, chiffré
> par **DPAPI** (portée utilisateur Windows, `launcher/src/credentials.rs`) ; `accounts.json` ne
> garde que l'identité du compte. Au démarrage, un catalogue historique encore en clair est **migré**
> : le coffre est écrit d'abord, puis le catalogue est réécrit sans les jetons
> (`migrate_legacy_secrets`). Si le coffre ne peut être écrit ou lu, les jetons sont effacés et le
> joueur se reconnecte (`drop_secrets`) : jamais de repli en clair.
>
> | Solution proposée ci-dessous | État |
> | --- | --- |
> | 1. Vérifier comment `access_token`/`refresh_token` sont écrits | **Fait** — `#[serde(skip_serializing)]` sur les deux champs ; test `account_catalogue_serialization_never_contains_session_tokens` |
> | 2. Coffre du système | **Fait** — DPAPI (`CryptProtectData`/`CryptUnprotectData`, portée utilisateur) plutôt que la crate `keyring` : aucune dépendance supplémentaire, même protection sous Windows. Test `dpapi_credentials_round_trip_without_writing_tokens_to_the_catalogue` (le fichier chiffré ne contient aucun jeton et se relit). |
> | 3. Ne garder que l'identifiant dans `accounts.json` | **Fait** — le catalogue réécrit ne contient ni `accessToken` ni `refreshToken` ; test `a_legacy_catalogue_is_migrated_into_the_vault_and_scrubbed` |
> | 4. Ne jamais journaliser un jeton | **Fait** — `Debug` de `Account`/`Session` masque les jetons (`debug_output_never_exposes_microsoft_session_tokens`), l'aperçu CLI écrit `<redacted>` (`command_preview_redacts_tokens_embedded_in_arguments`), et le journal de session ne reçoit que la sortie du jeu : `a_session_log_never_contains_the_access_token` lance un vrai processus dont la ligne de commande porte un jeton et vérifie que le journal n'en contient aucune trace. |
> | 5. Migration au premier lancement | **Fait** — `migrate_legacy_secrets` écrit le coffre **puis** réécrit le catalogue ; test `a_legacy_catalogue_is_migrated_into_the_vault_and_scrubbed` (catalogue source en clair → coffre → catalogue nettoyé → jetons relus). Un coffre inutilisable efface les jetons au lieu de les laisser en clair : `a_catalogue_loses_its_tokens_when_the_vault_cannot_be_written`. |
>
> Limite assumée : le coffre est spécifique à Windows (DPAPI, portée utilisateur) ; sur les autres
> cibles, le launcher refuse d'écrire les jetons au lieu de retomber sur un fichier en clair
> (`non_windows_builds_refuse_to_persist_microsoft_tokens_in_clear_text`).

**Constat.** Le rapport liste « jetons stockés via DPAPI/Keychain » comme amélioration *à venir* (§6.4), ce qui laisse penser qu'ils ne le sont pas encore. Le README annonce pourtant un support des comptes Microsoft.

**Risque.** Un jeton de rafraîchissement en clair dans `accounts.json` permet à tout logiciel tournant sous le même utilisateur de prendre le compte Minecraft.

**Solutions.**

1. Vérifier dans `launcher/src/accounts.rs` comment les champs `access_token` / `refresh_token` sont écrits.
2. Les stocker via le coffre du système : crate Rust [`keyring`](https://crates.io/crates/keyring) (Windows Credential Manager / DPAPI, Keychain macOS, Secret Service Linux).
3. Ne garder dans `accounts.json` que l'identifiant du compte et une référence vers le coffre.
4. Ne jamais journaliser un jeton : ajouter un test qui parcourt les logs d'une session simulée et échoue si une chaîne ressemblant à un jeton apparaît.
5. Prévoir une migration : au premier lancement, déplacer les jetons existants vers le coffre puis les effacer du JSON.

---

### R6 — Aucune release, aucune signature, aucune mise à jour · 🟠 ✅ Constaté

**Constat.** « Releases » et « Packages » sont vides. Les installateurs sont exposés comme fichiers du dépôt. Le point « Mise à jour du launcher » est ouvert dans l'analyse. Le dépôt n'a ni description ni *topics*.

**Solutions.**

1. **Releases GitHub automatisées** sur tag `v*` (annexe A), avec `SHA256SUMS`.
2. **Signature de code Windows** : certificat OV/EV, ou signature via Azure Trusted Signing. Sans signature, SmartScreen avertira à chaque téléchargement.
3. **Mise à jour** : le plugin `tauri-plugin-updater` gère un manifeste signé (clé Ed25519). Le rapport décrit déjà ce besoin ; c'est la solution la plus directe puisque la coquille livrée est Tauri.
4. **Fiche du dépôt** : ajouter une description (« Fabric Minecraft client + launcher with first-class offline mode »), des topics (`minecraft`, `fabric`, `launcher`, `rust`, `tauri`, `electron`), une capture dans le README.

---

### R7 — Nom du projet incohérent · 🟡 ✅ Constaté

**Constat.** Dépôt : `MimiMax`. README et code : `Aether Client` / `Aether Launcher` / paquet `dev.aether.client`. Binaire racine : `MimiMax.exe`. Un utilisateur ne sait pas quel est le nom du produit.

**Solutions.**

- Choisir **un** nom public. Soit renommer le dépôt en `aether-client` (GitHub redirige automatiquement l'ancienne URL), soit ajouter en tête du README une ligne « *MimiMax est le dépôt de développement d'Aether* ».
- Aligner le nom du binaire, du dossier de données (`~/Aether`) et du titre de fenêtre.

---

### R8 — Documentation divergente et fichiers volumineux · 🟡 ✅ Constaté

**Constat.**

- Le README décrit une interface **Electron** (`npm start`), alors que le journal d'analyse indique que « le shell livré est **Tauri** » (`launcher/desktop/`, `cargo tauri build`). La procédure de construction du produit réellement livré n'est donc pas dans le README.
- `ANALYSE-AMELIORATIONS.md` fait **1140 lignes / 87 Ko** et mélange état courant, historique, journal de session et listes de propositions. Certaines sections sont barrées mais conservées.
- Trois comptes de tests différents circulent (87, 127, 151) selon la date du document.
- `RAPPORT-FINAL.md` (14/11) et `ANALYSE-AMELIORATIONS.md` (14→17/11) se recoupent.

**Solutions.**

1. README : ajouter « Construire le launcher livré (Tauri) » avec `cargo build --release` puis `cargo tauri build`, et préciser clairement le rôle d'Electron (GUI de développement ou héritage).
2. Créer un **`CHANGELOG.md`** (format *Keep a Changelog*) et y déplacer le journal daté ; garder dans `docs/` un seul document « État et backlog » de moins de 200 lignes.
3. Déplacer `ANALYSE-AMELIORATIONS.md`, `RAPPORT-FINAL.md`, `NETTOYAGE.md` dans `docs/archive/` ; ne laisser à la racine que `README`, `LICENSE`, `CHANGELOG`, `CONTRIBUTING`, `SECURITY`.
4. Ne citer les chiffres de tests qu'à **un seul endroit**, ou les remplacer par un badge CI.
5. Ajouter `SECURITY.md` (comment signaler une faille) et `CONTRIBUTING.md`.

---

### R9 — Chiffres de FPS présentés sans contexte · 🟡 ✅ Constaté

**Constat.** Le README montre « HUD at 889 FPS ». Le rapport précise que ce chiffre est obtenu en **854×480, distance de rendu 8, RTX 4060 Ti, serveur local vide**.

**Risque.** Impression de promesse irréaliste ; un utilisateur avec une machine modeste sera déçu.

**Solutions.**

- Légender la capture : « *Conditions : 854×480, render distance 8, RTX 4060 Ti / Ryzen 7 5700X, monde vide.* »
- Implémenter le **benchmark A/B** déjà prévu (même scène, preset *restore* vs *max*, p50/p99) et publier le **gain relatif** (« +X % sur votre machine ») plutôt qu'un FPS absolu.
- Mesurer aussi en 1080p, sinon l'évaluation reste peu représentative.

---

### R10 — Pack d'optimisation à versions figées · 🟡 ✅ Traité

> **État au 22 novembre 2026 — ✅ traité**, prouvé par des tests exécutés contre un serveur Modrinth
> local (`launcher/src/test_server.rs`), pas par lecture.
>
> `launcher/src/performance.rs` construit le graphe **avant le premier octet** :
>
> - **Version épinglée respectée.** Une dépendance `required` qui porte un `version_id` est lue par
>   `/version/{id}` et installée dans **cette** version, avec le SHA-1 publié par cette version ; la
>   version Fabric la plus récente n'est jamais substituée. Le repli « plus récente compatible » ne
>   sert que lorsque la dépendance n'épingle aucun `version_id`.
> - **Dédoublonnage par `project_id`.** Cloth Config, listé dans le pack **et** requis par
>   MoreCulling, ne produit qu'un nœud et un téléchargement. Deux dépendances qui épinglent la même
>   version partagent ce nœud ; deux versions épinglées **différentes** du même projet gardent la
>   première et produisent un `Conflict` explicite dans le rapport (`reconcile`), jamais un choix
>   silencieux. Un pin explicite remplace la « plus récente » que le graphe aurait choisie seul.
> - **Validation de la version de jeu et du loader.** Chaque nœud est contrôlé contre le pack :
>   `game_versions` doit contenir sa version et `loaders` doit contenir `fabric`. Une version
>   épinglée qui n'annonce ni l'un ni l'autre est refusée (rien ne prouve qu'elle tourne sur ce
>   Minecraft) ; une réponse non épinglée dont un de ces champs manque est acceptée, car la requête
>   qui l'a produite filtrait déjà sur les deux. Un nœud non supporté devient `unresolved`, le graphe
>   est `aborted` et **aucun fichier n'est téléchargé**.
> - **Déclarations explicites.** `optional` et `embedded` sont consignées dans le rapport et ne sont
>   pas installées ; `incompatible` produit un `Conflict`.
>
> | Solution proposée ci-dessous | État |
> | --- | --- |
> | 1. Résolution automatique des dépendances, échec **avant** l'installation | **Fait** — `build_graph` / `reconcile` / `install_from` ; tests `a_pinned_dependency_is_installed_at_its_exact_version` (version 1.0 épinglée téléchargée avec son SHA-1 alors que 2.0 existe), `a_pin_replaces_the_newest_build_the_pack_would_have_installed` (l'entrée du pack est remplacée par le pin, puis `verify` propre), `two_pinned_versions_of_one_project_are_an_explicit_conflict` (une seule version posée, conflit rapporté), `two_mods_that_pin_the_same_version_download_it_once`, `a_dependency_that_does_not_support_the_pack_version_aborts_the_graph` et `a_pinned_version_published_for_another_loader_aborts_the_graph` (zéro requête `/files/` dans les deux cas), `the_pack_is_installed_from_its_dependency_graph_without_a_duplicate` (Cloth Config explicite, sans doublon, puis `verify` propre) et `a_missing_required_dependency_downloads_nothing_at_all` |
> | 2. Tester la compatibilité du pack à chaque changement de version de jeu | **Partiel** — la résolution refuse désormais hors ligne tout nœud qui n'annonce pas la version de jeu ou le loader du pack (prouvé par `a_dependency_that_does_not_support_the_pack_version_aborts_the_graph`, `a_pinned_version_published_for_another_loader_aborts_the_graph` et `a_newest_build_published_for_another_loader_is_refused`) ; il reste une tâche planifiée qui interroge la **vraie** API Modrinth pour lister ce qui existe déjà lors d'une montée de version, ce qu'un test hors ligne ne peut pas établir |
> | 3. `THIRD-PARTY-NOTICES.md` | **Fait** (racine du dépôt) |
> | 4. Mode dégradé : installer le reste et nommer le mod manquant | **Partiel, volontairement** — un mod **optionnel** qui échoue n'empêche pas le reste et le rapport le nomme ; un mod requis, une dépendance requise non résolue, une version de jeu ou un loader non supporté arrêtent tout, pour ne pas produire un pack qui ne démarre pas |
>
> Limite restante, écrite telle quelle : un pin qui contredit un autre pin est signalé mais la
> première déclaration l'emporte, puisqu'aucune des deux ne peut satisfaire l'autre.

**Constat.** Le pack Modrinth contient 11 mods, vérifiés par SHA-1. Un défaut réel est déjà survenu : `MoreCulling` et `Dynamic FPS` exigeaient `Cloth Config`, absent du pack, donc **le jeu ne démarrait pas**.

**Risques.** À chaque nouvelle version de Minecraft, un mod peut ne pas exister encore ; une dépendance oubliée casse le lancement ; les licences des mods diffèrent.

**Solutions.**

1. **Résolution automatique des dépendances** via l'API Modrinth (`/version/{id}` → champ `dependencies` avec `dependency_type: "required"`) et échec **avant** l'installation si une dépendance manque (c'est exactement le point 6.1-7 du rapport).
2. Tester la compatibilité du pack lors d'un changement de version de jeu dans la CI (liste des mods disponibles pour la version cible).
3. Ajouter un fichier `THIRD-PARTY-NOTICES.md` listant licence et source de chaque mod. Le launcher télécharge les mods depuis Modrinth au lieu de les redistribuer : c'est la bonne approche, à conserver.
4. Mode dégradé : si un mod est indisponible, installer le reste et afficher lequel manque.

---

### R11 — Windows uniquement et CI non vérifiée · 🟡 🔎 À vérifier

**Constat.** Les outils de test reposent sur des `.exe` (`fake-core.exe`, `hang.exe`), PowerShell (`tools/setup.ps1`, `tools/gui.ps1`), `%APPDATA%`, `makensis`. Le dossier `.github/workflows` existe mais n'a pas pu être inspecté ; la checklist E2E est décrite comme manuelle.

**Solutions.**

1. Vérifier ce que font réellement les workflows et les inscrire dans le README (badge de statut).
2. Matrice de CI minimale :

```yaml
strategy:
  matrix:
    os: [windows-latest, ubuntu-latest]
```

   avec `cargo test --release` et `cargo clippy --all-targets -- -D warnings` sur les deux, `gradle test` pour le mod, `npm run test:security` pour la GUI.
3. Rendre `fake-core` compilable sur toutes les plateformes (pas d'extension `.exe` en dur) pour pouvoir lancer les E2E sous `xvfb-run` sur Linux.
4. Ajouter `cargo fmt --check` : le journal signale **97 écarts de formatage** laissés en place ; un passage unique de `cargo fmt` les supprime.
5. Dependabot (`cargo`, `npm`, `gradle`, `github-actions`) + `cargo audit` / `npm audit` dans la CI.

---

### R12 — Dette technique déjà identifiée · 🟡 ✅ Partiellement traitée

> **État au 22 novembre 2026 — 🟡 partiellement traitée** : la partie « installation » est faite et
> prouvée par des tests exécutés ; les points d'interface en fin de tableau restent ouverts.
>
> | Point | État |
> | --- | --- |
> | **Vérifier les fichiers, pas seulement les tailles** | **Fait** — `aether-launcher verify` recalcule le SHA-1 du client, des bibliothèques, de l'index d'assets et de chaque asset, ainsi que celui des jars du pack (`performance::verify`). `verify --repair` ne retélécharge que les fichiers faux (`installer::repair_from`). Test : `a_verification_reports_a_missing_client_and_an_altered_asset`, qui vérifie aussi que le fichier valide n'est **pas** redemandé au serveur. |
> | **Annulation au milieu d'un fichier** | **Fait** — `AtomicBool` passé à `Client::download_cancellable` et lu **après chaque chunk** ; le `.part` est conservé, donc la reprise repart de ces octets. Test : `a_cancelled_download_stops_within_one_chunk_and_keeps_its_partial_file`. |
> | **Reprise après installation interrompue** | **Fait** — `install.pending.json` (`launcher/src/pending.rs`) est écrit avant le premier octet, avancé à chaque étape, supprimé à la fin ; `info` l'expose (`pendingInstall`) et `install --discard` le nettoie avec les `.part`. Test : `an_interrupted_pack_installation_resumes_from_its_journal` (jar déjà vérifié non retéléchargé, jar manquant téléchargé, journal effacé, `verify` propre). |
> | **Processus orphelin** | **Fait** — `game.pid` + `stop` (déjà présent avant cette passe). |
> | **`close_on_launch`** | **Non fait** — détacher le jeu du cycle de vie de la coquille reste à concevoir ; hors périmètre de cette passe. |
> | **Tests JavaScript** | **Partiel** — `formatBytes` et `paintRange` sont extraits dans `launcher/gui/src/ui/format.js` et couverts par `node --test launcher/gui/src/ui/format.test.mjs` (ajouté à la CI). `crashReport` construit du DOM et reste couvert par l'E2E : ce n'est pas une fonction pure. |
> | **Mise à jour du launcher** | **Non fait** — le workflow de release existe (R6), `tauri-plugin-updater` n'est pas branché. |
> | **`prefers-reduced-motion`** | **Fait** — règle globale déjà présente dans `styles.css`. |
> | **Écran de config incohérent**, **conflits de touches**, **HUD 4K** | **Non fait** — points du client Java, hors de cette passe. |

Tirée de la propre liste du projet, avec une solution concrète pour chacune.

| Point ouvert | Pourquoi c'est utile | Solution |
|---|---|---|
| **Vérifier les fichiers, pas seulement les tailles** (`install_assets` compare `metadata.len()`) | Un asset tronqué à la bonne taille passe | Commande `verify` qui recalcule le SHA-1 de chaque asset selon l'index et ne retélécharge que les fichiers faux |
| **Annulation au milieu d'un fichier** | Un gros jar finit son téléchargement après « Cancel » | Passer le `AtomicBool` à `Client::download` et le tester à chaque chunk de la boucle de lecture ; conserver le `.part` |
| **Reprise après installation interrompue** | L'utilisateur ne sait pas qu'une installation est en attente | Écrire un `install.pending.json` au début, le supprimer à la fin ; au démarrage, proposer « Reprendre » |
| **Processus orphelin** (jeu actif après redémarrage du launcher) | `game:stop` répond « No game is running » | Fichier PID `game.pid` ; au démarrage, vérifier le PID et proposer de l'arrêter |
| **`close_on_launch`** | Fonction demandée, retirée faute de conception | Détacher le jeu du cycle de vie de la coquille (processus indépendant) et rediriger la sortie vers un fichier plutôt qu'un tube |
| **Tests JavaScript** (aucun fichier `*.test.js`) | `paintRange`, `formatBytes`, `crashReport` ne sont testés que par l'E2E, lent | Extraire ces fonctions pures dans un module et utiliser `node --test` |
| **Mise à jour du launcher** | Les correctifs ne parviennent pas aux utilisateurs | Voir R6 |
| **`prefers-reduced-motion`** : 1 seule règle pour 16 transitions et 6 animations | Accessibilité | Une règle globale `@media (prefers-reduced-motion: reduce) { *{animation:none!important;transition:none!important} }` |
| **Client : écran de config incohérent** (`ConfigScreen` n'utilise pas `RenderUtil.card`) | Aspect disparate | Réutiliser la palette de `RenderUtil` |
| **Client : conflits de touches** (deux modules sur la même touche) | Réglages « qui ne répondent pas » | Détecter à l'enregistrement et afficher une pastille rouge |
| **HUD non validé en 4K / échelle GUI 4** | Positions possiblement fausses | Ajouter un test de positionnement paramétré sur les échelles 1 à 4 |

**Piège de fond à surveiller dans le mod.** L'incident « la config activée faisait planter le démarrage » (`minecraft.options == null`) vient d'une règle générale : *rien ne doit toucher à `options` avant le premier tick*. Au lieu de le corriger module par module, ajouter une garde centrale et un test :

```java
// ModuleManager
public void enable(Module m) {
    if (Minecraft.getInstance().options == null) {
        pendingEnable.add(m);          // différé jusqu'au premier tick
        return;
    }
    m.onEnable();
}
```

---

### R13 — Langue et accessibilité · 🟡 ✅ Constaté

**Constat.** Interface du launcher en anglais seulement, alors que le projet et sa documentation sont rédigés en français ; navigation clavier incomplète ; pas de thème clair ; pas de filtre dans les listes de serveurs et de mods.

**Solutions.**

1. i18n minimale : `gui/src/i18n/en.json` + `fr.json`, fonction `t('key')` partout où l'UI écrit du texte en dur, langue choisie d'après `navigator.language`. Se mettre d'accord d'abord sur le fichier maître (anglais) pour éviter la dérive.
2. Ordre de tabulation et focus visible sur tous les contrôles ; test `a11y.mjs` étendu aux pages Servers et Performance.
3. Champ de filtre dans les listes dès 10 entrées.
4. Variables CSS pour le thème (accent, fond, rayon) afin de préparer le thème clair sans réécrire `styles.css`.

---

## 4. Plan d'action recommandé

### Cette semaine (≈ 1 journée)

1. **R2** : scan de secrets sur tout l'historique, révocation si nécessaire, `.gitignore`.
2. **R1** : retirer les `.exe` du suivi Git.
3. **R3** : imposer `white-list=true`, `server-ip=127.0.0.1` et un mot de passe RCON aléatoire.
4. **R7 / R8** : décider du nom public, corriger le README (Tauri), ajouter la description et les topics.

### Ce mois-ci

5. **R6** : workflow de release avec `SHA256SUMS`.
6. **R5** : jetons dans le coffre du système. — ✅ **fait**
7. **R4** : modules de combat dans un jar séparé ou désactivés par défaut, indicateur `-Daether.fairplay`.
8. **R11** : CI sur Windows et Linux, `cargo fmt`, Dependabot.
9. **R10** : résolution des dépendances Modrinth avant installation. — ✅ **fait**

### Ensuite

10. Signature de code et mise à jour automatique (R6).
11. Points techniques du tableau R12 (vérification SHA-1, reprise, processus orphelin). — ✅ vérification, reprise et commande `verify` faits ; restent `close_on_launch`, `crashReport` et les points « client »
12. i18n et accessibilité (R13), benchmark A/B (R9).

---

## 5. Checklist de validation

- [ ] `gitleaks detect --log-opts="--all"` ne trouve rien
- [ ] `git ls-files | grep -i '\.exe$'` ne renvoie rien
- [ ] Un `server.properties` généré contient `white-list=true` et un `rcon.password` non vide
- [x] `accounts.json` ne contient aucun jeton en clair, la migration et l'absence de jeton dans les journaux sont testées (22 novembre 2026)
- [ ] Une release GitHub contient les installateurs **et** `SHA256SUMS`
- [ ] Le README décrit la construction du produit réellement livré (Tauri)
- [ ] Les modules de combat sont désactivés par défaut ou absents du jar principal
- [ ] La CI passe sur Windows **et** Linux
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` et `cargo test` sont verts (22 novembre 2026)
- [x] Un jar dont le SHA-1 ne correspond pas est refusé et n'est jamais posé
- [x] Une dépendance requise absente arrête l'installation avant tout téléchargement
- [x] Cloth Config n'est résolu qu'une fois quand le pack le liste et qu'un mod le requiert
- [x] Une installation interrompue laisse `install.pending.json` et reprend sans retélécharger les fichiers vérifiés
- [x] `verify` signale les fichiers manquants/altérés et `verify --repair` ne retélécharge que ceux-là
- [x] `node --test launcher/gui/src/ui/format.test.mjs` est vert (4 tests)
- [x] Une annulation au milieu d'un fichier laisse un `.part` repris par l'installation suivante
- [ ] `gradle test` et `npm run test:security` restent à vérifier sur une machine avec la toolchain Java

---

## Annexe A — Workflow de release (point de départ)

À placer dans `.github/workflows/release.yml` :

```yaml
name: release
on:
  push:
    tags: ['v*']

jobs:
  windows:
    runs-on: windows-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable
      - uses: actions/setup-java@v4
        with: { distribution: temurin, java-version: '21' }
      - uses: actions/setup-node@v4
        with: { node-version: '22' }

      - name: Tests
        working-directory: launcher
        run: |
          cargo test --release
          cargo clippy --release --all-targets -- -D warnings

      - name: Mod Fabric
        working-directory: aether-client
        run: gradle build

      - name: Launcher (cœur + coquille Tauri)
        working-directory: launcher
        run: |
          cargo build --release
          cd desktop
          cargo install tauri-cli --version "^2" --locked
          cargo tauri build

      - name: Empreintes
        shell: pwsh
        run: |
          $files = Get-ChildItem -Recurse launcher/desktop/target/release/bundle -Include *.exe
          $files | ForEach-Object {
            "$((Get-FileHash $_ -Algorithm SHA256).Hash.ToLower())  $($_.Name)"
          } | Out-File SHA256SUMS -Encoding ascii

      - uses: softprops/action-gh-release@v2
        with:
          files: |
            launcher/desktop/target/release/bundle/nsis/*.exe
            aether-client/build/libs/*.jar
            SHA256SUMS
```

> Les chemins exacts (`bundle/nsis`, nom des jars) sont déduits de la documentation : à ajuster après un premier essai.

---

## Annexe B — Fichiers à ajouter à la racine

| Fichier | Contenu minimal |
|---|---|
| `SECURITY.md` | Comment signaler une faille en privé, versions supportées |
| `CONTRIBUTING.md` | Prérequis, commandes de test, style (`cargo fmt`) |
| `CHANGELOG.md` | Une entrée par release, à partir du journal du 14 au 17 novembre |
| `THIRD-PARTY-NOTICES.md` | Licence et source de chaque mod du pack et de chaque dépendance |
| `.github/dependabot.yml` | `cargo`, `npm`, `gradle`, `github-actions`, rythme hebdomadaire |
| `docs/archive/` | Anciens rapports (`ANALYSE-AMELIORATIONS.md`, `RAPPORT-FINAL.md`, `NETTOYAGE.md`) |

---

## Conclusion

Le fond est bon : l'architecture est propre, les tests sont nombreux et les défauts sont corrigés avec des preuves. Les actions à plus fort rendement sont **de l'hygiène et de la sécurité**, pas du code applicatif : sortir les binaires du dépôt, s'assurer qu'aucun secret n'y traîne, sécuriser les valeurs par défaut du serveur hébergé, clarifier le périmètre des modules de combat, puis automatiser les releases. Une fois cela fait, le projet est présentable publiquement et les améliorations fonctionnelles de la liste (vérification SHA-1, reprise, mise à jour automatique) pourront s'appuyer sur une base de distribution fiable.
