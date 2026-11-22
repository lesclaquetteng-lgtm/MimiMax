# Remarque.md — Revue critique pré-publication de MimiMax / Aether

> **Décision au 22 novembre 2026 : NO-GO — ne pas publier de release utilisateur à partir du commit audité.**
>
> Dépôt : https://github.com/lesclaquetteng-lgtm/MimiMax  
> Branche : `main`  
> Commit examiné : `a392ad0bd38d132837d6a64c0639a9b03e5d07de`  
> Périmètre : launcher Rust, shell Tauri, interface HTML/CSS/JS, mod Fabric, hébergement local, documentation, configuration GitHub Actions et état CI visible au moment de l'audit.

Ce document remplace l'ancienne analyse. Plusieurs remarques de l'ancienne version étaient devenues fausses après les changements récents : les fichiers de sécurité et de contribution existent maintenant, le workflow de release existe, le stockage DPAPI des jetons est implémenté et les anciens exécutables à la racine ne figurent plus dans l'arbre actuel. Les constats ci-dessous sont réévalués sur le commit indiqué, pas recopiés depuis l'ancien rapport.

---

## 1. Synthèse exécutive

Le projet a une base technique sérieuse : séparation entre le cœur Rust et l'interface, shell Tauri avec CSP restrictive, protections de chemins et d'extraction, vérification de téléchargements, tests unitaires et d'intégration, stockage protégé des jetons Microsoft sous Windows, et workflows automatisés.

Cela ne suffit toutefois pas pour déclarer le logiciel prêt à être distribué au grand public.

### Bloquants confirmés

| ID | Sévérité | Problème | Décision |
|---|---|---|---|
| A-01 | **BLOQUANT** | La CI de `main` échoue sur Clippy à cause d'un import inutilisé | Corriger avant tout merge/release |
| A-02 | **CRITIQUE** | Le choix `onlineMode=true` n'est pas correctement appliqué à un nouveau serveur hébergé | Corriger et tester les deux modes |
| A-03 | **CRITIQUE** | Recréer un serveur déjà actif peut enregistrer de nouveaux ports sans modifier les ports réellement utilisés | Corriger la gestion des ports et des serveurs actifs |
| A-04 | **CRITIQUE** | La whitelist d'un serveur offline ne prouve pas l'identité du joueur ; la documentation donne une garantie excessive | Corriger le modèle de sécurité et les avertissements |
| A-05 | **ÉLEVÉ** | L'arrêt du jeu peut supprimer le fichier PID et retourner un succès sans confirmer l'arrêt du processus | Corriger et tester les échecs d'arrêt |
| A-06 | **ÉLEVÉ** | Les processus persistants sont retrouvés uniquement par PID, sans preuve qu'il s'agit encore du processus lancé par Aether | Vérifier l'identité du processus avant toute terminaison |
| A-07 | **ÉLEVÉ** | Le workflow de release ne reproduit pas l'ensemble des contrôles de la CI | Bloquer les releases sur une validation complète |
| A-08 | **ÉLEVÉ** | Les scénarios E2E de l'interface ne sont pas exécutés par les workflows CI visibles | Ajouter des tests automatisés de l'application installée |

**Conclusion :** tant que A-01 à A-08 ne sont pas traités ou explicitement résolus par une décision technique documentée, je déconseille de distribuer le launcher ou le mod comme version stable.

### Ce que cette revue prouve — et ne prouve pas

L'analyse porte sur l'arbre GitHub du commit identifié, les sources consultées, les fichiers de configuration et les journaux CI disponibles. Elle comprend une revue statique ciblée ; elle ne constitue pas une exécution indépendante du logiciel sur un PC Windows propre, une session Minecraft réelle ou un serveur public.

Je ne prétends donc pas avoir prouvé l'absence de toutes les autres erreurs. Un audit statique ne peut jamais garantir mathématiquement « zéro bug ». L'objectif réaliste est de rendre les défauts connus bloquants, d'augmenter la couverture des scénarios réels et d'exiger des preuves reproductibles avant chaque publication.

---

## 2. Défauts et risques détaillés

### A-01 — La CI de `main` est rouge : Clippy bloque le commit

<badge> BLOQUANT </badge>

