# Analyse d'améliorations - Aether Launcher

> **Suite :** le rapport final du 14 novembre 2026 (`RAPPORT-FINAL.md`) reprend l'analyse, les tests,
> les résultats FPS et la liste d'améliorations à jour, y compris les serveurs hors-ligne et
> l'hébergement local. Ce document reste l'historique de la revue précédente.
>
> **Mise à jour des 15, 16 et 17 novembre 2026 :** les sections datées de ces jours (« `truncate` en
> dichotomie », « Animation d'ouverture et cache de lignes », « Vérification visuelle du click GUI en
> jeu », « Rapport de plantage : code de sortie signé, lot `dist/` reconstruit », « Coût par frame du
> HUD et du click GUI », « Graphe FPS : colonnes fusionnées, barres de 2 px », ainsi que le §8 du
> 16/11) décrivent l'état actuel du client et du launcher, avec leurs mesures. Leurs références
> `fichier:ligne` ont été relevées sur la révision du 17 novembre 2026 (`HudModules.java` à 983
> lignes, **42 tests** verts) ; les comptes de tests plus faibles cités dans les sections du 15/11
> (25 puis 30) et du 16/11 (34) sont ceux de ces révisions-là.

*Document d'analyse du 14 novembre 2026. Chaque recommandation est ancrée dans le code du dépôt
(`fichier:ligne`) et les points déjà couverts par du code ou des tests sont signalés comme tels.*

- **Périmètre relu** : cœur Rust (`launcher/src/*.rs`), interface Electron (`launcher/gui/src/main.js`,
  `preload.js`, `gui/src/ui/app.js`, `styles.css`, `index.html`), outillage de vérification
  (`launcher/target/e2e/*`), configuration (`launcher/package.json`), documentation (`docs/`).
- **Méthode** : lecture du code, recherche des points d'entrée (IPC, commandes CLI), exécution des
  suites existantes, `cargo clippy --all-targets`, puis deux vérifications ciblées décrites en §0.
- **État mesuré de la base** :
  - `cargo test` : **87 tests, 0 échec** (58 lib + 6 download + 20 integration + 3 sha1).
  - `cargo test --release` (la commande de `package.json:12`) : **87 tests, 0 échec**.
  - Interface : **21/21** captures sans débordement, harnais E2E **41/41**.

---

## 0. Vérifications restées en suspens au passage précédent

### 0.1 Le glissement réel d'un slider met-il à jour `--fill` ? — **oui, prouvé**

Le doute venait de ce que la première preuve ne regardait que l'état après `applySettings()`, donc
après une écriture programmatique de `input.value`, sans passer par le widget.

Nouveau script `launcher/target/e2e/sliders.mjs` : il attache le protocole DevTools, lit la boîte du
slider, puis **envoie de vrais événements souris** (`Input.dispatchMouseEvent` : `mousePressed` à 10 %
de la piste, deux `mouseMoved`, `mouseReleased` à la position visée) et relit la valeur *et* la
variable CSS calculée. Résultat de l'exécution :

```
PASS play: memory — value 4096 → 13312, --fill 80% (attendu 80%)
PASS settings: maximum memory — value 4096 → 10752, --fill 63.33333333333333% (attendu 63%)
{"passed": 2, "total": 2}
SLIDERS_EXIT=0
```

Le glissement change donc bien `input.value`, l'événement `input` déclenche `paintRange`, et la
largeur peinte suit la valeur à moins d'un point de pourcentage. La preuve porte sur les deux
familles de sliders (Play `#range-memory`, Settings `#range-max-memory`), pas sur un seul.

### 0.2 `trim` d'`install.log` sur le vrai chemin d'écriture — **oui, prouvé**

Le test unitaire précédent appelait `logs::trim` directement. La question restante était : le
*lecteur de production* (`installer::log_download`) déclenche-t-il réellement le trim, à la bonne
borne, sans laisser de fichier temporaire ?

Plutôt que d'abaisser temporairement la constante (ce qui aurait demandé de modifier du code de
production pour un test), le test ajouté appelle **4000 fois le vrai `log_download`** avec des noms
d'assets longs (~200 octets par ligne, soit ~800 Kio écrits au total, au-delà des 512 Kio de
`logs::MAX_INSTALL_LOG_BYTES`) :

`launcher/src/installer.rs` — `the_log_of_the_installations_is_bounded_by_the_real_writer` :

- taille finale `<= 512 Kio` (la borne est respectée par l'écrivain, pas seulement par la fonction),
- taille finale `> 256 Kio` (le trim ne vide pas le fichier),
- la dernière ligne écrite est présente, les premières ont disparu,
- le fichier se termine par un saut de ligne (jamais de ligne coupée en deux),
- aucun `install.log.tmp` ne subsiste.

C'est plus strict que de baisser la constante : la valeur réellement utilisée en production est
exercée. Coût : ~1 s de test, aucun réseau.

**Limite documentée** : ce test écrit des lignes par `log_download`, pas par une installation
complète (qui demanderait ~550 Mo de téléchargement). La concurrence des 8 workers d'assets n'est pas
rejouée non plus ; le trim y est appelé depuis plusieurs threads et l'écriture est atomique
(`rename`), mais ce scénario n'est couvert que par la conception, pas par un test.

---

## 1. Constats vérifiés dans le code

### 1.1 Interface (renderer)

| # | Ancre | Constat |
| --- | --- | --- |
| A1 | `gui/src/ui/app.js:630-668` | `install()` fait `const result = await api.install({...})` (`:646`) **sans `try/catch/finally`**. Si l'IPC rejette (cœur tué, handler qui lève, renderer saturé), la barre de progression reste affichée, `#btn-install` reste désactivé et `state.installing` reste `true` : **plus aucune installation n'est possible** avant redémarrage de l'app. |
| A2 | `gui/src/ui/app.js:670-712` | Même schéma dans `launch()` (`:683`) : un rejet laisse `#btn-launch` désactivé et `#btn-stop` visible, avec un statut « Launching… » figé. |
| A3 | `gui/src/ui/app.js:710-720` | `stopGame()` : `await api.stop()` nu ; un rejet laisse le statut et les boutons dans l'état précédent. |
| A4 | `gui/src/ui/app.js:849-851` | La modale ne se ferme que par les boutons ou un clic sur le fond : **aucun `keydown`, donc pas de `Escape`**, pas de focus initial, pas de piège de focus, pas de restitution du focus au bouton d'origine. |
| A5 | `gui/src/ui/index.html:275-280` | La modale n'a ni `role="dialog"`, ni `aria-modal="true"`, ni `aria-labelledby` vers `#modal-title` (qui existe à `:277`). Un lecteur d'écran ne l'annonce pas comme une boîte de dialogue. |
| A6 | `gui/src/ui/index.html:285` | `#toasts` n'a pas `role="status"`/`aria-live` : les erreurs (échec d'installation, code de sortie du jeu) ne sont **jamais annoncées** à un lecteur d'écran. À comparer avec `#console` qui, lui, porte `aria-live="polite"` (`:124`). |
| A7 | `gui/src/ui/styles.css:1260`, `app.js:355-356` | Les interrupteurs de mods sont déjà corrects (`aria-checked`, `aria-label` explicite). Rien à faire de ce côté. |
| A8 | `gui/src/ui/app.js:20-30` | `toast()` empile sans limite et sans déduplication : une boucle d'erreurs (ex. logs illisibles) peut remplir l'écran. Mineur. |

### 1.2 Processus et IPC

| # | Ancre | Constat |
| --- | --- | --- |
| B1 | `gui/src/main.js:90-140` | `runCore()` n'a **aucun timeout** et ne tue pas l'enfant. Un cœur bloqué (réseau figé, antivirus, `versions` sur un DNS mort) laisse la promesse pendante pour toujours : l'onglet Paths reste vide, Settings ne se remplit pas, aucun message. `window-all-closed` (`:256`) quitte sans nettoyer. |
| B2 | `gui/src/main.js:334-343` | `install:start` n'a **pas de contrepartie** `install:stop`, alors que `game:stop` existe (`:370`). Une installation engagée ne peut être interrompue que par la fermeture de l'app (et les `.part` restent sur disque, ce qui est le comportement voulu pour la reprise, mais l'utilisateur n'a aucun bouton). |
| B3 | `gui/src/main.js:344-369` | `game:launch` conserve `activeGame` (`:15`) ; après un redémarrage du launcher pendant une partie, `game:stop` répond « No game is running » (`:370-374`) alors que le jeu tourne toujours — processus orphelin non tuable depuis l'UI. |
| B4 | `gui/src/preload.js:39-48` | Surface IPC déjà minimale et correcte (`contextBridge`, pas de `nodeIntegration` exposé) : bon point, pas d'action. |

### 1.3 Cœur Rust

| # | Ancre | Constat |
| --- | --- | --- |
| C1 | `src/installer.rs:107-130` | `install_version` ne prend **aucun jeton d'annulation** ; le seul arrêt possible est la mort du processus. Combiné à B2, l'annulation est absente de bout en bout. |
| C2 | `src/installer.rs:513-534` | `log_download` appelle `logs::trim` **à chaque ligne** (`:522-526`). Correct et désormais testé (voir §0.2), mais cela fait un `metadata()` + un `rename` potentiel sous les 8 workers d'assets, sans verrou : deux trims simultanés peuvent se marcher dessus (le `rename` reste atomique, donc pas de corruption, mais un trim peut en annuler un autre). |
| C3 | `src/settings.rs:37` | `close_on_launch` existe dans le modèle, est écrit dans `settings.json`… et **n'est lu nulle part** (aucune occurrence hors de `settings.rs`). Réglage mort : l'Interface n'expose pas de case et rien ne ferme le launcher au lancement du jeu. |
| C4 | `src/logs.rs:24-34` | `INSTALL_LOG`, `KEEP_SESSIONS = 20`, `MAX_INSTALL_LOG_BYTES = 512 Kio` : rétention et borne existent et sont testées. Le `prune` (`:83`) ne garde que les sessions, pas un total d'octets ; 20 sessions de plusieurs Mo restent possibles. |
| C5 | `src/http.rs:331-380` | Téléchargement : reprise par `Range`, reprise sur `416`, `MAX_ATTEMPTS` avec backoff, `.part` conservé pour la reprise, suppression en cas de checksum faux. **Robuste, déjà couvert** par 6 tests. |
| C6 | `src/http.rs:237`, `src/http.rs:795` | Deux avertissements `cargo clippy` : `Default` manquant pour `Client`, et une comparaison `== false` remplaçable par une négation. Triviaux, mais ils empêchent d'utiliser clippy comme garde-fou propre. |
| C7 | `src/game.rs:238` + `gui/src/ui/app.js:700-710` | Le cœur renvoie le code de sortie du jeu (`# exit code N` dans le log de session), mais l'UI se contente d'un toast `The game stopped with the code N.` : aucune piste (version, loader, Java, dernières lignes). Le log est pourtant déjà disponible dans l'onglet Logs. |
| C8 | `src/paths.rs`, `src/settings.rs` | Un seul `settings.json` global : pas de profils multiples (une version/loader/mémoire par instance ou par compte). Plusieurs comptes existent (`accounts.rs`), mais un seul jeu de réglages. |

### 1.4 Outillage et dette de tests

| # | Ancre | Constat |
| --- | --- | --- |
| D1 | `target/e2e/screens.mjs`, `target/e2e/drive.mjs`, `target/e2e/fake-core.exe` | Tout l'outillage E2E vit dans `target/`, c'est-à-dire dans le dossier que **`cargo clean` supprime** et qui n'est pas versionnable. Une vérification de 41 contrôles peut disparaître sans trace. |
| D2 | `gui/**` | **Aucun test JavaScript** (0 fichier `*.test.js`) : `paintRange`, `formatBytes`, `renderSessions`, la garde `sessionRequest` ne sont couverts que par l'E2E. |
| D3 | `package.json:12` | `"test": "cargo test --release"` alors que `docs/VERIFICATION.md:38` documente `cargo test`. Les deux passent (vérifié), mais le chemin release paie une compilation LTO complète à chaque exécution. |
| D4 | `docs/VERIFICATION.md:97-125` | La checklist E2E est **manuelle** ; elle ne mentionne ni `screens.mjs` ni `drive.mjs`, ni `sliders.mjs`. |

---

## 2. Plan par priorité

Notation : **impact** = ce que l'utilisateur gagne, **effort** = S (< 1 h) / M (demi-journée) /
L (plusieurs jours), **risque** = probabilité de régression.

### 2.1 À faire maintenant (fort impact, faible risque)

> **État au 16/11** : les **7 points sont clos**. 1 à 3 appliqués le 14/11 (§6) ; 4 (timeout) et
> 6 (clippy) vérifiés déjà appliqués le 16/11 ; 5 (outillage E2E) et 7 (`close_on_launch`)
> appliqués le 16/11 (§8).

| Amélioration | Preuve | Impact | Effort | Risque |
| --- | --- | --- | --- | --- |
| **1. Envelopper les `await` d'action dans `try/catch/finally`** — helper unique (`withBusy(button, work)`) réutilisé par `install`, `launch`, `stopGame`. | A1, A2, A3 | L'UI ne peut plus se bloquer définitivement sur un rejet IPC ; les boutons reviennent toujours à leur état. | S | Faible |
| **2. Rendre la modale accessible** — `role="dialog"`, `aria-modal`, `aria-labelledby`, focus initial sur le premier champ, `Escape`, restitution du focus. | A4, A5 | Utilisable au clavier et annoncée correctement ; `Escape` est le réflexe attendu. | S | Faible |
| **3. Annoncer les notifications** — `role="status"` + `aria-live="polite"` sur `#toasts` (et `role="alert"` pour les erreurs). | A6 | Les échecs cessent d'être silencieux pour les lecteurs d'écran. | S | Faible |
| **4. Timeout + `kill` dans `runCore`** — p. ex. 20 s (60 s pour `versions`), réponse `{ ok: false, error: 'The core did not answer' }`. | B1 | Fini les onglets vides sans explication ; le cœur bloqué est tué au lieu de rester en vie. | S | Moyen (timeout à calibrer) |
| **5. Sortir l'outillage E2E de `target/`** — `tools/e2e/`, scripts `npm run e2e`, mention dans `docs/VERIFICATION.md`. | D1, D4 | Les 41 contrôles et les 21 captures deviennent reproductibles et survivent à `cargo clean`. | S | Nul |
| **6. Nettoyer clippy** — `impl Default for Client`, `!x` au lieu de `x == false`. | C6 | `npm run lint` devient une garde utilisable (0 avertissement). | S | Nul |
| **7. Trancher `close_on_launch`** — soit l'implémenter (fermer la fenêtre après un lancement réussi), soit le retirer du modèle et de la CLI. | C3 | Plus de réglage fantôme qui laisse croire à un comportement inexistant. | S | Faible |

### 2.2 Ensuite (fort impact, effort ou risque moyen)

> **État au 16/11** : les **4 points sont appliqués et prouvés** — 8 (annulation), 9 (rapport de
> plantage) et 10 (profils) le 14/11 (§6), 11 (trim) le 16/11 (§8).

| Amélioration | Preuve | Impact | Effort | Risque |
| --- | --- | --- | --- | --- |
| **8. Annulation d'installation** — `Arc<AtomicBool>` passé à `install_version`, testé à chaque étape et dans la boucle des assets ; IPC `install:stop` + bouton « Cancel » à côté de la progression ; les `.part` sont conservés pour la reprise. | C1, B2 | Une installation de ~500 Mo peut être arrêtée proprement au lieu de forcer la fermeture de l'app. | M | Moyen (chemins de sortie à tester) |
| **9. Rapport de plantage** — à la fin d'une session avec code non nul : ouvrir l'onglet Logs sur cette session, afficher version/loader/Java/code et un bouton « Copy report ». | C7 | Un plantage devient diagnosticable sans aller fouiller `aether/logs`. | S-M | Faible |
| **10. Profils** — un profil nommé (version, loader, mémoire, taille de fenêtre) sélectionnable par compte, `settings.json` restant la valeur par défaut. | C8 | Plusieurs instances/mods sans tout reconfigurer ; c'est la demande naturelle d'un launcher. | M-L | Moyen |
| **11. Trim moins fréquent** — ne vérifier la taille que tous les N appels (ou via un compteur d'octets) plutôt qu'à chaque ligne. | C2 | Moins d'I/O pendant les ~2000 assets ; réduit la fenêtre de concurrence entre workers. | S | Faible |

### 2.3 Plus tard (valeur réelle mais coût ou dépendance externe)

| Amélioration | Preuve | Impact | Effort | Risque |
| --- | --- | --- | --- | --- |
| **12. Mise à jour du launcher** — `electron-updater` ou un manifeste signé ; aucun code aujourd'hui (`main.js` n'a pas de handler `app:update`, `package.json` ne fait que produire des artefacts). | B4 (absence), `package.json:24-70` | Les correctifs arrivent sans réinstallation manuelle. | M | Moyen (signature, rollback) |
| **13. Tests JavaScript unitaires** — extraire `paintRange`, le formatage des tailles et la garde `sessionRequest` en module testable ; `node --test`. | D2 | Les régressions d'affichage sont détectées sans lancer Electron. | M | Faible |
| **14. Plafond d'octets pour les sessions** — ~~`prune` ne borne que le nombre~~ **fait le 16/11** : `prune_with_budget`, 64 Mio, voir §8. | C4 | Un utilisateur qui enchaîne les parties très verbeuses garde 20 fichiers potentiellement gros. | S | Faible |
| **15. Processus orphelin** — au démarrage, détecter un jeu déjà lancé (fichier de PID) et proposer de l'arrêter. | B3 | Un launcher relancé ne perd plus le contrôle du jeu. | M | Moyen |

---

## 3. Déjà couvert — ne pas reproposer

Ces points sont souvent cités comme des manques mais **le code et les tests les traitent déjà** :

- **Reprise de téléchargement, retries, backoff, checksum, reprise après 416** : `http.rs:331-380`
  (`Client::download`), `.part` conservé, 6 tests dans `tests/download.rs`.
- **Protection zip-slip à l'extraction des natives** : `installer.rs`, `extract_zip`.
- **Index d'assets corrompu** : `installer.rs`, `read_asset_index` ; il est retéléchargé au lieu
  d'être cru sur parole, et `a_broken_asset_index_is_not_trusted` le vérifie.
- **Digest Fabric API** : `published_sha1` lit le `.sha1` publié par Maven au lieu de faire confiance
  au fichier.
- **Croissance du log d'installation** : `installer.rs:522-526` + `logs::MAX_INSTALL_LOG_BYTES` ; deux
  tests (dont celui de §0.2, sur le vrai écrivain).
- **Rétention des sessions** : `logs.rs:30` (`KEEP_SESSIONS = 20`), `prune` (`:83`), testé.
- **Clé de réglage inconnue refusée** : `settings.rs:74` (`patched`), avec la liste des clés valides.
- **Cœur manquant** : `main.js:92-98` répond avec un message explicite.
- **Sortie du jeu en direct** : `#console` porte `aria-live="polite"` (`index.html:124`).
- **Mise en page, débordements, états vides** : `screens.mjs` (21/21).
- **UUID hors ligne conforme à Minecraft** : `offline.rs` + tests d'intégration (20).
- **Sliders proportionnels** : `paintRange` + preuve de glissement réel (§0.1).

---

## 4. Limites de cette analyse

- Aucune installation réelle (550 Mo) n'a été rejouée : l'annulation (point 8), le trim sous 8
  workers concurrents (C2) et le diagnostic de plantage (C7) sont établis par lecture du code et
  tests unitaires, pas par une exécution complète.
- La connexion Microsoft (device code) n'est pas testée : elle exige un compte premium.
- Les mesures d'accessibilité sont des constats de structure (rôles/attributs), pas un audit
  automatisé (pas de Lighthouse/axe exécuté).
- Les efforts sont des ordres de grandeur, pas des mesures.

---

## 7. Pistes suivantes (brainstorm ordonné)

Ce qui reste, du plus rentable au plus accessoire. Chaque piste s'appuie sur un point du code
constaté, pas sur une envie générale.

