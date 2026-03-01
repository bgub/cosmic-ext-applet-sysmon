{
  lib,
  stdenv,
  rustPlatform,
  fetchFromGitHub,
  libcosmicAppHook,
  just,
  nix-update-script,
}:
rustPlatform.buildRustPackage (finalAttrs: {
  pname = "cosmic-ext-applet-sysmon";
  version = "0.1.0";

  src = fetchFromGitHub {
    owner = "bgub";
    repo = "cosmic-ext-applet-sysmon";
    tag = finalAttrs.version;
    hash = lib.fakeHash;
  };

  cargoHash = lib.fakeHash;

  nativeBuildInputs = [
    libcosmicAppHook
    just
  ];

  dontUseJustBuild = true;
  dontUseJustCheck = true;

  justFlags = [
    "--set"
    "prefix"
    (placeholder "out")
    "--set"
    "bin-src"
    "target/${stdenv.hostPlatform.rust.cargoShortTarget}/release/cosmic-ext-applet-sysmon"
  ];

  passthru.updateScript = nix-update-script { };

  meta = {
    description = "A system monitor applet for the COSMIC desktop";
    homepage = "https://github.com/bgub/cosmic-ext-applet-sysmon";
    changelog = "https://github.com/bgub/cosmic-ext-applet-sysmon/releases/tag/${finalAttrs.version}";
    license = lib.licenses.gpl3Only;
    mainProgram = "cosmic-ext-applet-sysmon";
    maintainers = with lib.maintainers; [ ];
    platforms = lib.platforms.linux;
  };
})