**Preuve :** exécution GitHub Actions `45074946784`, sur le commit audité. Le job `Rust core (ubuntu-latest)` a terminé en échec.

- La suite `cargo test --locked` a réussi.
- L'étape `cargo clippy --locked --all-targets -- -D warnings` a échoué.
- Erreur exacte : import inutilisé `aether_launcher::logs::SessionLog`.
- Emplacement : `launcher/tests/integration.rs:13`.

Avec `-D warnings`, un simple avertissement devient une erreur de compilation. Une branche qui échoue à cette règle ne doit pas être considérée comme validée.

**Correctif exigé :**
1. Supprimer l'import s'il n'est plus utilisé, ou l'utiliser si le test devait réellement s'en servir.
2. Relancer localement `cargo clippy --locked --all-targets -- -D warnings`.
3. Relancer la CI complète sur Windows et Linux.
4. Vérifier que tous les jobs du commit final sont verts, pas uniquement le job Rust Linux.

**Critère de clôture :** aucun job en échec, annulé ou ignoré sans justification sur le commit candidat à la release.

### A-02 — Le paramètre de mode premium du serveur n'est pas respecté à la création

<badge> CRITIQUE </badge>

**Fichiers :** `launcher/src/hosting.rs`, fonctions `properties` et `create` ; `launcher/desktop/src/commands.rs`, commande `hosts_create`.

La commande Tauri accepte explicitement `onlineMode=true` et transmet l'option au cœur. Cependant :

- la fonction qui génère un nouveau fichier `server.properties` écrit en dur `online-mode=false` ;
- l'appel de cette fonction lors de la création ne lui transmet pas le paramètre `online_mode` ;
- la branche qui traite un fichier déjà existant ne force `online-mode=true` que lorsque l'option est vraie ;
- lorsqu'on demande le mode offline pour un serveur existant, le fichier n'est pas nécessairement remis à `online-mode=false` ;
- les métadonnées peuvent donc annoncer un mode qui ne correspond pas à la configuration effective du serveur.

**Impact :** l'utilisateur peut croire qu'il a créé un serveur avec authentification premium alors que le serveur démarre en offline, ou inversement retrouver une configuration ancienne qui ne correspond pas à son choix.

**Correctif exigé :**
1. Faire de la valeur demandée une donnée effectivement utilisée lors de la génération du fichier.
2. Définir une politique explicite pour les serveurs existants : appliquer le choix, ou refuser la modification et demander une confirmation claire. Ne jamais afficher un état différent de celui du fichier réellement utilisé.
3. Relire et valider le contenu final de `server.properties` avant d'écrire les métadonnées.
4. Distinguer le mode demandé, le mode configuré et le mode réellement observé lorsque le serveur tourne.

**Tests obligatoires :**

| État initial | Choix demandé | Résultat attendu |
|---|---|---|
| Nouveau serveur | Offline | `online-mode=false` |
| Nouveau serveur | Premium | `online-mode=true` |
| Serveur existant offline | Premium | Mode effectif premium, ou refus explicite |
| Serveur existant premium | Offline | Mode effectif offline, ou refus explicite |

Les tests doivent lire le fichier généré, pas uniquement vérifier les métadonnées JSON.

### A-03 — Les ports peuvent être désynchronisés lors de la recréation d'un serveur actif

<badge> CRITIQUE </badge>

**Fichier :** `launcher/src/hosting.rs`, fonction `create`, allocation des ports et mise à jour des métadonnées.

Lors d'un nouvel appel à `create`, le code réessaie de réserver le port existant à travers `free_port`. Si le serveur tourne encore, le port est déjà occupé : la fonction peut alors choisir un autre port libre.

Or, pour un fichier `server.properties` déjà présent, le code met à jour certaines propriétés — whitelist et éventuellement mot de passe RCON — mais ne réécrit pas nécessairement `server-port` ni `rcon.port`. Les nouvelles valeurs peuvent pourtant être enregistrées dans les métadonnées du serveur.

**Conséquence concrète :** le processus continue d'écouter sur ses anciens ports alors que le launcher enregistre de nouveaux ports. L'adresse affichée devient fausse et l'arrêt via RCON peut viser le mauvais port ; le launcher finit alors par tenter de tuer le processus.