| # | Piste | Ancre | Pourquoi maintenant |
| --- | --- | --- | --- |
| 1 | ~~**Profils nommés**~~ — **fait le 14/11**, voir §6 | `settings.rs` (`Profile`, `profile_for`), `accounts.rs` (`profile`) | Fait : le prochain levier est le rapport d'installation (piste 2). |
| 2 | ~~**Rapport pour une installation échouée**~~ — **fait le 16/11**, voir §8 | `installer.rs` (`log_download`), `app.js` (`install()`) | Fait : `showInstallReport` ouvre la modale sur `install.log`, prouvée par `failure.mjs` (23/23). |
| 3 | ~~**Lien « Open the logs » dans le rapport**~~ — **fait le 16/11**, voir §8 | `app.js` (`showCrashReport`), `renderSessions` | Fait : les deux rapports ont le bouton, `crash-report.mjs` ouvre la session nommée (12/12). |
| 4 | **Vérifier les fichiers, pas seulement les tailles** | `install_assets` compare `metadata.len()` à la taille de l'index | Un asset tronqué à la bonne taille passe. Un « Verify files » qui revalide les SHA1 sans retélécharger éviterait les réparations inutiles. |
| 5 | **Annulation au milieu d'un fichier** | `http.rs` (`Client::download`, boucle de lecture) | Le jeton n'arrête qu'entre deux fichiers : un gros jar finit toujours son téléchargement. |
| 6 | **Harnais E2E partagé** | `a11y.mjs`, `cancel.mjs`, `stubborn.mjs`, `failure.mjs`, `crash-report.mjs`, `timeout.mjs` | ~60 lignes de CDP (attach, `evaluate`, `waitFor`, `clickElement`) sont copiées six fois ; un `harness.mjs` les réduirait à un import. |
| 7 | **Reprise après installation interrompue** | `installer.rs`, `paths.rs` | Le `.part` est conservé, mais rien ne dit à l'utilisateur qu'une installation est en attente ; un marqueur au démarrage permettrait un bouton « Resume ». |
| 8 | ~~**Journal des sessions borné en octets**~~ — **fait le 16/11**, voir §8 | `logs.rs` (`KEEP_SESSIONS = 20`, `prune`) | Fait : `MAX_SESSIONS_BYTES = 64 Mio`, la plus récente toujours gardée, 3 tests. |
| 9 | ~~**Trim moins fréquent**~~ — **fait le 16/11**, voir §8 | `installer.rs:513-534` | Fait : compteur d'octets atomique, un check tous les 64 Kio, test de fréquence dédié. |
| 10 | **Tests JS unitaires** | `gui/src/ui/app.js` (`paintRange`, `formatBytes`, `crashReport`) | Ces fonctions sont pures et ne sont couvertes que par l'E2E, qui est lent et fragile. |

Deux choses restaient à trancher plutôt qu'à coder : la langue de l'interface (tout est en anglais
en dur, **toujours ouvert**) et l'emplacement de l'outillage E2E — **tranché le 16/11** : scripts,
sources du faux cœur et fixtures vivent dans `tools/e2e/`, plus rien dans `target/` (§8).

---

## 5. Récapitulatif

| Priorité | Nombre | Thèmes |
| --- | --- | --- |
| Maintenant | 7 (7 clos) | Robustesse des `await`, accessibilité de la modale et des toasts, timeout du cœur, outillage E2E hors de `target/`, clippy, réglage mort |
| Ensuite | 4 (4 faits) | Annulation d'installation ✅, rapport de plantage ✅, profils ✅, fréquence du trim ✅ |
| Plus tard | 4 (1 fait) | Mise à jour, tests JS, plafond d'octets des sessions ✅, processus orphelin |

Le déséquilibre est net : ce ne sont pas les fonctions manquantes qui coûtent le plus, mais les
**chemins d'erreur** (A1-A3, B1) qui laissent l'interface dans un état impossible à quitter. Ce sont
eux qui méritent le prochain passage.

---

## 6. Suites données aux points 1 à 3 (14/11)

| Point | Changement | Preuve |
| --- | --- | --- |
| 1 | `withBusy(reset, work)` dans `app.js` (`try/catch/finally`) utilisé par `install`, `launch` et `stopGame` : les commandes reviennent toujours à leur état, l'erreur du pont devient un toast. | **Prouvé par une panne réelle**, plus seulement par la sémantique du helper : `failure.mjs` fait échouer le vrai cœur (`AETHER_FAKE_FAIL`, `fake_core.rs`) pendant **Install** et **Launch** cliqués à la souris dans une vraie fenêtre Electron — 18/18 en mode `documents`, 7/7 en mode `launch-crash`. `a11y.mjs` couvre en plus le rejet direct du pont (9/9). |
| 2 | Modale : `role="dialog"`, `aria-modal`, `aria-labelledby`, focus sur le premier champ, `Tab` confiné, `Escape`, focus rendu au bouton d'origine ; pile de toasts en `role="status" aria-live="polite"`, erreurs en `role="alert"`. | `a11y.mjs` : 9/9 (annonce, focus, piège de tabulation, `Escape`, restitution du focus, live region, alerte). |
| 3 | `runCore` : `coreTimeout` (20 s, 45 s pour `versions`/`loaders`), minuterie annulée à la réponse, `killProcessTree` de l'arbre en cas de dépassement, `init()` affiche la raison au lieu d'une trace. | `timeout.mjs` avec `hang.exe` comme cœur : « did not answer within 20 s » dans le panneau et en alerte, plus aucun `hang.exe` vivant. |
| 9 | Rapport de plantage unifié : `showCrashReport({ title, problem, exitCode })` (`app.js`) couvre **les deux morts** — le jeu qui sort sur un code non nul (`exitCode`) et le cœur qui meurt sans répondre (`crashed`, marqué par `runCoreStreaming`). Il lit la dernière session (`logs:tail` + `logs:read`) et compose problème, code (quand il y en a un), version, loader, Java, nom du fichier et 40 dernières lignes ; « Copy report » passe par `app:copy` (`clipboard.writeText` côté processus principal). | `crash-report.mjs` en deux modes : `launch-exit` (jeu) 10/10 et `launch-crash` (cœur) 10/10 — titre, problème, présence/absence de la ligne de code, session, lignes, copie, fermeture, interface idle. |
| 8b | Kill de secours de l'annulation : si le cœur n'obéit pas à `cancel`, le pont tue l'arbre au bout de 10 s et l'interface conclut quand même sur « cancelled ». | `stubborn.mjs` avec `AETHER_FAKE_FAIL=install-stubborn` (cœur qui n'écoute **jamais** son entrée standard) : 10/10, attente mesurée **10,2 s**, aucun `aether-launcher.exe` survivant, état idle complet. |
| 10 | Profils nommés : `Profile { name, gameVersion, loaderVersion, maxMemory, minMemory, windowWidth, windowHeight }` et `profiles`/`activeProfile` dans `Settings` ; `Settings::load` migre en place un fichier sans profil en y matérialisant le profil `Default` construit sur les champs plats, et `Settings::save` resynchronise `Default` depuis ces champs (les deux ne peuvent pas diverger) ; `Settings::profile_for(choix du compte)` résout compte → profil actif → `Default`, utilisé par `install` et `launch_options` ; `Account::profile` + `AccountStore::set_profile` ; les règles vivent dans `Settings::put_profile` / `Settings::remove_profile` (`Default` protege) et `Launcher::add_profile` / `Launcher::remove_profile` (qui delie les comptes), la CLI `profiles list\|add\|use\|assign\|remove` ne fait plus que lire ses arguments ; interface : selecteur `#select-profile` et deux boutons d'icone dans la meme rangee, **+** (modale `New profile`, qui copie le profil de l'account) et **Delete** (modale de confirmation) ; les controles de la page Play (version, loader, memoire, fenetre) ecrivent dans le **profil resolu** via `profiles update`, donc les profils cessent d'etre des copies identiques ; `info` renvoie le profil resolu. | `cargo test` 94/94, dont `a_profile_replaces_the_one_with_the_same_name`, `the_default_profile_cannot_be_removed`, `a_settings_file_without_a_profile_gets_the_default_one`, `the_default_profile_follows_the_flat_settings` et `the_profile_of_an_account_wins_over_the_active_one` ; `profiles.mjs` 13/13 : deux profils listes, `Hardcore` choisi dans le selecteur, `info` renvoie `1.20.1` / `0.20.1` / 8192 Mo pour ce compte, choix stocke sur le compte, **`Cinematic` cree au clic** par la modale (copie de `Hardcore`, modale refermee), jouable, **supprime au clic** apres confirmation (annulation comprise) avec retour du compte au profil actif, `Default` refuse par un toast ; edition isolee prouvee : memoire 10240 et largeur 1280 sur `Hardcore`, `Default` reste a 4096/854, les champs plats ne bougent pas. 24/24. |
| 8 | Annulation coopérative : `install_version` et `download_assets` reçoivent un `&AtomicBool` lu avant chaque étape et avant chaque asset (`InstallError::Cancelled`) ; `main.rs` écoute `cancel` sur l'entrée standard ; IPC `install:stop` + bouton « Cancel install » ; les `.part` restent sur disque pour la reprise ; un cœur qui ignore la demande est tué au bout de 10 s. | `cancel.mjs` avec `AETHER_FAKE_FAIL=install-slow` (cœur qui ne s'arrête que sur `cancel`) : 12/12 ; 2 tests Rust (`an_installation_that_is_cancelled_before_it_starts_does_nothing`, `the_assets_stop_when_the_installation_is_cancelled`) ; 89 tests au total. |

- **Le bouton « Install » sortait de la fenêtre.** La première version des contrôles de profil ajoutait
  une rangée sous le sélecteur (~50 px). Les cartes de la page Play partagent les hauteurs de
  grille : la carte Profile plus haute a poussé la carte Session vers le bas, et le centre du
  bouton « Install » est passé à `y = 761` pour une fenêtre de 760 px — six contrôles E2E qui
  cliquent à la souris ont échoué d'un coup. Les contrôles tiennent maintenant dans la rangée du
  sélecteur (`y = 710`, `elementFromPoint` rend bien `btn-install`), la carte Profile garde sa
  hauteur.

- **Un fichier de réglages partiel effaçait tout.** Le test de migration du point 10 a révélé que
  `Settings` n'avait aucun `#[serde(default)]` : un `settings.json` auquel il manquait un seul champ
  ne se désérialisait pas et `Settings::load` retombait **entièrement** sur les valeurs par défaut.
  Ajouter un réglage aurait donc remis à zéro la configuration du joueur. Le conteneur porte
  maintenant `#[serde(default)]` : un champ absent prend sa valeur par défaut sans toucher aux autres.

### Interface in-game (Aether Client, 14/11)

| Sujet | Changement | Preuve |
| --- | --- | --- |
| Design system | `RenderUtil` gagne une palette (`SURFACE`, `SURFACE_RAISED`, `BORDER`, `INNER`…), `shadow`, `card`, `bordered`, `bar` et `scrollbar` ; le HUD et le click GUI les partagent au lieu de couleurs en dur. | `gradle build` + 19 tests. |
| Click GUI | Refonte : sidebar avec compteur de modules par categorie et barre d'accent, en-tete avec recherche encadree, lignes de module avec puce d'accent, chip de raccourci et interrupteur anime, ligne de description, pied de page contextuel, barre de defilement. Les nombres sont de vrais **sliders** : clic pour sauter a une valeur, glisser pour suivre la souris, molette pour affiner, double-clic pour revenir au defaut. | `MouseButtonEvent`/`mouseDragged` implementes ; compile et tests verts. |
| Options inertes | `Session.Layout` (`Horizontal`) etait declare et ignore : implemente (largeur et hauteur mesurees selon le mode). Les onglets `Player` et `Interface`, vides, sont supprimes. Les largeurs du HUD (coordonnees, marque du serveur, objet en main, potions) sont mesurees au lieu d'etre devinees, et l'armure suit la couleur d'accent au lieu d'un violet en dur. | `ModuleCategory`, `HudModules`, `HudElement` ; build vert. |
| Optimisation | Le HUD ne mesure plus ses elements qu'une frame sur cinq (formatage de chaines hors du chemin chaud) ; la molette du click GUI n'ecrit plus le fichier de configuration a chaque cran, l'ecran sauve a la fermeture. | `HudManager.render`, `ClickGuiScreen.mouseScrolled`. |

Limite assumee : aucun client Minecraft n'a ete lance ici (pas d'affichage, pas de session) ; la verification s'arrete au build et aux 19 tests du mod, pas a un rendu observe.

### Click GUI refait de zéro (19/11)

| Sujet | Changement | Preuve |
| --- | --- | --- |
| Composition | Le panneau quitte les cartes pour un **rail de catégories** à gauche (marque, compteurs par catégorie, pastille d'accent qui glisse d'une catégorie à l'autre) et une **liste de modules** à droite : nom, description sur une seconde ligne, chip de raccourci, chevron de réglages, interrupteur animé. L'en-tête porte le nom de la catégorie, le nombre de modules visibles et la recherche encadrée. | `gui/ClickGuiScreen.java`, build vert. |
| Géométrie | Une seule fonction de mise en page, `rows()`, rend chaque rectangle de ligne ; le dessin, le survol, le clic, le glisser et la molette lisent tous ces rectangles. Les positions qui étaient calculées deux fois (barre de réglage peinte dix pixels à côté de la zone cliquable, première catégorie peinte 34 px au-dessus de sa zone de clic) passent par des accesseurs uniques : `categoryTop`, `sliderX`, `toggleX`, `chevronX`, `chipTop`, `searchX`. | Aucune constante de position recopiée dans le code d'entrée. |
| Interactions | Clic sur une ligne : bascule le module. Clic sur le chevron (ou clic droit) : déplie les réglages. Clic sur le chip : capture la touche suivante (Échap délie). Recherche : filtre nom et description. Les nombres restent de vrais sliders (clic pour situer, glisser pour suivre, double-clic pour le défaut) et un clic sur le **nom** d'un réglage numérique ne le remet plus à zéro. | `mouseClicked`, `mouseDragged`, `keyPressed`, `charTyped`. |
| Fenêtre étroite | Le rail se réduit avant la liste (120 px minimum), la liste garde au moins 120 px, les descriptions disparaissent sous 300 px de large : plus de largeur négative sur une petite fenêtre. | Calcul dans `init()`. |
| Vérification | Build et 87 tests verts ; la composition a été contrôlée par un rendu statique des mêmes coordonnées, **pas** par une capture du jeu (la fenêtre native ne peut pas être pilotée ici). | `./gradlew build` ; mockup temporaire supprimé. |

### Deux défauts révélés par la panne réelle (corrigés)

- **Une progression tardive repeignait la barre après l'échec.** Le cœur peut écrire une ligne
  `progress` juste avant son document d'erreur ; livrée après la remise à zéro, elle remettait la
  barre à 25 % alors qu'elle devait être vide et masquée. Le gestionnaire `onInstallProgress`
  (`gui/src/ui/app.js`) ignore désormais tout événement quand `state.installing` est faux.
- **Un cœur qui meurt sans document passait pour un succès.** `runCoreStreaming` (`gui/src/main.js`)
  résolvait `{ ok: true }` par défaut : un plantage sec (code 3, aucune sortie) affichait
  « The game closed. ». La fermeture lit maintenant le code de sortie et, sans document, un code
  non nul devient `{ ok: false, error: 'The launcher core stopped with the code N.' }`.

Outillage : `fake_core.rs` accepte `AETHER_FAKE_FAIL` (`install`, `launch`, `launch-crash`) ;
`failure.mjs <port> documents|crash` clique réellement sur Install puis Launch et relit l'état
complet (bouton, barre, contrôles gelés, drapeaux `installing`/`launching`).
### Le preset `balanced` empêchait la JVM de démarrer (15/11)

Symptôme rapporté : « the game stopped with the code 1 », session `session-20261115-013227.log`
contenant `Error: VM option 'G1NewSizePercent' is experimental and must be enabled via
-XX:+UnlockExperimentalVMOptions` puis `The unlock option must precede 'G1NewSizePercent'`.

Cause : `jvm_preset_flags("balanced")` (`launcher/src/game.rs:121-127`) passait
`-XX:G1NewSizePercent=20` sans `-XX:+UnlockExperimentalVMOptions`. Le preset `balanced` est celui
que les réglages par défaut utilisent (`"jvmPreset": "balanced"` dans `aether/settings.json`), donc
tout lancement avec les réglages d'usine échouait avant même le chargement des classes. Le preset
`performance` avait bien le drapeau, mais placé après `-XX:MaxGCPauseMillis` : tant qu'aucune option
expérimentale ne le précédait, cela passait — un simple réordonnancement l'aurait cassé aussi.

Correctif, en deux endroits :

- `launcher/src/game.rs:124` : `balanced` commence désormais par `-XX:+UnlockExperimentalVMOptions`.
- `launcher/src/game.rs:164-190` : `unlock_experimental_options` normalise la liste finale des
  arguments — il retire tout `-XX:+UnlockExperimentalVMOptions` existant et le réinsère juste avant
  la première option expérimentale de G1 (`G1NewSizePercent`, `G1MaxNewSizePercent`,
  `G1HeapWastePercent`, `G1MixedGCLiveThresholdPercent`, `G1RSetUpdatingPauseTimePercent`). Appelé
  depuis `build_jvm_arguments` (`game.rs:246`), il couvre donc les presets, les arguments de la
  version et tout réglage utilisateur, quel que soit l'ordre d'arrivée.

Preuves :

| Contrôle | Résultat |
| --- | --- |
| `cargo test` (launcher) | 100 tests lib + 6 + 20 + 3, **0 échec** ; deux nouveaux tests : `an_experimental_option_is_always_unlocked_first` (les trois presets) et `the_unlock_flag_is_moved_in_front_of_a_late_one`. |
| JVM seule, ancien ordre | `java -XX:G1NewSizePercent=20 -XX:+UnlockExperimentalVMOptions -version` → l'erreur exacte du rapport, reproduite avec le Java 21 du rapport. |
| JVM seule, nouvel ordre | les drapeaux du preset `balanced` complets, puis `openjdk version "21.0.13"` — la JVM démarre. |
| Lancement réel (cœur release, `AETHER_HOME=tools/aether-home`, preset `ultra`) | fenêtre « Minecraft* 1.21.11 », `Setting user: Notch`, `Backend library: LWJGL version 3.3.3-snapshot`, écran titre avec la marque Aether et `OFFLINE MODE` (`aether-client/build/shots/07-launcher-launch.png`). |
| Binaire livré | `cargo build --release` puis `launcher/desktop` reconstruit ; `dist/Aether-Launcher-1.0.0-portable.exe` remplacé, empreinte SHA-256 mise à jour dans `dist/LISEZ-MOI.txt`. |

L'installateur `dist/Aether-Launcher-Setup-1.0.0.exe` a été régénéré depuis ce même cœur corrigé
le 15/11 : voir « Le lot distribué reconstruit » plus bas.
### Le lot distribué reconstruit (15/11)

Le correctif du preset `balanced` n'avait été appliqué qu'au cœur et à l'exe portable ; l'installateur
NSIS livré datait du 15/11 00:08 et embarquait encore l'ancien cœur. Tout le lot est maintenant
reconstruit depuis les mêmes binaires release, et l'identité du cœur embarqué est vérifiée au lieu
d'être supposée.

Chaîne de fabrication, dans l'ordre :

| Étape | Commande | Résultat |
| --- | --- | --- |
| Cœur corrigé | `cargo build --release` (`launcher/`) | `target/release/aether-launcher.exe`, 2 207 744 o, SHA-256 `D20E27E5…86BE65710` |
| Coquille + bundle | `cargo tauri build` (`launcher/desktop/`) | `target/release/aether-desktop.exe` puis `Running makensis` → `bundle/nsis/Aether Launcher_1.0.0_x64-setup.exe` (4,25 Mo). `tauri-cli 2.12.1` et NSIS en cache (`%LOCALAPPDATA%\tauri\NSIS`) : aucune étape réseau, EXIT=0 (`tools/tauri-bundle7.log`) |
| Livraison | copie des deux binaires dans `dist/` | portable 8,53 Mo, installateur 4,25 Mo |

Vérification du contenu, par inspection et non par confiance :

- **Le cœur patché est bien dans la coquille** : recherche de la séquence d'octets du cœur release
  dans `aether-desktop.exe` → trouvée à l'offset 4 833 802 ; les octets extraits ont le même SHA-256
  que le cœur (`D20E27E5…86BE65710`), donc identiques au binaire qui a démarré le jeu plus haut.
- **L'installateur contient bien cette coquille** : installation silencieuse réelle de l'installateur
  régénéré (`/S /D=…\tools\setup-unpack`) ; le fichier installé `aether-desktop.exe` (8 947 200 o)
  porte le cœur patché au même offset 4 833 802, avec le même SHA-256 `D20E27E5…86BE65710`. Son
  empreinte globale (`64F5A72E…18DD7BE0`) diffère de celle de `bundle/nsis` car NSIS réécrit
  l'en-tête au moment de l'installation — c'est le cœur embarqué qui fait foi, et il est identique.
- **Nettoyage** : désinstallation silencieuse (`uninstall.exe /S`), dossier temporaire supprimé,
  plus aucune clé `Aether Launcher` dans `HKCU\…\Uninstall` ; il n'existait aucune installation
  préalable de l'utilisateur, donc rien n'a été écrasé.
- **L'exe portable livré s'ouvre** : `dist/Aether-Launcher-1.0.0-portable.exe` lancé, fenêtre
  « Aether Launcher » (`aether-desktop`, PID 20236) présente, puis fermée.

Empreintes consignées dans `dist/LISEZ-MOI.txt` (tailles corrigées au passage) :

| Fichier | Taille | SHA-256 |
| --- | --- | --- |
| `Aether-Launcher-Setup-1.0.0.exe` | 4 460 475 o (4,25 Mo) | `B521FDA170218E6AFDD557903FCBC7B1FA89C70E780A797F47AE197BB9B63DDA` |
| `Aether-Launcher-1.0.0-portable.exe` | 8 947 200 o (8,53 Mo) | `EA43AE3FB65F52472C8ECCB5230B2C347F5A8879F099A29E94F76ECBA27B3181` |

Limite assumée : le parcours d'installation graphique (assistant, raccourcis, désinstallateur depuis
« Applications ») n'a pas été cliqué ; seule l'installation silencieuse a été exécutée, et la preuve
porte sur le contenu déposé, pas sur l'apparence de l'assistant.
### L'exe portable retrouve seul son installation (15/11)

