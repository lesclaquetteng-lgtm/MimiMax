# Rapport final — Aether Client / Aether Launcher

*14 novembre 2026. Minecraft 1.21.11, Fabric Loader 0.20.1, Fabric API 0.141.6, Java 21 (runtime),
Windows 11, RTX 4060 Ti, Ryzen 7 5700X.*

Ce rapport couvre les quatre demandes : l'analyse du projet, le test des interfaces avec un verdict
franc, la preuve des tests (FPS, serveurs non officiels), et une liste d'améliorations avancées.

---

## 1. Analyse du projet

### 1.1 Ce qu'est le produit

Deux binaires qui se parlent par un contrat JSON :

| Partie | Techno | Rôle |
| --- | --- | --- |
| `aether-client/` | Mod Fabric, Java 21, mappings Mojang | Tout ce qui tourne **dans** Minecraft : ClickGUI, HUD, modules, configuration, détection de session hors-ligne. |
| `launcher/` | Cœur Rust + interface Electron | Comptes, version/loader, installation, lancement, serveurs, hébergement, presets de performance. |

Le cœur Rust ne duplique jamais la logique dans l'interface : chaque commande imprime un document
JSON par ligne (et des événements `{"type": ...}` pour les flux), que l'interface consomme. C'est ce
qui permet de tout vérifier en ligne de commande, sans lancer Electron.

### 1.2 Ce qui a été ajouté depuis la dernière discussion