**Correctif exigé :**
- Interdire la recréation ou la modification des ports d'un serveur en cours d'exécution.
- Pour un serveur arrêté, synchroniser atomiquement les propriétés et les métadonnées.
- Ne jamais choisir un nouveau port uniquement parce que le port historique est occupé par le serveur que l'on gère déjà.
- Séparer « vérifier qu'un port est libre » de « attribuer et persister un port ».
- Gérer explicitement le conflit de port au démarrage et afficher le port réellement utilisé.

**Tests obligatoires :**
1. Créer un serveur, le démarrer, rappeler la commande de création : les ports doivent rester cohérents.
2. Arrêter puis redémarrer le serveur : mêmes ports tant qu'aucune modification n'a été demandée.
3. Simuler un port occupé par un autre processus.
4. Vérifier que le bouton Stop utilise le vrai port RCON et que le monde est sauvegardé.

### A-04 — La whitelist ne protège pas l'identité sur un serveur offline exposé

<badge> CRITIQUE — SÉCURITÉ </badge>

**Fichiers :** `launcher/src/hosting.rs`, fonctions `properties`, `ensure_whitelist` et `create` ; documentation « Offline servers » du README.

Le projet utilise des serveurs `online-mode=false`. Dans ce mode, le serveur ne vérifie pas auprès de Microsoft/Mojang que le joueur contrôle réellement le compte correspondant au pseudo. L'identité offline est dérivée du nom envoyé par le joueur.

La whitelist peut donc filtrer les noms non autorisés, mais **elle ne constitue pas une authentification** : un client qui peut atteindre le serveur peut tenter de se présenter sous le nom offline d'un joueur autorisé. Le UUID offline étant déterministe à partir du nom, il ne prouve pas que le client est le propriétaire légitime de ce nom.

Le README indique que la whitelist empêche un tiers de prendre le nom d'un opérateur. Cette formulation est trop forte et peut donner un faux sentiment de sécurité.

Autre point : les nouveaux fichiers générés sont configurés avec `server-ip=127.0.0.1`, mais la branche qui traite un `server.properties` déjà présent ne réimpose pas cette valeur. Un ancien fichier ou un fichier modifié peut donc conserver une adresse d'écoute plus large. Il ne faut pas généraliser la protection loopback à tous les serveurs existants.

**Correctif exigé :**
1. Réécrire la documentation : whitelist offline = filtre de noms, pas preuve d'identité.
2. Pour le mode offline, conserver le loopback comme valeur par défaut et vérifier l'adresse d'écoute effective avant le démarrage.
3. Pour toute exposition au LAN ou à Internet, afficher un avertissement explicite sur l'usurpation de pseudo.
4. Ne jamais présenter un serveur offline exposé comme sécurisé par la seule whitelist.
5. Pour un serveur public, recommander une authentification réelle côté serveur/proxy et une configuration réseau maîtrisée. À défaut, ne pas autoriser l'exposition publique depuis le launcher.
6. Définir comment le launcher traite les serveurs existants dont les propriétés sont anciennes ou incohérentes.

**Test de sécurité obligatoire :** sur un serveur de test isolé, tenter de joindre le serveur avec un client offline utilisant le pseudo d'un joueur autorisé. Documenter le résultat réel et adapter les garanties affichées à ce comportement.

### A-05 — L'arrêt du jeu peut annoncer un succès alors que le jeu tourne toujours

<badge> ÉLEVÉ </badge>

**Fichier :** `launcher/src/game.rs`, fonction `stop`.

Après avoir demandé l'arrêt du processus, le code vérifie son état pendant une durée limitée. À la fin de cette boucle, il supprime le fichier PID et retourne le PID comme résultat sans vérifier une dernière fois que le processus a effectivement disparu.

Si l'arrêt échoue ou prend plus longtemps que le délai prévu, le launcher peut :
- afficher un arrêt réussi ;
- supprimer la seule référence persistante au processus ;
- laisser Minecraft tourner en arrière-plan ;
- ne plus pouvoir retrouver ce processus lors de la prochaine session du launcher.

