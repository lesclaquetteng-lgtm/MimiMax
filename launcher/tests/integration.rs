//! The tests of the launcher.
//!
//! They cover the offline identifier, the metadata of Mojang, the rules, the argument templates
//! and the command line of the game.

use std::collections::HashMap;

use aether_launcher::accounts::AccountStore;
use aether_launcher::game::{
    build_classpath, build_command, build_game_arguments, build_jvm_arguments, LaunchOptions,
    CLASS_PATH_SEPARATOR,
};
use aether_launcher::minecraft::{
    apply_replacements, evaluate_argument_template, evaluate_rules, is_native_coordinate,
    maven_path, merge_libraries, merge_version_json, parse_maven_coordinate, resolve_libraries,
    Argument, Arguments, Downloads, FeatureContext, Library, OsRule, Rule, VersionJson,
};
use aether_launcher::offline::{is_valid_username, offline_player_uuid, undash_uuid};
use aether_launcher::settings::Settings;

// Produced with java.util.UUID.nameUUIDFromBytes("OfflinePlayer:" + name).
const REFERENCE: [(&str, &str); 4] = [
    ("Notch", "b50ad385-829d-3141-a216-7e7d7539ba7f"),
    ("Steve", "5627dd98-e6be-3c21-b8a8-e92344183641"),
    ("Player123", "3da1e349-17b0-38ed-a07b-1e78c8ebd956"),
    ("aether_test", "192d7399-e38b-3155-afd3-d30d29bf7d22"),
];

#[test]
fn the_offline_identifier_matches_the_reference_implementation_of_minecraft() {
    for (name, expected) in REFERENCE {
        assert_eq!(
            offline_player_uuid(name),
            expected,
            "uuid mismatch for {}",
            name
        );
    }
}

#[test]
fn the_offline_identifier_is_a_version_three_uuid() {
    let uuid = offline_player_uuid("Notch");
    assert_eq!(&uuid[14..15], "3");
    assert!(["8", "9", "a", "b"].contains(&&uuid[19..20]));
}

#[test]
fn the_usernames_follow_the_rules_of_minecraft() {
    assert!(is_valid_username("Notch"));
    assert!(is_valid_username("a_b_1"));
    assert!(!is_valid_username("ab"));
    assert!(!is_valid_username(&"a".repeat(17)));
    assert!(!is_valid_username("has space"));
}

#[test]
fn the_dashes_are_removed() {
    assert_eq!(
        undash_uuid("b50ad385-829d-3141-a216-7e7d7539ba7f"),
        "b50ad385829d3141a2167e7d7539ba7f"
    );
}

#[test]
fn the_account_store_keeps_the_offline_accounts() {
    let mut store = AccountStore::default();
    let account = store.add_offline("Notch").unwrap();
    assert_eq!(account.uuid, "b50ad385-829d-3141-a216-7e7d7539ba7f");
    assert!(account.is_offline());
    assert_eq!(store.selected.as_deref(), Some(account.id.as_str()));

    store.add_offline("Notch").unwrap();
    assert_eq!(store.accounts.len(), 1, "the account was duplicated");

    store.remove(&account.id);
    assert!(store.accounts.is_empty());
    assert!(store.selected.is_none());
}

#[test]
fn the_maven_coordinates_are_parsed() {
    let parts = parse_maven_coordinate("org.ow2.asm:asm:9.10.1");
    assert_eq!(parts.group, "org.ow2.asm");
    assert_eq!(parts.artifact, "asm");
    assert_eq!(parts.version, "9.10.1");
    assert!(parts.classifier.is_none());
    assert_eq!(parts.extension, "jar");
}

#[test]
fn the_maven_coordinates_become_library_paths() {
    assert_eq!(
        maven_path("org.ow2.asm:asm:9.10.1"),
        "org/ow2/asm/asm/9.10.1/asm-9.10.1.jar"
    );
    assert_eq!(
        maven_path("org.lwjgl:lwjgl:3.3.3:natives-windows"),
        "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar"
    );
}

