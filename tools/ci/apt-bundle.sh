#!/bin/bash
# The .debs of one set of apt packages (a line of tools/ci/apt-sets.txt), in one zstd tarball on
# this repository's release ci-bundles: CI installs them from there (tools/ci/apt.sh) and never
# reads an apt mirror, which was down (2026-11-24, as in masu). .github/workflows/ci-bundles.yml
# uploads each set that is not there yet.
#   tools/ci/apt-bundle.sh name package...       the asset's name: Ubuntu's release and the set
#   tools/ci/apt-bundle.sh build DIR package...  DIR/<name> and DIR/<name>.sha256: every package
#                                                the set needs beyond a bare ubuntu image of the
#                                                runner's release, fetched in that image
set -euo pipefail
release() {
  (. /etc/os-release && echo "$VERSION_ID")
}
name() {
  echo "apt-$(release)-$(printf '%s' "$*" | sha256sum | cut -c1-16).tar.zst"
}
case "${1:-}" in
  name)
    shift
    name "$@"
    ;;
  build)
    out=$2
    shift 2
    [ $# -gt 0 ] || { echo "apt-bundle: no packages" >&2; exit 2; }
    n=$(name "$@")
    d=$(mktemp -d)
    docker run --rm -v "$d:/out" -e PKGS="$*" "ubuntu:$(release)" bash -euo pipefail -c '
      export DEBIAN_FRONTEND=noninteractive
      case " $PKGS" in *:i386*) dpkg --add-architecture i386 ;; esac
      o=(-o Acquire::Retries=5 -o Acquire::http::Timeout=30 -o Dir::Cache::Archives=/out)
      apt-get "${o[@]}" update -q
      # (word splitting of PKGS is the point: one argument per package)
      apt-get "${o[@]}" install -yq --no-install-recommends --download-only $PKGS
      rm -rf /out/partial /out/lock'
    debs=$(cd "$d" && ls -- *.deb)
    [ -n "$debs" ] || { echo "apt-bundle: nothing fetched for $*" >&2; exit 1; }
    mkdir -p "$out"
    # shellcheck disable=SC2086 # (one argument per .deb; their names have no spaces)
    tar -C "$d" --zstd -cf "$out/$n" $debs
    (cd "$out" && sha256sum "$n" > "$n.sha256")
    echo "$n: $(echo "$debs" | wc -l) packages, $(du -h "$out/$n" | cut -f1) ($*)"
    sudo rm -rf "$d"
    ;;
  *)
    echo "usage: tools/ci/apt-bundle.sh name package... | build DIR package..." >&2
    exit 2
    ;;
esac
