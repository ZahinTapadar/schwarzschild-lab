#!/bin/sh
# Assemble the essay: shell + section fragments + section scripts.
# Writes index.html (standalone, with doctype) and artifact.html (body-only, for the Artifact publisher).
set -e
cd "$(dirname "$0")"
head='<title>Journey to the Pillars</title>
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<meta name="claude:viewport-fit-sides" content="cover">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:ital,wght@0,400;0,500;0,600;0,700;1,400&display=swap">
<link rel="stylesheet" href="lib/core.css">'
{
  echo "$head"
  echo '<main>'
  for f in sections/[0-9][0-9]-*.html; do cat "$f"; echo; done
  echo '</main>'
  echo '<script src="https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js"></script>'
  echo '<script src="https://cdnjs.cloudflare.com/ajax/libs/gsap/3.12.5/gsap.min.js"></script>'
  echo '<script src="lib/core.js"></script>'
  echo '<script src="lib/stage3d.js"></script>'
  echo '<script src="lib/pillars-volume.js"></script>'
  for f in sections/[0-9][0-9]-*.js; do [ -f "$f" ] && echo "<script src=\"$f\"></script>"; done
} > artifact.html
{ echo '<!doctype html><html lang="en"><head><meta charset="utf-8">'; sed -n '1,/^<link rel="stylesheet" href="lib/p' artifact.html; echo '</head><body>'; sed '1,/^<link rel="stylesheet" href="lib/d' artifact.html; echo '</body></html>'; } > index.html
echo "built: $(grep -c '<figure' artifact.html) figures, $(ls sections/[0-9][0-9]-*.js 2>/dev/null | wc -l | tr -d ' ') scripts"

# ./build.sh <dest>: also export a self-contained static copy (page, libs, scripts, referenced images).
if [ -n "$1" ]; then
  mkdir -p "$1/lib" "$1/sections" "$1/assets/img"
  cp index.html "$1/"; cp lib/*.css lib/*.js "$1/lib/"; cp sections/[0-9][0-9]-*.js "$1/sections/"
  for f in $(grep -ohE "assets/img/[A-Za-z0-9._-]+" sections/*.html sections/*.js | sort -u); do cp "$f" "$1/$f"; done
  echo "exported to $1 ($(du -sh "$1" | cut -f1))"
fi