#[test]
fn the_native_libraries_are_detected() {
    assert!(is_native_coordinate(
        "org.lwjgl:lwjgl:3.3.3:natives-windows"
    ));
    assert!(is_native_coordinate(
        "io.netty:netty-transport-native-epoll:4.2.7:linux-x86_64"
    ));
    assert!(!is_native_coordinate("org.lwjgl:lwjgl:3.3.3"));
}

fn windows() -> aether_launcher::minecraft::OsContext {
    aether_launcher::minecraft::OsContext {
        name: "windows".to_string(),
        arch: "x86_64".to_string(),
        version: "10.0.19045".to_string(),
    }
}

#[test]
fn the_rules_are_evaluated() {
    let allow_osx = vec![Rule {
        action: Some("allow".to_string()),
        os: Some(OsRule {
            name: Some("osx".to_string()),
            version: None,
            arch: None,
        }),
        features: None,
    }];

    assert!(!evaluate_rules(
        &allow_osx,
        &windows(),
        &FeatureContext::default()
    ));
    assert!(evaluate_rules(&[], &windows(), &FeatureContext::default()));

    let disallow = vec![
        Rule {
            action: Some("allow".to_string()),
            os: None,
            features: None,
        },
        Rule {
            action: Some("disallow".to_string()),
            os: Some(OsRule {
                name: Some("windows".to_string()),
                version: None,
                arch: None,
            }),
            features: None,
        },
    ];
    assert!(!evaluate_rules(
        &disallow,
        &windows(),
        &FeatureContext::default()
    ));
}

#[test]
fn the_argument_templates_honour_their_rules() {
    let template = vec![
        Argument::Value("--always".to_string()),
        Argument::Conditional {
            rules: vec![Rule {
                action: Some("allow".to_string()),
                os: None,
                features: None,
            }],
            value: serde_json::json!(["--width", "${w}"]),
        },
    ];

    let mut features = HashMap::new();
    features.insert("has_custom_resolution".to_string(), true);
    let context = FeatureContext { features };

    let arguments = evaluate_argument_template(&template, &windows(), &context);
    assert_eq!(arguments, vec!["--always", "--width", "${w}"]);
}

#[test]
fn the_placeholders_are_replaced() {
    let mut replacements = HashMap::new();
    replacements.insert("a".to_string(), "1".to_string());
    assert_eq!(apply_replacements("${a}-${b}", &replacements), "1-${b}");
}

#[test]
fn the_fabric_profile_is_merged_into_the_vanilla_version() {
    let vanilla = VersionJson {
        id: "1.21.11".to_string(),
        main_class: "net.minecraft.client.main.Main".to_string(),
        libraries: vec![Library {
            name: "a:b:1".to_string(),
            downloads: None,
            rules: vec![],
            natives: None,
            url: None,
        }],
        arguments: Arguments {
            game: vec![Argument::Value("--username".to_string())],
            jvm: vec![Argument::Value("-cp".to_string())],
        },
        ..Default::default()
    };

    let fabric = VersionJson {
        id: "fabric-loader-0.20.1-1.21.11".to_string(),
        main_class: "net.fabricmc.loader.impl.launch.knot.KnotClient".to_string(),
        inherits_from: Some("1.21.11".to_string()),
        libraries: vec![Library {
            name: "a:b:1".to_string(),
            downloads: None,
            rules: vec![],
            natives: None,
            url: Some("https://maven.fabricmc.net/".to_string()),
        }],
        ..Default::default()
    };

    let merged = merge_version_json(&vanilla, &fabric);
    assert_eq!(
        merged.main_class,
        "net.fabricmc.loader.impl.launch.knot.KnotClient"
    );
    assert_eq!(merged.inherits_from.as_deref(), Some("1.21.11"));
    assert_eq!(merged.libraries.len(), 1, "the libraries were not merged");
    assert!(merged
        .arguments
        .game
        .iter()
        .any(|entry| matches!(entry, Argument::Value(value) if value == "--username")));
}

