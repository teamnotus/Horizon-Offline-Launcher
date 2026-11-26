use super::*;
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "horizon-launcher-servers-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn s(v: &str) -> Tag {
    Tag::String(v.into())
}

/// A servers.dat as the game writes it: one shown server with a picture and the fields the
/// launcher doesn't use, then one joined through Direct Connection.
fn vanilla_file(home: &Path) -> Vec<u8> {
    let png = [0x89, b'P', b'N', b'G', 1, 2, 3];
    let icon = base64::engine::general_purpose::STANDARD.encode(png);
    let root = NamedTag {
        name: "".into(),
        tag: Tag::compound([(
            "servers",
            Tag::List(vec![
                Tag::compound([
                    ("name", s("Friends")),
                    ("ip", s("friends.example:25570")),
                    ("icon", s(&icon)),
                    ("acceptTextures", Tag::Byte(1)),
                    ("acceptedCodeOfConduct", Tag::Int(-12345)),
                    ("hidden", Tag::Byte(0)),
                ]),
                Tag::compound([
                    ("name", s("Minecraft Server")),
                    ("ip", s("direct.example")),
                    ("hidden", Tag::Byte(1)),
                ]),
            ]),
        )]),
    };
    let bytes = nbt::write_named(&root, Limits::default()).unwrap();
    fs::write(home.join(FILE), &bytes).unwrap();
    bytes
}

fn entries(home: &Path) -> Vec<Tag> {
    let bytes = fs::read(home.join(FILE)).unwrap();
    match nbt::read_named(&bytes, Limits::default())
        .unwrap()
        .0
        .tag
        .get("servers")
    {
        Some(Tag::List(list)) => list.clone(),
        other => panic!("no servers list: {other:?}"),
    }
}

fn ips(home: &Path) -> Vec<String> {
    List::load(home)
        .servers()
        .into_iter()
        .map(|s| s.ip)
        .collect()
}

#[test]
fn reads_the_shown_servers_with_their_pictures() {
    let home = scratch("reads");
    vanilla_file(home.as_path());
    let list = List::load(home.as_path());
    assert_eq!(
        list.servers(),
        vec![Server {
            name: "Friends".into(),
            ip: "friends.example:25570".into(),
            icon: Some(vec![0x89, b'P', b'N', b'G', 1, 2, 3]),
        }]
    );
    // No file, or one that isn't NBT: an empty list, as in the game.
    assert!(List::load(&home.as_path().join("none"))
        .servers()
        .is_empty());
    fs::write(home.as_path().join(FILE), b"not nbt").unwrap();
    assert!(List::load(home.as_path()).servers().is_empty());
}

#[test]
fn adding_and_removing_keeps_every_other_field_and_the_hidden_servers() {
    let home = scratch("keeps");
    let original = vanilla_file(home.as_path());
    let before = entries(home.as_path());

    let mut list = List::load(home.as_path());
    list.add("  Hub ", " hub.example ").unwrap();
    assert!(list.add("x", "  ").is_err());
    list.save(home.as_path()).unwrap();
    // The game's own backup of what was there.
    assert_eq!(
        fs::read(home.as_path().join("servers.dat_old")).unwrap(),
        original
    );
    assert!(!home.as_path().join("servers.dat_new").exists());

    let after = entries(home.as_path());
    assert_eq!(after.len(), 3);
    assert_eq!(
        after[0], before[0],
        "the first server is written back untouched"
    );
    assert_eq!(
        after[1],
        Tag::compound([
            ("name", s("Hub")),
            ("ip", s("hub.example")),
            ("hidden", Tag::Byte(0)),
        ]),
        "a new server goes after the shown ones, before the hidden ones"
    );
    assert_eq!(after[2], before[1], "the hidden server stays");

    let mut list = List::load(home.as_path());
    let (i, found) = find(&list, "friends").unwrap();
    assert_eq!((i, found.ip.as_str()), (0, "friends.example:25570"));
    assert_eq!(find(&list, "2").unwrap().1.name, "Hub");
    assert_eq!(find(&list, "HUB.example").unwrap().0, 1);
    assert!(find(&list, "3").is_err(), "the hidden one can't be named");
    assert_eq!(list.remove(0).unwrap().name, "Friends");
    assert!(list.remove(5).is_err());
    list.save(home.as_path()).unwrap();
    assert_eq!(
        entries(home.as_path()),
        vec![after[1].clone(), before[1].clone()]
    );
}

#[test]
fn a_server_without_a_name_gets_the_games_default() {
    let home = scratch("default");
    let mut list = List::load(home.as_path());
    list.add("", "play.example").unwrap();
    list.save(home.as_path()).unwrap();
    assert!(!home.as_path().join("servers.dat_old").exists());
    assert_eq!(
        List::load(home.as_path()).servers()[0].name,
        "Minecraft Server"
    );
}

/// A list left by an older launcher: the shipped server at its current and former addresses, with
/// the marker that told it not to offer the server again.
fn shipped(home: &Path) {
    vanilla_file(home);
    let mut list = List::load(home);
    list.add("SMP", SHIPPED_ADDRESS).unwrap();
    for address in SHIPPED_ADDRESSES {
        list.add("Old SMP", address).unwrap();
    }
    list.save(home).unwrap();
    fs::write(home.join(SHIPPED_MARKER), format!("{SHIPPED_ADDRESS}\n")).unwrap();
}

#[test]
fn the_servers_horizon_shipped_are_taken_out() {
    let home = scratch("shipped");
    shipped(&home);
    let before = entries(&home);
    remove_shipped(&home).unwrap();
    assert_eq!(ips(&home), ["friends.example:25570"]);
    let after = entries(&home);
    assert_eq!(after.len(), before.len() - 1 - SHIPPED_ADDRESSES.len());
    assert_eq!(after[0], before[0], "the player's server stays untouched");
    assert_eq!(
        after.last(),
        before.last(),
        "the hidden server stays untouched"
    );
    assert!(!home.join(SHIPPED_MARKER).exists());
    // Nothing to do the next time: not even a rewrite of the file.
    let file = fs::read(home.join(FILE)).unwrap();
    remove_shipped(&home).unwrap();
    assert_eq!(fs::read(home.join(FILE)).unwrap(), file);
}

#[test]
fn shipped_servers_are_taken_out_in_any_spelling() {
    let home = scratch("spelling");
    vanilla_file(&home);
    let mut list = List::load(&home);
    list.add("SMP", &SHIPPED_ADDRESS.to_ascii_uppercase())
        .unwrap();
    list.add(
        "Old",
        &format!("{}:25565", SHIPPED_ADDRESSES[0].to_ascii_uppercase()),
    )
    .unwrap();
    list.add("Other port", &format!("{}:25570", SHIPPED_ADDRESSES[0]))
        .unwrap();
    list.save(&home).unwrap();
    remove_shipped(&home).unwrap();
    assert_eq!(
        ips(&home),
        vec![
            "friends.example:25570".to_owned(),
            format!("{}:25570", SHIPPED_ADDRESSES[0])
        ],
        "only the shipped addresses go; another port is another server"
    );
}

#[test]
fn a_broken_list_is_left_alone() {
    let home = scratch("broken");
    fs::write(home.join(FILE), b"not nbt").unwrap();
    fs::write(home.join(SHIPPED_MARKER), b"anything\n").unwrap();
    assert!(remove_shipped(&home).is_err());
    assert_eq!(fs::read(home.join(FILE)).unwrap(), b"not nbt");
    assert!(home.join(SHIPPED_MARKER).exists());
}