Constat : lancé sans variable d'environnement, l'exe portable travaillait sur `~/Aether`, un dossier
laissé vide par un premier démarrage, alors que l'installation réelle (1.21.11, Fabric 0.20.1, mods,
compte Notch) était dans `tools/aether-home`. L'interface proposait donc d'installer ce qui existait
déjà, et le bouton Play ne pouvait pas jouer.

Correctif, dans `launcher/src/paths.rs` :

- `resolve_home` remplace l'ancienne affectation en dur : `AETHER_HOME` gagne toujours ; sinon
  `~/Aether` n'est gardé que s'il contient une vraie installation (`is_installation` : une version
  avec son `json`, ou un index d'assets téléchargé, pas un dossier vide) ; sinon le launcher cherche
  une installation à côté de l'exécutable (`find_installation_near`, parcours borné à trois niveaux,
  alphabétique, qui évite les dossiers d'un home et les dossiers de build).
- `search_anchor` : la coquille Tauri écrit le cœur embarqué dans `%LOCALAPPDATA%\dev.aether.launcher\bin`,
  donc `current_exe()` du cœur pointe sur ce dossier et non sur `dist/`. La coquille passe désormais
  `AETHER_ANCHOR` (son propre dossier) au cœur (`anchored` dans `launcher/desktop/src/core.rs`, sur les
  deux points de lancement) ; le cœur lancé seul retombe sur son propre dossier.
- `AETHER_ANCHOR` est documenté dans le tableau des variables de `launcher/README.md`.

Preuves :

| Contrôle | Résultat |
| --- | --- |
| `cargo test` (launcher) | **107 tests lib** + 6 + 20 + 3, 0 échec ; 7 nouveaux tests dans `paths::tests` (home vide, version téléchargée, index d'assets, découverte depuis `dist/` vers `tools/aether-home`, priorité au home voisin de l'exe, non-descente dans les dossiers d'un home, voisinage vide). |
| Résolution en ligne de commande | `AETHER_ANCHOR=<racine>/dist aether-launcher.exe info` → `home = …\tools\aether-home`, `game = …\tools\aether-home\.minecraft`, profil `Default` 1.21.11, alors que `~/Aether` est vide. |
| Exe portable livré, sans aucune variable | lancement de `dist/Aether-Launcher-1.0.0-portable.exe` : l'interface affiche **Notch / OFFLINE**, profil `Default · 1.21.11 · 4096 MB`, Fabric `0.20.1`, statut **Ready** (`tools/ui-autodiscover.png`) — plus de version 26.3 ni de dossier vide. |
| Jeu lancé depuis cet état | clic sur **Launch Minecraft** (479, 713) → processus `java` PID 15532, fenêtre « Minecraft* 1.21.11 », journal `session-20261115-021450.log` : `Loading Minecraft 1.21.11 with Fabric Loader 0.20.1`, `Setting user: Notch`, `Backend library: LWJGL version 3.3.3-snapshot` (`tools/play-by-launcher2.png`). |
| Fermeture | clic sur **Stop** → 0 processus `java` ; fermeture de la fenêtre → plus aucun processus `Aether-Launcher*`. |

Livraison : `cargo tauri build` a reconstruit l'ensemble (`tools/tauri-bundle9.log`) ; `dist/` porte
la version découverte automatiquement. Empreintes à jour dans `dist/LISEZ-MOI.txt` (elles remplacent
celles du paragraphe précédent) :

| Fichier | Taille | SHA-256 |
| --- | --- | --- |
| `Aether-Launcher-Setup-1.0.0.exe` | 4 463 768 o (4,26 Mo) | `FE964ACFB5B0F9A9C6988993B35623EEAC2138B6177897627A3A04B968723451` |
| `Aether-Launcher-1.0.0-portable.exe` | 8 965 120 o (8,55 Mo) | `FBD828DE3C9261B05A658DDCDFAC1C7E9569426577D340D68AA57F4776E7787C` |

### La preuve de lancement par le bouton Play (15/11, tour précédent)

Le jeu a été démarré **par l'interface**, pas par la ligne de commande : `dist/Aether-Launcher-1.0.0-portable.exe`
lancé, page Play affichée (compte Notch, profil `Default · 1.21.11 · 4096 MB`, statut Ready), puis un
clic réel sur « Launch Minecraft » à (479, 713) envoyé dans la fenêtre (`mouse_event`, après
`SetForegroundWindow`, helper `tools/gui.ps1`).

- Conséquence côté launcher : statut **Playing**, bouton « Launch Minecraft » grisé, bouton **Stop**
  apparu, sortie de session en direct (`tools/ui-play-02.png`).
- Conséquence côté jeu : fenêtre « Minecraft* 1.21.11 » (PID 21468) et journal
  `tools/aether-home/aether/logs/session-20261115-015726.log` ouvert à 01:57:26 — `Loading Minecraft
  1.21.11 with Fabric Loader 0.20.1`, `Setting user: Notch`, `Backend library: LWJGL version
  3.3.3-snapshot` ; capture de l'écran titre avec la marque Aether et le badge OFFLINE (modded)
  (`tools/play-by-launcher.png`).
- Fermeture propre : clic sur **Stop** (585, 713) → retour à **Ready** (`tools/ui-play-03.png`), puis
  fermeture de la fenêtre du launcher.

Réserve de ce tour-là : le portable avait été lancé avec `AETHER_HOME` pointé sur `tools/aether-home`,
puisqu'il ne savait pas encore trouver ce dossier seul ; c'est précisément ce que le correctif
ci-dessus supprime. Le tour suivant a refait le même parcours **sans aucune variable** et obtenu le
même résultat (`session-20261115-021450.log`).
### Découverte resserrée et installation retenue (15/11)

Relu à froid, le mécanisme du tour précédent avait deux défauts. D'abord il parcourait l'arborescence
autour de l'exécutable (trois niveaux, avec une liste d'exclusions) : trop de surface pour un besoin
qui se résume à deux emplacements. Ensuite sa marque d'installation était « une version avec un json,
ou un index d'assets » — or c'est exactement ce qu'un `.minecraft` étranger contient. La vérification
en conditions réelles l'a montré : un cœur copié dans un dossier temporaire a adopté
`%TEMP%\aether-paths-<pid>-installed-home`, un dossier laissé par les tests unitaires, au lieu de
rester sur `~/Aether`.

Correctif, toujours dans `launcher/src/paths.rs` :

- **Plus aucun parcours de disque.** `find_installation_near` n'essaie qu'une liste fixe : le dossier
  de l'exécutable et son parent, chacun testé lui-même puis avec les noms `aether-home`, `.aether`,
  `Aether`, `tools/aether-home` (`NEAR_NAMES`). Dix `stat`, déterministes ; `SEARCH_DEPTH`,
  `SEARCH_SKIPPED` et la fonction récursive ont disparu.
- **Une marque que seul Aether écrit.** `is_installation` exige désormais `aether/settings.json`
  (le fichier de réglages du launcher) **et** une version téléchargée avec son json. Un `.minecraft`
  d'un autre launcher, un dossier de test, un simple index d'assets ne passent plus.
- **L'installation trouvée est retenue.** `remember_home` écrit son chemin dans
  `%APPDATA%\Aether\home.txt` (`$XDG_CONFIG_HOME/aether/home.txt` ou `~/.config/aether/home.txt`
  ailleurs), hors de toute installation et hors du dossier de l'exécutable, qui peut être en lecture
  seule une fois installé. `resolve_home` lit ce fichier en deuxième position, après `AETHER_HOME` et
  avant `~/Aether`, et ne le garde que s'il désigne encore une installation ; `AETHER_SHARED_DIR`
  désactive la recherche, puisque le jeu est alors ailleurs et que le home garde ses réglages.

Preuves :

| Contrôle | Résultat |
| --- | --- |
| `cargo test` (launcher) | **111 tests lib** + 6 + 20 + 3, 0 échec ; nouveaux tests : marque Aether absente malgré une version, `.minecraft` d'un autre launcher, index d'assets seul, voisin nommé hors liste, pointeur relu, pointeur vide ignoré. |
| Découverte puis mémorisation | pointeur supprimé, `AETHER_ANCHOR=dist aether-launcher.exe info` → `home = …\tools\aether-home` ; `%APPDATA%\Aether\home.txt` créé avec ce chemin. |
| Rappel entre deux emplacements | cœur copié seul dans un dossier temporaire (aucune installation voisine) → `home = …\tools\aether-home`, trouvé par le pointeur seul. |
| Garde contre un dossier sans rapport | dossier voisin nommé `Aether` avec `versions/1.21.11/1.21.11.json` mais sans `aether/settings.json`, pointeur supprimé → `home = C:\Users\ee\Aether` (valeur par défaut) et **aucun** pointeur écrit : le faux home est refusé. |
| Exe portable livré | lancé sans aucune variable : interface sur **Notch**, profil `Default · 1.21.11 · 4096 MB`, statut Ready (`tools/ui-remembered-01.png`) et pointeur écrit. |
| Cœur embarqué, emplacement déplacé | l'exe portable copié dans `%TEMP%\aether-moved` puis son cœur embarqué interrogé avec `AETHER_ANCHOR=%TEMP%\aether-moved` → `home = …\tools\aether-home`. La capture d'écran de ce cas-là est inutilisable : une autre application plein écran occupait le premier plan, la preuve est donc la sortie du cœur, pas une image. |

Livraison : `cargo tauri build` (`tools/tauri-bundle10.log`), `dist/` mis à jour ; empreintes dans
`dist/LISEZ-MOI.txt` :

| Fichier | Taille | SHA-256 |
| --- | --- | --- |
| `Aether-Launcher-Setup-1.0.0.exe` | 4 464 548 o (4,26 Mo) | `751F1BB12FBFDF5A4793C10DA5230FB17415FE25A8C66CDD01F0A7DE88584A62` |
| `Aether-Launcher-1.0.0-portable.exe` | 8 959 488 o (8,54 Mo) | `3782E5CCAEEBAE6113DD9A97FFD50F6E93C9DC9C0DDB660007EA742179B7D1A3` |

Limite : le pointeur est propre à la session Windows de l'utilisateur ; un profil Windows différent
refait une découverte à partir de son propre `%APPDATA%`. Le cas « installation dans `tools/aether-home` »
reste reconnu par la liste fixe, mais une disposition inédite demanderait soit `AETHER_HOME`, soit un
premier lancement à côté de l'installation.
### Priorité testée et découverte simplifiée (15/11)

La priorité de `resolve_home` n'était vérifiée par aucun test : elle lisait l'environnement, le
pointeur, le disque et lançait la découverte dans la même fonction, donc l'ordre ne pouvait être
exercé qu'en manipulant des variables du processus.

Extraction, dans `launcher/src/paths.rs` :

- `select_home(explicit, remembered, default, shared, discover) -> Selection` porte désormais toute la
  priorité, sans toucher à l'environnement ni écrire sur le disque ; la découverte est une closure
  `FnOnce`, qu'un test peut donc observer — ou interdire. `Selection { path, origin }` et
  `enum Origin { Explicit, Remembered, Default, Discovered }` remplacent le retour nu, si bien que
  l'appelant sait s'il doit mémoriser sans booléen ambigu.
- `resolve_home` se réduit à lire l'environnement, appeler `select_home`, puis écrire le pointeur
  quand l'origine est `Discovered`.

Sept tests de priorité (dossiers temporaires réels) : `the_explicit_home_wins_over_everything`,
`the_remembered_home_comes_before_the_default_one`, `a_remembered_home_that_disappeared_is_skipped`,
`a_usable_default_home_comes_before_the_discovery`,
`the_discovery_is_used_when_the_default_home_is_empty` (origine `Discovered`),
`a_shared_game_folder_disables_the_discovery` (la closure `panic!` si elle était appelée) et
`nothing_found_leaves_the_default_home`.

Simplifications trouvées en relisant ce tour :

- `env_value(name)` remplace trois lectures d'environnement répétées (`AETHER_HOME`,
  `AETHER_SHARED_DIR`, `AETHER_ANCHOR`).
- `is_installation` tient en une ligne (`settings.json` **et** `has_downloaded_version`) ; la boucle
  imbriquée de lecture de répertoires part dans `has_downloaded_version` et `contains_json`.
- `find_installation_near` n'alloue plus de `Vec` de candidats : deux `for` imbriqués, retour au
  premier trouvé.
- Les enrobages `remembered_home`/`remember_home` (une ligne chacun, un seul appelant) disparaissent ;
  la raison d'être du pointeur passe dans la doc de `pointer_directory`, et l'ordre n'est plus
  documenté deux fois (uniquement sur `select_home`).

Résultats : `cargo test` → **118 tests lib** + 6 + 20 + 3 = **147, 0 échec** ;
`cargo clippy --all-targets` sans avertissement ; `cargo fmt --check` ne signale plus rien dans
`paths.rs` (les 97 écarts antérieurs du reste du dépôt sont laissés en place, hors sujet).
Comportement inchangé, vérifié en ligne de commande : `AETHER_ANCHOR=dist … info` →
`home = …\tools\aether-home`.

Livraison de cette révision : `cargo build --release` puis `cargo tauri build`
(`tools/tauri-bundle11.log`, EXIT=0), `dist/` remplacé. Le cœur embarqué a été vérifié par
inspection : 2 211 328 o, SHA-256 `86537D92…B1A9591`, trouvé à l'offset 4 840 970 dans l'exe portable
**et** dans le binaire déposé par l'installateur (installation silencieuse puis désinstallation, aucune
clé résiduelle). Empreintes dans `dist/LISEZ-MOI.txt` :

| Fichier | Taille | SHA-256 |
| --- | --- | --- |
| `Aether-Launcher-Setup-1.0.0.exe` | 4 462 463 o (4,26 Mo) | `2A1CF4E466EF541F5B657E74EA3A62B4370F6D9855ED0305DA32CBC7D6B503B0` |
| `Aether-Launcher-1.0.0-portable.exe` | 8 958 976 o (8,54 Mo) | `37390F68ADB6A3E963DF96998D2A42FB7C2F6A6E36E07E21143FCD00D74F887C` |
### Animation d'ouverture et cache de lignes (15/11)

Première livraison des axes performance et animations, dans le client.

- **Ouverture animée du click GUI** (`gui/ClickGuiScreen.java:48`, `:73`, `:120`, `:132-148`, `:213`).
  Le panneau démarre 12 px plus bas et remonte en 170 ms (courbe ease-out cubique, `entrance()`),
  pendant que le voile sombre s'éclaircit de 0 à 78 % d'opacité. La position n'est pas un simple
  décalage de dessin : `layout(top)` recalcule `panelY`, `listY`, `footerY` et `listHeight`, donc les
  tests de clic lisent exactement la position affichée. Une frame immobile ne recalcule rien d'autre.
- **Cache des lignes** (`:86-88`, `:171-198`, `:497`). `rows()` reconstruisait une liste
  d'enregistrements et une liste de réglages à chaque appel — donc à chaque frame et à chaque
  mouvement de souris. Le résultat est gardé tant que `listY` et le décalage de défilement n'ont pas
  bougé ; le cache est invalidé par la recherche, le changement de catégorie et l'ouverture d'un
  module. Pendant l'animation, `listY` change à chaque frame, donc le cache se reconstruit — puis se
  fige dès que le panneau est immobile.

Vérification : `gradle test build --offline` → **BUILD SUCCESSFUL**, 25 tests ; jar
`aether-client-1.0.0.jar` de 122 423 o, copié dans `tools/aether-home/.minecraft/mods/aether-client.jar`
pour que le prochain lancement l'utilise.

Limite de ce tour : **aucune capture du rendu**. Une autre application occupait le premier plan
pendant la vérification et mes clics automatisés ont été annulés aussitôt ; l'animation et le cache
sont donc prouvés par la compilation et la relecture du code, pas par une image. `dist/` n'a pas été
reconstruit : le changement est dans le mod, pas dans le launcher.

### Propositions classées (performance, visuel, animations, jouabilité, textures, launcher)

**Performance**

1. `RenderUtil.truncate` — **fait**, voir « `truncate` en dichotomie » : la mesure est dichotomique
   (`util/TextFit.java:30`, appelée depuis `util/RenderUtil.java:243`) et passe de 283 à 9 mesures
   pour un texte de 300 caractères.
2. `HudModules` formatait à chaque frame — **fait pour les coordonnées** : la chaîne est gardée tant
   que x, y, z, l'orientation, le biome et les deux interrupteurs n'ont pas bougé
   (`module/modules/HudModules.java:621-654`). Restent le chronomètre de session (`:804`) et la barre
   de vie du target HUD (`:876`), listés dans « Coût par frame… ».
3. `RenderUtil.shadow` dessinait trois rectangles arrondis par carte — **fait** : une seule passe
   depuis `util/RenderUtil.java:117`. Gain mesuré dans « Coût par frame du HUD et du click GUI ».
4. Étendre la mesure « une frame sur cinq » de `HudManager` (`:render`) aux listes de potions et
   d'armure, dont la largeur ne change qu'à l'usage d'un objet.

**Visuel / UI / GUI**

5. Le survol d'une ligne est binaire (`renderRow`, `:225`) : un facteur par module, interpolé sur
   80 ms, rendrait le panneau aussi doux que sa nouvelle ouverture.
6. Changer de catégorie recrée l'écran (`mouseClicked`, `:411`), ce qui rejoue l'animation et perd la
   position de défilement : garder un seul écran et fondre la liste serait plus propre et moins cher.