| Ajout | Où | État |
| --- | --- | --- |
| Page **Servers** : liste, ping, sonde de login, verdict hors-ligne | `launcher/src/servers.rs`, `gui/src/ui/*` | testé sur un vrai serveur (Hypixel) et sur un serveur local |
| **Hébergement** d'un serveur hors-ligne en un clic (Fabric partagé, RCON, arrêt propre) | `launcher/src/hosting.rs` | démarré en 6–8 s, arrêté en 726 ms, monde sauvegardé |
| Page **Performance** : pack Modrinth vérifié SHA-1, presets `options.txt`, presets JVM | `launcher/src/performance.rs` | 11/11 mods installés, preset `max` stable après une session |
| HUD **FPS Graph** (3 s d'historique, barres vert/orange/rouge) | `aether-client/.../HudModules.java` | visible en jeu, placé sous le watermark |
| Lancement direct sur un serveur (`--server`, `--quickPlayMultiplayer`) | `launcher/src/game.rs`, `lib.rs` | « Connecting to 127.0.0.1, 25565 » puis « Notch joined the game » |
| Sonde de login réelle (pas une heuristique sur le MOTD) | `servers.rs` | `encryption request` = premium, `set compression` = hors-ligne accepté |
| Correctifs issus des tests (voir §4) | client + cœur | 4 défauts réels trouvés et corrigés |

### 1.3 Santé du code

- **127 tests Rust** (98 unitaires, 6 téléchargement, 20 intégration, 3 SHA-1), `cargo clippy
  --release --all-targets` : **0 avertissement**, binaire de **2,1 Mo**.
- **25 tests Java** (5 suites), jar remappé de **118,9 Ko**.
- **16 contrôles de mise en page** de l'interface, **0 débordement**.
- Aucun `TODO` bloquant ; la dette restante est listée en §6.

---

## 2. Les interfaces sont-elles parfaites ?

**Réponse honnête : non, pas au sens absolu — mais elles sont cohérentes, lisibles et sans défaut de
mise en page mesurable.** Voici ce qui a été mesuré, puis ce qui reste perfectible.

### 2.1 Ce qui est prouvé

Le harnais `tools/ui-check.mjs` pilote la vraie application par CDP, sur un `AETHER_HOME` isolé, et
mesure trois choses par page : débordement du document, débordement de la colonne de contenu, et
liste des éléments dont le contenu dépasse leur boîte.

| Taille | Contrôles | Débordement document | Débordement contenu | Éléments fautifs |
| --- | --- | --- | --- | --- |
| 1180×760 | 7 pages | 0 | 0 | 0 |
| 940×620 | 9 contrôles | 0 | 0 | 0 |

Les pages affichaient de **vraies données** lues dans le cœur : 1 serveur enregistré (Hypixel, verdict
« premium » avec 26 819/200 000 joueurs et 397 ms), 1 serveur local arrêté, **11/11** mods
d'optimisation, JVM `ultra`, preset d'options `max`.

Captures : `docs/images/launcher-play.png`, `launcher-servers.png`,
`launcher-performance.png`, plus les variantes étroites `*-narrow.png`.

Deux défauts d'interface réels ont été trouvés par ces tests et corrigés :

1. La page Performance affichait `10/11` alors que le pack était complet (manifeste non partagé entre
   l'installation de test et le dossier de jeu). Après correction : `11/11`.
2. En jeu, le graphe FPS se superposait au watermark en haut à droite. Il est maintenant à
   `screenWidth − 124, 22`, juste sous le watermark.

### 2.2 Ce qui n'est pas parfait

- **Le FPS est affiché deux fois** quand le graphe est actif (compteur en haut à gauche + valeur dans
  le panneau du graphe). Le réglage `Value` du graphe permet de le masquer, mais le défaut par défaut
  reste perfectible.
- **Une seule teinte d'accent**, pas de thème clair, pas de réglage d'opacité ni de rayon.
- **Pas de recherche globale** dans le ClickGUI (seulement par catégorie), pas de favoris.
- **Pas de tri ni de recherche** dans les listes de serveurs/mods : au-delà de ~20 entrées, il
  faudra un filtre.
- **Interface du launcher en anglais uniquement** (pas d'i18n), pas de navigation clavier complète.
- **Pas d'écran de réglages in-game** pour le HUD autre que l'éditeur (pas d'aimantation à la grille,
  pas de multi-sélection, pas de profils de disposition).
- Le rendu du HUD n'a pas été validé sur un écran 4K/échelle GUI 4 : les positions par défaut sont
  calculées à partir de la taille GUI, ce qui devrait tenir, mais ce n'est pas mesuré.

**Verdict** : « professionnel et sans bug visible » — oui, sur ce qui est testé. « Parfait » — non,
et la liste ci-dessus dit exactement pourquoi.

---

## 3. Tests réalisés

### 3.1 Chaîne complète, hors ligne, de bout en bout

```text
hosts start "Local Test"          → serveur prêt en 6 s (Done 0.370s)
servers probe 127.0.0.1:25565     → offline accepté, premier paquet « set compression »
launch --server 127.0.0.1:25565   → Connecting to 127.0.0.1, 25565
serveur                           → Notch[/127.0.0.1:57215] logged in
                                    Notch joined the game
hosts stop "Local Test"           → 726 ms, « All chunks are saved », RCON arrêté
```

Le client a démarré avec **Sodium 0.8.15, Lithium 0.21.4, FerriteCore 8.2.0, ImmediatelyFast 1.14.3,
EntityCulling 1.12.1, MoreCulling 1.6.2, Cloth Config 21.11.153, Krypton 0.2.10, BadOptimizations
2.4.1, Dynamic FPS 3.11.6, Clumps 29.0.0.1** et le mod Aether, et a affiché
`Aether Client 1.0.0 ready - session: Offline`.

### 3.2 Accès aux serveurs qui acceptent les versions non officielles

La question « ce serveur accepte-t-il un compte craqué ? » est tranchée par un vrai début de login :
le launcher envoie `Login Start` avec le pseudo et lit le **premier paquet** de la réponse.

| Serveur | Premier paquet | Verdict |
| --- | --- | --- |
| `mc.hypixel.net` | `0x01 Encryption Request` | `premium` — compte Microsoft obligatoire |
| `127.0.0.1:25565` (hébergé) | `0x03 Set Compression` | `offline` — le pseudo est accepté tel quel |

C'est une différence de nature avec les launchers qui se contentent de lire le MOTD ou de tester une
connexion TCP : ici, on sait **avant** de lancer le jeu. Le protocole utilisé est celui que le serveur
annonce lui-même dans son ping (774 ici), donc la sonde reste valable quand la version change.

**Limite à assumer** : un serveur en `online-mode=true` ne peut pas accepter un compte hors-ligne, et
aucun client ne peut y changer quoi que ce soit sans contourner l'authentification Mojang — ce que ce
projet ne fait pas. Le produit le dit dans l'interface au lieu de laisser l'utilisateur échouer.

### 3.3 FPS mesurés

| Situation | FPS |
| --- | --- |
| Overlay F3 juste après la connexion | **653** (`p50 = 692`, `p2 = 402`) |
| HUD propre, preset `max`, serveur local | **889** |
| Pic observé pendant la session | **1044** |

Preuves : `docs/images/fps-debug-sodium.png` (F3, renderer Sodium, ImmediatelyFast, RTX 4060 Ti) et
`docs/images/fps-offline-hud.png` (HUD Aether, 889 FPS, 0 ms, serveur hors-ligne).

Point important : le fichier `options.txt` a été comparé **octet par octet** avant et après la
session ; il est identique. Les réglages ne sont donc pas « annulés » par le jeu au démarrage.

---

## 4. Optimisation FPS : ce qui est fait, et pourquoi ça tient

### 4.1 Le pack (11 mods, vérifiés SHA-1)

Sodium (rendu), Lithium (logique du monde), FerriteCore (mémoire), ImmediatelyFast (interface
immédiate), EntityCulling et MoreCulling (culling), Krypton (réseau), BadOptimizations (divers),
Dynamic FPS (baisse la charge en arrière-plan), Clumps (XP), Cloth Config (bibliothèque requise).

### 4.2 Les presets d'options

`performance tune --preset max` écrit : distance de rendu 8, simulation 6, particules minimales,
nuages coupés, ombres d'entités coupées, VSync off, plafond 260 FPS, fade des chunks à 0, mipmaps 2,
`prioritizeChunkUpdates` par défaut.

Le point **non évident** est `graphicsPreset:"custom"`. Avec `graphicsPreset:"fast"`, le jeu
réapplique au démarrage le bundle « Fast » complet et écrase l'occlusion ambiante, les nuages, les
particules et la distance des entités : la moitié du preset ne servait à rien. C'est mesuré, corrigé,
et verrouillé par un test.

### 4.3 Les presets JVM

`balanced`, `performance`, `ultra` (G1 : jeune génération large, `MaxGCPauseMillis` ajusté, pas de
biais de verrouillage, `AlwaysPreTouch` désactivé pour ne pas payer au démarrage).

### 4.4 Où sont les gains restants

Les 889 FPS ont été obtenus en 854×480 et distance 8 : le chiffre dépend surtout de ces deux
paramètres. Les gains réels restants sont listés en §6.1 (LOD, Nvidium, profilage par mod, plafond
adaptatif au taux de rafraîchissement).

---

## 5. Défauts trouvés par les tests (et corrigés)

| Symptôme | Cause | Correction |
| --- | --- | --- |
| `Incompatible mods found!`, le jeu ne démarre pas | MoreCulling et Dynamic FPS exigent Cloth Config, absent du pack | `cloth-config` ajouté, test dédié |
| `Error parsing option value 0 for option Anisotropic Filtering` | `maxAnisotropyBit:0` hors de `[1:4]` | presets corrigés, test de bornes |
| La moitié du réglage disparaissait après une session | `graphicsPreset:"fast"` réapplique son bundle | `graphicsPreset:"custom"` |
| Les réglages du client étaient perdus | un BOM UTF-8 faisait échouer Gson | BOM retiré avant lecture, test dédié |
| Le graphe FPS chevauchait le watermark | deux éléments au même coin | position par défaut décalée |

---

## 6. Améliorations possibles, et très avancées

### 6.1 Performance / FPS

1. **Profilage intégré** : capture de 30 s (façon Spark) agrégée par mod et par section de frame,
   avec un rapport lisible et des conseils contextuels (« EntityCulling est désactivé → +12 % sur ce
   monde »). C'est le chaînon manquant entre « un pack » et « un diagnostic ».
2. **Benchmark A/B automatique** : lancement scripté de deux presets, moyenne p50/p99, graphique et
   export CSV. Rend les promesses de gains vérifiables sur la machine de l'utilisateur.
3. **Preset adaptatif au GPU** : détection du modèle et de la VRAM, choix du render distance et du
   pack (Nvidium pour les RTX, désactivation de modules redondants).
4. **Plafond de FPS intelligent** : synchronisation avec le taux de rafraîchissement et VRR
   (G-Sync/FreeSync), limite séparée pour le menu, le monde et l'arrière-plan.
5. **LOD / rendu lointain** (Distant Horizons, Voxy) proposé comme pack « exploration » séparé, avec
   avertissement de compatibilité shaders.
6. **Chasse aux stutters** : préchargement des chunks autour du spawn, réglage de
   `chunkSectionFadeInTime`, verrouillage du ramasse-miettes sur le tas adapté à la RAM réelle,
   `-XX:ActiveProcessorCount` sur les CPU hybrides.
7. **Validation croisée des mods** : lecture des `fabric.mod.json` pour détecter les dépendances
   manquantes **avant** l'installation (le cas Cloth Config ne doit plus pouvoir arriver).

### 6.2 Serveurs, réseau, hébergement

1. **Sonde enrichie** : détection de proxy (Velocity/BungeeCord via le canal `bungeecord`), de la
   liste blanche, des bans, du mode « offline derrière proxy » — et cache avec TTL.
2. **Console RCON dans l'interface** : commandes, autocomplétion, flux des logs en direct, boutons
   op/whitelist/ban, redémarrage planifié.
3. **Sauvegardes** : archive zip du monde avant chaque arrêt, rotation, restauration en un clic.
4. **Jouer avec des amis** : ouverture de port assistée (UPnP/NAT-PMP), affichage de l'IP publique,
   ou tunnel (playit.gg) en option, avec avertissement de sécurité.
5. **Import/export** : `servers.dat` de Minecraft, JSON, partage d'une liste par lien.
6. **Reconnexion automatique** au dernier serveur après un crash ou une coupure réseau.
7. **Ping Bedrock/Geyser** (UDP) pour couvrir les serveurs inter-éditions.

### 6.3 Client in-game

1. **Profils de HUD** (PvP, survie, streaming), aimantation à la grille, échelle par élément,
   multi-sélection, verrouillage.
2. **Presets partageables** : export/import de la configuration complète par code court, favoris,
   thème clair, opacité et rayon réglables.
3. **Modules différenciants** : zoom type OptiFine, freelook, TPS serveur, graphe de ping, mémoire,
   durabilité d'armure, timers de potions, nametags, onglets de chat, éditeur de crosshair par
   glisser-déposer, physique d'items, couleurs de hit.
4. **Mode sans risque** : si le jeu plante au démarrage, le client désactive automatiquement le
   dernier module activé (journal + drapeau) au lieu de laisser l'utilisateur sans solution.
5. **ViaFabricPlus** proposé pour les serveurs 1.8 (PvP), avec bascule de protocole dans le launcher.

### 6.4 Launcher / produit

1. **Mises à jour signées** (delta), canaux stable/bêta, signature de code Windows (SmartScreen).
2. **Gestionnaire de mods complet** : recherche Modrinth/CurseForge, résolution de dépendances,
   détection de conflits, mise à jour par hash, packs exportables.
3. **Profils isolés** (un `.minecraft` par profil, quota disque), multi-instances.
4. **Authentification Microsoft robuste** : refresh silencieux, multi-comptes, affichage de la skin,
   messages d'erreur 401/entitlement lisibles.
5. **i18n FR/EN** avec pluriels, formats de date/nombres, et navigation clavier complète.
6. **Sécurité** : jetons stockés via DPAPI/Keychain, vérification SHA-1 déjà en place, verrouillage
   de l'interface pendant les opérations destructrices.
7. **Observabilité** : journal de crash analysé (stack trace résumée), bouton « copier le rapport »,
   télémétrie strictement opt-in.
8. **CI** : matrice Windows/macOS/Linux, E2E CDP, tests de charge de l'interface (100 serveurs,
   500 mods), tests de migration de configuration, instantanés des presets d'options.

### 6.5 Très avancé (différenciant)

1. **Synchronisation cloud chiffrée** des réglages, du HUD et de la liste de serveurs (E2E).
2. **Mode streamer** : masquage automatique de l'IP, du pseudo, des coordonnées et du chat.
3. **Analyse de compatibilité anti-triche** : documenter chaque module, proposer un mode « fair
   play » qui masque les modules de combat avant de rejoindre un serveur strict.
4. **API de modules versionnée** avec rechargement à chaud, pour un écosystème de plugins internes.
5. **Manettes et overlay Discord** (état « joue à … sur Aether »).
6. **Assistant de premier lancement** : détection du matériel, choix du pack (PvP / survie / shaders),
   estimation de la RAM utile, création du compte hors-ligne.

---

## 7. Ce qui reste à assumer

- **Hors-ligne ≠ sécurisé** : un serveur `online-mode=false` accepte n'importe quel pseudo. C'est
  affiché dans l'interface et dans la documentation ; aucun contournement d'`online-mode=true` n'est
  proposé.
- **Les modules de combat** peuvent être interdits sur certains serveurs : le mode « fair play » est
  une piste, pas encore implémenté.
- **Les FPS dépendent de la machine** : 889 FPS en 854×480, distance 8. Le preset et la liste de mods
  sont reproductibles, le chiffre ne l'est pas.
- **L'interface est en anglais** et une seule langue est supportée aujourd'hui.

---

## 8. Conclusion

Le projet tient la promesse « client complet avec mode hors-ligne de première classe » : un compte
suffit, un serveur local s'héberge et s'arrête proprement, un serveur distant est testé avant d'être
rejoint, et le pack d'optimisation est installé, vérifié et réellement appliqué (le fichier d'options
ne bouge plus après une session). Les interfaces sont solides et mesurées — zéro débordement, zéro
défaut de mise en page — mais pas « parfaites » : la liste du §2.2 est le chemin exact qui reste à
parcourir. Les gains de performance restants ne sont plus dans les mods, ils sont dans le diagnostic
(§6.1), et c'est la prochaine étape logique.