#[test]
fn the_duplicate_libraries_are_merged() {
    let merged = merge_libraries(
        &[Library {
            name: "a:b:1".to_string(),
            downloads: None,
            rules: vec![],
            natives: None,
            url: None,
        }],
        &[Library {
            name: "a:b:1".to_string(),
            downloads: None,
            rules: vec![],
            natives: None,
            url: Some("https://example.org/".to_string()),
        }],
    );
    assert_eq!(merged.len(), 1);
}

#[test]
fn the_libraries_are_resolved_into_downloadable_files() {
    let version = VersionJson {
        libraries: vec![
            Library {
                name: "org.lwjgl:lwjgl:3.3.3".to_string(),
                downloads: None,
                rules: vec![],
                natives: None,
                url: Some("https://maven.fabricmc.net/".to_string()),
            },
            Library {
                name: "org.example:only:mac".to_string(),
                downloads: None,
                rules: vec![Rule {
                    action: Some("allow".to_string()),
                    os: Some(OsRule {
                        name: Some("osx".to_string()),
                        version: None,
                        arch: None,
                    }),
                    features: None,
                }],
                natives: None,
                url: Some("https://example.org/".to_string()),
            },
        ],
        ..Default::default()
    };

    let resolved = resolve_libraries(&version, &windows());
    assert_eq!(resolved.len(), 1, "the rules were not applied");
    assert_eq!(
        resolved[0].url,
        "https://maven.fabricmc.net/org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3.jar"
    );
}

fn launch_options() -> LaunchOptions {
    LaunchOptions {
        username: "Notch".to_string(),
        uuid: "b50ad385-829d-3141-a216-7e7d7539ba7f".to_string(),
        access_token: "0".to_string(),
        offline: true,
        xuid: String::new(),
        client_id: String::new(),
        version: VersionJson {
            id: "1.21.11".to_string(),
            main_class: "net.minecraft.client.main.Main".to_string(),
            inherits_from: Some("1.21.11".to_string()),
            kind: Some("release".to_string()),
            downloads: Downloads::default(),
            arguments: Arguments {
                game: vec![
                    Argument::Value("--username".to_string()),
                    Argument::Value("${auth_player_name}".to_string()),
                    Argument::Value("--accessToken".to_string()),
                    Argument::Value("${auth_access_token}".to_string()),
                    Argument::Value("--uuid".to_string()),
                    Argument::Value("${auth_uuid}".to_string()),
                ],
                jvm: vec![
                    Argument::Value("-cp".to_string()),
                    Argument::Value("${classpath}".to_string()),
                ],
            },
            ..Default::default()
        },
        client_jar: "C:/mc/versions/1.21.11/1.21.11.jar".into(),
        extra_jars: vec!["C:/mc/mods/aether-client.jar".into()],
        natives_dir: "C:/mc/versions/1.21.11/natives".into(),
        game_dir: "C:/mc".into(),
        assets_dir: "C:/mc/assets".into(),
        asset_index: "29".to_string(),
        library_dir: "C:/mc/libraries".into(),
        java_path: "C:/java/bin/java.exe".into(),
        max_memory: 4096,
        min_memory: 1024,
        window_width: 854,
        window_height: 480,
        use_g1gc: true,
        jvm_preset: "balanced".to_string(),
        quick_play_server: None,
    }
}

#[test]
fn the_classpath_contains_the_game_and_the_mods() {
    let classpath = build_classpath(&launch_options());
    assert!(classpath
        .iter()
        .any(|entry| entry.to_string_lossy().contains("1.21.11.jar")));
    assert!(classpath
        .iter()
        .any(|entry| entry.to_string_lossy().contains("aether-client.jar")));
}

