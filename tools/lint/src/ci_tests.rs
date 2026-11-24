use super::*;

const MINGW: &str = "gcc-mingw-w64-i686 gcc-mingw-w64-x86-64 binutils-mingw-w64-i686 binutils-mingw-w64-x86-64 mingw-w64-common mingw-w64-i686-dev mingw-w64-x86-64-dev";

fn known() -> BTreeSet<String> {
    sets(&format!("# sets\n{MINGW}\nz3\n"))
}

#[test]
fn the_set_is_read_up_to_the_end_of_its_command() {
    assert_eq!(apt_set("        run: tools/ci/apt.sh z3 && runner/build.sh exe").as_deref(), Some("z3"));
    assert_eq!(apt_set("      - run: (tools/ci/apt.sh a  b)").as_deref(), Some("a b"));
    assert_eq!(apt_set("tools/ci/apt.sh a b; runner/build.sh").as_deref(), Some("a b"));
    assert_eq!(apt_set("        run: tools/ci/apt.sh").as_deref(), Some(""));
    assert_eq!(apt_set("cp tools/ci/apt.sh.orig x"), None);
}

#[test]
fn apt_at_ci_time_is_found() {
    // (each way back to the mirror: what windows.yml and shirusu.yml did before 2026-11-24)
    for (rel, text) in [
        (".github/workflows/shirusu.yml", "          else\n            sudo apt-get update -q\n"),
        (".github/workflows/windows.yml", "            sudo dpkg -i ~/debs/*.deb >/dev/null\n"),
        (".github/workflows/windows.yml", "        with:\n          path: ~/debs\n"),
        ("runner/build.sh", "sudo apt install -y nasm\n"),
        ("tests/x.sh", "sudo apt update\n"),
        (".github/workflows/test.yml", "          path: ~/.apt-cache\n"),
        (".github/workflows/test.yml", "        run: tools/ci/apt.sh nasm\n"),
        (".github/workflows/test.yml", "        run: tools/ci/apt.sh\n"),
    ] {
        let r = apt_bundle(rel, text, &known());
        assert_eq!(r.len(), 1, "{rel}: {text:?}: {r:?}");
        assert!(r[0].starts_with(&format!("{rel}:")) && r[0].contains("apt-bundle"), "{r:?}");
    }
}

#[test]
fn the_bundles_and_their_scripts_pass() {
    let wf = format!("      # (apt-get is not run here: tools/ci/apt.sh)\n      - run: tools/ci/apt.sh {MINGW}\n      - run: tools/ci/apt.sh z3 && z3 --version\n");
    assert!(apt_bundle(".github/workflows/windows.yml", &wf, &known()).is_empty());
    assert!(apt_bundle("tools/ci/apt.sh", "sudo -E apt-get install -yq --no-download x\n", &known()).is_empty());
    assert!(apt_bundle("tools/ci/apt-bundle.sh", "apt-get update -q\n", &known()).is_empty());
}

#[test]
fn the_repository_passes_and_a_mirror_read_put_back_is_found() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    assert_eq!(check(&root), Ok(()));
    let sets = sets(&std::fs::read_to_string(root.join(APT_SETS)).unwrap_or_default());
    let rel = ".github/workflows/windows.yml";
    let wf = std::fs::read_to_string(root.join(rel)).unwrap_or_default();
    let back = wf.replace(&format!("tools/ci/apt.sh {MINGW}"), &format!("sudo apt-get install -y -q --no-install-recommends {MINGW}"));
    assert_ne!(back, wf, "windows.yml no longer installs mingw-w64 through apt.sh");
    assert!(!apt_bundle(rel, &back, &sets).is_empty());
}
