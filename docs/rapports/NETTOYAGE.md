# Nettoyage et organisation du dépôt

**Dernière passe :** 22 novembre 2026

Ce rapport décrit la disposition du dépôt et ce qui a été retiré de la racine au fil des passes.
Les rapports de la racine ont été regroupés dans `docs/rapports/` pour que le premier niveau ne
contienne que les points d'entrée.

## Disposition actuelle

```
MimiMax/
├── MimiMax.exe                  # Lanceur racine (double-clic), généré par build.bat
├── Aether-Launcher.exe          # Alias du même lanceur
├── build.bat                    # Build complet : core Rust, coquille Tauri, mod, exécutables
├── README.md                    # Documentation d'accueil, index des docs
├── LICENSE
├── .github/  .vscode/           # CI et réglages d'éditeur
│
├── aether-client/               # Mod Fabric (Java)
│   ├── src/                     # Sources
│   ├── build/libs/              # Jar produit (généré)
│   └── logs/                    # Journaux d'exécution (généré, ignoré par git)
│
├── launcher/                    # Lanceur
│   ├── src/                     # Core Rust
│   ├── gui/                     # Interface WebView (HTML/CSS/JS)
│   └── desktop/                 # Coquille Tauri
│       ├── installer/           # Template NSIS, licence FR, art de la fenêtre
│       └── README.md            # Build, installateur, recettes de test
│
├── dist/                        # Exécutables prêts à livrer (généré, ignoré par git)
├── docs/                        # Documentation de référence
│   ├── ARCHITECTURE.md
│   ├── DESIGN-SYSTEM.md
│   ├── FEATURES.md
│   ├── THE-FOLD.md
│   ├── VERIFICATION.md
│   ├── EXEMPLE-UI.md            # Copie d'interface de référence
│   ├── images/                  # Captures citées par les docs
│   └── rapports/                # Rapports datés
│       ├── ANALYSE-AMELIORATIONS.md
│       ├── RAPPORT-FINAL.md
│       └── NETTOYAGE.md
│
└── tools/                       # Outillage et environnement local (non versionné)
    ├── jdk25/  gradle-9.8.2/    # Chaîne de compilation portable
    ├── aether-home/             # Environnement de test moddé
    ├── e2e/                     # Scénarios et tests d'interface
    └── *.ps1 / *.mjs            # Sondes, captures, vérifications
```

## Retiré lors de cette passe

| Élément | Raison |
| --- | --- |
| `MimiMax.pdb` (1,2 Mo) | Symboles de débogage d'une compilation `rustc` ponctuelle, régénérés au besoin ; versionné par erreur. `*.pdb` est désormais ignoré. |
| `aether-client/logs/` | Journaux d'exécution du client (`latest.log`, rotations `.log.gz`) ; le dossier est maintenant ignoré par git. |
| Rapports à la racine | `ANALYSE-AMELIORATIONS.md`, `RAPPORT-FINAL.md` et `NETTOYAGE.md` déplacés dans `docs/rapports/` (l'historique git suit le déplacement). |
| `Exemple.md` | Renommé `docs/EXEMPLE-UI.md` : c'est la copie d'interface de référence, pas un fichier de code. |

## Ce qui reste local, jamais versionné

- `tools/jdk25/`, `tools/gradle-9.8.2/`, `tools/aether-home/` : chaîne de compilation portable et
  environnement de test (plusieurs centaines de Mo, téléchargés par `tools/setup.ps1`).
- `launcher/**/target/`, `launcher/gui/node_modules/` : sorties de compilation.
- `dist/` : exécutables assemblés.
- `docs/images/` reste versionné : ce sont les preuves citées par `docs/VERIFICATION.md`.

## Vérifier l'état après une passe

```bash
git status --short           # rien d'inattendu hors travail en cours
git ls-files | grep -i pdb   # doit être vide
node --test tools/e2e/click.test.mjs
```
