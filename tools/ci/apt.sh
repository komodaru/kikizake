#!/bin/bash
# apt packages from the zstd tarball of their .debs on this repository's release ci-bundles
# (tools/ci/apt-bundle.sh makes it, .github/workflows/ci-bundles.yml uploads it): no apt mirror
# and no apt-get update at CI time (2026-11-24: the mirror failed runs). The set is a line of
# tools/ci/apt-sets.txt, as written there. A package the runner has at the bundle's version or a
# newer one is left as it is; the others are installed from the bundle.
#   tools/ci/apt.sh package...
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
export DEBIAN_FRONTEND=noninteractive
have() {
  local p
  for p in "$@"; do
    dpkg-query -W -f='${Status}' "$p" 2>/dev/null | grep -qx 'install ok installed' || return 1
  done
}
if ! grep -qxF -- "$*" "$here/apt-sets.txt"; then
  echo "apt: no bundle for '$*': add it as a line of tools/ci/apt-sets.txt (ci-bundles.yml uploads it)" >&2
  exit 1
fi
if have "$@"; then
  echo "apt: already installed: $*"
  exit 0
fi
n=$("$here/apt-bundle.sh" name "$@")
url=${GITHUB_SERVER_URL:-https://github.com}/${GITHUB_REPOSITORY:-komodaru/kikizake}/releases/download/ci-bundles
d=$(mktemp -d)
trap 'rm -rf "$d"' EXIT
for f in "$n" "$n.sha256"; do
  if ! curl -fsSL --retry 5 --retry-all-errors --connect-timeout 30 -o "$d/$f" "$url/$f"; then
    echo "apt: $url/$f could not be fetched: is the set uploaded (the ci-bundles workflow)?" >&2
    exit 1
  fi
done
(cd "$d" && sha256sum -c --quiet "$n.sha256")
mkdir "$d/debs"
tar -C "$d/debs" --zstd -xf "$d/$n"
case " $*" in
  *:i386*) dpkg --print-foreign-architectures | grep -qx i386 || sudo dpkg --add-architecture i386 ;;
esac
take=()
for f in "$d"/debs/*.deb; do
  read -r p a v < <(dpkg-deb --show --showformat='${Package} ${Architecture} ${Version}\n' "$f")
  q=$p
  [ "$a" = all ] || q=$p:$a
  now=$(dpkg-query -W -f='${db:Status-Status} ${Version}' "$q" 2>/dev/null || true)
  if [ "${now%% *}" = installed ] && dpkg --compare-versions "${now#* }" ge "$v"; then
    continue
  fi
  take+=("$f")
done
echo "apt: $n: ${#take[@]} packages to install ($*)"
if [ ${#take[@]} -gt 0 ]; then
  # (apt looks for each file again under its archives directory)
  sudo -E apt-get -o Dir::Cache::Archives="$d/debs" install -yq --no-install-recommends --no-download "${take[@]}"
fi
have "$@" || { echo "apt: $* still not installed after the bundle" >&2; exit 1; }
