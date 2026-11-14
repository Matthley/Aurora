#!/usr/bin/env bash
# Hamlib universale (arm64 + x86_64) per Aurora su macOS: rigctld e le sue librerie in desktop/src-tauri/hamlib-bundle
# (rigctld accanto e le librerie in libs/, percorsi riscritti a @executable_path/libs/).
#
# La macchina di GitHub e' Apple Silicon: `brew install` darebbe solo arm64 (segnalazione #59, «Bad CPU type» sui Mac Intel).
# Qui si scaricano le bottiglie di Homebrew di ENTRAMBE le architetture (`brew fetch --bottle-tag=...`, senza installarle),
# si raccolgono rigctld e le librerie che usa davvero (otool -L), si riscrivono i percorsi, si uniscono con `lipo -create`
# e si firma ad hoc. Alla fine si controlla che ogni file sia «fat» con x86_64 e arm64: altrimenti lo script fallisce.
#
# Uso (solo su macOS, con Homebrew): scripts/hamlib-macos-universale.sh [cartella di destinazione]
set -euo pipefail

dest="${1:-desktop/src-tauri/hamlib-bundle}"
formula=hamlib
# dalle piu' vecchie alle piu' nuove: la prima bottiglia disponibile gira sul maggior numero di versioni di macOS
x86_tags=(big_sur monterey ventura sonoma sequoia tahoe)
arm_tags=(arm64_big_sur arm64_monterey arm64_ventura arm64_sonoma arm64_sequoia arm64_tahoe)

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# le formule che servono a rigctld (hamlib e le sue dipendenze di esecuzione, oggi libusb)
formulas=("$formula")
while IFS= read -r dep; do
  [ -n "$dep" ] && formulas+=("$dep")
done < <(brew deps "$formula")
echo "formule: ${formulas[*]}"

# scarica ed estrae le bottiglie di un'architettura in $work/bottles/<arch>
fetch_arch() {
  local arch="$1"; shift
  local tags=("$@")
  local out="$work/bottles/$arch"
  mkdir -p "$out"
  local f tag file got
  for f in "${formulas[@]}"; do
    got=""
    for tag in "${tags[@]}"; do
      if brew fetch --force --bottle-tag="$tag" "$f" >/dev/null 2>&1; then
        file="$(brew --cache --bottle-tag="$tag" "$f")"
        if [ -s "$file" ]; then
          echo "$arch: $f dalla bottiglia $tag"
          tar -xf "$file" -C "$out"
          got=1
          break
        fi
      fi
    done
    if [ -z "$got" ]; then
      echo "::error::nessuna bottiglia $arch di $f (provati: ${tags[*]})"
      exit 1
    fi
  done
}

# copia rigctld e, seguendo otool -L, le librerie che usa in $work/thin/<arch>, con i percorsi riscritti
bundle_arch() {
  local arch="$1"
  local src="$work/bottles/$arch"
  local out="$work/thin/$arch"
  mkdir -p "$out/libs"
  local rig
  rig="$(find "$src" -path "*/bin/rigctld" -type f | head -n 1)"
  if [ -z "$rig" ]; then
    echo "::error::rigctld non trovato nella bottiglia $arch"
    exit 1
  fi
  cp -L "$rig" "$out/rigctld"
  chmod u+w "$out/rigctld"

  local queue=("$out/rigctld")
  local file dep base found self
  while [ "${#queue[@]}" -gt 0 ]; do
    file="${queue[0]}"
    queue=("${queue[@]:1}")
    self="$(basename "$file")"
    if [ "$file" != "$out/rigctld" ]; then
      install_name_tool -id "@executable_path/libs/$self" "$file"
    fi
    # le dipendenze (la prima riga e' il nome del file; per le librerie la seconda e' il loro id, saltato sotto perche' uguale a se')
    while IFS= read -r dep; do
      case "$dep" in
        /usr/lib/*|/System/*|@executable_path/libs/*) continue ;;
      esac
      base="$(basename "$dep")"
      [ "$base" = "$self" ] && continue
      if [ ! -e "$out/libs/$base" ]; then
        found="$(find "$src" -name "$base" \( -type f -o -type l \) | head -n 1)"
        if [ -z "$found" ]; then
          echo "::error::$arch: libreria $base (usata da $self) non trovata nelle bottiglie"
          exit 1
        fi
        cp -L "$found" "$out/libs/$base"
        chmod u+w "$out/libs/$base"
        queue+=("$out/libs/$base")
      fi
      install_name_tool -change "$dep" "@executable_path/libs/$base" "$file"
    done < <(otool -L "$file" | tail -n +2 | awk '{print $1}')
  done
  # la firma della bottiglia non vale piu' dopo install_name_tool: si toglie, si rifirma dopo lipo
  for file in "$out/rigctld" "$out"/libs/*; do
    codesign --remove-signature "$file" 2>/dev/null || true
    lipo -archs "$file" | grep -qw "$arch" || { echo "::error::$file non e' $arch: $(lipo -archs "$file")"; exit 1; }
  done
  echo "$arch:"; ls -la "$out" "$out/libs"
}

fetch_arch x86_64 "${x86_tags[@]}"
fetch_arch arm64 "${arm_tags[@]}"
bundle_arch x86_64
bundle_arch arm64

# le due architetture devono avere gli stessi file
diff <(cd "$work/thin/x86_64" && find . -type f | sort) <(cd "$work/thin/arm64" && find . -type f | sort) || {
  echo "::error::le bottiglie x86_64 e arm64 non hanno le stesse librerie"
  exit 1
}

rm -rf "$dest"
mkdir -p "$dest/libs"
while IFS= read -r rel; do
  lipo -create "$work/thin/x86_64/$rel" "$work/thin/arm64/$rel" -output "$dest/$rel"
done < <(cd "$work/thin/arm64" && find . -type f | sed 's|^\./||')
chmod +x "$dest/rigctld"
codesign --force -s - "$dest"/libs/*.dylib
codesign --force -s - "$dest/rigctld"

# controllo: ogni file Mach-O deve contenere x86_64 e arm64
bad=0
while IFS= read -r file; do
  archs="$(lipo -archs "$file")"
  echo "$file: $archs"
  if ! echo "$archs" | grep -qw x86_64 || ! echo "$archs" | grep -qw arm64; then
    echo "::error::$file non e' universale ($archs)"
    bad=1
  fi
done < <(find "$dest" -type f)
[ "$bad" -eq 0 ]

"$dest/rigctld" --version
# Intel tramite Rosetta, se c'e' sulla macchina
if arch -x86_64 /usr/bin/true 2>/dev/null; then
  arch -x86_64 "$dest/rigctld" --version
else
  echo "::warning::Rosetta assente: la parte x86_64 di rigctld non e' stata avviata"
fi