C'est particulièrement problématique parce que la logique d'arrêt du serveur hébergé adopte une meilleure politique : elle conserve le PID lorsqu'elle n'a pas réussi à arrêter le processus.

**Correctif exigé :**
- Après le délai, vérifier une dernière fois l'état du processus.
- Ne supprimer le PID qu'après confirmation de l'arrêt.
- En cas d'échec, retourner une erreur explicite et conserver l'enregistrement.
- Faire remonter à l'interface « arrêt demandé », « arrêt confirmé » et « arrêt impossible » comme trois états distincts.

**Tests obligatoires :** arrêt normal, processus lent à s'arrêter, refus d'arrêt, processus déjà terminé, échec de la commande système et fermeture du launcher pendant l'arrêt.

### A-06 — Un PID seul ne prouve pas que le processus appartient encore au launcher

<badge> ÉLEVÉ — RISQUE PROCESSUS </badge>

**Fichiers :** `launcher/src/game.rs`, `launcher/src/hosting.rs`, fonctions de lecture PID et `process_alive`.

Le launcher persiste un nombre PID puis vérifie essentiellement si ce PID existe :
- sous Windows, via `tasklist` ;
- sur les autres systèmes, via `kill -0`.

Un PID peut être réutilisé après la fermeture du processus initial. Un ancien fichier `game.pid` ou `server.pid` peut alors pointer vers un processus sans rapport avec Minecraft ou Aether. Une commande Stop pourrait tenter de tuer ce processus.

**Correctif exigé :**
- Stocker une identité de processus plus robuste que le PID seul : PID, date/heure de création, exécutable attendu et identifiant de session généré au lancement.
- Vérifier cette identité avant toute commande de terminaison.
- En cas de doute, ne pas tuer le processus ; demander une action manuelle et préserver le diagnostic.
- Nettoyer les enregistrements orphelins sans toucher aux processus qui ne peuvent pas être identifiés avec certitude.

**Test obligatoire :** simuler un fichier PID obsolète dont le numéro est réutilisé par un autre processus. Le launcher ne doit jamais tuer ce dernier.

### A-07 — Le workflow de release n'impose pas tous les contrôles de la CI

<badge> ÉLEVÉ </badge>

**Fichier :** `.github/workflows/release.yml`.

Le workflow de release existe désormais et crée un fichier `SHA256SUMS`, ce qui est une bonne amélioration. Mais il ne reproduit pas toute la validation du workflow CI.

La release exécute notamment les tests et Clippy du cœur, construit le mod Fabric et le shell Tauri. En revanche, le workflow de release ne reprend pas l'ensemble des contrôles de la CI visibles :
- vérification de formatage Rust ;
- tests du shell Tauri ;
- contrats et vérifications de l'interface JavaScript ;
- vérification des tokens CSS, de l'Identity Studio et de la parité du design system ;
- validation de l'ensemble des jobs de la matrice Windows/Linux.

La CI principale est déclenchée par les PR et les pushes de la branche principale, tandis que la release est déclenchée par un tag. Le workflow de release ne vérifie pas à lui seul que le commit pointé par ce tag possède une CI complètement verte.

**Correctif exigé :**
1. Faire dépendre la publication d'une validation complète du commit tagué.
2. Ne jamais publier si un contrôle requis est absent, en échec ou annulé.
3. Ajouter un contrôle de cohérence entre tag, version Gradle, version Cargo, version Tauri, nom du JAR et version affichée dans l'application.
4. Empêcher la publication accidentelle d'un tag sur un commit non validé.
5. Conserver les empreintes SHA-256, mais ajouter la signature des exécutables Windows et une provenance de build vérifiable.

**État de distribution :** l'API GitHub ne retourne actuellement aucune release publiée pour ce dépôt. Le workflow est présent, mais cela ne prouve pas encore qu'une release complète et installable a été publiée et vérifiée.

### A-08 — Les tests E2E de l'interface ne sont pas branchés à la CI visible

<badge> ÉLEVÉ </badge>

**Fichiers :** `.github/workflows/ci.yml`, `tools/e2e/`, `CONTRIBUTING.md`, `docs/VERIFICATION.md`.

