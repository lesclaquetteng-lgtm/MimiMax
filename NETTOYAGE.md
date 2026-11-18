# Rapport de Nettoyage et d'Organisation du Projet MimiMax

**Date :** 18 novembre 2026  
**Espace total libéré :** ~63.5 MB (367 fichiers temporaires et obsolètes supprimés)

---

## 1. Éléments Nettoyés et Supprimés

### Fichiers temporaires à la racine
- `tmp-pre.png` : Capture temporaire résiduelle (38.2 KB).
- `MimiMax.pdb` : Symboles de débogage temporaires générés par rustc (1.28 MB).

### Fichiers en vrac dans `tools/` (253 fichiers)
- **88 fichiers de logs obsolètes** : `build1.log` à `build12.log`, `compile1.log` à `compile3.log`, `runclient*.log`, `tauri-build*.log`, `tauri-bundle*.log`, `clippy-*.log`, etc.
- **131 captures d'écran de tests temporaires** : captures jetables des sessions précédentes (`fps-ingame*.png`, `fullscreen*.png`, `after-gui*.png`, `crash-report*.png`, `final-gui*.png`, etc.).
- **14 fichiers de dumps et logs d'exécution** : `game-run*.ndjson`, `.ndjson.err`, `server-run*.ndjson`.
- **Dépotoirs d'analyse et traces bytecode** : `jstack-19004.txt`, `jstack-5928.txt`, `options-javap.txt`, `particles-javap.txt`, `preset-javap.txt`, `mc-cmdline.txt`, `launch-command*.json`, etc.

### Dossiers d'anciennes captures obsolètes dans `tools/` (102 fichiers / ~15 MB)
- `tools/ui-shots/` (17 captures obsolètes)
- `tools/ui-shots2/` (17 captures obsolètes)
- `tools/ui-shots3/` (17 captures obsolètes)
- `tools/ui-shots4/` (17 captures obsolètes)
- `tools/tauri-shots/` (17 captures obsolètes)
- `tools/portable-shots/` (17 captures obsolètes)

---

## 2. Structure Épurée et Organisée Conservée

```
MimiMax/
├── MimiMax.exe                    # Lanceur principal optimisé (218 KB)
├── Aether-Launcher.exe            # Alias du lanceur à la racine (218 KB)
├── build.bat                      # Script de build global
├── README.md                      # Documentation d'accueil
├── ANALYSE-AMELIORATIONS.md       # Synthèse des optimisations de performances
├── RAPPORT-FINAL.md               # Spécifications et récapitulatif
├── NETTOYAGE.md                   # Le présent rapport d'organisation
│
├── aether-client/                 # Mod Fabric In-Game (Java 25)
│   ├── src/                       # Sources avec GUI CyberDeck et HUD modules
│   └── build/libs/                # Jar compilé prêt pour le modpack
│
├── launcher/                      # Lanceur de bureau (Rust + Electron UI)
│   ├── src/                       # Core engine en Rust (téléchargements, auth, process)
│   ├── gui/src/ui/styles.css      # Design System Linear/Vercel refondu
│   └── tests/                     # 151 tests automatisés
│
├── dist/                          # Exécutables d'installation et versions portables finales
├── docs/                          # Documentation technique
│
└── tools/                         # Outillage et environnement portable
    ├── jdk25/                     # JDK 25 portable requis pour la compilation
    ├── gradle-9.8.2/              # Gradle portable
    ├── aether-home/               # Environnement de test moddé avec le jar synchronisé
    ├── new-ui-shots/              # 8 captures officielles validées des tests UI Playwright
    ├── run-ui-tests.ps1           # Runner officiel de test UI Playwright (8/8 validés)
    ├── ui-runner.mjs              # Moteur headless DevTools
    ├── check-ids.mjs              # Validateur des 85 DOM IDs du launcher
    └── launcher-runner.rs         # Source de compilation des exécutables racine
```