#[test]
fn the_classpath_is_passed_exactly_once() {
    // A duplicated classpath would be read by the virtual machine as the main class.
    let options = launch_options();
    let classpath = build_classpath(&options);
    let arguments = build_jvm_arguments(&options, &classpath);
    let joined = classpath
        .iter()
        .map(|entry| entry.to_string_lossy().to_string())
        .collect::<Vec<String>>()
        .join(CLASS_PATH_SEPARATOR);

    let occurrences = arguments.iter().filter(|entry| *entry == &joined).count();
    assert_eq!(occurrences, 1, "the classpath was passed more than once");
}

#[test]
fn the_arguments_carry_the_identity_of_the_player() {
    let arguments = build_game_arguments(&launch_options());

    let position = |name: &str| arguments.iter().position(|entry| entry == name).unwrap();
    assert_eq!(arguments[position("--username") + 1], "Notch");
    assert_eq!(arguments[position("--accessToken") + 1], "0");
    assert_eq!(
        arguments[position("--uuid") + 1],
        "b50ad385-829d-3141-a216-7e7d7539ba7f"
    );
}

#[test]
fn the_premium_session_forwards_the_token() {
    let mut options = launch_options();
    options.offline = false;
    options.access_token = "real-token".to_string();

    let arguments = build_game_arguments(&options);
    let position = |name: &str| arguments.iter().position(|entry| entry == name).unwrap();
    assert_eq!(arguments[position("--accessToken") + 1], "real-token");
}

#[test]
fn the_command_starts_the_main_class_of_the_version() {
    let built = build_command(&launch_options());
    assert!(built
        .arguments
        .contains(&"net.minecraft.client.main.Main".to_string()));
    assert!(built.arguments.contains(&"-Xmx4096M".to_string()));
    assert!(built
        .arguments
        .contains(&"-Dfabric.gameVersion=1.21.11".to_string()));
}

#[cfg(windows)]
#[test]
fn a_session_log_never_contains_the_access_token() {
    let directory = std::env::temp_dir().join(format!("aether-session-log-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();

    let mut options = launch_options();
    options.offline = false;
    options.access_token = "secret-microsoft-access-token".to_string();
    // A real child process, so the launcher's output path runs end to end. It rejects the JVM
    // arguments and exits at once, which is enough: the log must hold what the child printed, never
    // the command line that carries the token.
    options.java_path = std::env::var("SystemRoot")
        .map(|root| format!("{root}\\System32\\where.exe"))
        .unwrap_or_else(|_| "where.exe".to_string())
        .into();

    // The canary is armed: the token really travels in the command line of the child.
    let command = build_command(&options);
    assert!(command
        .arguments
        .iter()
        .any(|argument| argument.contains("secret-microsoft-access-token")));

    let mut session = SessionLog::create(directory.join("session.log")).unwrap();
    let mut printed: Vec<String> = Vec::new();
    let _ = aether_launcher::game::launch(
        &options,
        |_| {},
        |line| {
            session.line(line);
            printed.push(line.to_string());
        },
    );
    session.finish(1);

    let log = std::fs::read_to_string(directory.join("session.log")).unwrap();
    assert!(
        !log.trim().is_empty(),
        "the output of the child was not written to the session log"
    );
    assert!(
        !log.contains("secret-microsoft-access-token"),
        "the session log holds the token: {log}"
    );
    assert!(!printed.join("\n").contains("secret-microsoft-access-token"));

    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_settings_survive_a_round_trip() {
    let directory = std::env::temp_dir().join(format!("aether-settings-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let file = directory.join("settings.json");

    let settings = Settings {
        max_memory: 8192,
        ..Settings::default()
    };
    settings.save(&file).unwrap();

    let loaded = Settings::load(&file);
    assert_eq!(loaded.max_memory, 8192);
    assert_eq!(loaded.game_version, "1.21.11");

    std::fs::remove_dir_all(&directory).ok();
}