Le dépôt dispose de scripts E2E, de captures d'écran, de vérifications d'accessibilité et de scripts pilotant l'interface. Cependant, le workflow CI visible exécute principalement :
- les tests Rust ;
- la compilation et les tests du shell ;
- la compilation et les tests du mod ;
- des contrôles de syntaxe et de contrats de l'interface.

Il ne lance pas une véritable session automatisée de l'application installée avec WebView2 pour parcourir les écrans et tester les interactions comme le fera un utilisateur.

**Risque :** une compilation réussie et des contrats statiques ne détectent pas nécessairement une fenêtre blanche, un bouton inactif, un événement IPC mal relié, une régression de navigation, un débordement visuel ou une différence entre l'environnement de test et le build livré.

**Correctif exigé :**
- Ajouter un job Windows qui construit puis lance l'application dans un environnement isolé.
- Exécuter les parcours critiques : installation, compte offline, compte Microsoft (lorsque les secrets de test sont disponibles), sélection de version, installation/annulation/reprise, mods, serveurs, performance, logs, lancement/arrêt du jeu.
- Tester au minimum les tailles de fenêtre supportées et un affichage à mise à l'échelle Windows élevée.
- Conserver les captures et rapports comme artefacts CI pour chaque PR importante.

---

## 3. Autres remarques importantes avant publication

### B-01 — Identité de marque incohérente

**Fichiers :** README, `launcher/desktop/tauri.conf.json`, historique du dépôt.

L'identité publique n'est pas uniformisée :
- dépôt GitHub : MimiMax ;
- README et packages : Aether Client / Aether Launcher ;
- produit et titre de fenêtre Tauri : CUBE Launcher ;
- éditeur/publisher : Aether ;
- l'introduction du README mentionne encore un ancien `MimiMax.exe` à la racine, absent de l'arbre actuel.

**À faire :**
- choisir un nom public unique ;
- aligner nom de fenêtre, nom du produit, éditeur, identifiant d'application, dossier de données, installateur, README, releases et captures ;
- supprimer les références aux anciens exécutables lorsqu'elles ne décrivent plus le fonctionnement courant ;
- vérifier le nom affiché dans Applications installées, le menu Démarrer, le désinstalleur et les propriétés du binaire.

Ce n'est pas seulement cosmétique : une identité différente entre l'installateur et l'application réduit la confiance de l'utilisateur et complique le support.

### B-02 — La documentation de vérification ne reflète pas toujours l'état réel du commit

**Fichier :** `docs/VERIFICATION.md`.

Le document contient des résultats locaux et des nombres de tests qui ne sont pas identiques à ceux visibles dans les derniers logs CI. Il annonce notamment une validation Clippy réussie pour les vérifications locales, alors que la CI du commit audité échoue sur Clippy.

Il faut distinguer explicitement :
- résultat local et date de la machine ;
- résultat CI et SHA exact ;
- test automatisé à chaque PR ;
- vérification manuelle non exécutée à chaque commit ;
- test réel en jeu ;
- résultat historique qui n'est plus représentatif.

**À faire :** ajouter un tableau de preuve contenant commande, plateforme, SHA, date, résultat et lien CI. Ne jamais laisser un ancien résultat « vert » être interprété comme preuve que le commit courant est vert.

### B-03 — Le dépôt contient beaucoup de captures et de fichiers de validation historiques

L'arbre actuel représente environ 24,6 Mo de fichiers suivis, dont environ 19,9 Mo de PNG. Plusieurs répertoires semblent contenir des séries de captures ou des états de QA successifs : `tools/theme-pages/`, `tools/theme-pages-clair/`, `tools/qa/`, `tools/ui-shots-*/`, `tools/e2e/shots-verify/` et `tools/installed-shots2/`.

Les deux exécutables sous `tools/e2e/` semblent être des fixtures de test accompagnées de leurs sources Rust : ne pas les supprimer aveuglément. Il faut plutôt séparer les fixtures nécessaires des captures temporaires et des exports historiques.

**À faire :**
- conserver les captures réellement utiles à la documentation et aux tests ;
- déplacer les résultats temporaires et les gros rapports vers les artefacts CI ;
- compresser les images qui n'ont pas besoin d'une définition élevée ;
- vérifier les fichiers déjà suivis malgré leur présence dans `.gitignore` ;
- éviter d'ajouter des captures redondantes à chaque itération.

