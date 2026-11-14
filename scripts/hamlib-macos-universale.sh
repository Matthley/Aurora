#!/usr/bin/env bash
# Hamlib universale (arm64 + x86_64) per Aurora su macOS: rigctld e le sue librerie in desktop/src-tauri/hamlib-bundle
# (rigctld nella radice, le librerie in libs/, percorsi riscritti a @executable_path/libs/, licenze e VERSIONE.txt accanto).
#
# La macchina di GitHub e' Apple Silicon e Homebrew non pubblica piu' bottiglie x86_64 di hamlib (release 0.11.256):
# qui si COMPILANO dai sorgenti ufficiali libusb e Hamlib, una volta per arm64 e una per x86_64 (clang -arch), si uniscono
# con `lipo -create` e si firma ad hoc. Alla fine si controlla che ogni file sia «fat» con x86_64 e arm64: altrimenti si fallisce.
#
# Uso (solo su macOS, con gli strumenti di Xcode): scripts/hamlib-macos-universale.sh [cartella di destinazione]
set -euo pipefail

dest="${1:-desktop/src-tauri/hamlib-bundle}"

# versioni fissate con l'impronta sha256 (Hamlib: file SHA256SUM della release; libusb: impronta dell'allegato su GitHub)
HAMLIB_VER=4.7.2
HAMLIB_URL="https://github.com/Hamlib/Hamlib/releases/download/${HAMLIB_VER}/hamlib-${HAMLIB_VER}.tar.gz"
HAMLIB_SHA256=ae1fcf2dbc80ea0786ea8f047b09399c3f7737d1930442f61a031708ed33e88f
LIBUSB_VER=1.0.30
LIBUSB_URL="https://github.com/libusb/libusb/releases/download/v${LIBUSB_VER}/libusb-${LIBUSB_VER}.tar.bz2"
LIBUSB_SHA256=fea36f34f9156400209595e300840767ab1a385ede1dc7ee893015aea9c6dbaf
MACOS_MIN=11.0
archs=(arm64 x86_64)

fail() { echo "::error::$*" >&2; exit 1; }

[ "$(uname -s)" = Darwin ] || fail "questo script gira solo su macOS"
for tool in clang make lipo otool install_name_tool codesign shasum curl tar; do
  command -v "$tool" >/dev/null || fail "manca lo strumento $tool"
done

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
jobs="$(sysctl -n hw.ncpu 2>/dev/null || echo 2)"

# scarica un archivio e ne controlla l'impronta
fetch() {
  local url="$1" sha="$2" out="$3" got
  echo "scarico $url"
  curl -fsSL --retry 3 --connect-timeout 30 -o "$out" "$url" || fail "download non riuscito: $url"
  got="$(shasum -a 256 "$out" | awk '{print $1}')"
  [ "$got" = "$sha" ] || fail "impronta sbagliata per $url (attesa $sha, trovata $got)"
}

fetch "$LIBUSB_URL" "$LIBUSB_SHA256" "$work/libusb.tar.bz2"
fetch "$HAMLIB_URL" "$HAMLIB_SHA256" "$work/hamlib.tar.gz"

# compila libusb e Hamlib per un'architettura, ognuna in una cartella sorgente pulita e con il suo prefisso
build_arch() {
  local arch="$1"
  local pre="$work/prefix/$arch"
  local src="$work/src/$arch"
  local host_args=()
  # la macchina e' arm64: per x86_64 si compila «incrociato»
  [ "$arch" = x86_64 ] && host_args=(--host=x86_64-apple-darwin)
  mkdir -p "$src" "$pre"
  tar -xjf "$work/libusb.tar.bz2" -C "$src"
  tar -xzf "$work/hamlib.tar.gz" -C "$src"

  local cc="clang -arch $arch"
  local cflags="-O2 -arch $arch -mmacosx-version-min=$MACOS_MIN"
  local ldflags="-arch $arch -mmacosx-version-min=$MACOS_MIN"

  echo "== libusb $LIBUSB_VER per $arch"
  (
    cd "$src/libusb-$LIBUSB_VER"
    CC="$cc" CFLAGS="$cflags" LDFLAGS="$ldflags" \
      ./configure --prefix="$pre" ${host_args[@]+"${host_args[@]}"} --disable-dependency-tracking \
      --enable-shared --disable-static --disable-examples-build --disable-tests-build >"$work/libusb-$arch.log" 2>&1 \
      || { tail -n 40 "$work/libusb-$arch.log"; fail "configure di libusb ($arch) non riuscito"; }
    make -j"$jobs" >>"$work/libusb-$arch.log" 2>&1 && make install >>"$work/libusb-$arch.log" 2>&1 \
      || { tail -n 40 "$work/libusb-$arch.log"; fail "compilazione di libusb ($arch) non riuscita"; }
  )

  echo "== Hamlib $HAMLIB_VER per $arch"
  (
    cd "$src/hamlib-$HAMLIB_VER"
    # libusb indicata esplicitamente (e pkg-config isolato): niente librerie di Homebrew della macchina
    CC="$cc" CXX="clang++ -arch $arch" CFLAGS="$cflags -I$pre/include" CXXFLAGS="$cflags" \
      LDFLAGS="$ldflags -L$pre/lib" \
      PKG_CONFIG_PATH="$pre/lib/pkgconfig" PKG_CONFIG_LIBDIR="$pre/lib/pkgconfig" \
      LIBUSB_CFLAGS="-I$pre/include/libusb-1.0" LIBUSB_LIBS="-L$pre/lib -lusb-1.0" \
      ./configure --prefix="$pre" ${host_args[@]+"${host_args[@]}"} --disable-dependency-tracking \
      --enable-shared --disable-static --disable-html-matrix \
      --without-readline --without-indi --without-xml-support \
      --without-cxx-binding --without-perl-binding --without-python-binding --without-tcl-binding --without-lua-binding \
      >"$work/hamlib-$arch.log" 2>&1 \
      || { tail -n 40 "$work/hamlib-$arch.log"; fail "configure di Hamlib ($arch) non riuscito"; }
    make -j"$jobs" >>"$work/hamlib-$arch.log" 2>&1 && make install >>"$work/hamlib-$arch.log" 2>&1 \
      || { tail -n 60 "$work/hamlib-$arch.log"; fail "compilazione di Hamlib ($arch) non riuscita"; }
  )
  [ -x "$pre/bin/rigctld" ] || fail "rigctld ($arch) non prodotto"
}

