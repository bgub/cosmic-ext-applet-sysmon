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
  pname = "cosmic-ext-applet-vitals";
  version = "0.1.0";

  src = fetchFromGitHub {
    owner = "bgub";
    repo = "cosmic-ext-applet-vitals";
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
    "target/${stdenv.hostPlatform.rust.cargoShortTarget}/release/cosmic-ext-applet-vitals"
  ];

  passthru.updateScript = nix-update-script { };

  meta = {
    description = "A system vitals applet for the COSMIC desktop";
    homepage = "https://github.com/bgub/cosmic-ext-applet-vitals";
    changelog = "https://github.com/bgub/cosmic-ext-applet-vitals/releases/tag/${finalAttrs.version}";
    license = lib.licenses.mpl20;
    mainProgram = "cosmic-ext-applet-vitals";
    maintainers = with lib.maintainers; [ ];
    platforms = lib.platforms.linux;
  };
})