### B-04 — Scan de secrets de tout l'historique Git non prouvé par cette revue

Le dépôt contient maintenant des règles d'ignorance pour plusieurs répertoires locaux et le coffre DPAPI est implémenté. Cela ne prouve pas qu'aucun secret n'a été commité dans le passé.

Je n'ai pas exécuté Gitleaks ou TruffleHog sur l'intégralité de l'historique Git. Je ne conclus donc ni qu'un secret est présent, ni qu'il n'y en a aucun.

**Obligation avant diffusion :**
1. scanner tout l'historique, toutes les branches et tous les tags ;
2. inspecter les fichiers de test contenant des comptes JSON, journaux, chemins utilisateur et configurations de serveurs ;
3. révoquer immédiatement tout secret qui aurait été publié, puis purger l'historique si nécessaire ;
4. activer la protection contre les secrets et le push protection ;
5. ajouter un job automatisé de scan à la CI.

Un fichier ignoré aujourd'hui peut toujours exister dans un ancien commit.

### B-05 — Dépendances et licences tierces : rendre la conformité vérifiable

**Fichier :** `THIRD-PARTY-NOTICES.md`.

Le document identifie les principales familles de dépendances et donne des liens vers les projets Modrinth. C'est une bonne base, mais une distribution publique doit permettre d'identifier précisément ce qui est effectivement utilisé.

**À faire :**
- publier un inventaire des dépendances effectivement incluses dans chaque release ;
- conserver version exacte, source, licence déclarée et attribution requise ;
- différencier les bibliothèques intégrées au JAR, les dépendances téléchargées à l'installation et les dépendances utilisées uniquement à la compilation ;
- vérifier les conditions de redistribution des captures, polices, icônes et ressources graphiques ;
- inclure les notices nécessaires dans le package lorsque les licences l'exigent.

Ne pas supposer que le lien vers la page d'accueil du projet suffit pour toutes les obligations d'une version donnée.

### B-06 — Signature et mise à jour du logiciel

Le workflow de release construit les exécutables et calcule leurs empreintes SHA-256. C'est utile, mais une empreinte publiée à côté d'un binaire ne remplace pas une signature de code vérifiable.

Je ne vois pas de signature de code Windows dans le workflow de release consulté ni de mécanisme d'auto-mise à jour effectivement branché dans le shell.

**À faire :**
- signer les installateurs et exécutables Windows ;
- vérifier la signature dans le processus de publication ;
- publier les empreintes depuis le workflow de build ;
- documenter la procédure de vérification pour l'utilisateur ;
- concevoir le mécanisme de mise à jour avec manifeste signé, contrôle de version, gestion des erreurs et possibilité de revenir à une version connue ;
- ne jamais installer silencieusement un binaire qui n'a pas passé la vérification cryptographique.

### B-07 — Les modules de combat restent un risque de réputation et de support

Le README annonce clairement les fonctions Kill Aura, Auto Crystal et Anti Knockback et indique qu'elles peuvent entraîner un bannissement. Il précise aussi que les modules de combat sont désactivés par défaut.

C'est transparent, mais « désactivé par défaut » ne constitue pas une limitation technique : l'utilisateur peut les activer et les utiliser sur un serveur qui les interdit.

**À faire :**
- séparer clairement le client de confort/optimisation et les modules de combat dans la présentation produit ;
- ne jamais faire de promesse de compatibilité anti-cheat ou d'indétectabilité ;
- conserver l'avertissement avant activation et le rendre impossible à manquer ;
- tester explicitement qu'une nouvelle configuration n'active aucun module de combat ;
- documenter les risques de bannissement dans l'interface, pas uniquement dans le README.

### B-08 — Le support système doit être annoncé sans ambiguïté

Le shell livré est construit autour de Tauri et WebView2 sous Windows. Le stockage sécurisé des jetons Microsoft repose sur DPAPI ; les autres plateformes refusent de persister ces secrets plutôt que de les écrire en clair.

Cette stratégie est préférable à un stockage non sécurisé, mais la matrice de support doit être nette : système officiellement supporté, système compilable uniquement pour développement, fonctionnalités indisponibles et comportement attendu.

