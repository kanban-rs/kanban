{
  lib,
  stdenv,
  ffmpeg,
}:

let
  cargoVersion = (lib.importTOML ../Cargo.toml).workspace.package.version;
  demoSrc = ../demo;
in
stdenv.mkDerivation {
  pname = "kanban-web";
  version = cargoVersion;

  src = lib.cleanSource ./.;

  nativeBuildInputs = [ ffmpeg ];

  buildPhase = ''
    substitute index.html index.html.out \
      --replace-fail "@VERSION@" "${cargoVersion}"
    ffmpeg -i ${demoSrc}/demo.gif -pix_fmt yuva420p -c:v libvpx-vp9 -b:v 0 -crf 33 -an demo.webm
  '';

  installPhase = ''
    mkdir -p $out/demo
    cp index.html.out $out/index.html
    cp styles.css $out/
    cp demo.webm $out/demo/demo.webm
  '';

  meta = {
    description = "Kanban landing page - keyboard-driven project management for developers";
    homepage = "https://kanban.yoon.se";
    license = lib.licenses.asl20;
    maintainers = with lib.maintainers; [ fulsomenko ];
    platforms = lib.platforms.all;
  };
}