# copia rigctld e, seguendo otool -L, le librerie che usa (solo quelle del prefisso) in $work/thin/<arch>, percorsi riscritti
bundle_arch() {
  local arch="$1"
  local pre="$work/prefix/$arch"
  local out="$work/thin/$arch"
  mkdir -p "$out/libs"
  cp -L "$pre/bin/rigctld" "$out/rigctld"
  chmod u+w "$out/rigctld"

  local queue=("$out/rigctld")
  local file dep base self
  while [ "${#queue[@]}" -gt 0 ]; do
    file="${queue[0]}"
    queue=("${queue[@]:1}")
    self="$(basename "$file")"
    [ "$file" != "$out/rigctld" ] && install_name_tool -id "@executable_path/libs/$self" "$file"
    while IFS= read -r dep; do
      case "$dep" in
        /usr/lib/*|/System/*|@executable_path/libs/*) continue ;;
      esac
      base="$(basename "$dep")"
      [ "$base" = "$self" ] && continue
      if [ ! -e "$out/libs/$base" ]; then
        [ -e "$pre/lib/$base" ] || fail "$arch: libreria $base (usata da $self, $dep) non e' tra quelle compilate"
        cp -L "$pre/lib/$base" "$out/libs/$base"
        chmod u+w "$out/libs/$base"
        queue+=("$out/libs/$base")
      fi
      install_name_tool -change "$dep" "@executable_path/libs/$base" "$file"
    done < <(otool -L "$file" | tail -n +2 | awk '{print $1}')
  done
  # i backend USB di Hamlib devono esserci: libusb deve risultare tra le dipendenze
  [ -e "$out/libs/libusb-1.0.0.dylib" ] || fail "$arch: Hamlib compilata senza libusb (manca libusb-1.0.0.dylib)"
  for file in "$out/rigctld" "$out"/libs/*; do
    codesign --remove-signature "$file" 2>/dev/null || true
    lipo -archs "$file" | grep -qw "$arch" || fail "$file non e' $arch: $(lipo -archs "$file")"
  done
  echo "$arch:"; ls -la "$out" "$out/libs"
}

for a in "${archs[@]}"; do build_arch "$a"; done
for a in "${archs[@]}"; do bundle_arch "$a"; done

# le due architetture devono avere gli stessi file
diff <(cd "$work/thin/x86_64" && find . -type f | sort) <(cd "$work/thin/arm64" && find . -type f | sort) \
  || fail "le compilazioni x86_64 e arm64 non hanno le stesse librerie"

rm -rf "$dest"
mkdir -p "$dest/libs"
while IFS= read -r rel; do
  lipo -create "$work/thin/x86_64/$rel" "$work/thin/arm64/$rel" -output "$dest/$rel"
done < <(cd "$work/thin/arm64" && find . -type f | sed 's|^\./||')
chmod +x "$dest/rigctld"
codesign --force --sign - "$dest"/libs/*.dylib
codesign --force --sign - "$dest/rigctld"

# licenze (Hamlib: GPL per rigctld, LGPL per la libreria; libusb: LGPL) e versioni
src_arm="$work/src/arm64"
cp "$src_arm/hamlib-$HAMLIB_VER/COPYING" "$dest/COPYING-hamlib-GPL.txt"
cp "$src_arm/hamlib-$HAMLIB_VER/COPYING.LIB" "$dest/COPYING-hamlib-LGPL.txt"
cp "$src_arm/libusb-$LIBUSB_VER/COPYING" "$dest/COPYING-libusb-LGPL.txt"
cat >"$dest/VERSIONE.txt" <<EOF
Hamlib $HAMLIB_VER ($HAMLIB_URL, sha256 $HAMLIB_SHA256)
libusb $LIBUSB_VER ($LIBUSB_URL, sha256 $LIBUSB_SHA256)
Compilati dai sorgenti per arm64 e x86_64 (macOS $MACOS_MIN o successivo), uniti con lipo, firma ad hoc.
EOF

# controllo: ogni file Mach-O deve contenere x86_64 e arm64
bad=0
while IFS= read -r file; do
  file -b "$file" | grep -q 'Mach-O' || continue
  a="$(lipo -archs "$file")"
  echo "$file: $a"
  if ! echo "$a" | grep -qw x86_64 || ! echo "$a" | grep -qw arm64; then
    echo "::error::$file non e' universale ($a)"
    bad=1
  fi
done < <(find "$dest" -type f)
[ "$bad" -eq 0 ] || fail "file non universali nel pacchetto Hamlib"

"$dest/rigctld" --version
# Intel tramite Rosetta, se c'e' sulla macchina
if arch -x86_64 /usr/bin/true 2>/dev/null; then
  arch -x86_64 "$dest/rigctld" --version
else
  echo "::warning::Rosetta assente: la parte x86_64 di rigctld non e' stata avviata"
fi
ls -la "$dest" "$dest/libs"