**À faire :** publier une matrice Windows 10/11 validée, préciser le statut des autres systèmes, puis ne pas afficher une fonctionnalité Microsoft comme disponible là où le coffre n'est pas pris en charge.

### B-09 — Versions et artefacts de build trop liés à des noms figés

**Fichier :** `launcher/desktop/build.rs`.

Le script de build attend le JAR précis `aether-client-1.0.0.jar`. Cette dépendance au nom exact est fragile lors d'une future montée de version : un changement de version Gradle qui n'est pas répercuté au même endroit peut empêcher la compilation du shell ou lui faire embarquer un artefact incorrect.

**À faire :**
- centraliser les versions du produit ;
- faire vérifier que la version du JAR, du cœur, du shell et du tag sont identiques ;
- éviter les noms de fichier codés en dur lorsque le système de build peut fournir l'artefact explicitement ;
- ajouter un test qui échoue dès qu'une version diverge.

### B-10 — Portabilité de l'arrêt des processus

**Fichier :** `launcher/src/hosting.rs`, fonction `kill_tree`.

Sur Windows, la commande utilise `taskkill /T /F`. Sur les autres systèmes, la branche observée envoie `TERM` au PID ciblé ; elle ne démontre pas qu'elle termine toute l'arborescence de processus, malgré les commentaires qui parlent de l'ensemble de l'arbre.

Si Linux ou macOS restent des cibles de compilation ou d'exécution, ce comportement doit être corrigé ou clairement déclaré non supporté. Les tests Linux de la bibliothèque ne remplacent pas une validation du comportement de terminaison.

---

## 4. Points positifs à préserver

Ces éléments sont utiles et ne doivent pas être cassés lors des corrections :

- Séparation du cœur Rust, du shell natif Tauri et de l'interface.
- CSP restrictive et permissions Tauri limitées à la fenêtre principale.
- Téléchargements en fichiers temporaires, reprise et vérification de hash avant remplacement.
- Contrôles de chemins et protections contre l'extraction ZIP hors du répertoire cible.
- Écritures atomiques et sauvegardes de configuration.
- Coffre DPAPI pour les jetons Microsoft sous Windows, avec refus de stockage en clair sur les plateformes non prises en charge.
- Protections contre plusieurs installations ou lancements concurrents.
- Tests de récupération de configuration, de téléchargement, de session et de modules.
- Documentation des limites du mode offline et des risques des modules de combat.

Le but n'est pas de refaire l'architecture, mais de corriger les comportements qui ne respectent pas les garanties annoncées.

---

## 5. Plan de correction prioritaire

### Phase 0 — Avant toute publication

- [ ] Corriger A-01 et obtenir une CI totalement verte sur le commit exact.
- [ ] Corriger A-02 : le mode d'authentification doit correspondre à la demande et au fichier serveur.
- [ ] Corriger A-03 : plus aucune divergence entre ports persistés, propriétés et processus actif.
- [ ] Corriger A-04 : supprimer la fausse garantie de sécurité liée à la whitelist offline et valider les serveurs existants.
- [ ] Corriger A-05 : ne jamais annoncer un arrêt réussi sans confirmation.
- [ ] Corriger A-06 : vérifier l'identité du processus avant de le terminer.
- [ ] Corriger A-07 : le workflow de release doit exiger toutes les validations.
- [ ] Corriger A-08 : ajouter les parcours E2E Windows à la CI.
- [ ] Effectuer le scan de secrets de l'historique complet.
- [ ] Uniformiser le nom public du produit.

### Phase 1 — Validation fonctionnelle réelle

- [ ] Installer sur un Windows 10 propre.
- [ ] Installer sur un Windows 11 propre.
- [ ] Tester sans Java installé, avec Java incompatible et avec Java valide.
- [ ] Tester une installation neuve, une réparation, une annulation et une reprise après interruption.
- [ ] Tester les réponses réseau 404, 429, 500, timeout, coupure et téléchargement corrompu.
- [ ] Tester les comptes offline et Microsoft, le renouvellement de session et la déconnexion.
- [ ] Tester les versions Minecraft/Fabric réellement annoncées.
- [ ] Tester le serveur offline neuf et existant, le mode premium, les conflits de ports, les redémarrages et les arrêts.
- [ ] Tester la fermeture du launcher pendant une installation et pendant une partie.
- [ ] Tester la récupération après crash du launcher et du jeu.
- [ ] Tester les écrans réels, les boutons, les modales, la navigation clavier, la mise à l'échelle Windows et les petites fenêtres.
- [ ] Tester l'installation, la mise à jour et la désinstallation depuis le package final, pas depuis l'environnement de développement.