7. Les bordures d'un pixel de GUI disparaissent à l'échelle 3 : dessiner les traits en 1 pixel d'écran
   (ou les doubler au-delà de l'échelle 2) rendrait les cartes nettes sur les écrans HiDPI.
8. `ConfigScreen` (312 lignes) n'utilise pas `RenderUtil.card` et détonne à côté du click GUI ;
   l'aligner sur la même palette et les mêmes marges est un gain visible immédiat.
9. L'accent sert à tout (sélection, interrupteur, barres, ascenseur) : une échelle à trois teintes
   (accent discret / accent plein / texte sur accent) calmerait l'ensemble.

**Animations**

10. `onClose` (`:584`) rend l'écran précédent d'un coup : rejouer `entrance()` à l'envers pendant
    120 ms avant de rendre la main donnerait une sortie cohérente avec l'entrée.
11. L'ouverture des réglages d'un module ajoute 12 px fixes (`rows()`, `:168`) : interpoler la hauteur
    ferait glisser les lignes au lieu de les faire sauter.

**Jouabilité**

12. Deux modules peuvent porter la même touche : détecter le conflit et l'afficher en pastille rouge
    sur la ligne concernée évite des réglages « qui ne répondent pas ».
13. L'éditeur de HUD (`gui/HudEditorScreen.java`, 126 lignes) laisse glisser librement : un aimantage
    sur une grille de 4 px, avec repères d'alignement, rendrait les HUD propres.
14. Les profils existent ; les appliquer automatiquement à l'entrée dans un monde (par nom de serveur)
    relierait enfin profils et jouabilité.

**Textures**

15. Le client n'a aucune texture : tout est vectoriel. Un atlas de 16 px pour les icônes du HUD
    (armure, potion, ping) remplacerait des mots par des pictogrammes lisibles d'un coup d'œil.
16. Prévoir un dossier `assets/aether/textures/gui` surchargeable par un resource pack permettrait de
    re-thémer le client sans recompiler.

**Launcher**

17. Le terminal de session affiche les traces Java brutes (`app.js:1774`, `onGameOutput`) : replier les
    lignes `at …` derrière un « détails » donnerait une console lisible — on voit aujourd'hui une
    trace Realms de quinze lignes au premier lancement.
18. Aucun chemin de mise à jour : lire un manifeste de version au démarrage et afficher une bannière
    dans Settings serait la brique manquante pour livrer des correctifs comme ceux de cette session.
19. `styles.css` contient 16 transitions et 6 animations mais une seule règle
    `prefers-reduced-motion` : la respecter partout, et animer le changement de page, rendrait
    l'interface plus vive sans devenir fatigante.

**Priorités suggérées** : 1 et 2 (coût par frame, gain immédiat), 17 (lisibilité du launcher), 10 et 11
(cohérence des animations), puis 12 et 13 (jouabilité).
### `truncate` en dichotomie, avec la mesure (15/11)

Point 1 des propositions. L'ancienne version retirait un caractère à la fois et mesurait `prefix + "…"`
à chaque essai : un texte de 300 caractères coûtait près de 300 appels à `font.width`, à chaque frame
pour l'en-tête, le pied de page et les descriptions dépliées du click GUI.

- `util/TextFit.java` (nouveau, sans dépendance au jeu) : `truncate(value, maxWidth, measure)` cherche
  par dichotomie le plus long préfixe qui tient, avec la fonction de mesure injectée — donc testable et
  mesurable hors du client. Sémantique conservée : un texte qui tient est rendu tel quel, au moins un
  caractère est gardé, l'ellipse est `…`.
- `RenderUtil.truncate(Font, String, int)` délègue à `TextFit.truncate(value, maxWidth, font::width)` :
  aucun appelant ne change.
- `TextFitTest` (5 tests) : texte qui tient mesuré une seule fois ; plus long préfixe ; un caractère
  gardé quand rien ne tient ; **500 comparaisons aléatoires** contre l'ancienne version caractère par
  caractère (résultats identiques) ; et le compte d'appels de la dichotomie borné à 12 pour 300
  caractères.

Mesure réelle des appels à la fonction de largeur (programme jetable compilé avec `TextFit.java`,
largeur simulée à 6 px par caractère) :

| Longueur du texte | Largeur disponible | Ancien | Dichotomie | Gain |
| --- | --- | --- | --- | --- |
| 24 | 60 px | 17 | 5 | ×3,4 |
| 40 | 120 px | 23 | 7 | ×3,3 |
| 80 | 120 px | 63 | 8 | ×7,9 |
| 120 | 240 px | 83 | 8 | ×10,4 |
| 300 | 120 px | 283 | 9 | ×31,4 |
| 600 | 240 px | 563 | 10 | ×56,3 |
| 600 | 60 px | 593 | 10 | ×59,3 |

Le cas « le texte tient déjà » reste à **1 appel** dans les deux versions. Vérification :
`gradle test build --offline` → **BUILD SUCCESSFUL**, **30 tests** ; jar de 123 512 o copié dans
`tools/aether-home/.minecraft/mods/aether-client.jar`.

### Vérification visuelle du click GUI en jeu (15/11)

Fenêtre `Minecraft* 1.21.11` en 854×480 relancée par le launcher portable, Right Shift envoyé à la
fenêtre `java`, capture par `tools/gui.ps1` (premier plan pris le temps du clic et de la photo).
Captures : `tools/gui-open-0*.png` (avant corrections), `tools/gui-fixed-0*.png` (après).

**Ce qui marche.**

- Le panneau s'ouvre complet : sidebar (5 catégories avec leurs compteurs), en-tête « RENDER », champ de
  recherche, 5 modules, pied de page, barre de défilement.
- Clic gauche sur *Fullbright* (`575,138`) : l'interrupteur passe en accent, le compteur passe de
  « 1 of 5 enabled » à « 2 of 5 », et la description du module survolé devient *« Removes the darkness
  of the… »* — c'est le `truncate` en dichotomie qui s'affiche, ellipse comprise.
- Clic droit sur *FOV* (`575,194`) : la ligne se déplie, la description « Changes the field of view » et
  le réglage « Value 90 » avec sa barre apparaissent, et les lignes suivantes descendent correctement.
  Les clics suivants atteignent bien les lignes déplacées.
- L'animation d'ouverture n'a pas pu être filmée (170 ms pour un cycle de capture de l'ordre de 300 ms) :
  la preuve porte sur la position finale, identique à celle d'un panneau ouvert depuis longtemps, et sur
  la concordance dessin/clic ci-dessous.

**Deux défauts vus à l'écran, corrigés dans la foulée.**

1. *Le panneau ne tenait pas dans une petite fenêtre.* À 854×480 avec GUI scale 3, le panneau ne fait
   que 120 unités de haut : les 5 catégories (5 × 26 unités) débordaient sur l'aide « Right Shift /
   opens the menu », le compteur « 1 of 5 enabled » passait **sous** le champ de recherche, et le « 0% »
   du pied de page était coupé par le bord. Corrigé dans `gui/ClickGuiScreen.java` :
   `categorySpacing()` (26 unités, réduites jusqu'à 16 pour tenir), l'aide n'est dessinée que si les
   catégories laissent la place (`CATEGORY_HINT_HEIGHT`), `searchFieldWidth()` (150 unités, au plus la
   moitié de la colonne, jamais moins de 70) et le compteur n'est dessiné que s'il tient devant le champ,
   et le pied de page réserve 34 unités pour le pourcentage. Avant/après : `gui-open-01-s.png` /
   `gui-fixed-01-s.png`.
2. *Le clic ne tombait pas sur la bonne catégorie.* `mouseClicked` gardait `24` et `+= 26` en dur alors
   que le dessin utilisait l'espacement calculé : un clic sur la dernière ligne (`150,330`) ouvrait
   **Movement** au lieu de **Misc** (`gui-fixed-02-s.png`). Dessin et clics lisent maintenant la même
   fonction `categorySpacing()` ; le même clic ouvre **Misc** (`gui-fixed-04-s.png`).

### Correction : l'état enregistré faisait planter le jeu au démarrage (15/11)

Trouvé par la vérification ci-dessus, et c'est le défaut le plus grave de la session. Activer
*Fullbright* dans le click GUI écrit `"fullbright": true` dans `.minecraft/config/aether/config.json` ;
au lancement suivant, `AetherClient.onInitializeClient` appliquait la configuration pendant que le
client se construit, donc avant que `minecraft.options` existe :

```
java.lang.RuntimeException: Could not execute entrypoint stage 'client' due to errors, provided by 'aether'
Caused by: java.lang.NullPointerException: Cannot invoke "net.minecraft.class_315.method_42473()" because "minecraft.field_1690" is null
	at dev.aether.client.module.modules.RenderModules$Fullbright.onEnable(RenderModules.java:33)
	at dev.aether.client.config.AetherConfig.applyTo(AetherConfig.java:145)
	at dev.aether.client.AetherClient.onInitializeClient(AetherClient.java:63)
```

Le jeu ne redémarrait plus **tant que le module restait activé** — et le même piège attendait FOV,
No View Bobbing et Auto Jump, qui écrivent aussi dans `options`. `AetherClient` garde maintenant la
configuration et l'applique au **premier tick**, quand `minecraft.options` existe :

```java
if (!configApplied && minecraft.options != null) {
	configApplied = true;
	config.applyTo(moduleManager);
}
```

Preuve : la configuration contenait toujours `fullbright` activé ; avec le correctif, la session
`session-20261115-040135.log` démarre normalement (fenêtre `Minecraft* 1.21.11`, aucune trace du mod,
seules les erreurs Realms attendues pour un compte hors-ligne), et le click GUI s'ouvre avec
« 2 of 5 enabled » et l'interrupteur *Fullbright* allumé : l'état a bien été restauré, un tick plus tard.

### Rapport de plantage : code de sortie signé, lot `dist/` reconstruit (15/11)

Le rapport annonçait « exit code 4294967295 » là où le journal de session écrit « # exit code -1 » : la
coquille reçoit le code du jeu en `u32` et `u32::MAX` est la façon dont Windows rapporte un processus
terminé par `TerminateProcess`. Corrigé dans `launcher/gui/src/ui/app.js` en relisant le code en entier
signé 32 bits (`Number(exitCode) | 0`) aux deux endroits qui l'affichent : `showCrashReport` (ligne 1271,
champ `exit code` du rapport) et `launch` (ligne 1357, toast « The game stopped with the code … » et
champ `problem` du rapport, qui portaient la même valeur brute). Le journal, `launcher/src/logs.rs:107`,
était déjà juste.

Reconstruction du lot distribué, dans l'ordre :

| Étape | Commande | Résultat |
| --- | --- | --- |
| Cœur | `cargo build --release` (`launcher/`) | inchangé, `aether-launcher.exe` 2 211 328 o, SHA-256 `86537D92…B1A9591` |
| Coquille + bundle | `cargo tauri build` (`launcher/desktop/`) | 1 min 30 s, EXIT=0 (`tools/tauri-bundle12.log`) : `aether-desktop.exe` puis `Running makensis` → `Aether Launcher_1.0.0_x64-setup.exe` (4,26 Mo) |
| Livraison | copie dans `dist/` | portable 8 958 976 o, installateur 4 462 907 o |

| Fichier | Taille | SHA-256 |
| --- | --- | --- |
| `Aether-Launcher-Setup-1.0.0.exe` | 4 462 907 o (4,26 Mo) | `B40946122DED854A1C2642C07E0F08297562791339876E0B379E0354BD25CADE` |
| `Aether-Launcher-1.0.0-portable.exe` | 8 958 976 o (8,54 Mo) | `4F995224D1503ACE324D461E7CBE98B6DA7F49D150FAB37AEA06F8A02A5C3056` |

Empreintes recopiées dans `dist/LISEZ-MOI.txt`. Le cœur embarqué a été vérifié par inspection : la
séquence d'octets du `aether-launcher.exe` release se retrouve à l'offset **4 841 138** de l'exe
portable livré. L'installateur NSIS compresse sa charge, donc la même recherche n'y trouve rien — d'où
la vérification par exécution ci-dessous.

Preuve du correctif, sur le binaire livré et non sur les sources : `dist/Aether-Launcher-1.0.0-portable.exe`
lancé, « Launch Minecraft », puis le processus `java` terminé (`Stop-Process -Force`, exactement le cas
qui donnait 4294967295). Le journal dit `# exit code -1`
(`tools/aether-home/aether/logs/session-20261115-041651.log`) et le rapport affiche désormais
« problem the game stopped with the code -1 » et « exit code -1 »
(`tools/crash-report-code-s.png`).

La charge de l'installateur a été contrôlée par une installation réelle dans un dossier temporaire :
`dist/Aether-Launcher-Setup-1.0.0.exe` lancé avec `/S /D=…\tools\setup-unpack12` → **exit 0 en 1 558 ms**,
trois fichiers déposés (`aether-desktop.exe` 8 958 976 o, `AETHER-LICENSE-FR.txt`, `uninstall.exe`).
L'exe installé porte le cœur attendu à l'offset **4 841 138** : SHA-256 `86537D92…B1A9591`, identique au
`aether-launcher.exe` release. Il ne diffère du portable livré que par **3 octets** — NSIS réécrit son
marqueur au moment de l'installation — d'où une empreinte globale distincte
(`97BA142C…7F4EF7D3`) pour un contenu fonctionnellement identique.

Désinstallation silencieuse (`uninstall.exe /S`) : exit 0, dossier temporaire supprimé, aucun raccourci
`Aether` sur le Bureau ni dans le menu Démarrer, aucune clé dans `HKCU\…\Uninstall`. L'installation
`%LOCALAPPDATA%\Aether Launcher` déjà présente (restes d'un essai antérieur, sans clé de désinstallation
ni raccourci) n'a pas bougé : mêmes trois fichiers, même `aether-desktop.exe` de 8 946 688 o, SHA-256
`50B0B62B…5CEA22` avant et après.

---

## 8. État vérifié et lot appliqué (16/11)

Relecture complète des points ouverts du journal, confrontée au code **actuel** (le shell livré est
Tauri, `launcher/desktop/`, plus Electron) ; six améliorations appliquées, chacune avec sa preuve.

### 8.0 Ce que la vérification a trouvé déjà fait

- **Point 4 (B1, timeout du cœur)** : appliqué dans **les deux** coquilles. Electron
  (`gui/src/main.js`, `killProcessTree` + timeout) et Tauri (`desktop/src/core.rs`, `timeout_for` +
  `kill_tree`, test `the_timeouts_follow_the_network` : 45 s réseau, 900 s hosting/install, 20 s
  local). Rien à refaire.
- **Point 6 (C6, clippy)** : `cargo clippy --release --all-targets` → **0 avertissement** sur le
  cœur et sur le shell, vérifié avant et après le lot.
- **Points 1 à 3** : `withBusy` enveloppe bien `install`, `launch`, `stopGame`, `cancelInstall` ;
  modale et toasts accessibles (rôles, Escape, piège de focus) — l'E2E `a11y.mjs` les couvre déjà.

### 8.1 Trim : un contrôle de taille tous les 64 Kio écrits (point 11, C2)

`installer.rs` : `log_download` appelait `logs::trim` — donc `metadata()` — **à chaque ligne**, sous
les 8 workers d'assets. Remplacé par un compteur d'octets atomique :

- `LOG_BYTES_SINCE_CHECK: AtomicU64` + `TRIM_CHECK_BYTES = 64 Kio` ; la ligne est écrite, puis
  `fetch_add` compte ses octets ;
- le worker qui **gagne le `compare_exchange`** est le seul à tailler pour l'intervalle : la fenêtre
  de double trim constatée en C2 se réduit à un contrôle par 64 Kio au lieu d'un par ligne ;
- borne documentée et testée : `MAX_INSTALL_LOG_BYTES + TRIM_CHECK_BYTES` (576 Kio) au lieu de
  512 Kio stricts — c'est le prix du contrôle espacé, assumé dans le commentaire et l'assertion.

Preuves : nouveau test `the_size_of_the_log_is_not_asked_on_every_line` — un fichier **déjà** au-delà
de la limite n'est pas coupé à la ligne suivante (l'ancien écrivain l'aurait coupé), puis il est
coupé une fois l'intervalle écrit ; `the_log_of_the_installations_is_bounded_by_the_real_writer`
passe avec la nouvelle borne. Les tests du journal partagent un mutex et remettent le compteur à
zéro pour être déterministes.

### 8.2 Sessions bornées en octets (point 14, C4)

`logs.rs` : `prune` ne gardait que 20 fichiers, quel que soit leur poids. Ajout de
`MAX_SESSIONS_BYTES = 64 Mio` et `prune_with_budget(directory, keep, max_bytes)` (`prune` délègue) :

- une session saute si elle est au-delà du nombre **ou** si la garder dépasse le budget d'octets ;
- **la plus récente n'est jamais supprimée** (index 0), même seule au-delà du budget : c'est celle
  qui s'écrit et celle que lit un rapport de plantage ;
- seuls les octets des fichiers **gardés** sont comptés ; budget 0 = pas de limite d'octets.

Preuves : 3 tests — 4 sessions de 30 Mio → les 2 plus anciennes sautent ; une session plus grosse
que le budget survit avec la petite ancienne supprimée ; budget 0 garde tout sous le nombre.

### 8.3 `close_on_launch` retiré (point 7, C3) — décision motivée

Le champ n'existait que dans `settings.rs` (modèle + défaut) : lu par **aucune** coquille, aucune
case dans l'UI, aucune doc. Retiré du modèle plutôt qu'implémenté, et ce n'est pas un choix par
paresse : **fermer le launcher au lancement tuerait le jeu** dans l'état actuel du code — Tauri
appelle `state::kill_active` sur `RunEvent::Exit` (`desktop/src/main.rs:85`) et Electron tue l'arbre
de processus sur `window-all-closed`. Une vraie implémentation exige de détacher le jeu du cycle de
vie de la coquille **et** de détourner la stdout du cœur (un cœur qui continue de `println!` dans un
tube fermé panique ; un jeu dont le tube de stdout se remplit se bloque). C'est une fonctionnalité à
part entière, pas une ligne — elle reste au backlog avec cette note de conception.