### Phase 2 — Publication fiable

- [ ] Choisir et uniformiser le nom et les métadonnées du produit.
- [ ] Valider la nomenclature des versions sur Cargo, Gradle, Tauri et le tag.
- [ ] Générer une release depuis un commit complètement validé.
- [ ] Signer les binaires Windows.
- [ ] Vérifier les hashes et la signature sur une machine propre.
- [ ] Fournir les notices de licences exactes des composants livrés.
- [ ] Publier les notes de version, les prérequis, les limites et une procédure de signalement de bugs.
- [ ] Faire une installation pilote avant l'annonce publique.

---

## 6. Portes de sortie — conditions minimales pour déclarer une version publiable

Une version ne doit être marquée stable que si toutes les conditions suivantes sont satisfaites :

| Porte | Condition obligatoire | Preuve attendue |
|---|---|---|
| Build | Tous les artefacts sont construits depuis le commit tagué | Logs CI et SHA du commit |
| Qualité | Tests, Clippy et formatage sont verts | Résultats CI complets |
| Interface | Parcours E2E critiques réussis sur Windows | Rapport et captures CI |
| Jeu | Minecraft démarre, rejoint un monde et s'arrête correctement | Logs de test réels |
| Serveurs | Modes d'authentification et ports vérifiés | Tests de propriétés et intégration |
| Sécurité | Aucune faiblesse critique non résolue, aucun secret publié | Rapport de scan et revue |
| Processus | Aucun processus étranger ne peut être tué par un PID obsolète | Tests de réutilisation PID |
| Distribution | Installateur, désinstallateur et mise à jour vérifiés | Test sur machine propre |
| Intégrité | Signature et hashes vérifiables | Signature valide et SHA-256 |
| Documentation | README, notices, prérequis et changelog alignés avec le binaire | Revue de release |

**Règle stricte :** un test non exécuté n'est pas un test réussi. Un résultat historique n'est pas une preuve pour un nouveau commit. Un avertissement ignoré doit être justifié et accepté explicitement ; il ne doit pas être masqué.

---

## 7. Sources internes consultées

- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`
- `.github/dependabot.yml`
- `README.md`
- `SECURITY.md`
- `CONTRIBUTING.md`
- `THIRD-PARTY-NOTICES.md`
- `docs/ARCHITECTURE.md`
- `docs/FEATURES.md`
- `docs/VERIFICATION.md`
- `launcher/src/hosting.rs`
- `launcher/src/game.rs`
- `launcher/src/credentials.rs`
- `launcher/src/accounts.rs`
- `launcher/src/installer.rs`
- `launcher/src/logs.rs`
- `launcher/src/http.rs`
- `launcher/desktop/src/commands.rs`
- `launcher/desktop/src/main.rs`
- `launcher/desktop/src/state.rs`
- `launcher/desktop/tauri.conf.json`
- `launcher/desktop/capabilities/default.json`
- `launcher/desktop/build.rs`
- `launcher/gui/src/ui/bridge.js`
- `aether-client/build.gradle`
- `aether-client/src/main/java/dev/aether/client/module/Module.java`
- `aether-client/src/main/java/dev/aether/client/module/ModuleManager.java`
- `aether-client/src/main/java/dev/aether/client/config/AetherConfig.java`
- `aether-client/src/main/java/dev/aether/client/module/modules/CombatModules.java`

**État final de cette revue :** le dépôt possède de bonnes fondations et plusieurs protections réelles, mais le commit audité ne satisfait pas encore un niveau de qualité acceptable pour une release publique stable. Corriger les défauts prouvés, automatiser les tests manquants, puis refaire une revue sur le commit candidat à la publication.