Preuves de la retrait : `settings` ne sérialise plus `closeOnLaunch` ;
`settings --key closeOnLaunch --value true` → `unknown setting: closeOnLaunch, the settings are: …`
(la liste des clés valides est dérivée du modèle, elle s'est mise à jour seule) ; un réglage valide
(`showGameOutput`) passe toujours. Les anciens `settings.json` restent compatibles : pas de
`deny_unknown_fields`, la clé morte est ignorée au chargement.

### 8.4 Rapport d'installation échouée + lien « Open the logs » (pistes 2 et 3 du §7)

`gui/src/ui/app.js` (partagé par les deux coquilles) :

- **`showInstallReport(problem)`** : même forme que le rapport de plantage — problème, version,
  loader, chemin d'`install.log`, 40 dernières lignes, bouton « Copy report ». S'ouvre
  automatiquement quand `install()` reçoit `!result.ok` (l'annulation, elle, reste un simple toast).
- **`showPage(name)`** extrait du gestionnaire de navigation : les rapports l'utilisent pour amener
  le joueur aux logs. Les **deux** rapports ont un bouton « Open the logs » : celui de plantage
  ouvre la page Logs **et la session dont il est issu** (`openSession`), celui d'installation ouvre
  la page (le journal d'installation y est rendu).

Preuves, sur le **vrai Electron**, faux cœur, clics souris réels (CDP) :

| Script | Résultat | Nouveaux contrôles |
| --- | --- | --- |
| `failure.mjs 9222 documents` | **23/23** | le rapport s'ouvre seul ; problème/version/log ; « Copy report » ; lien présent ; seconde tentative après **Escape** |
| `crash-report.mjs 9223 game` | **12/12** | « the link opens the logs page » ; « the link opens the session the report was built from » (`session-20261114-153516.log`) |
| `screens.mjs 9224` | **21/21** | la refactorisation `showPage` ne casse aucune navigation, aucun débordement |

`failure.mjs` a dû être mis à jour : la modale interceptait le clic de la seconde tentative — il
ferme désormais le rapport par Escape (ce qui teste au passage la fermeture clavier) et vérifie le
rapport avant de retenter.

### 8.5 Outillage E2E sorti de `target/` (point 5, D1/D4)

`cargo clean` ne peut plus supprimer les checks :

- **`tools/e2e/`** : les 11 scripts `.mjs`, `fake_core.rs` et `hang.rs` **recompilés sur place**
  (`rustc -O`, aucun dépendance hors std), et les 12 fixtures `home-*` déplacées ;
- `launcher/package.json` : 10 scripts `npm run e2e:*` (drive, screens, failure, crash, cancel,
  stubborn, timeout, a11y, profiles, sliders) ;
- `docs/VERIFICATION.md` **§4.1** : procédure complète (construction des faux cœurs, variables
  d'environnement, port de debug) et tableau de ce que chaque script prouve.

Le fixture `home-screens` était incomplet (le script attend une ligne de compte et une session) :
compte hors-ligne `Notch` ajouté (`b50ad385-…`, l'UUID de référence du README) et session
`session-20261114-153516.log` copiée — `screens.mjs` 21/21 après ce complément.

### 8.6 Tests, clippy et lot distribué reconstruit

| Étape | Résultat |
| --- | --- |
| `cargo test --release` (cœur) | **151 passés, 0 échec** : 122 unitaires (+4 nouveaux) + 6 download + 20 integration + 3 sha1 |
| `cargo clippy --release --all-targets` | 0 avertissement, cœur **et** shell |
| `cargo test --release` (shell) | 6 passés, 0 échec |
| `cargo build --release` + `cargo tauri build` | EXIT=0, bundle NSIS 4,26 Mio |

Cœur embarqué vérifié par inspection : les 2 211 328 octets du `aether-launcher.exe` release se
retrouvent **identiques octet par octet** à l'offset 5 841 562 du portable livré (comparaison
complète, pas seulement une sonde). La coquille Tauri n'a pas changé dans ce lot ; sa fenêtre n'a
pas été rejouée, l'interface partagée est prouvée par l'E2E Electron ci-dessus.

| Fichier | Taille | SHA-256 |
| --- | --- | --- |
| `Aether-Launcher-Setup-1.0.0.exe` | 4 463 927 o (4,26 Mo) | `B59A035C80432501B3412469FBE662C9DF9692EC9D65FB1A05F56A0BCBC9DBA4` |
| `Aether-Launcher-1.0.0-portable.exe` | 8 959 488 o (8,54 Mo) | `958267FC92E5C9D580897B46B99A7C7CFAE167DAC8F95431A434785D398DA537` |

Empreintes recopiées dans `dist/LISEZ-MOI.txt`. Le portable ré-extrait le cœur au premier lancement
(le mécanisme compare les octets et réécrit si différent) : les installations existantes prennent le
nouveau cœur sans réinstallation.

### 8.7 Ce qui reste ouvert, par ordre de rentabilité

1. **Vérifier les fichiers, pas seulement les tailles** (§7 piste 4) — un « Verify files » qui
   revalide les SHA1 des assets sans retélécharger.
2. **Annulation au milieu d'un fichier** (§7 piste 5) — le jeton n'arrête qu'entre deux fichiers.
3. **Harnais E2E partagé** (§7 piste 6) — les ~60 lignes de CDP sont toujours copiées dans chaque
   script ; le déplacement est fait, la factorisation reste.
4. **Tests JS unitaires** (§7 piste 10, point 13) — `paintRange`, `formatBytes`, `showInstallReport`
   sont pures mais `app.js` est un script classique non modulaire ; il faudrait l'extraire en
   modules importables par `node --test` sans casser le chargement Tauri/Electron.
5. **Reprise après installation interrompue** (§7 piste 7), **processus orphelin** (point 15),
   **mise à jour du launcher** (point 12), **langue de l'interface** (tout en anglais en dur).
6. **`close_on_launch` en vraie fonctionnalité** — avec la note de conception du §8.3 (détacher le
   jeu et la stdout du cœur avant de fermer la fenêtre).

---

## Coût par frame du HUD et du click GUI : mesure, gains, reste à faire (15/11)

Point 1 des propositions de performance, traité jusqu'au bout : d'abord mesurer, puis réduire ce qui
pèse, puis re-mesurer. Les valeurs ci-dessous sont des **temps CPU relevés en jeu**, pas des
estimations.

### Comment c'est mesuré

Une classe temporaire `util/Profile.java` (aujourd'hui supprimée, elle ne fait pas partie du client
livré) enveloppait le rendu du HUD et celui du click GUI : chaque section était bornée par
`System.nanoTime()`, accumulée, et une ligne de synthèse était écrite dans le journal de session tous
les **240 frames**, avec la moyenne par appel de section. `System.nanoTime()` coûte 30 ns sur cette
machine (mesuré : 200 000 appels, `tools/NanoBench.java`), soit 0,02 % d'une frame à 60 fps, donc
l'instrumentation ne déforme pas les chiffres du tableau principal ; elle explique en revanche le
bruit des sections les plus petites (< 5 µs).

- Fenêtre `Minecraft* 1.21.11` en **854×480**, GUI scale 3, Fabric 1.21.11, compte hors-ligne
  `Notch`, launcher portable lancé par `dist/Aether-Launcher-1.0.0-portable.exe`.
- Click GUI mesuré à l'écran titre, panneau ouvert, après l'animation d'ouverture (240 frames = ~4 s).
- HUD mesuré dans un monde solo (« New World »), **4 éléments activés** : FPS, FPS Graph, Ping, CPS.
  `AetherClient.onHudRender` (`AetherClient.java:194-201`) ne dessine rien tant qu'un écran est
  ouvert, et les éléments n'ont de sens qu'en jeu : le HUD a donc été mesuré dans ce monde, pas à
  l'écran titre. Le click GUI, lui, se mesure partout.
- Ce qui est mesuré : le temps CPU passé à préparer et soumettre le dessin sur le fil de rendu. Le
  temps GPU n'est pas mesuré.

### Coûts unitaires de référence

Les mêmes sections donnent, pour un même appel, un coût unitaire comparable d'une section à l'autre :

| Primitive | Coût mesuré | Contexte du relevé |
| --- | --- | --- |
| `graphics.fill` (un rectangle, mêmes coordonnées) | **0,08 µs** | micro-benchmark, 2 000 appels |
| `RenderUtil.roundedRect` rayon 0 (rectangle droit) | **0,27 µs** | micro-benchmark, chemin court à un seul `fill` |
| `RenderUtil.text` (un `drawString` avec ombre) | **3,0 µs** | section « nom de ligne » du click GUI, 5 lignes |
| `RenderUtil.roundedRect` rayon 6, 24 px de haut | **12 à 17 µs** | section « pastille » (switch) et « fond de ligne » du click GUI |
| `RenderUtil.roundedRect` d'une carte de HUD (rayon 5) | **3 à 5 µs** | éléments FPS et Ping, hors scissor |

Un `roundedRect` soumet **2 + 2r rectangles** (`util/RenderUtil.java:75-102`, boucle de l'arc
`:94-101`) : 2 pour les parties droites, puis un `fill` par colonne de chaque coin. Pour un rayon de 6,
cela fait 14 rectangles ; pour la pastille d'un interrupteur (rayon 7), 16. Les cordes de l'arc sont
déjà en cache (`chords(int)` en `:43`, table `CHORD_CACHE`), donc le chemin chaud ne fait plus que
des `fill` : les 12 à 17 µs mesurés sont du coût de soumission, pas du calcul.

Deux chiffres à ne pas réutiliser tels quels : en rafale de 2 000 appels consécutifs, un
`roundedRect` monte à 263–381 µs, parce que le tampon de sommets grandit puis se vide en cours de
boucle. Le coût représentatif est celui **dans une frame** (tableau ci-dessus), pas celui d'une rafale.

### Click GUI au repos

| Section | Temps par frame | Part |
| --- | --- | --- |
| **Frame complète** | **437 à 468 µs** | 100 % |
| `list` (les 5 lignes visibles, la découpe et l'ascenseur) | 304 à 306 µs | ~70 % |
| `sidebar` (fond, 5 catégories, aide) | 50 µs | ~11 % |
| `header` (titre, compteur, champ de recherche) | 42 µs | ~10 % |
| `footer` (description survolée, pourcentage) | 21 µs | ~5 % |
| reste (voile, ombre et bord du panneau, ligne d'accent) | ~16 à 25 µs | ~4 % |

Détail d'une frame de `list` : fond de ligne 20,5 µs, nom 3,2 µs, pastille d'interrupteur 33 µs,
pastille de touche 0,1 µs, découpe (`enableScissor`/`disableScissor`) 2 à 10 µs, `rows()` mise en
cache 0,2 à 0,8 µs. Les **rectangles arrondis représentent environ les trois quarts de la frame** :
une vingtaine d'appels (2 par ligne de catégorie active, 1 fond + 1 pastille par ligne de module,
2 pour le champ de recherche, 2 pour l'ascenseur, 3 pour le panneau et son ombre).

### HUD à quatre éléments

| Élément | Temps par frame |
| --- | --- |
| **Total** | **123 à 139 µs** |
| FPS Graph | 35 à 40 µs |
| Ping | 40 à 44 µs |
| CPS | 27 à 30 µs |
| FPS | 10 à 13 µs |

La somme des quatre éléments tombe 10 à 15 µs sous le total : c'est la boucle de `HudManager` sur
les éléments désactivés et la mesure de taille, qui n'apparaissent dans aucune section d'élément.

Le graphe est le seul élément dont le coût vient d'une boucle : **un rectangle de 1 px par
échantillon**, 54 échantillons (`module/modules/HudModules.java:193-201`, `SAMPLES = 54` en `:150`).
Il mesurait 67 à 72 µs avant la réduction de l'ombre, 35 à 40 µs après ; son rendu a été refait
depuis (voir « Graphe FPS : colonnes fusionnées, barres de 2 px »). Ping mesure 40 à 44 µs dans
la même session alors qu'il ne dessine qu'une carte et deux textes : l'écart entre les deux éléments
situe l'amplitude de la dispersion (±20 %), et le tableau ci-dessus se lit comme un classement
grossier, pas à la microseconde près. Chaque élément paie en plus sa propre carte :
`HudModules.drawPanel` (`:41`) appelle `RenderUtil.card` (`util/RenderUtil.java:129`), soit une ombre
et une bordure arrondie par élément. La mesure de taille n'a lieu qu'une frame sur cinq
(`hud/HudManager.java:39-42`), ce qui n'apparaît pas dans les chiffres ci-dessus mais reste le bon
compromis.

### Ce qui a déjà été appliqué, et ce que cela a rapporté

| Changement | Code | Avant | Après | Gain |
| --- | --- | --- | --- | --- |
| Ombre en une passe au lieu de trois couches | `util/RenderUtil.java:117-124` | 3 `roundedRect` par carte | 1 | −2 rectangles par carte |
| Interrupteur en pilule pleine au lieu d'un contour | `gui/ClickGuiScreen.java:427-439` | 2 `roundedRect` + bouton | 1 + bouton | −16 rectangles par interrupteur |
| Frame du click GUI | — | 539–566 µs | **437–468 µs** | −13 à −23 % |
| `list` du click GUI | `gui/ClickGuiScreen.java:336-378` | 403–431 µs | **304–306 µs** | **−26 %** |
| Pastille d'interrupteur | `gui/ClickGuiScreen.java:427-439` | 51–57 µs | **32–33 µs** | **−36 %** |
| Élément FPS Graph du HUD | `module/modules/HudModules.java:193-201` | 67–72 µs | 35–40 µs | −45 % |
| Graphe FPS : colonnes voisines fusionnées, barres de 2 px | `util/BarChart.java:26`, `module/modules/HudModules.java:207` | 54 rectangles de 1 px | 1 à 54 rectangles de 2 px | −53 à 0 `fill` par frame |
| Total du HUD | — | 123–140 µs | 123–139 µs | **inchangé** |

Le total du HUD n'a pas bougé alors que le graphe a été divisé par deux : sur la même mesure, Ping est
passé de 27 à 40 µs et CPS de 13 à 27 µs, sans changement de code les concernant. C'est de la
dispersion entre deux sessions (même résolution, même monde, mais deux lancements et deux états de
JIT) : les chiffres du HUD se lisent à ±20 % près, ceux du click GUI, relevés dans la même session
avant/après sur la même machine, sont fiables à quelques pour cent.

Vérification visuelle des deux changements : captures `tools/after-gui-s.png` (après la réduction) et
`tools/final-gui-s.png` (build livré, instrumentation retirée). L'interrupteur éteint reste lisible
(pilule grise, bouton blanc) et l'ombre du panneau ne bave pas.

### Graphe FPS : colonnes fusionnées, barres de 2 px (17/11)

Premier point de la liste précédente, appliqué. Le graphe dessinait **54 rectangles de 1 px**, un par
échantillon, avec un pixel vide entre deux : la moitié des 108 px du graphe était perdue, et chaque
frame payait 54 `fill`.

- `util/BarChart.java` (nouveau, sans dépendance au jeu) : `runs(values, peak, height, colour, out)`
  (`:26`) regroupe les échantillons **voisins de même hauteur et de même couleur** et écrit les
  paliers dans un tableau fourni par l'appelant — aucun objet alloué par frame. La fonction est pure,
  la couleur arrivant par un `IntUnaryOperator`, donc testable et mesurable hors du client.
- `module/modules/HudModules.java` : `BAR_WIDTH = 2` (`:154`), deux tableaux réutilisés (`:158`,
  `:160`), et `renderBars` (`:207`) qui appelle `BarChart.runs` (`:212`). La largeur du graphe devient
  `SAMPLES * BAR_WIDTH`, soit les 108 px d'avant, désormais pleins.
- `BarChartTest` (8 tests) : un taux stable donne **un** rectangle ; une valeur qui change à chaque
  échantillon en donne 54 ; les paliers sont découpés au bon endroit ; une valeur nulle ne dessine
  rien ; 200 historiques aléatoires sont couverts exactement une fois ; chaque palier a la hauteur de
  tous ses échantillons ; et le nombre de rectangles ne dépasse jamais 54.

**Rectangles par frame**, comptés par `tools/BarChartBench.java` exécuté sur `BarChart` seul :

| Historique | Avant | Après |
| --- | --- | --- |
| 60 fps stable | 54 | **1** |
| gigue 58–62 fps | 54 | **25** |
| dents de scie 30–105 fps | 54 | 54 |

Le regroupement coûte **132 ns par graphe** (2 000 000 d'appels mesurés), soit le quart du prix d'un
seul rectangle : il ne peut pas annuler le gain qu'il apporte.

**Gain attendu en jeu** : les 54 barres pesaient 25 à 30 µs de la frame du HUD (54 rectangles à
~0,5 µs, voir la section précédente). Un taux stable ne coûte plus qu'un rectangle, une gigue une
vingtaine : **−25 µs sur un taux stable, −14 µs sur un historique qui bouge**, soit −20 % du HUD. Ce
chiffre est une déduction à partir du coût mesuré d'un rectangle, pas une mesure en jeu : le 17/11,
un jeu (Fortnite) occupait le premier plan pendant toute la vérification, et lancer Minecraft l'aurait
interrompu. La procédure de la mesure en jeu est décrite dans « Vérifier et re-mesurer ».

Vérification : `gradle test build --offline` → **BUILD SUCCESSFUL**, **42 tests, 0 échec**
(`tools/gradle-graph.log`) ; jar de 130 843 o copié dans `tools/aether-home/.minecraft/mods/`.

Le rendu lui-même n'a pas encore été vu en jeu, pour la même raison : il faut contrôler aux GUI
scale 2 et 3 que les barres jointives de 2 px ne bavent pas sur les bords du graphe et que les
paliers tombent au pixel près.

### Reste à faire, par gain attendu

1. **Ombre propre à chaque élément du HUD** — `module/modules/HudModules.java:41` →
   `util/RenderUtil.java:129` puis `util/RenderUtil.java:117`. Supprimer l'ombre des cartes du HUD
   (ou la partager entre les éléments empilés). Gain estimé : **3 à 5 µs par élément activé**, donc
   12 à 20 µs avec quatre éléments (~10 à 15 % du HUD). Risque : les cartes paraissent plus plates
   sur un ciel clair ; à vérifier par capture en monde.
2. **Arcs de `roundedRect`** — `util/RenderUtil.java:75-102`, boucle `:94-101`. Les cordes sont déjà
   précalculées (`chords(int)` en `:43`) ; ce qui reste, c'est le **nombre** de rectangles : 2r par
   forme, un par colonne de chaque coin. Des bandes de 2 px (r/2 pas de deux rectangles) divisent ce
   nombre par deux. Gain estimé : **10 à 15 % de la frame du click GUI** (45 à 70 µs), puisque les
   rectangles arrondis en font les trois quarts. Risque : marche d'escalier visible aux GUI scale 1
   et 2 ; à contrôler sur la pastille des interrupteurs, la forme la plus exposée.
3. **Formatages par frame restants** — chronomètre de session `module/modules/HudModules.java:821`
   et barre de vie du target HUD `:893` : `String.format` à chaque frame. Mémoriser la chaîne par
   seconde / par dixième de cœur, comme le fait déjà `Coordinates`
   (`module/modules/HudModules.java:621-654`). Gain : quelques microsecondes et surtout des
   allocations en moins ; risque nul, à condition que la clé de cache inclue les réglages
   d'affichage.
4. **Lignes « at … » du launcher** — `launcher/gui/src/ui/app.js:1844` (`api.onGameOutput`) affiche
   les traces Java brutes : au premier lancement, une erreur Realms occupe quinze lignes. Replier les
   lignes d'une pile derrière un « détails » rend la console lisible. Gain : lisibilité, aucun effet
   sur les performances ; risque : masquer une information utile si le repli est trop agressif.
5. **`ConfigScreen` détonne** — `gui/ConfigScreen.java:58-120` dessine son panneau à la main
   (`RenderUtil.roundedRect` en `:63`, pastilles en `:95` et `:114`) au lieu d'utiliser
   `RenderUtil.card`/`bordered` comme le click GUI : pas d'ombre, rayons et marges différents. Gain :
   cohérence visuelle entre les deux écrans ; risque nul sur le comportement.

### Vérifier et re-mesurer

Commande de vérification du client (celle qui a servi pour ce document) :

```bash
cd aether-client
export JAVA_HOME="$(cat ../tools/jdk25-path.txt | tr -d '\r\n')"
../tools/gradle-9.8.2/bin/gradle.bat test build --offline
```

Résultat au 17/11 : **BUILD SUCCESSFUL in 13s**, **42 tests, 0 échec, 0 ignoré**
(`tools/gradle-graph.log`), jar `aether-client-1.0.0.jar` de 130 843 o (le 16/11 : 34 tests, 129 422 o
et un build de 39 s, `tools/gradle-doc.log`). Un `runClient` concurrent bloque le verrou du projet : ne
pas le lancer en parallèle de la mesure.

Le regroupement des barres se mesure sans le jeu, puisqu'il ne dépend d'aucune classe de Minecraft :

```bash
cd aether-client
JAVA_HOME_DIR="$(cat ../tools/jdk25-path.txt | tr -d '\r\n')"
"$JAVA_HOME_DIR/bin/javac" -d ../tools/bench-classes \
  src/main/java/dev/aether/client/util/BarChart.java ../tools/BarChartBench.java
"$JAVA_HOME_DIR/bin/java" -cp ../tools/bench-classes BarChartBench
```

État à rétablir pour refaire la mesure du HUD sur cette machine :

- le jar de test vient d'être recopié : `tools/aether-home/.minecraft/mods/aether-client.jar`
  (130 843 o, 17/11 19:10) correspond au build de la révision courante ; le recopier après chaque
  reconstruction ;
- `dist/` date du **16/11 23:22** (portable 8 959 488 o, installateur 4 463 927 o) et ne contient pas
  la révision du client mesurée le 15/11 : pour re-mesurer, reconstruire le mod puis relancer le
  launcher, ou repartir du jar courant ;
- l'instrumentation n'est plus dans le code : il faut la réintroduire temporairement (même forme que
  décrite en tête de section) et la retirer avant de livrer — elle n'a pas sa place dans le client ;
- un monde solo est nécessaire pour le HUD : `AetherClient.onHudRender` (`AetherClient.java:194-201`)
  sort immédiatement tant qu'un écran est ouvert, et les éléments n'ont de valeurs qu'en jeu.

---

## Coquille Electron retirée, jeu orphelin réparé (18/11)

### Trois défauts constatés

| # | Défaut | Preuve |
| --- | --- | --- |
| 1 | Le lanceur de la racine ouvrait **Electron**, alors que le produit livré est la coquille Tauri | Avant : `tools/launcher-runner.rs:50` cherchait `launcher/node_modules/electron/dist/electron.exe`. Or `dist/Aether-Launcher-1.0.0-portable.exe` et `launcher/desktop/target/release/aether-desktop.exe` avaient **la même taille, 8 959 488 octets** : c'est la coquille native qui est publiée. Sur une machine sans `npm install`, `MimiMax.exe` n'ouvrait donc rien du tout. |
| 2 | La doc présentait Electron comme l'interface active | `README.md:11,121,134,158-165,231-243`, `docs/ARCHITECTURE.md:9-21,55-61,142`, `launcher/README.md:9,100-112` |
| 3 | Le pid du jeu n'était écrit nulle part | `launcher/src/lib.rs` gardait l'enfant en mémoire et `launcher/desktop/src/state.rs:31-45` tuait l'arbre à la fermeture. Une coquille tuée au gestionnaire de tâches laissait Java vivant, et le bouton Stop de la session suivante répondait « No game is running. » |

### Ce qui a été fait

1. **Une seule coquille : Tauri.** Supprimés : `launcher/gui/src/main.cjs`, `preload.js`, `security.cjs`,
   `launcher/tests/security.test.cjs`, la dépendance `electron`, les scripts `test:security` / `pack` /
   `dist` et le bloc de packaging `electron-builder` (`launcher/package.json`, lockfile réduit à un
   paquet sans dépendance), les deux jobs GitHub Actions qui exigeaient Electron. L'interface
   n'a pas bougé : `gui/src/ui/*` est servi par WebView2 et `bridge.js` expose toujours `window.aether`.
2. **Le lanceur racine ouvre la coquille native** : `tools/launcher-runner.rs:44-58` la cherche à trois
   endroits (à côté de l'exécutable, dans l'arbre de sources, dans `dist/`), et le message d'erreur
   donne la commande de compilation. `build.bat` la compile en étape [2/4].
3. **Le jeu est enregistré** : `launcher/src/game.rs:402-462` (`GAME_PID_FILE`, `pid_file`,
   `record_pid`, `running`, `stop`), appelé dès l'existence du processus Java (`game.rs:369`,
   `on_start(child.id())`), écrit depuis `lib.rs:467`, effacé à la sortie (`lib.rs:480,484`), exposé
   par `Launcher::running_game` / `stop_game` (`lib.rs:495-505`).
4. **Arrêt possible même orphelin** : commande `aether-launcher stop` (`main.rs:38,468-482`) et repli
   de `game_stop` (`desktop/src/commands.rs:677-702`) quand la coquille n'a plus d'enfant en mémoire :
   elle demande alors au cœur de tuer l'arbre enregistré au lieu de répondre qu'aucun jeu ne tourne.
5. **Doc remise d'aplomb** : `README.md` (interface native, et une section « What this client is, and
   what it is not » qui dit noir sur blanc que les modules de combat ne sont pas présentés comme
   indétectables), `docs/ARCHITECTURE.md` (schéma, tableau de la coquille, section « The record of the
   game »), `launcher/README.md`, `docs/VERIFICATION.md` §4, plus une ligne dans `NETTOYAGE.md` et
   `RAPPORT-FINAL.md`. Les commentaires des scripts `tools/e2e/*.mjs` ne parlent plus d'Electron.

### Preuves

- `cargo test` (cœur) : **155 tests** (126 unitaires + 6 download + 20 intégration + 3 sha1), **0 échec**,
  dont 4 nouveaux : `the_record_of_the_game_is_written_and_read_back`,
  `a_record_that_names_a_dead_process_is_cleared`, `stopping_without_a_record_says_so`, et
  `stop_kills_the_process_named_by_the_record` (lance un vrai processus, vérifie que `stop` le tue puis
  que le fichier disparaît).
- `cargo build --release` (cœur) puis `cargo build --release --manifest-path desktop/Cargo.toml` :
  2 min 07 s, 0 erreur, `aether-desktop.exe` = 8 986 624 octets ; `cargo test --release --manifest-path
  desktop/Cargo.toml` : 6 tests, 0 échec.
- `aether-launcher stop` sans jeu : `{"error":"no game is running","ok":false}`, code de sortie 0.
- `MimiMax.exe` recompilé (205 312 octets) et lancé pour de vrai : une seule fenêtre « Aether Launcher »
  apparaît (pid 15444). L'ancien runner, lui, exigeait `launcher/node_modules/electron`.

### Ce qui n'a pas été vérifié

- Les scripts `tools/e2e/*.mjs` n'ont pas été rejoués contre WebView2. Ils sont maintenant lancés avec
  `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=…"` (`tools/run-ui-tests.ps1`) ; le
  job CI a été remplacé par un job qui compile et teste la coquille.
- La fenêtre n'a pas été inspectée visuellement : seul son titre a été relevé.
- `dist/Aether-Launcher-1.0.0-portable.exe` reste l'ancien binaire ; `cargo tauri build` le régénère.
- `launcher/node_modules` (Electron installé, ~150 Mo) n'a pas été supprimé : plus référencé par
  `package.json`, il est supprimable sans risque.

---

## Direction artistique du launcher, et preuve qu'elle s'affiche (18/11)

### Le diagnostic : `tokens.css` ne pilotait rien

`launcher/gui/src/ui/styles.css` redéclarait sa propre palette dans son `:root` (lignes 8-55) :
`--bg`, `--accent`, `--surface`, `--radius`, `--shadow*`, `--ring`, `--ease`… en valeurs **codées en
dur**. Le fichier de tokens n'était donc lu qu'à travers les quelques alias qui pointaient encore
dessus (`--bg`, `--surface`, `--border`), et changer un token ne changeait presque rien. Le script
d'audit ajouté pour ce pass le mesure : **41/44 tokens résolus (93,2 %), 3 manquants, 1 doublon**
(`--radius-sm` déclaré dans les deux fichiers). Les trois manquants étaient de vrais trous :
`--fill` (la position du curseur d'un slider, écrite par `app.js`), `--success` et `--text-muted`
(utilisés par `app.js:1510` dans un style en ligne) — ces deux derniers ne résolvaient rien du tout.

### L'identité livrée — « ciel d'hiver »

`tokens.css` devient la seule source : palette, échelles d'espacement, rayons, ombres, typographie,
états et durées.

| Rôle | Avant | Après |
| --- | --- | --- |
| Fond / surface / surface haute | `#0B0E14` / `#14181F` / `#1C222B` | `#070B12` / `#101826` / `#17212F` |
| Bordure / bordure au survol | `#232B38` / `rgba(124,92,252,.35)` | `#223040` / dérivée de l'accent |
| Texte principal / secondaire / discret | `#F1F5F9` / `#94A3B8` / `#64748B` | `#EEF4FA` / `#A3B5C6` / `#7C8FA1` |
| Accent par défaut | `#6C5CE7` (violet) | `#3B82F6` (azur), secondaire `#8B5CF6` |
| Rayons | 8 / 12 / 14 px | 6 / 10 / 14 px (échelle `xs→full`) |
| Anneau de focus | `0 0 0 2px rgba(124,92,252,.5)` | double anneau fond + accent, lisible sur toute surface |
| Statuts | `#10B981` / `#F59E0B` / `#EF4444` | `#34D399` / `#FBBF24` / `#F87171` (+ variantes texte) |

Trois décisions qui font la cohérence plutôt que le coup de peinture :

1. **Les teintes de l'accent sont dérivées, pas figées.** `--accent-dim/soft/mid/text`, `--accent-hover`
   et `--border-hover` sont calculés avec `color-mix()` à partir de `--accent`, donc de la couleur que
   le joueur a choisie. Avant ce pass, un accent personnalisé laissait des bordures bleues et une
   pastille violette autour de lui.
2. **L'accent du joueur ne gèle plus le thème.** `app.js` écrit `--accent-user` et non `--accent` ;
   l'alias `--accent: var(--accent-user, var(--color-accent))` garde l'identité quand aucune couleur
   n'a été choisie. Les deux valeurs qui ont été livrées comme défauts (`#6C5CE7`, `#7C5CFC`) sont
   traitées comme « pas un choix » : un fichier de réglages qui les porte encore bascule sur le
   nouveau défaut. `DEFAULT_ACCENT` (`launcher/src/settings.rs:10`) et le sélecteur
   (`index.html:476-477`) passent à `#3B82F6`.
3. **Les états sont visibles, pas supposés.** Un `:focus-visible` global (`outline` 2 px + décalage,
   spécificité nulle pour ne rien écraser) et un état désactivé (`opacity: .45`, curseur interdit)
   couvrent les contrôles qui n'en avaient pas. Le canevas du graphe FPS lit `--accent` au lieu d'un
   `#38BDF8` écrit dans `app.js`.

Aucune structure d'écran, aucun composant, aucune balise n'a été ajouté ou déplacé : la capture le
montre, la mise en page est identique au pixel près.

### Preuve 1 — zéro token fantôme

```powershell
node tools/check-theme-tokens.mjs
# tokens: 104 defined, 60 referenced, 60/60 resolved (100.0%), 0 missing, 0 duplicated, 44 unused
```

Le script (`tools/check-theme-tokens.mjs`) lit `tokens.css`, `styles.css`, `index.html`, `app.js` et
`bridge.js`, ignore les déclarations en commentaire, et sort en **code 1** s'il reste un `var(--…)`
sans définition ou un token déclaré dans deux fichiers. Les 44 « unused » sont l'échelle du système
(espacements, tailles de texte, poids) : les règles actuelles n'en consomment qu'une partie.

### Preuve 2 — la fenêtre réelle, avant / après

`tools/capture-window.ps1` lance la coquille, met la fenêtre au premier plan et capture son rectangle.
`tools/theme-probe.mjs` interroge le moteur par le protocole DevTools de WebView2 et rend les valeurs
**calculées**.

| Point de la capture | Avant | Après |
| --- | --- | --- |
| Barre de titre | `#0B0E14` | `#070B12` |
| Carte | `#14181F` | `#101826` |
| Pastille « 1.21.11 FABRIC » | `#23223F` (violet) | `#191521` (teinté par l'accent du joueur) |
| Valeur « 4096 MB » | `#BAA0D7` (violet) | `#B38E80` |
| Piste des sliders | `#580B0A` | `#590B0A` (inchangée : c'est l'accent choisi) |
| Pixels modifiés (> 24) | — | **8,5 %** de l'image |

Sonde sur la même fenêtre : `background: rgb(7, 11, 18)`, `cardRadius: 10px`,
`cardBackground: rgb(16, 24, 38)`, `primaryButton` = accent, `missing: []`, 2 feuilles chargées.

Le joueur a enregistré un accent `#560b0b` (rouge presque noir) : l'interface réelle est donc déclinée
autour de ce rouge, et c'est voulu — c'est sa couleur. Pour prouver l'identité **par défaut**, une
seconde capture a été prise avec un `AETHER_HOME` neuf (`tools/theme-apres-defaut.png`) : accent
`#3B82F6`, pastilles et sliders azur, crête du logo azur. Les deux écrans restent lisibles, la
hiérarchie est intacte et aucun élément n'apparaît dégradé : l'état vide « No account yet » (bordure
pointillée, icône, texte d'aide) se lit correctement sur la nouvelle surface.

Captures : `tools/theme-avant.png`, `tools/theme-apres.png`, `tools/theme-apres-defaut.png`.

### Artefact publié

`cargo tauri build` a été exécuté (`launcher/desktop`, tauri-cli 2.12.1) : build 2 min 08 s, bundle NSIS
`Aether Launcher_1.0.0_x64-setup.exe` de 4,27 MiB. `dist/Aether-Launcher-1.0.0-portable.exe` et
`dist/Aether-Launcher-Setup-1.0.0.exe` ont été **régénérés** à partir de cette build.

### Garde-fous

- `cargo test` : **155 tests**, 0 échec (126 + 6 + 20 + 3).
- `cargo clippy --all-targets -- -D warnings` : 0 avertissement.
- `node tools/check-theme-tokens.mjs` : 100 % résolu, 0 doublon, code de sortie 0 (ajouté à la CI,
  job `theme tokens`).

### Ce qui n'est pas vérifié

- Les scripts `tools/e2e/*.mjs` n'ont pas été rejoués (même limite que le pass précédent).
- Le sélecteur de couleur des réglages n'a pas été manipulé à la souris : `applyAccent` est prouvé par
  les deux états au démarrage (accent par défaut et accent enregistré), pas par un clic.
- Le bas de la page Play dépasse la hauteur de l'écran dans la capture (fenêtre 1180×760 sur un écran
  plus court) : c'est antérieur à ce pass, la carte « GAME SESSION » est simplement coupée par le bord.

---

## Sélecteur d'accent et huit pages, prouvés dans le moteur (18/11)

### Les deux angles morts du pass précédent

`applyAccent` n'était prouvé que par les deux états au démarrage, et seule la page Play avait été
regardée. Ce pass les ferme avec un pilote qui parle au vrai moteur (`tools/theme-sweep.mjs`) : il
clique les entrées de la barre latérale et le sélecteur d'accent avec de vrais événements souris
(`Input.dispatchMouseEvent`), lit les valeurs **calculées** par WebView2 et capture chaque page.

### 1. Le sélecteur d'accent, exercé par le chemin utilisateur

Clic réel sur `.nav-item[data-page="settings"]`, puis sur `#input-accent` — le dialogue natif de Windows s'ouvre côté système, le script envoie
`Échap` mais ne peut pas vérifier cette fenêtre depuis le moteur — puis la paire `input`/`change` que
ce dialogue émet sur
`#input-accent`. Le script n'écrit jamais `--accent` ni `--accent-user` : c'est `applyAccent` qui les
pose, via les écouteurs de `app.js`.

| Lecture | Avant le geste | Après le geste |
| --- | --- | --- |
| `#output-accent` | `#3B82F6` | `#560B0B` |
| `--accent` | `#3B82F6` | `#560b0b` |
| `--accent-dim` | `color-mix(in srgb, #3B82F6 14%, transparent)` | `… #560b0b 14% …` |
| `--accent-text` | `… #3B82F6 38%, #ffffff …` | `… #560b0b 38%, #ffffff …` |
| `--accent-hover` | `… #3B82F6 68%, #000 …` | `… #560b0b 68%, #000 …` |
| `--border-hover` | `… #3B82F6 45%, transparent …` | `… #560b0b 45%, transparent …` |
| **Bouton primaire (fond)** | `color(srgb 0.1897 0.4180 0.7911)` | `color(srgb 0.2765 0.0354 0.0354)` |
| **Pastille de valeur (texte)** | `color(srgb 0.7079 0.8137 0.9866)` | `color(srgb 0.7482 0.6364 0.6364)` |
| **Pastille de valeur (fond)** | `… / 0.14` du bleu | `… / 0.14` du rouge |
| **Bordure d'un champ survolé** | `… / 0.45` du bleu | `… / 0.45` du rouge |

La persistance est réelle : `tools/theme-sweep-home/aether/settings.json` contenait (home de test supprimé depuis)
`"accentColor": "#560b0b"` — l'événement `change` a traversé le pont, le cœur Rust et le disque.

### 2. Le défaut trouvé, et corrigé à la source

L'audit de contraste (WCAG, en composant les fonds semi-transparents) a mesuré **3,68:1** sur
`.btn--primary` : blanc sur `#3B82F6`, pour un plancher de 4,5:1 à 13 px. Sept boutons sur cinq pages
étaient concernés — le défaut existait déjà avec l'ancien violet (3,4:1).

- `--accent-fill: color-mix(in srgb, var(--accent) 82%, #000)` : le fond d'un contrôle qui porte un
  libellé est plus sombre que l'accent. Blanc sur ce bleu mesure maintenant **5,21:1**.
- `--on-accent` : `app.js` calcule l'encre par luminance (`inkFor`) et bascule en quasi-noir quand
  l'accent est clair. Vérifié avec `#7DD3FC` : le bouton rend `color: rgb(11, 17, 26)` sur
  `color(srgb 0.4020 0.6785 0.8104)`, et les huit pages restent à 0 problème de contraste.
- `--accent-hover` passe à 68 % (le survol était devenu invisible, même valeur que le fond).
- `--danger-fill` pour `.btn--danger:hover` et la croix de fermeture : blanc sur `#F87171` mesurait
  2,6:1.
- Le dégradé de la carte de preset active était resté **violet** (`--accent-cyan`) : il dérive
  maintenant de `--accent-text`.

### 3. Les huit pages, trois accents

24 états audités (Play, Accounts, Versions, Mods, Servers, Performance, Logs, Settings × accent par
défaut, accent du joueur, accent clair) :

| Contrôle | Résultat |
| --- | --- |
| `var(--…)` non résolus dans le moteur | **0** sur les 24 états |
| Textes sous le plancher de contraste | **0** après correction (7 avant) |
| Couleurs de l'ancienne palette encore rendues | **0** |
| Jetons statiques (`check-theme-tokens`) | 61/61 résolus, 0 manquant, 0 doublon |

Captures : `tools/theme-pages/` (défaut + accent du joueur), `tools/theme-pages-clair/` (accent clair),
rapports JSON à côté. Inspection visuelle : les huit pages avec l'accent par défaut, Performance et
Serveurs et Settings et Play avec l'accent du joueur, Play en accent clair — la carte de preset active
est passée du violet au bleu puis au rouge, le bouton primaire est lisible dans les trois cas.

### 4. Artefact publié

`cargo tauri build` relancé après les corrections (bundle NSIS 4,27 MiB) ;
`dist/Aether-Launcher-1.0.0-portable.exe` (8 988 672 o) et `dist/Aether-Launcher-Setup-1.0.0.exe`
(4 478 560 o) régénérés. L'exe publié a été **relancé** : son bouton primaire rend
`color(srgb 0.1897 0.4180 0.7911)` (le fond dérivé, pas l'accent brut), fond `rgb(7, 11, 18)`,
rayon 10 px, `missing: []`.

### Ce qui n'est pas vérifié

- Le dialogue couleur natif de Windows n'est pas automatisé (il est ouvert puis refermé par `Échap`) :
  c'est la paire d'événements qu'il émet qui est rejouée, pas son interface.
- Les scripts `tools/e2e/*.mjs` restent hors de ce pass.

---

## Relecture : simplifier le thème, puis rejouer l'e2e (19/11)

### L'encre du bouton primaire, ramenée à une décision

`inkFor` comparait le contraste du blanc et celui du noir pur, mais rendait `#0b111a` : la bande de
luminance entre les deux planchers (0,175 → 0,1855) laissait un accent sans encre lisible. La nouvelle
version ne calcule plus qu'une chose — le blanc tient-il encore 4,5:1 sur le fond dérivé ? — et
répond blanc ou noir. Les deux planchers se recouvrent (blanc jusqu'à 0,1833 ; noir dès 0,175), donc
aucun accent ne tombe entre les deux. Douze lignes au lieu de vingt, plus de cas à trois chiffres.

Mesuré après coup, sur 32 états (8 pages × accents défaut, joueur et clair) : `contraste=0` partout ;
accent `#560b0b` → `rgb(255, 255, 255)` sur `color(srgb 0.2765 0.0354 0.0354)` ; accent `#7DD3FC`
→ `rgb(0, 0, 0)` sur `color(srgb 0.4020 0.6785 0.8104)`.

### Le balayage, dégraissé

`tools/theme-sweep.mjs` : coupé la lecture de persistance (le pont ne rendait qu'`undefined`, le
`settings.json` du home l'avait déjà prouvée), le titre, l'audit initial qui dupliquait l'état
« défaut », trois consommateurs qui ne dérivent pas de l'accent (`.input`, `.nav-item.is-active`,
`.card`) et le champ `accent` redondant du rapport. Restent les jetons avant/après, les consommateurs
dérivés (bouton primaire, pastille de valeur) et le survol d'un champ.

### Les dix scripts e2e, rejoués contre la coquille

`tools/e2e/replay.ps1` rejoue tout d'une traite : une fenêtre par scénario, un home neuf et unique,
le mode de cœur attendu, le semis que les contrôles supposent (compte hors ligne, trois mods, une
session terminée), un port décalé à chaque exécution, et — jusqu'au 19/11 — une vue de 1180×1000
tenue ouverte pendant le contrôle, qui compensait des clics envoyés sous la ligne de flottaison.
Cette vue est supprimée depuis : voir « Les clics e2e dans la vraie fenêtre ».

| Scénario | Contrôles |
| --- | --- |
| `a11y` | 9/9 |
| `sliders` | 2/2 |
| `screens` | 21/21 captures sans débordement |
| `drive` | 41/41 |
| `profiles` | 26/26 |
| `failure` | 23/23 |
| `crash-report` | 12/12 |
| `cancel` | 12/12 |
| `stubborn` | 10/10 (cœur tué à 10,2 s) |
| `timeout` | 2/2 |

**137 `PASS`, 0 `FAIL`.** Trois réglages ont été nécessaires, tous côté harnais : `AETHER_REAL_CORE`
est obligatoire (le faux cœur délègue les comptes et les profils au vrai), `AETHER_FAKE_FAIL` doit
porter `launch` et non `launch-exit` pour le mode `documents`, et la vue doit être tenue pendant le
contrôle (l'override DevTools disparaît avec sa session).

### Incident : un harnais qui a tourné dans le vrai profil

Les premières versions de `replay.ps1` affectaient `$home` — une variable **automatique** de
PowerShell, dont l'affectation est ignorée : les dix scénarios ont tourné sur `C:\Users\ee`, le vrai
profil, au lieu d'un home de test. Dégâts constatés et réparés :

- ✅ retirés : les trois faux mods (`aether-client.jar`, `sodium.jar`, `iris.jar.disabled`, contenu
  « dummy … »), la fausse session `session-20261118-160000.log`, le profil `Hardcore`, la mémoire
  ramenée à 4 096 Mo et `useG1gc` à `true` (les valeurs par défaut de `settings.rs`) ;
- ⚠️ non récupérable : `screens.mjs` vide la liste des comptes et le dossier des mods avant de les
  photographier. Si un compte hors ligne ou des mods vivaient dans ce profil, ils ne sont plus là et
  aucun outil ne les a sauvegardés. L'accent a été remis à `#560b0b`, la valeur du joueur.

Le script est corrigé (`$dataHome`), et chaque scénario tourne désormais dans son propre dossier.

### Garde-fous et artefact

Audit : 61/61 jetons résolus, 0 manquant, 0 doublon. `cargo test` : 160 tests, 0 échec.
`cargo clippy --all-targets -- -D warnings` : 0 avertissement. `cargo tauri build` relancé ;
`dist/Aether-Launcher-1.0.0-portable.exe` (8 991 744 o) et `dist/Aether-Launcher-Setup-1.0.0.exe`
(4 477 894 o) régénérés, l'exe publié relancé : bouton primaire `color(srgb 0.1897 0.4180 0.7911)`,
`missing: []`.

---

## Les clics e2e dans la vraie fenêtre (19/11)

### Ce qui a changé

Les huit scripts qui cliquent ou glissent à la souris (`a11y`, `cancel`, `crash-report`, `failure`,
`hit`, `profiles`, `sliders`, `stubborn`) passent tous par **un seul helper**, `tools/e2e/click.mjs` :
`scrollIntoView` de la cible au milieu de la page, puis **le point qui va être pressé est vérifié avec
`document.elementFromPoint`** avant qu'un seul événement soit envoyé. Si ce point touche autre chose —
un toast, un fond de modale, une couche oubliée — le contrôle s'arrête en quelques millisecondes avec
le nom de ce qui est devant, au lieu de cliquer dans le vide puis d'attendre le délai de 30 s. Un
point sur un **enfant** de la cible vaut la cible (`element.contains`), et un contrôle absent, caché,
désactivé ou hors fenêtre est refusé par son nom.

L'override de vue qui compensait (`tools/e2e/viewport.mjs`, lancé par `replay.ps1` pendant chaque
scénario) est **supprimé**, fichier compris : les contrôles tournent dans la fenêtre que le joueur
reçoit réellement, 1180×760.

### Ce que la suppression a révélé

`sliders` échouait sans la vue haute — `FAIL play: memory — value 4096 → 4096, --fill 20%` : le
glissement partait d'un `y` mesuré hors fenêtre, sur la page Play. Après le `scrollIntoView` du
curseur : `4096 → 13312`, `--fill 80%` (attendu 80 %) et, sur Settings, `4096 → 10752`,
`--fill 63,33%` (attendu 63 %). Les autres scénarios n'ont rien demandé : la barre d'action de Play
tient désormais dans la fenêtre (recommandation 1), donc `#btn-install` est déjà visible à 760 px —
l'override ne servait plus qu'aux curseurs, et le harnais sait maintenant faire ce qu'un joueur
fait : défiler jusqu'à ce qu'il veut cliquer.

### Preuve — les onze scénarios, sans aucun override

`powershell -NoProfile -ExecutionPolicy Bypass -File tools/e2e/replay.ps1` (exécution complète
avec le contrôle des points de clic et le nouveau scénario `hit`) :

| Scénario | Contrôles | Sortie |
| --- | --- | --- |
| `a11y` | 9/9 | `EXIT a11y=0` |
| `sliders` | 2/2 | `EXIT sliders=0` |
| `screens` | 21/21 captures sans débordement | `EXIT screens=0` |
| `drive` | 41/41 | `EXIT drive=0` |
| `profiles` | 26/26 | `EXIT profiles=0` |
| `failure` | 23/23 | `EXIT failure=0` |
| `crash-report` | 12/12 | `EXIT crash=0` |
| `cancel` | 12/12 | `EXIT cancel=0` |
| `stubborn` | 10/10 (cœur tué à 10,2 s) | `EXIT stubborn=0` |
| `timeout` | 2/2 | `EXIT timeout=0` |
| `hit` | 12/12 | `EXIT hit=0` |
| **Total** | **170** | **0 `FAIL`** |

### Les tests du garde lui-même

Deux contrôles, l'un en vol, l'autre statique :

- **`tools/e2e/hit.mjs`** (12/12, nouveau scénario de `replay.ps1`) exerce le garde dans les deux
  sens sur la vraie page : le bouton d'installation est atteignable en `329,534` ; une couche fixe
  plein écran sur la page le fait refuser **`#btn-install is covered by #probe-overlay at 329,534`**
  et **aucun événement n'est envoyé** (`installing:false`, barre cachée, bouton toujours actif) ; la
  couche retirée, il redevient atteignable ; un point sur l'enfant d'un bouton est accepté
  (`hit=#probe-child`) ; un bouton fixé à `top:-400px` est refusé `is outside the window at 90,-380`.
  Les cinq refus reviennent en **62 ms au plus** (délai d'un contrôle : 30 000 ms).
- **`tools/e2e/click.test.mjs`** (8 tests, `node --test`) garde la garde : `elementFromPoint`
  précède `Input.dispatchMouseEvent` dans `click.mjs` (commentaires retirés avant la mesure) ; la
  ligne qui décide `reachable` doit lire le point et non une constante ; les huit scripts qui cliquent
  importent ce helper ; `sliders.mjs` est le seul à garder sa paire presse/relâche (son point de départ
  passe par `pointAt`) ; aucun script n'envoie de presse à une coordonnée écrite en dur ; aucun `try`
  n'entoure un clic (le seul `catch` de `hit.mjs` est `expectRefusal`, qui **lève** quand le clic
  passe) ; `hit.mjs` est enregistré dans `replay.ps1` ; les trois documents annoncent le même nombre
  de scripts gardés que le code.

### La mutation du garde : la preuve qu'il mord

Un contrôle présent n'est pas un contrôle porteur d'échec. Trois mutations ont donc été appliquées à
une copie de `tools/e2e/click.mjs` (sauvegarde `click.mjs.bak`), puis le fichier a été restauré à
l'octet près : `sha256` avant et après, **`b628bc93df0265eee057fce208340cbc60230741172a453dfeffcd4870ba3bdc`**,
sauvegarde supprimée, aucun résidu dans `tools/e2e`.

**Mutation 1 — le contrôle neutralisé.**
`const reachable = !!at && (at === element || element.contains(at));` devient `const reachable = true;`
(l'appel à `elementFromPoint` reste dans le fichier, mais sa réponse ne décide plus rien).

| Contrôle | Avant | Sous mutation 1 |
| --- | --- | --- |
| `hit.mjs` | 12/12 | **8/12, `EXIT hit=1`**, quatre `FAIL` : `a covered button stops the click — the click went through`, `the refusal names the point it would have hit`, `an element outside the window is refused — the point was accepted`, `every refusal comes back under a second…` ; puis le scénario s'arrête sur `Error: the click went through, the guard refused nothing (124 ms)` |
| `click.test.mjs` | 8/8 | **d'abord 8/8 : le test était complaisant.** Il ne vérifiait que la présence et l'ordre de la chaîne, pas la décision. Renforcé par le test « the reachable decision comes from the hit test, not from a constant » (qui épingle la ligne exacte et interdit `reachable = true`), il tombe : `✖ the reachable decision comes from the hit test, not from a constant — the hit test has to decide what is reachable`. |

**Mutation 2 — le contrôle déplacé.** Le bloc `createClick` (qui construit `Input.dispatchMouseEvent`)
est passé avant `pointAt` : le clic est fabriqué avant la vérification. Le comportement ne change pas —
`hit.mjs` reste **12/12, `EXIT hit=0`** —, et seul le test statique peut le voir :
`✖ the shared helper asks elementFromPoint before it presses — the hit test has to run before the mouse
event is built`. Cette mutation a aussi révélé que ce test lisait le **commentaire d'en-tête** du
fichier (qui parle d'`elementFromPoint`) : il mesure désormais le code commentaires retirés, et il
échoue bien.

**Mutation 3 — les deux à la fois** (fichier de test final) : `✖` sur les deux tests de `click.mjs`
ci-dessus, les six autres restant verts.

Après restauration : `click.test.mjs` **8/8**, `hit.mjs` **12/12**, suite complète **170 contrôles,
0 `FAIL`**, onze `EXIT` à `0`.

Limite : le harnais clique désormais le centre d'une cible défilée jusqu'au milieu de la page ; un
élément plus haut que la fenêtre serait centré sans être entièrement visible — aucun contrôle ne
clique un tel élément aujourd'hui.

---

## Gameplay in-game : HUD, animations et glyphes (19/11)

### Ce qui était cassé

Trois classes de défauts, toutes dans le client Fabric, trouvées en relisant le code plutôt qu'en
regardant une capture (impossible ici : aucun client Minecraft ne démarre sur cette machine).

| Défaut | Où | Ce qu'on voyait | Correctif |
| --- | --- | --- | --- |
| `new Animation(initial)` laissait la vitesse à **zéro** : le facteur d'interpolation valait 0 | `util/Animation.java` | une animation qui existe, qui a une cible, et qui ne bouge jamais | `DEFAULT_SPEED = 12`, vitesse nulle remplacée par ce défaut, delta négatif ignoré (une horloge qui recule ne jette pas la valeur de l'autre côté) — 6 tests dans `AnimationTest` |
| Le fondu de la cible était **calculé puis jeté** : `fadeAnim` ne servait que de seuil `> 0.01` | `HudModules.TargetHud` | la carte apparaissait et disparaissait d'un coup, et le flash de dégâts était coupé net après 180 ms | `RenderUtil.card(…, opacity)` + `ColorUtil.fade(…)` appliqués au panneau, au nom, au texte et aux barres ; le flash devient une `Animation` qui décroît |
| Même carte sans aucun fondu, et les mêmes défauts d'emoji | `HudModules.TargetInfo`, `Speed`, `Reach`, `Combo` | apparition sèche, glyphes vides à la place des emoji, valeurs qui sautent | `TargetInfo` reçoit le même fondu et le même flash ; `Speed` et `Reach` glissent leurs valeurs ; les emoji sont remplacés par une pastille dessinée |
| Le compteur **Combo n'était jamais alimenté** : `recordHit()` était déclaré, documenté « incremented by the attack mixin », et appelé par personne | `HudModules.ComboCounter`, absent des mixins | le HUD affichait « COMBO ×0 » pour toute la session | `MouseHandlerMixin` compte un clic gauche **dans le jeu** quand le réticule est sur une entité ; une perte de vie dans `onTick` termine la série. Aucun module de combat n'est touché. |
| La pastille de catégorie se déplaçait de `0.28` **par image** | `gui/ClickGuiScreen` | glissement plus vif à 240 Hz qu'à 60 Hz : le même clic ne rendait pas pareil d'un écran à l'autre | deux `Animation` sur l'horloge des frames |
| Emoji hors du plan de base (`🏃 🎨 📊 🎲 🖊 🎯 ⚡`) dans le GUI et le HUD | `ClickGuiScreen`, `HudModules` | la police du jeu n'a pas ces glyphes : chaque tabulation et chaque bouton affichait des carrés vides | noms de catégories sans emoji, bouton « HUD », pastilles dessinées (`roundedRect`) |
| `RenderUtil.roundedRectSelective` ignorait trois de ses quatre rayons, et n'était appelé nulle part | `util/RenderUtil.java` | une API qui promet un coin par angle et n'en dessine qu'un | supprimée |

### Comment c'est prouvé, et ce qui ne l'est pas

- `./gradlew build` (JDK 25 pour Loom, Java 21 pour le mod) : **BUILD SUCCESSFUL**, **87 tests,
  0 échec**, dont les 6 de `AnimationTest` (défaut qui bouge, indépendance à la fréquence, delta
  négatif ignoré, pas de dépassement, animation en cours, vitesse nulle rattrapée), le test de
  `ColorUtil.fade` qui vérifie qu'une surface déjà translucide garde son propre alpha, et les 2 de
  `LangCoverageTest` qui refusent une clé de traduction manquante.
- `grep -rn 'ud83c\|ud83d' aether-client/src/main/java` : **aucun** emoji hors plan de base restant.
- `node tools/dead-api.mjs` : **288** déclarations publiques, **1** sans appelant — l'entrypoint
  `onInitializeClient`, que Fabric appelle lui-même. Le module `Anti Knockback` n'en fait plus
  partie depuis qu'il est branché sur le paquet de vélocité (ligne 20).
- La revue de code du 19/11 a corrigé les lignes 21 à 33 ; les tests passent de 74 à **87**
  (`ModuleLifecycleTest` 3, `AetherConfigTest` 1 de plus, `EspTargetsTest` 6, `ModuleCatalogTest` 3).
- **Rien de tout cela n'a été vu en jeu** : le recul, les keybinds, les curseurs, les emoji, les deux
  overlays et le glow de l'ESP ne sont prouvés que par le calcul, la compilation, la lecture des
  sources vanilla et les tests hors session. En particulier l'annulation de `renderFire`, celle de
  `bobHurt`, le zoom par `getFov` et les deux nouveaux comportements d'`AutoTool`/`AutoGG` n'ont pas
  été observés manche allumée. Le client a toutefois été lancé (`runClient`) le
  19/11 : il atteint le menu principal sans erreur d'injection, ce qui prouve que les deux mixins
  s'appliquent (`required: true`, `defaultRequire: 1` : un point d'injection manquant ferait planter
  le chargement des classes) — pas que le feu soit invisible ni l'inclinaison neutralisée.
- `node tools/check-theme-tokens.mjs`, `git check-ignore -v` : les preuves des lignes 14 et des trois
  lignes retirées du tableau.
- Limite : **aucun rendu observé**. Le mod compile et ses tests passent, mais l'apparence du HUD, la
  fluidité perçue et les nouveaux glyphes demandent une session de jeu, que cette machine ne permet
  pas. C'est un correctif de code mesuré par ses tests, pas une capture avant/après.

### Trouvaille de dépôt, corrigée au passage

La règle `.gitignore` `Aether/` (état d'exécution du launcher) masquait **tout le code Java du mod** :
le dossier de paquet `dev/aether/` correspond au motif sur un système de fichiers insensible à la
casse, si bien que 46 fichiers sources et de test n'étaient suivis par personne — un clone du dépôt
n'avait pas le mod. Les motifs sont désormais ancrés à la racine (`/Aether/`, `/.aether/`) ; l'état d'exécution du
launcher vit de toute façon hors du dépôt.

---

## Recommandations

Classement par rapport **impact / effort** : d'abord ce qui casse l'usage, ensuite ce qui protège les
données, puis la dette. « **Implémentée** » = faite dans cette campagne, le fichier qui la porte est
cité ; « **Proposée** » = constatée et mesurée, pas encore faite. Chaque preuve est un fichier de ce
dépôt, un test exécuté ou une mesure reproductible — aucune ligne sans preuve.

**État au 19/11 : 17 entrées au tableau, 17 implémentées, 0 proposée.** Les numéros 6, 13 et 15 en
sont sortis : ils décrivaient des défauts réels de l'interface du launcher (CSS/HTML), listés juste
après le tableau avec leur mesure — ni oubliés, ni maquillés en « faits ». La numérotation garde ces
trous pour que les renvois existants (« voir ligne 2 ») restent valides.

| # | Impact / effort | Problème constaté | Amélioration proposée | État | Preuve |
| --- | --- | --- | --- | --- | --- |
| 1 | élevé / faible | Sur Play, l'action principale sortait de l'écran : `#btn-install` mesurait `top 749,25` / `bottom 786,75` dans une vue de 760 px (`fullyVisible: false`), et `#btn-cancel-install` `top 779,75` / `bottom 817,25`. | Donner à la page Play la hauteur de la fenêtre : la bannière et les cartes de compte et de profil défilent dans un conteneur interne, la carte Game Session reste ancrée en bas. | **Implémentée** | `styles.css` (page Play en colonne, `height: 100%`, `.play-scroll` défilant, en-tête et console resserrés sous 900 px de haut), `index.html` (`.play-scroll` autour de la bannière et de la grille). Mesure nommée `tools/e2e/play-fold.mjs` : avant `749,25/786,75` hors fenêtre ; après `515,16/552,66` entièrement visible, et à l'identique en installation pour les deux boutons (`play-fold-before*.json`, `play-fold-after*.json`). Vérifié aussi à `980×700` : `484,75/522,25` au repos et pour les deux boutons en installation, avec une zone défilante de `355` / `324` px qui atteint le bas de ses cartes et prend le focus clavier (`role=region`, nom accessible) — `play-fold-980x700*.json`. |
| 2 | élevé / faible | `screens.mjs` supprime le premier compte puis vide le dossier `mods` du profil sur lequel il tourne, sans vérifier que c'est un home de test : c'est ce qui a effacé des données réelles pendant cette campagne. | Refuser de démarrer si `AETHER_HOME` n'est pas sous `tools/` — garde dans `replay.ps1` **et** dans les scripts qui suppriment. | **Implémentée** | `tools/e2e/guard.mjs`, importé et appelé par les dix scripts ; `tools/e2e/replay.ps1` applique le même contrôle avant toute suppression. Refus vérifiés : `AETHER_HOME` absent → `guard.mjs:22`, `AETHER_HOME=C:\\Users\\ee` → `guard.mjs:35` ; `AETHER_E2E_ALLOW_ANY_HOME=1` laisse passer (le script échoue alors sur le port, pas sur la garde). Rappel du dégât qui l'a motivée : `tools/e2e/screens.mjs:207`, `:264-266`. |
| 3 | élevé / faible | `clickElement` calcule le centre d'un élément sans le faire défiler : sous la ligne de flottaison, le clic réel tombe hors page et le contrôle attend jusqu'au délai — observé sur `failure`, `crash`, `cancel` et `stubborn`. | `scrollIntoView({ block: 'center' })` + une frame d'attente avant d'envoyer les événements souris, dans chaque script. | **Implémentée** | Un seul helper, `tools/e2e/click.mjs` : `scrollIntoView` de la cible au milieu de la page, puis **vérification du point par `document.elementFromPoint`** avant tout événement — un clic qui tomberait sur autre chose échoue en quelques millisecondes avec le nom de l'obstacle, au lieu d'attendre le délai. Il est utilisé par les huit scripts qui cliquent — `a11y`, `cancel`, `crash-report`, `failure`, `hit`, `profiles`, `stubborn`, et `sliders` via `pointAt` pour le point de départ du glissement. `tools/e2e/viewport.mjs` supprimé, `replay.ps1` ne lance plus d'override. Onze scénarios rejoués dans la fenêtre réelle (1180×760) : **170 contrôles, 0 échec** (`a11y` 9/9, `sliders` 2/2, `screens` 21/21, `drive` 41/41, `profiles` 26/26, `failure` 23/23, `crash-report` 12/12, `cancel` 12/12, `stubborn` 10/10, `timeout` 2/2, `hit` 12/12) ; le nouveau `tools/e2e/hit.mjs` prouve le garde dans les deux sens (bouton couvert refusé en 59 ms sans aucun événement, enfant accepté, hors fenêtre refusé), et `tools/e2e/click.test.mjs` (8 tests, dont un sur la décision `reachable` et un sur le nombre de scripts gardés cité par la doc) interdit tout clic qui ne passerait pas par le helper — mutation à l'appui, voir « La mutation du garde ». La suppression de l'override avait trouvé un cas de plus : `sliders` échouait sans la vue haute. |
| 4 | élevé / faible | L'initiale d'un compte était blanche sur la couleur du compte : blanc sur `rgb(95,208,57)` mesurait **1,99:1** (plancher 4,5). Le balayage ne le voyait pas, faute de compte sur un home neuf. | Choisir l'encre de l'avatar par la luminance de son fond, comme pour le bouton primaire. | **Implémentée** | `app.js:384` (`hslToRgb`), `:406` (`paintAvatar`), `:1985` (`contrastInk`, partagé avec `inkFor`) ; mesure après correctif : avatar `color: rgb(0, 0, 0)` sur `rgb(95,208,57)`, `install-state.json` à **0 problème** contre 1,99 avant (`install-state-legacy.json`). Contexte : `styles.css:883-893`. |
| 5 | élevé / faible | Blanc sur `#F87171` mesure **2,77:1** pour `.btn--danger` : arrêt d'installation, arrêt de session, suppression d'un compte ou d'un serveur. | `--danger-fill` (danger ×72 % de noir) donne **4,98:1**, utilisé pour le fond et la bordure au survol. | **Implémentée** | `launcher/gui/src/ui/styles.css:35`, `:167`, `:822-823` ; mesure WCAG 2,77 → 4,98 (mêmes formules que l'audit). |
| 7 | faible / faible | L'UUID d'un compte, en 10 px, mesurait **4,49:1** sur le fond de la ligne sélectionnée (`rgb(124,143,161)` sur `rgb(22,39,67)`) : juste sous le plancher de 4,5. | Passer cet UUID à `--text-secondary`. | **Implémentée** | `launcher/gui/src/ui/styles.css:904-908` ; mesure après correctif : `color: rgb(163, 181, 198)` (`#a3b5c6`) sur le même fond = **7,10:1**, 0 problème dans `install-state.json`. |
| 8 | moyen / faible | L'audit de contraste ne mesurait jamais un bouton danger : les deux de la page Play sont `hidden`, les autres n'existent qu'avec une ligne de compte ou de serveur. C'est pour cela que le défaut de la ligne 5 n'a été trouvé qu'à la lecture des règles. | Ajouter au balayage un état « installation en cours » (bouton Annuler visible puis survolé) pour mesurer les `.btn--danger`. | **Implémentée** | `tools/theme-sweep.mjs` : `sweepInstall()`, `AETHER_INSTALL_STATE=1`, `--install-only` ; `tools/theme-pages-install/install-state.json` (bouton Annuler survolé, aucun problème) et `install-state-legacy.json` (ancienne valeur forcée → `.btn--danger` blanc sur `rgb(248,113,113)` = **2,77** signalé) ; capture `installation.png`. |
| 9 | élevé / faible | Blanc sur l'accent brut mesure **3,68:1** (< 4,5 exigé à 13 px) sur 7 boutons de 5 pages ; le défaut existait déjà avec l'ancien violet (3,4:1). | `--accent-fill` (accent ×82 % de noir, **5,17:1**) et `--on-accent` choisi par `inkFor` selon la luminance du fond. | **Implémentée** | `styles.css:34`, `:751-753`, `launcher/gui/src/ui/app.js:1958-1971` ; mesure 3,68 → 5,17 ; `tools/theme-pages/report.json` : 0 problème sur 32 états. |
| 10 | moyen / faible | La carte de preset active gardait un dégradé **violet** (`--accent-cyan` = `#8B5CF6`) au milieu d'une interface bleue puis rouge : survivance de l'ancienne palette. | Dériver le dégradé de `--accent-text` et supprimer l'alias devenu inutile. | **Implémentée** | `styles.css:1705` ; captures `tools/theme-pages/defaut-performance.png` et `tools/theme-pages/choisi-performance.png`. |
| 11 | élevé / faible | `replay.ps1` affectait `$home`, variable **automatique** de PowerShell : l'affectation est ignorée et les dix scénarios ont tourné dans le vrai profil. | `$dataHome` + un home neuf et unique par scénario, plus une garde de chemin (voir ligne 2). | **Implémentée** (renommage, homes uniques et garde de chemin — voir ligne 2) | `tools/e2e/replay.ps1` ; `tools/e2e/replay.log` : `home …\tools\e2e\home-replay-a11y-1358703063`. |
| 12 | moyen / faible | La procédure documentée de l'e2e ne donnait que trois commandes pour onze scripts, et ne disait ni que `AETHER_REAL_CORE` est obligatoire (sans lui le faux cœur panique), ni la matrice `AETHER_FAKE_FAIL`, ni le semis attendu. | Remplacer les trois commandes de `docs/VERIFICATION.md` §4.2 par `replay.ps1` et y reporter la matrice. | **Implémentée** | `docs/VERIFICATION.md` §4.2 remplacé : la commande unique, le tableau des onze scénarios (script, port, cœur, `AETHER_FAKE_FAIL`, semis) recopié de `tools/e2e/replay.ps1:11-23`, pourquoi `AETHER_REAL_CORE` est obligatoire, la garde de home et les ports. |
| 14 | faible / faible | Chaque exécution laissait des captures et des homes non ignorés dans l'arbre de travail : 17 fichiers (1,1 Mo) pour le thème, 21 fichiers (1,5 Mo) pour l'e2e. | Ignorer les dossiers de captures, les homes jetables et les journaux produits par les outils. | **Implémentée** | `.gitignore` : `tools/theme-pages-install/`, `tools/e2e/shots-replay/`, `tools/e2e/home-replay-*/`, `tools/e2e/pids-replay.txt`, `tools/ui-home/`, `tools/test-home/` ; `git check-ignore -v` nomme la règle pour chacun et `git status --short` ne montre plus ces artefacts. Les 17 fichiers de `tools/theme-pages/` sont **déjà suivis** — ce document les cite comme preuves : les ignorer n'aurait aucun effet rétroactif, et les retirer de l'index supprimerait des preuves. Ils restent suivis, c'est un choix assumé, pas un oubli. |
| 16 | élevé / faible | La règle `.gitignore` `Aether/` (état d'exécution du launcher) masquait tout le code Java du mod : le dossier de paquet `dev/aether/` correspond au motif sur un système de fichiers insensible à la casse — 46 sources et fichiers de test suivis par personne, donc absents d'un clone. | Ancrer les motifs à la racine (`/Aether/`, `/.aether/`) : l'état d'exécution du launcher vit hors du dépôt. | **Implémentée** | `git check-ignore -v aether-client/src/main/java/dev/aether/client/util/Animation.java` ne répond plus rien (la sortie nommait `.gitignore:23:Aether/`) ; `git status --short aether-client` liste désormais les sources ; `build/`, `aether-client/run/` et `tools/aether-home/` restent ignorés. |
| 17 | élevé / faible | Les interrupteurs du Click GUI étaient avancés depuis le tick client, par `updateAnimations(50L)` : vingt pas par seconde alors que l'écran se peint à 60, 144 ou 240 images par seconde — l'animation la plus visible de l'interface était donc saccadée, et figée dès que l'écran se fermait. | Avancer l'animation sur l'horloge des frames, dans l'écran qui la dessine, avec le delta réel borné à 100 ms. | **Implémentée** | `ClickGuiScreen.advanceModuleAnimations()`, appelé en tête de `render` ; `AetherClient.onTick` n'appelle plus `updateAnimations`. L'indépendance à la fréquence est couverte par `AnimationTest`. |
| 18 | moyen / faible | Le mod n'embarquait aucun fichier de langue : l'écran des contrôles affichait les clés brutes `key.aether.click_gui`, `key.aether.hud_editor` et `key.aether.kill_aura` pour les 46 modules, faute de `en_us.json`. | Livrer `assets/aether/lang/en_us.json` et un test qui refuse une clé manquante. | **Implémentée** | `src/main/resources/assets/aether/lang/en_us.json` (50 clés) ; `LangCoverageTest` (2 tests) reconstruit la clé de chaque module comme `Module.createKeyBinding` et échoue si elle manque. |
| 19 | faible / faible | Quatorze API publiques étaient déclarées et jamais appelées : `Animation.setSpeed`, `ColorUtil.luminance`, `ColorUtil.argb`, `RenderUtil.horizontalGradient`, `HudManager.find`, `HudManager.getHudModules`, `HudManager.getElements`, `HudElement.isDragging`, `Module.isDefaultEnabled`, `ModuleManager.setCategoryEnabled`, `ModuleManager.getModules`, `AetherConfig.normalize`, `IntegerSetting.getDefaultValue` et le constructeur `ConfigScreen(Screen)`. | Les retirer : une API que personne n'appelle promet ce que le client ne sait pas faire, et invite à coder contre un contrat mort. | **Implémentée** | Scan de `tools/dead-api.mjs` : 282 déclarations publiques, **1 sans appelant** après la passe (16 au départ) ; le seul restant est l'entrypoint `onInitializeClient`, appelé par Fabric — le module de combat `Anti Knockback` est traité à la ligne 20. Le même balayage a trouvé un champ jamais lu hors API : l'option `Health` de l'ESP, enregistrée dans le GUI et sauvegardée, que la seule ligne de rendu du module (le tag lumineux, coloré par l'équipe de l'entité) n'utilisait pas — supprimée plutôt que laissée comme une promesse vide. |
| 20 | élevé / moyen | Le module **Anti Knockback** ne modifiait rien : `onDamage`, ses deux facteurs, `isActive()` et les trois `getSavedDelta*()` n'avaient aucun appelant, donc les réglages Horizontal et Vertical ne changeaient pas le recul d'un pixel — le module s'affichait, se sauvegardait, et ne faisait rien. | Brancher le module là où le client reçoit réellement le recul, et sortir le calcul dans une classe sans dépendance Minecraft pour le prouver hors jeu. | **Implémentée** | `mixin/ClientPacketListenerMixin` corrige la vitesse du joueur local au tail de `handleSetEntityMotion` ; `util/KnockbackReduction` porte l'arithmétique ; `AntiKnockbackTest` (6 tests) prouve que le module réduit ou annule le delta, que désactivé il le rend intact et que les réglages le pilotent ; `KnockbackReductionTest` (11 tests) couvre les valeurs par défaut, les facteurs hors bornes et un delta nul. `MixinConfigTest` vérifie le descripteur exact de `handleSetEntityMotion(Lnet/minecraft/network/protocol/game/ClientboundSetEntityMotionPacket;)V`, et un relevé bytecode montre que `LivingEntity#knockback` n'est appelé que par `hurtServer`, `doSweepAttack`, `causeExtraKnockback` et `blockedByItem`, tous côté serveur — d'où le paquet de vélocité et non le knockback vanilla. Le recul d'explosion ne remplace pas la vitesse, il s'y **ajoute** (`handleExplosion` → `playerKnockback().ifPresent(player::addDeltaMovement)`) : un second hook au même endroit lit la poussée dans le paquet, la retire puis remet la poussée réduite (`KnockbackReduction#applyToAdded`), ce qui laisse la marche et la chute intactes ; une poussée non finie est ignorée comme `addDeltaMovement` l'ignore, faute de quoi la soustraction rendrait la vitesse `NaN` (5 tests). La relecture de la passe a aussi retiré un helper de test jamais appelé (`hasMethod`) et fait choisir à `MixinConfigTest` la surcharge exacte du descripteur plutôt que la première méthode portant le nom. Enfin les trois conditions qui autorisent le module à agir — module actif, paquet nommant le joueur local, explosion portant une poussée — sont sorties de l'inline du mixin dans `KnockbackFilter`, pur et testé par 6 tests (`KnockbackFilterTest`) : paquet d'une autre entité, module éteint et explosion sans poussée sont ignorés. |
| 21 | élevé / faible | Les keybinds de modules ne faisaient rien : `Kill Aura`, `Auto Crystal` et `Zoom` en ont un par défaut, le Click GUI permet d'en changer, le jeu les liste dans ses contrôles — mais aucun code ne lisait jamais `consumeClick`, et un keybind créé depuis le GUI n'était même pas enregistré auprès de Fabric, donc aucun clic ne lui était livré. | Enregistrer la mapping à sa création et consommer les appuis dans le tick client, hors écran. | **Implémentée** | Chaque module reçoit au démarrage une mapping enregistrée via `KeyBindingHelper` (non liée sauf les trois défauts) : `KeyBindingRegistryImpl` lève `IllegalStateException` dès que les options du jeu existent, donc le GUI ne peut pas en créer une — il déplace la touche (`KeyMapping#setKey`) et masque la pastille d'une mapping non liée ; `AetherClient#onTick` consomme les appuis (`consumeClick`) hors écran et sauvegarde une fois. |
| 22 | moyen / faible | Deux emoji hors du plan de base étaient réapparus : la loupe du champ de recherche du Click GUI (`\uD83D\uDD0D`) et le stylo du bouton « HUD EDITOR » de `ConfigScreen` (`\uD83D\uDD8A`) — la police du jeu les dessine en deux carrés vides. | Les remplacer par du texte. | **Implémentée** | `grep -rn 'uD83D\|uD83C' aether-client/src/main/java` → **0** (il en restait 2). |
| 23 | moyen / faible | `Module.reset()`, appelé par le bouton « RESET MODULES », écrivait le drapeau `enabled` directement : un module qui avait modifié une option du jeu (gamma, FOV, view bobbing, auto jump) était éteint sans `onDisable`, donc l'option restait à la valeur du module alors qu'il ne tournait plus. | Passer par `setEnabled`, qui déclenche les callbacks. | **Implémentée** | `ModuleLifecycleTest` (3 tests) : le reset d'un module activé appelle `onDisable` une fois, restaure les réglages et remet l'interrupteur à zéro sans animation ; un module jamais touché n'appelle rien. |
| 24 | moyen / faible | `AetherConfig.applyTo` activait le module avant d'appliquer ses réglages : `Fullbright` lit son option `Gamma` dans `onEnable`, donc une configuration « Fullbright activé, Gamma désactivé » appliquait quand même le gamma à 1.0, et rien ne la corrigeait (le module n'a pas de tick). | Appliquer les réglages d'abord, l'état ensuite. | **Implémentée** | `AetherConfigTest` : un module qui note la valeur lue dans `onEnable` reçoit **9**, la valeur sauvegardée, et non le défaut 5. |
| 25 | moyen / faible | Le curseur des réglages numériques ne correspondait pas à sa zone d'entrée : le `DoubleSetting` était peint 10 px à gauche de l'endroit où le clic était interprété, et un clic sur le **nom** du réglage était lu comme un clic à l'extrême gauche de la barre, donc remettait la valeur au minimum. | Une seule géométrie (`sliderX`) pour le dessin et l'entrée, et édition limitée à la barre. | **Implémentée** | `ClickGuiScreen#sliderX`, `SLIDER_WIDTH`, `SLIDER_LABEL_WIDTH` ; la zone de clic tolère 4 px autour de la barre, le glissement utilise les mêmes bornes. |
| 26 | faible / faible | Trois doublons morts : `ModuleManager.render` n'était appelé nulle part (le rendu était recopié dans `AetherClient`), `ModuleManager.updateAnimations` non plus (le Click GUI bouclait lui-même sur tous les modules), et `Animation.setValue` n'avait aucun appelant. | Faire passer le rendu et les animations par le manager, supprimer le setter mort. | **Implémentée** | `AetherClient#onHudRender` appelle `moduleManager.render`, qui exclut les `HudElement` (sinon chaque élément serait dessiné deux fois, une fois à l'origine) ; `ClickGuiScreen` appelle `moduleManager.updateAnimations` ; `Animation.setValue` supprimée. |
| 27 | élevé / moyen | `Hide Fire Overlay` existait mais n'était **pas enregistré** : il ne figurait pas dans le GUI et rien ne masquait les flammes à l'écran. | Brancher un mixin sur le seul dessin de l'overlay de feu, et enregistrer le module. | **Implémentée** | `mixin/ScreenEffectRendererMixin` annule `ScreenEffectRenderer#renderFire(Lcom/mojang/blaze3d/vertex/PoseStack;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/renderer/texture/TextureAtlasSprite;)V` à la tête ; `MixinConfigTest` vérifie le descripteur contre la classe réelle ; `hide fire overlay` est enregistré. Les overlays de bloc et d'eau passent par `renderTex` et `renderWater` et restent intacts. |
| 28 | moyen / moyen | `Hide Hurt Overlay` annonçait « l'overlay rouge » : 1.21.11 n'en dessine aucun, le retour d'un coup est l'inclinaison de caméra de `GameRenderer#bobHurt`. Le module n'était pas enregistré non plus. | Annuler le dessin de l'inclinaison plutôt que toucher à l'option. | **Implémentée** | `mixin/GameRendererMixin` annule `bobHurt(Lcom/mojang/blaze3d/vertex/PoseStack;F)V` à la tête ; module enregistré (`hide hurt overlay`), description corrigée. La première tentative écrivait `damageTiltStrength` à 0 : cette option est sauvegardée dans `options.txt`, donc au lancement suivant la « valeur d'origine » relue était le 0 du module et la désactivation restaurait 0 — abandonnée pour le hook. |
| 29 | élevé / moyen | `ESP` posait le tag lumineux sans jamais le retirer : une entité sortie de portée restait lumineuse pour toute la session, et la désactivation éteignait le glow de **toutes** les entités du niveau, y compris celles rendues lumineuses par le serveur (flèche spectrale, effet). | Suivre les entités réellement marquées et relâcher celles qui ne sont plus sélectionnées. | **Implémentée** | `util/EspTargets` (pur) : `selects` (mode + portée, une distance `NaN` est refusée) et `stale` (différence entre l'ancienne et la nouvelle sélection) ; `Esp` garde l'ensemble des ids marqués, relâche à la sortie de portée comme à la désactivation, et ne touche qu'à ses propres entités. La relecture a ajouté le suivi du `ClientLevel` : un id n'est unique que dans un monde, donc un changement de dimension jette l'ensemble sans le relâcher (sinon le glow d'une entité sans rapport, qui réutilise l'id, serait éteint). `EspTargetsTest` (6 tests). |
| 30 | élevé / faible | Trois modules listés et activables ne faisaient rien : `Tracers` dessinait un point de 2×2 au centre de l'écran pour chaque cible (aucune ligne, aucune projection), `Motion Blur` n'avait qu'un réglage et aucun effet, `Blink` comptait des ticks sans jamais retenir un paquet. `HudModules.TargetInfo` était en plus un élément HUD complet jamais enregistré. | Retirer plutôt que simuler : une suppression propre vaut mieux qu'une fausse implémentation. | **Implémentée** | Classes, enregistrements et clés de langue supprimés (`tracers`, `motion blur`, `blink`, `target info`) ; `grep -rn "Tracers\|Blink\|MotionBlur\|TargetInfo" src/` → aucun résultat ; imports devenus inutiles retirés de `RenderModules` et `MovementModules` ; la liste des HUD reste à 19 classes pour 19 éléments enregistrés. |
| 31 | élevé / faible | Chaque mixin écrivait le nom du module qu'il lit en toutes lettres (`"hide fire overlay"`, `"zoom"`, `"crosshair"`…), alors que le module est enregistré sous la forme minuscule de son nom : renommer l'un sans l'autre rendait le hook muet, sans aucune erreur. Le test a d'ailleurs trouvé une dérive réelle : `HudModules.ComboCounter` s'appelle « Combo » mais la liste d'enregistrement portait « combo counter ». | Une constante par clé lue par un mixin, une seule source de vérité pour la liste des modules, et un test qui interroge le vrai `ModuleManager`. | **Implémentée** | `module/ModuleKeys` (`READ_BY_MIXINS`) ; `module/ModuleCatalog` ne porte plus de clés et rend la liste des modules, que `AetherClient` enregistre ; `ModuleCatalogTest` (3 tests) vérifie que chaque clé résout un module, qu'aucun module n'est perdu par un nom en double et que la clé est bien le nom minuscule du module — c'est ce test qui a levé le cas « combo counter ». |
| 32 | moyen / moyen | `Zoom` écrivait le FOV dans l'option vanilla et restaurait une valeur littérale : comme `damageTiltStrength`, `options.txt` garde le FOV zoomé d'une session à l'autre, donc un zoom activé au moment de quitter laissait le jeu zoomé au lancement suivant et la désactivation restaurait un nombre faux. | Diviser le FOV calculé plutôt que l'option. | **Implémentée** | `GameRendererMixin` injecte au retour de `getFov(Lnet/minecraft/client/Camera;FZ)F` et divise la valeur par le facteur du module ; `Zoom` n'a plus d'état ni d'écriture d'option, seulement `getZoomFactor()` ; descripteur validé par `MixinConfigTest`. |
| 33 | moyen / faible | Audit des modules restants : `AutoTool` changeait d'objet dès que la visée tombait sur un bloc, sans que le joueur mine ; `AutoGG` annonçait « en fin de partie » alors que son seul déclencheur est la mort du joueur ; `AntiBlind` et `KillAura` faisaient déjà ce qu'ils annoncent. | Rendre chacun vrai : condition réelle pour `AutoTool`, description exacte pour `AutoGG`, conserver les deux autres. | **Implémentée** | `AutoTool` n'agit plus que touche d'attaque enfoncée (et rend l'objet précédent dès qu'elle est relâchée) ; `AutoGG` est décrit comme « envoie le message choisi à la mort du joueur » ; `AntiBlind` (retrait client de `BLINDNESS`/`DARKNESS`) et `KillAura` (attaque la cible la plus proche, rotation et indicateur) conservés sans changement. |

### Retirées du classement (passe in-game du 19/11)

Trois lignes décrivaient des défauts réels de l'**interface du launcher** : les corriger demande
d'éditer `styles.css` ou `index.html`, que cette passe s'interdit. Elles sortent du classement au
lieu d'y rester « Proposées » sans échéance ; le constat et sa mesure restent intacts ici.

| Problème constaté | Pourquoi hors périmètre | Preuve du constat |
| --- | --- | --- |
| 44 jetons de thème sur 105 ne sont référencés nulle part (échelles typographique et d'espacement, ombres, rayons, durées). | Les supprimer ou les rebrancher demande d'éditer `styles.css`. | `node tools/check-theme-tokens.mjs` → `UNUSED …` (44), `0 missing, 0 duplicated`. |
| L'accent ne se choisit que par `<input type="color">`, donc par un dialogue natif de Windows que rien ne pilote. | Des pastilles d'accent demandent `index.html` et `styles.css`. | `launcher/gui/src/ui/index.html:476` ; `tools/theme-sweep.mjs` rejoue la paire `input`/`change`. |
| `getComputedStyle(root).getPropertyValue('--accent-dim')` rend `color-mix(in srgb, …)`, pas une couleur, donc tout audit doit passer par les éléments consommateurs. | Exposer des jetons résolus touche aux jetons CSS du launcher. | `tools/theme-pages/report.json` : `tokens` = chaînes `color-mix(…)`, `consumers` = `color(srgb …)`. |

### Hypothèses assumées

Deux affirmations ci-dessus reposent sur une déduction, pas sur une mesure directe ; elles sont
marquées ici plutôt que fondues dans le tableau.

- **Ligne 3** : je déduis qu'un défilement avant le clic suffit. Ce qui est mesuré, c'est que les
  quatre scénarios passent dès qu'on agrandit la vue — le rendu d'un `scrollIntoView`, lui, n'a pas
  été essayé. (La ligne 1 n'est plus une hypothèse : `play-fold.mjs` la vérifie à chaque exécution.)
- **Premier point des lignes retirées** : la suppression des 44 jetons est sûre pour le code actuel (l'audit prouve l'absence de
  référence), mais elle suppose qu'aucun écran futur ne les prévoyait. C'est un choix de produit, pas
  une mesure.
- **Ligne 13** : « le parcours clavier n'est pas prouvé » est un constat d'absence (aucun contrôle ne
  l'exerce), pas un défaut mesuré du sélecteur.
